//! The audio thread's side: it plays the graph it was last given, block after block, never
//! allocating, never waiting.

use crate::graph::{Gains, Graph, Lever, LiveArp, CHUNK, EVENTS};
use crate::meters::{self, Meters};
use crate::scope::Scope;
use rtrb::{Consumer, Producer};
use sonevo_dsp::{Context, Event, Sample};
use sonevo_project::PPQ;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

pub enum Command {
    Swap(Box<Graph>),
    Play {
        once: bool,
    },
    Pause,
    /// Pause, and back to where playing started.
    Stop,
    Seek(f64),
    Note {
        channel: usize,
        key: u8,
        velocity: f32,
        on: bool,
    },
    Param {
        slot: usize,
        index: usize,
        value: f64,
    },
    ChannelLevel {
        channel: usize,
        volume: f32,
        pan: f32,
    },
    InsertLevel {
        insert: usize,
        volume: f32,
        pan: f32,
    },
    Metronome(bool),
    /// A sound listened to as it is, over the song; none stops it.
    Listen(Option<Arc<Sample>>),
    /// Every voice and every tail cut.
    Panic,
}

/// What the interface reads of the audio thread.
pub struct Shared {
    position: AtomicU64,
    playing: AtomicBool,
    /// The most of a block's time a block took to play, since the last look, as bits of a
    /// float.
    load: AtomicU32,
    pub meters: Meters,
    /// How far each effect slot brought the sound down, in dB, the most since the last look.
    pub reductions: Meters,
    pub scope: Scope,
}

impl Shared {
    pub fn new() -> Shared {
        Shared {
            position: AtomicU64::new(0f64.to_bits()),
            playing: AtomicBool::new(false),
            load: AtomicU32::new(0),
            meters: Meters::new(),
            reductions: Meters::with(1024),
            scope: Scope::new(),
        }
    }

    /// Where playing is, in ticks.
    pub fn position(&self) -> f64 {
        f64::from_bits(self.position.load(Ordering::Relaxed))
    }

    pub fn playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// The most of its time a block took since the last look: 1 is all of it.
    pub fn load(&self) -> f32 {
        f32::from_bits(self.load.swap(0, Ordering::Relaxed))
    }
}

/// The click of the metronome: a short tone, higher on a bar's first beat.
struct Click {
    left: usize,
    phase: f32,
    step: f32,
}

const CLICK_SECONDS: f32 = 0.03;
/// A sound listened to a little under its level, beside the song.
const LISTEN_GAIN: f32 = 0.8;

pub struct Renderer {
    commands: Consumer<Command>,
    garbage: Producer<Box<Graph>>,
    shared: Arc<Shared>,
    graph: Option<Box<Graph>>,
    sample_rate: f32,
    playing: bool,
    /// Stops at the loop's end instead of going round: a song played to a file.
    once: bool,
    position: f64,
    started_at: f64,
    previews: [Option<(usize, Event)>; 64],
    metronome: bool,
    click: Click,
    /// The sound listened to, and where in it playing is, in its own frames.
    listening: Option<(Arc<Sample>, f64)>,
}

