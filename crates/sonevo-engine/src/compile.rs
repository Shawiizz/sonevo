//! A song turned into a [`Graph`], keeping the processors the graph before already has.

use crate::graph::{
    AudioClip, AutomationClip, ChannelNode, Gains, Graph, InsertNode, Lane, Lever, NoteEvent, CHUNK, EVENTS,
};
use crate::meters;
use crate::samples::Samples;
use sonevo_dsp::{builtin, Param, Processor, Sample};
use sonevo_project::{ClipContent, Device, DeviceKind, Id, Insert, Pattern, Song, Target, Ticks, PPQ};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

/// What plays: the whole song, one pattern over and over, or the song as one mixer track
/// alone sends it to the master (the master itself: what goes to it straight).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Song,
    Pattern(Id),
    Stem(Id),
}

/// A device the audio thread has, as the engine remembers it.
pub struct Held {
    pub slot: usize,
    pub params: &'static [Param],
    kind: DeviceKind,
    values: Vec<f64>,
    sample: Option<String>,
}

impl Held {
    /// Remembers a setting made at once, as a knob turns.
    pub fn set(&mut self, index: usize, value: f64) {
        self.values[index] = value;
    }
}

/// What the audio thread has, by id.
#[derive(Default)]
pub struct Mirror {
    pub devices: HashMap<Id, Held>,
    pub channels: Vec<Id>,
    /// The master last.
    pub inserts: Vec<Id>,
}

pub struct Compiled {
    pub graph: Box<Graph>,
    pub mirror: Mirror,
    /// What could not be played, said plainly.
    pub missing: Vec<String>,
}

/// The plugins songs name, by their bundle and their id.
pub trait Plugins: Send + Sync {
    /// The settings the plugin lets be moved, as its knobs show them.
    fn params(&self, bundle: &str, id: &str) -> &'static [Param];
    /// Starts the plugin the device `device` plays for blocks at `sample_rate`, or says why it
    /// could not.
    fn start(
        &self,
        device: Id,
        bundle: &str,
        id: &str,
        sample_rate: f32,
    ) -> Result<Box<dyn Processor>, String>;
}

struct Slots<'a> {
    before: &'a Mirror,
    plugins: Option<&'a dyn Plugins>,
    sample_rate: f32,
    slots: Vec<Option<Box<dyn Processor>>>,
    keep: Vec<(usize, usize)>,
    kept: HashSet<usize>,
    settings: Vec<(usize, usize, f64)>,
    samples: Vec<(usize, Option<Arc<Sample>>)>,
    devices: HashMap<Id, Held>,
    missing: Vec<String>,
}

impl Slots<'_> {
    fn add(&mut self, device: &Device, samples: &mut Samples) -> Option<usize> {
        if !device.enabled {
            return None;
        }
        let params = match &device.kind {
            DeviceKind::Builtin { name } => match builtin(name) {
                Some(made) => made.params,
                None => {
                    self.missing.push(format!("{name} is not available"));
                    return None;
                }
            },
            DeviceKind::Clap { bundle, plugin } => self.plugins.map_or(&[][..], |p| p.params(bundle, plugin)),
        };
        let values: Vec<f64> =
            params.iter().map(|p| device.params.get(p.id).map_or(p.default, |&v| p.clamp(v))).collect();
        let sample = device.sample.as_deref().and_then(|name| {
            let loaded = samples.get(name);
            if loaded.is_none() {
                self.missing.push(format!("the sound {name} could not be read"));
            }
            loaded
        });
        let slot = self.slots.len();
        let before = self
            .before
            .devices
            .get(&device.id)
            .filter(|h| h.kind == device.kind && !self.kept.contains(&h.slot));
        if let Some(before) = before {
            self.keep.push((slot, before.slot));
            self.kept.insert(before.slot);
            self.slots.push(None);
            for (i, (&now, &was)) in values.iter().zip(&before.values).enumerate() {
                if now != was {
                    self.settings.push((slot, i, now));
                }
            }
            if device.sample != before.sample {
                self.samples.push((slot, sample));
            }
        } else {
            let mut processor = match &device.kind {
                DeviceKind::Builtin { name } => builtin(name).expect("found above").make(self.sample_rate),
                DeviceKind::Clap { bundle, plugin } => {
                    let started = match self.plugins {
                        Some(plugins) => plugins.start(device.id, bundle, plugin, self.sample_rate),
                        None => Err("plugins are not played here".into()),
                    };
                    match started {
                        Ok(processor) => processor,
                        Err(e) => {
                            self.missing.push(format!("{plugin}: {e}"));
                            return None;
                        }
                    }
                }
            };
            for (i, (&value, param)) in values.iter().zip(params).enumerate() {
                if value != param.default {
                    processor.set(i, value);
                }
            }
            processor.set_sample(sample);
            self.slots.push(Some(processor));
        }
        let held = Held { slot, params, kind: device.kind.clone(), values, sample: device.sample.clone() };
        self.devices.insert(device.id, held);
        Some(slot)
    }
}