impl Renderer {
    pub(crate) fn new(
        commands: Consumer<Command>,
        garbage: Producer<Box<Graph>>,
        shared: Arc<Shared>,
        sample_rate: f32,
    ) -> Renderer {
        Renderer {
            commands,
            garbage,
            shared,
            graph: None,
            sample_rate,
            playing: false,
            once: false,
            position: 0.0,
            started_at: 0.0,
            previews: [None; 64],
            metronome: false,
            click: Click { left: 0, phase: 0.0, step: 0.0 },
            listening: None,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    fn retire(&mut self, graph: Box<Graph>) {
        if let Err(rtrb::PushError::Full(graph)) = self.garbage.push(graph) {
            // nowhere to send it to be freed: kept forever rather than freed here
            std::mem::forget(graph);
        }
    }

    fn swap(&mut self, mut next: Box<Graph>) {
        if let Some(mut old) = self.graph.take() {
            for &(here, there) in &next.keep {
                next.slots[here] = old.slots.get_mut(there).and_then(Option::take);
            }
            // a kept instrument's keys stay down, let go of by the new graph's notes, or at
            // the latest where the loop goes round
            for channel in &old.channels {
                let Some(slot) = channel.slot else { continue };
                let Some(&(here, _)) = next.keep.iter().find(|&&(_, there)| there == slot) else { continue };
                if let Some(c) = next.channels.iter_mut().find(|c| c.slot == Some(here)) {
                    c.held = channel.held;
                    c.release = channel.release;
                    c.applied = channel.applied;
                    match (&mut c.live, channel.live) {
                        (Some(new), Some(old)) => *new = LiveArp { arp: new.arp, ..old },
                        // an arpeggiator turned on: the keys held go to it, their notes let go of
                        (Some(new), None) => {
                            new.keys = channel.held;
                            c.release |= channel.held;
                        }
                        // turned off: its note sounding let go of
                        (None, Some(old)) => {
                            if let Some((key, _)) = old.sounding {
                                c.release |= 1u128 << (key & 127);
                            }
                        }
                        (None, None) => {}
                    }
                }
            }
            for (new, old) in next.inserts.iter_mut().zip(&old.inserts) {
                new.applied = old.applied;
            }
            self.retire(old);
        }
        for &(slot, index, value) in &next.settings {
            if let Some(p) = next.slots[slot].as_mut() {
                p.set(index, value);
            }
        }
        let mut samples = std::mem::take(&mut next.samples);
        for (slot, sample) in &mut samples {
            if let Some(p) = next.slots[*slot].as_mut() {
                *sample = p.set_sample(sample.take());
            }
        }
        next.samples = samples;
        if self.position >= next.loop_end as f64 || self.position < 0.0 {
            self.position = next.loop_start as f64;
        }
        next.seek(self.position);
        self.graph = Some(next);
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Swap(graph) => self.swap(graph),
            Command::Play { once } => {
                if !self.playing {
                    self.started_at = self.position;
                }
                self.playing = true;
                self.once = once;
            }
            Command::Pause => {
                self.playing = false;
                if let Some(g) = &mut self.graph {
                    g.release_all();
                }
            }
            Command::Stop => {
                self.playing = false;
                self.position = self.started_at;
                if let Some(g) = &mut self.graph {
                    g.release_all();
                    g.seek(self.position);
                }
            }
            Command::Seek(tick) => {
                self.position = tick.max(0.0);
                self.started_at = self.position;
                if let Some(g) = &mut self.graph {
                    if self.playing {
                        g.release_all();
                    }
                    g.seek(self.position);
                }
            }
            Command::Note { channel, key, velocity, on } => {
                if let Some(free) = self.previews.iter_mut().find(|p| p.is_none()) {
                    *free = Some((channel, Event { at: 0, key, velocity, on }));
                }
            }
            Command::Param { slot, index, value } => {
                if let Some(p) =
                    self.graph.as_mut().and_then(|g| g.slots.get_mut(slot)).and_then(Option::as_mut)
                {
                    p.set(index, value);
                }
            }
            Command::ChannelLevel { channel, volume, pan } => {
                if let Some(c) = self.graph.as_mut().and_then(|g| g.channels.get_mut(channel)) {
                    c.volume = volume;
                    c.pan = pan;
                }
            }
            Command::InsertLevel { insert, volume, pan } => {
                if let Some(i) = self.graph.as_mut().and_then(|g| g.inserts.get_mut(insert)) {
                    i.volume = volume;
                    i.pan = pan;
                }
            }
            Command::Metronome(on) => self.metronome = on,
            // the engine keeps the sound too: never freed here
            Command::Listen(sample) => self.listening = sample.map(|s| (s, 0.0)),
            Command::Panic => {
                if let Some(g) = &mut self.graph {
                    for p in g.slots.iter_mut().flatten() {
                        p.reset();
                    }
                    for c in &mut g.channels {
                        c.held = 0;
                        c.release = 0;
                        if let Some(live) = &mut c.live {
                            *live = LiveArp::new(live.arp);
                        }
                    }
                }
            }
        }
    }

    /// Plays a block into `left` and `right`, as long as each other.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let start = std::time::Instant::now();
        while let Ok(command) = self.commands.pop() {
            self.command(command);
        }
        for (l, r) in left.chunks_mut(CHUNK).zip(right.chunks_mut(CHUNK)) {
            self.chunk(l, r);
        }
        self.shared.position.store(self.position.to_bits(), Ordering::Relaxed);
        self.shared.playing.store(self.playing, Ordering::Relaxed);
        let given = left.len() as f32 / self.sample_rate;
        if given > 0.0 {
            let load = start.elapsed().as_secs_f32() / given;
            self.shared.load.fetch_max(load.to_bits(), Ordering::Relaxed);
        }
    }