/// Whether the sound of `from` comes to `to`, by `edges`.
fn reaches(from: usize, to: usize, edges: &[Vec<usize>]) -> bool {
    let mut seen = vec![false; edges.len()];
    let mut stack = vec![from];
    while let Some(at) = stack.pop() {
        if at == to {
            return true;
        }
        if !std::mem::replace(&mut seen[at], true) {
            stack.extend(&edges[at]);
        }
    }
    false
}

/// The mixer tracks in an order where each comes before those it goes to.
fn order(edges: &[Vec<usize>]) -> Vec<usize> {
    fn visit(at: usize, edges: &[Vec<usize>], seen: &mut [bool], done: &mut Vec<usize>) {
        seen[at] = true;
        for &to in &edges[at] {
            if !seen[to] {
                visit(to, edges, seen, done);
            }
        }
        done.push(at);
    }
    let mut seen = vec![false; edges.len()];
    let mut done = Vec::with_capacity(edges.len());
    for at in 0..edges.len() {
        if !seen[at] {
            visit(at, edges, &mut seen, &mut done);
        }
    }
    done.reverse();
    done
}

/// Every mixer track a soloed one leaves heard: those soloed, those that feed them, those
/// they go on to.
fn audible(inserts: &[&Insert], edges: &[Vec<usize>]) -> Vec<bool> {
    let soloed: Vec<usize> = (0..inserts.len()).filter(|&i| inserts[i].solo).collect();
    if soloed.is_empty() {
        return inserts.iter().map(|i| !i.mute).collect();
    }
    let mut heard = vec![false; inserts.len()];
    let reach = |from: usize, forward: bool, heard: &mut Vec<bool>| {
        let mut stack = vec![from];
        while let Some(at) = stack.pop() {
            if heard[at] && at != from {
                continue;
            }
            heard[at] = true;
            for (i, out) in edges.iter().enumerate() {
                if forward && i == at {
                    stack.extend(out.iter().copied().filter(|&t| !heard[t]));
                } else if !forward && out.contains(&at) && !heard[i] {
                    stack.push(i);
                }
            }
        }
    };
    for &s in &soloed {
        reach(s, true, &mut heard);
        reach(s, false, &mut heard);
    }
    heard.iter().zip(inserts).map(|(&h, i)| h && !i.mute).collect()
}

/// Where `tick` of a pattern plays, swung: every second sixteenth `late` ticks later, the
/// ticks between stretched to fit.
fn swung(tick: Ticks, late: Ticks) -> Ticks {
    const STEP: Ticks = PPQ / 4;
    if late == 0 {
        return tick;
    }
    let pair = tick.div_euclid(2 * STEP) * 2 * STEP;
    let at = tick - pair;
    pair + if at < STEP {
        at * (STEP + late) / STEP
    } else {
        STEP + late + (at - STEP) * (STEP - late) / STEP
    }
}

/// The note events of `pattern` played as a clip from `start` to `end`, `offset` into it.
fn pattern_events(
    pattern: &Pattern,
    channels: &HashMap<Id, usize>,
    start: Ticks,
    end: Ticks,
    offset: Ticks,
    events: &mut Vec<NoteEvent>,
) {
    let length = pattern.length.max(1);
    let late = (f64::from(pattern.swing.clamp(0.0, 1.0)) * (PPQ / 8) as f64).round() as Ticks;
    let base = start - offset;
    let first = offset.div_euclid(length);
    let last = (offset + (end - start)).div_euclid(length);
    for (id, notes) in &pattern.notes {
        let Some(&channel) = channels.get(id) else { continue };
        for repeat in first..=last {
            let from = base + repeat * length;
            for note in notes {
                let on = from + swung(note.start, late);
                if on < start || on >= end || note.start >= length {
                    continue;
                }
                let off = (from + swung(note.start + note.length.max(1), late)).max(on + 1).min(end);
                let velocity = note.velocity.clamp(0.0, 1.0);
                events.push(NoteEvent { tick: on, channel, key: note.key, velocity, on: true });
                events.push(NoteEvent { tick: off, channel, key: note.key, velocity: 0.0, on: false });
            }
        }
    }
}