    /// Plays a block into interleaved frames of `channels` samples, the song's two in the
    /// first two and silence in the others.
    pub fn render_interleaved(&mut self, out: &mut [f32], channels: usize) {
        let mut left = [0.0f32; CHUNK];
        let mut right = [0.0f32; CHUNK];
        let channels = channels.max(1);
        for frames in out.chunks_mut(CHUNK * channels) {
            let n = frames.len() / channels;
            self.render(&mut left[..n], &mut right[..n]);
            for (i, frame) in frames.chunks_exact_mut(channels).enumerate() {
                frame.fill(0.0);
                if channels == 1 {
                    frame[0] = (left[i] + right[i]) * 0.5;
                } else {
                    frame[0] = left[i];
                    frame[1] = right[i];
                }
            }
        }
    }

    fn chunk(&mut self, out_left: &mut [f32], out_right: &mut [f32]) {
        let frames = out_left.len();
        let Some(mut graph) = self.graph.take() else {
            out_left.fill(0.0);
            out_right.fill(0.0);
            return;
        };
        let (next, ended) = self.events(&mut graph, frames);
        if self.playing {
            automate(&mut graph, self.position);
        }
        let context = Context { sample_rate: self.sample_rate, tempo: graph.tempo, key: None };
        for insert in &mut graph.inserts {
            insert.left[..frames].fill(0.0);
            insert.right[..frames].fill(0.0);
        }

        let Graph { slots, channels, inserts, scratch_left, scratch_right, .. } = &mut *graph;
        for channel in channels.iter_mut() {
            let Some(processor) = channel.slot.and_then(|s| slots[s].as_mut()) else {
                channel.events.clear();
                continue;
            };
            let (l, r) = (&mut scratch_left[..frames], &mut scratch_right[..frames]);
            l.fill(0.0);
            r.fill(0.0);
            processor.process(&context, &channel.events, l, r);
            channel.events.clear();
            let gains = channel.gains();
            glide(l, r, channel.applied, gains);
            channel.applied = gains;
            self.shared.meters.hear(channel.meter, meters::peak(l).max(meters::peak(r)));
            let insert = &mut inserts[channel.insert];
            mix(&mut insert.left[..frames], l, 1.0);
            mix(&mut insert.right[..frames], r, 1.0);
        }

        if self.playing {
            self.audio(&mut graph, frames);
        }

        let master = graph.master();
        for k in 0..graph.order.len() {
            let i = graph.order[k];
            let Graph { slots, inserts, scratch_left, scratch_right, .. } = &mut *graph;
            for e in 0..inserts[i].effects.len() {
                let (slot, key) = inserts[i].effects[e];
                let Some(p) = slots[slot].as_mut() else { continue };
                // the track listened to was played before this one: its sound is final
                let key = match key {
                    Some(k) => {
                        scratch_left[..frames].copy_from_slice(&inserts[k].left[..frames]);
                        scratch_right[..frames].copy_from_slice(&inserts[k].right[..frames]);
                        Some((&scratch_left[..frames], &scratch_right[..frames]))
                    }
                    None => None,
                };
                let insert = &mut inserts[i];
                p.process(
                    &Context { key, ..context },
                    &[],
                    &mut insert.left[..frames],
                    &mut insert.right[..frames],
                );
                self.shared.reductions.hear(slot, p.reduction());
            }
            let insert = &mut inserts[i];
            let gains = insert.gains();
            let (l, r) = (&mut insert.left[..frames], &mut insert.right[..frames]);
            glide(l, r, insert.applied, gains);
            insert.applied = gains;
            self.shared.meters.hear(insert.meter, meters::peak(l).max(meters::peak(r)));
            if i == master {
                continue;
            }
            // the sound goes on to the tracks after: taken out to be added there
            let (from_left, from_right) =
                (std::mem::take(&mut insert.left), std::mem::take(&mut insert.right));
            let sends = insert.sends.len();
            for route in 0..=sends {
                let (to, amount) = match (route < sends, inserts[i].output) {
                    (true, _) => inserts[i].sends[route],
                    (false, Some(output)) => (output, 1.0),
                    (false, None) => break,
                };
                let target = &mut inserts[to];
                mix(&mut target.left[..frames], &from_left[..frames], amount);
                mix(&mut target.right[..frames], &from_right[..frames], amount);
            }
            let insert = &mut inserts[i];
            insert.left = from_left;
            insert.right = from_right;
        }

        let master = &graph.inserts[master];
        for (o, s) in out_left.iter_mut().zip(&master.left[..frames]) {
            *o = safe(*s);
        }
        for (o, s) in out_right.iter_mut().zip(&master.right[..frames]) {
            *o = safe(*s);
        }
        self.shared.scope.keep(out_left, out_right);
        if self.metronome && self.playing {
            self.metronome(&graph, out_left, out_right);
        }
        self.listen(out_left, out_right);
        self.position = next;
        if ended {
            self.playing = false;
        }
        self.graph = Some(graph);
    }

    /// Each channel's note events for the chunk, at their frames; where playing is after it,
    /// and whether it came to the end of a song played once.
    fn events(&mut self, graph: &mut Graph, frames: usize) -> (f64, bool) {
        for channel in &mut graph.channels {
            for key in Keys(std::mem::take(&mut channel.release) & channel.held) {
                push(channel, Event { at: 0, key, velocity: 0.0, on: false });
            }
        }
        for preview in &mut self.previews {
            let Some((channel, event)) = preview.take() else { continue };
            if let Some(c) = graph.channels.get_mut(channel) {
                match &mut c.live {
                    Some(live) => live.press(&event),
                    None => push(c, event),
                }
            }
        }
        let per_frame = ticks_per_frame(graph.tempo, self.sample_rate);
        for c in &mut graph.channels {
            let Some(mut live) = c.live else { continue };
            live.run(frames, live.arp.step as f64 / per_frame, |event| push(c, event));
            c.live = Some(live);
        }
        if !self.playing {
            return (self.position, false);
        }
        let (mut frame, mut tick) = (0usize, self.position);
        while frame < frames {
            let left = (graph.loop_end as f64 - tick).max(0.0);
            let n = ((left / per_frame).ceil() as usize).clamp(1, frames - frame);
            let until = (tick + n as f64 * per_frame).min(graph.loop_end as f64);
            let wraps = until >= graph.loop_end as f64;
            while let Some(e) = graph.events.get(graph.cursor) {
                if e.tick as f64 >= until {
                    break;
                }
                // the first frame at or past the event, a hair's rounding forgiven
                let at = ((e.tick as f64 - tick) / per_frame - 1e-6).ceil().max(0.0) as usize;
                if at >= n && !wraps {
                    break;
                }
                let at = frame + at.min(n - 1);
                let event = Event { at: at as u32, key: e.key, velocity: e.velocity, on: e.on };
                push(&mut graph.channels[e.channel], event);
                graph.cursor += 1;
            }
            frame += n;
            if wraps {
                let at = (frame - 1) as u32;
                for c in &mut graph.channels {
                    for key in Keys(c.held) {
                        push(c, Event { at, key, velocity: 0.0, on: false });
                    }
                }
                tick = graph.loop_start as f64;
                graph.cursor = graph.events.partition_point(|e| e.tick < graph.loop_start);
                if self.once {
                    // what is left of the chunk plays the tails
                    arpeggios_in_time(graph);
                    return (tick, true);
                }
            } else {
                tick = until;
            }
        }
        arpeggios_in_time(graph);
        (tick, false)
    }