pub fn compile(
    song: &Song,
    mode: Mode,
    before: &Mirror,
    sample_rate: f32,
    samples: &mut Samples,
    plugins: Option<&dyn Plugins>,
) -> Compiled {
    let mut slots = Slots {
        before,
        plugins,
        sample_rate,
        slots: Vec::new(),
        keep: Vec::new(),
        kept: HashSet::new(),
        settings: Vec::new(),
        samples: Vec::new(),
        devices: HashMap::new(),
        missing: Vec::new(),
    };

    let all: Vec<&Insert> = song.inserts.iter().chain([&song.master]).collect();
    let master = all.len() - 1;
    let index: HashMap<Id, usize> = all.iter().enumerate().map(|(i, insert)| (insert.id, i)).collect();
    // each track's output, or the master where outputs would go round in a loop
    let mut outputs: Vec<usize> = all
        .iter()
        .enumerate()
        .map(|(i, insert)| {
            insert.output.and_then(|o| index.get(&o).copied()).filter(|&o| o != i).unwrap_or(master)
        })
        .collect();
    outputs[master] = master;
    for i in 0..master {
        let mut at = outputs[i];
        for _ in 0..all.len() {
            at = outputs[at];
        }
        if at != master {
            outputs[i] = master;
        }
    }
    let mut edges: Vec<Vec<usize>> =
        (0..all.len()).map(|i| if i == master { Vec::new() } else { vec![outputs[i]] }).collect();
    let mut sends: Vec<Vec<(usize, f32)>> = vec![Vec::new(); all.len()];
    for (i, insert) in all.iter().enumerate().take(master) {
        for send in &insert.sends {
            let Some(&to) = index.get(&send.to) else { continue };
            // a send that would bring the sound back where it comes from is left out
            if to != i && !reaches(to, i, &edges) && !edges[i].contains(&to) {
                edges[i].push(to);
                sends[i].push((to, send.amount.clamp(0.0, 1.0)));
            }
        }
    }
    // an effect listening to another track is played after it, unless that makes a loop
    let mut after = edges.clone();
    let mut keys: HashMap<Id, usize> = HashMap::new();
    for (i, insert) in all.iter().enumerate() {
        for device in &insert.effects {
            let Some(key) = device.key.and_then(|k| index.get(&k).copied()) else { continue };
            if key != i && key != master && !reaches(i, key, &after) {
                after[key].push(i);
                keys.insert(device.id, key);
            }
        }
    }
    let order = order(&after);
    let stem = match mode {
        Mode::Stem(id) => Some(index.get(&id).copied().unwrap_or(master)),
        _ => None,
    };
    // whether what goes straight to the master is heard: not when another track is alone
    let straight = stem.is_none_or(|s| s == master);
    let heard = audible(&all, &edges);
    let inserts: Vec<InsertNode> = all
        .iter()
        .enumerate()
        .map(|(i, insert)| {
            let effects = insert
                .effects
                .iter()
                .filter_map(|d| Some((slots.add(d, samples)?, keys.get(&d.id).copied())))
                .collect();
            // a track alone: only its sound comes to the master, every other's goes nowhere
            let output = match stem {
                _ if i == master => None,
                Some(s) if s == i => Some(master),
                Some(_) if outputs[i] == master => None,
                _ => Some(outputs[i]),
            };
            let sends = std::mem::take(&mut sends[i]);
            InsertNode {
                effects,
                volume: insert.volume.clamp(0.0, 1.25),
                pan: insert.pan,
                audible: heard[i],
                sends,
                output,
                applied: Gains::SILENT,
                left: vec![0.0; CHUNK],
                right: vec![0.0; CHUNK],
                meter: if i == master { meters::MASTER } else { meters::insert(i) },
            }
        })
        .collect();

    let channel_index: HashMap<Id, usize> =
        song.channels.iter().enumerate().map(|(i, c)| (c.id, i)).collect();
    let channels: Vec<ChannelNode> = song
        .channels
        .iter()
        .enumerate()
        .map(|(i, channel)| {
            let insert = channel.insert.and_then(|id| index.get(&id).copied()).unwrap_or(master);
            (i, channel, insert)
        })
        .map(|(i, channel, insert)| ChannelNode {
            slot: slots.add(&channel.instrument, samples),
            volume: channel.volume.clamp(0.0, 1.0),
            pan: channel.pan,
            audible: !channel.mute && (straight || insert != master),
            insert,
            applied: Gains::SILENT,
            events: Vec::with_capacity(EVENTS),
            held: 0,
            release: 0,
            meter: meters::channel(i),
        })
        .collect();

    let mut events = Vec::new();
    let mut audio = Vec::new();
    let mut lanes = Vec::new();
    let bar = song.signature.bar().max(PPQ);
    let (loop_start, loop_end) = match mode {
        Mode::Pattern(id) => {
            let length = song.pattern(id).map_or(bar, |p| p.length.max(PPQ));
            if let Some(pattern) = song.pattern(id) {
                pattern_events(pattern, &channel_index, 0, length, 0, &mut events);
            }
            (0, length)
        }
        Mode::Song | Mode::Stem(_) => {
            let muted: HashSet<Id> = song.tracks.iter().filter(|t| t.mute).map(|t| t.id).collect();
            let mut automation: BTreeMap<&Target, Vec<AutomationClip>> = BTreeMap::new();
            for clip in song.clips.iter().filter(|c| !muted.contains(&c.track)) {
                let end = clip.start + clip.length;
                match &clip.content {
                    ClipContent::Pattern { pattern } => {
                        if let Some(pattern) = song.pattern(*pattern) {
                            pattern_events(
                                pattern,
                                &channel_index,
                                clip.start,
                                end,
                                clip.offset,
                                &mut events,
                            );
                        }
                    }
                    ClipContent::Audio { .. } if !straight => {}
                    ClipContent::Audio { sample, gain } => match samples.get(sample) {
                        Some(sample) => audio.push(AudioClip {
                            start: clip.start,
                            end,
                            offset: clip.offset,
                            sample,
                            gain: gain.max(0.0),
                        }),
                        None => slots.missing.push(format!("the sound {sample} could not be read")),
                    },
                    ClipContent::Automation { target, points } => {
                        let mut points = points.clone();
                        points.sort_by_key(|p| p.at);
                        points.iter_mut().for_each(|p| p.value = p.value.clamp(0.0, 1.0));
                        let clip = AutomationClip { start: clip.start, end, offset: clip.offset, points };
                        automation.entry(target).or_default().push(clip);
                    }
                }
            }
            for (target, mut clips) in automation {
                let lever = match target {
                    Target::ChannelVolume { channel } => {
                        channel_index.get(channel).map(|&c| Lever::ChannelVolume(c))
                    }
                    Target::ChannelPan { channel } => {
                        channel_index.get(channel).map(|&c| Lever::ChannelPan(c))
                    }
                    Target::InsertVolume { insert } => index.get(insert).map(|&i| Lever::InsertVolume(i)),
                    Target::InsertPan { insert } => index.get(insert).map(|&i| Lever::InsertPan(i)),
                    Target::Param { device, param } => slots.devices.get(device).and_then(|held| {
                        let index = held.params.iter().position(|p| p.id == param)?;
                        Some(Lever::Param { slot: held.slot, index, param: held.params[index] })
                    }),
                    Target::Tempo => Some(Lever::Tempo),
                };
                if let Some(lever) = lever {
                    clips.sort_by_key(|c| c.start);
                    lanes.push(Lane { lever, clips, last: f64::NAN });
                }
            }
            match song.loop_region.filter(|r| r.end > r.start) {
                Some(region) => (region.start.max(0), region.end),
                None => {
                    let length = song.length();
                    (0, ((length + bar - 1) / bar * bar).max(bar))
                }
            }
        }
    };
    // a key let go of before one pressed on the same tick: a note repeated plays again
    events.sort_unstable_by_key(|e| (e.tick, e.on, e.channel, e.key));

    let mirror = Mirror {
        devices: slots.devices,
        channels: song.channels.iter().map(|c| c.id).collect(),
        inserts: all.iter().map(|i| i.id).collect(),
    };
    let graph = Box::new(Graph {
        slots: slots.slots,
        keep: slots.keep,
        settings: slots.settings,
        samples: slots.samples,
        channels,
        inserts,
        order,
        events,
        audio,
        lanes,
        tempo: song.tempo.clamp(10.0, 999.0),
        beats: i64::from(song.signature.beats.max(1)),
        loop_start,
        loop_end,
        cursor: 0,
        scratch_left: vec![0.0; CHUNK],
        scratch_right: vec![0.0; CHUNK],
    });
    Compiled { graph, mirror, missing: slots.missing }
}