    /// The audio clips' sound for the chunk, into the master.
    fn audio(&self, graph: &mut Graph, frames: usize) {
        let per_frame = ticks_per_frame(graph.tempo, self.sample_rate);
        let seconds_per_tick = 60.0 / (graph.tempo * PPQ as f64);
        let (start, end) = (self.position, self.position + per_frame * frames as f64);
        let loop_end = graph.loop_end as f64;
        let Graph { audio, inserts, .. } = graph;
        for clip in audio.iter().filter(|c| (c.start as f64) < end && (c.end as f64) > start) {
            let insert = &mut inserts[clip.insert];
            let sample = &clip.sample;
            let rate = f64::from(sample.sample_rate);
            let last = sample.frames() as f64 - 2.0;
            for i in 0..frames {
                let tick = start + i as f64 * per_frame;
                if tick < clip.start as f64 || tick >= clip.end as f64 || tick >= loop_end {
                    continue;
                }
                let along = (tick - clip.start as f64 + clip.offset as f64) * seconds_per_tick * rate;
                let at = if clip.reverse { last - along } else { along };
                if at < 0.0 || at > last {
                    continue;
                }
                let index = at as usize;
                let fraction = (at - index as f64) as f32;
                let lerp = |s: &[f32]| s[index] + (s[index + 1] - s[index]) * fraction;
                let mut gain = clip.gain;
                if clip.fade_in > 0 {
                    gain *= ((tick - clip.start as f64) / clip.fade_in as f64).min(1.0) as f32;
                }
                if clip.fade_out > 0 {
                    gain *= ((clip.end as f64 - tick) / clip.fade_out as f64).min(1.0) as f32;
                }
                insert.left[i] += lerp(&sample.left) * gain;
                insert.right[i] += lerp(&sample.right) * gain;
            }
        }
    }

    /// The sound listened to, added as it is, at its own rate.
    fn listen(&mut self, left: &mut [f32], right: &mut [f32]) {
        let Some((sample, at)) = &mut self.listening else { return };
        let step = f64::from(sample.sample_rate / self.sample_rate);
        let frames = sample.frames();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let index = *at as usize;
            if index + 1 >= frames {
                self.listening = None;
                return;
            }
            let fraction = (*at - index as f64) as f32;
            let lerp = |s: &[f32]| s[index] + (s[index + 1] - s[index]) * fraction;
            *l += lerp(&sample.left) * LISTEN_GAIN;
            *r += lerp(&sample.right) * LISTEN_GAIN;
            *at += step;
        }
    }

    fn metronome(&mut self, graph: &Graph, left: &mut [f32], right: &mut [f32]) {
        let per_frame = ticks_per_frame(graph.tempo, self.sample_rate);
        let length = (CLICK_SECONDS * self.sample_rate) as usize;
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            let tick = self.position + i as f64 * per_frame;
            let beat = (tick / PPQ as f64).ceil() as i64;
            let beat_tick = (beat * PPQ) as f64;
            if beat_tick >= tick && beat_tick < tick + per_frame && beat_tick < graph.loop_end as f64 {
                let frequency = if beat % graph.beats == 0 { 1760.0 } else { 1320.0 };
                self.click = Click { left: length, phase: 0.0, step: frequency / self.sample_rate };
            }
            if self.click.left > 0 {
                let envelope = self.click.left as f32 / length as f32;
                let s = (self.click.phase * std::f32::consts::TAU).sin() * envelope * envelope * 0.3;
                self.click.phase = (self.click.phase + self.click.step).fract();
                self.click.left -= 1;
                *l += s;
                *r += s;
            }
        }
    }
}

/// The events of every channel with a live arpeggio in time order.
fn arpeggios_in_time(graph: &mut Graph) {
    for c in graph.channels.iter_mut().filter(|c| c.live.is_some()) {
        in_time(&mut c.events);
    }
}

/// `events` in time order, those at the same frame as they came: the live arpeggio's notes
/// and the song's, made apart, played together.
fn in_time(events: &mut [Event]) {
    for i in 1..events.len() {
        let mut j = i;
        while j > 0 && events[j - 1].at > events[j].at {
            events.swap(j - 1, j);
            j -= 1;
        }
    }
}

fn set_held(held: &mut u128, event: &Event) {
    let bit = 1u128 << (event.key & 127);
    if event.on {
        *held |= bit;
    } else {
        *held &= !bit;
    }
}

fn push(channel: &mut crate::graph::ChannelNode, event: Event) {
    if channel.events.len() < EVENTS {
        set_held(&mut channel.held, &event);
        channel.events.push(event);
    }
}

/// The keys of a set of keys down.
struct Keys(u128);

impl Iterator for Keys {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if self.0 == 0 {
            return None;
        }
        let key = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Some(key)
    }
}

fn ticks_per_frame(tempo: f64, sample_rate: f32) -> f64 {
    tempo * PPQ as f64 / 60.0 / f64::from(sample_rate)
}

fn automate(graph: &mut Graph, tick: f64) {
    let Graph { lanes, channels, inserts, slots, tempo, .. } = graph;
    for lane in lanes.iter_mut() {
        let Some(clip) = lane.clips.iter().rev().find(|c| (c.start as f64) <= tick && tick < c.end as f64)
        else {
            continue;
        };
        let value = lane.lever.value(clip.at(tick));
        if value == lane.last {
            continue;
        }
        lane.last = value;
        match lane.lever {
            Lever::ChannelVolume(c) => channels[c].volume = value as f32,
            Lever::ChannelPan(c) => channels[c].pan = value as f32,
            Lever::InsertVolume(i) => inserts[i].volume = value as f32,
            Lever::InsertPan(i) => inserts[i].pan = value as f32,
            Lever::Param { slot, index, .. } => {
                if let Some(p) = slots[slot].as_mut() {
                    p.set(index, value);
                }
            }
            Lever::Tempo => *tempo = value,
        }
    }
}

/// Applies gains gliding from `from` to `to` over the chunk, so that a fader moved makes no
/// click.
fn glide(left: &mut [f32], right: &mut [f32], from: Gains, to: Gains) {
    let n = left.len().max(1) as f32;
    if from == to {
        left.iter_mut().for_each(|s| *s *= to.left);
        right.iter_mut().for_each(|s| *s *= to.right);
        return;
    }
    for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
        let x = (i + 1) as f32 / n;
        *l *= from.left + (to.left - from.left) * x;
        *r *= from.right + (to.right - from.right) * x;
    }
}

fn mix(into: &mut [f32], from: &[f32], amount: f32) {
    for (a, b) in into.iter_mut().zip(from) {
        *a += b * amount;
    }
}

/// What goes to the speakers: never anything but a number, never past +12 dB.
fn safe(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-4.0, 4.0)
    } else {
        0.0
    }
}
