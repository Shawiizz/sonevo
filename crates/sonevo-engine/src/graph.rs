//! A song compiled for the audio thread: everything it plays found by index, every buffer
//! allocated beforehand.

use sonevo_dsp::{Event, Param, Processor, Sample};
use sonevo_project::Point;
use std::sync::Arc;

/// The most frames played at once; a longer block is played this many at a time.
pub const CHUNK: usize = 256;
/// The most note events a channel takes in a chunk; more are left out.
pub const EVENTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    pub tick: i64,
    pub channel: usize,
    pub key: u8,
    pub velocity: f32,
    pub on: bool,
}

/// A left and a right gain, as volume and pan give them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gains {
    pub left: f32,
    pub right: f32,
}

impl Gains {
    pub const SILENT: Gains = Gains { left: 0.0, right: 0.0 };

    pub fn new(gain: f32, pan: f32) -> Gains {
        let pan = pan.clamp(-1.0, 1.0);
        Gains { left: gain * (1.0 - pan).min(1.0), right: gain * (1.0 + pan).min(1.0) }
    }
}

pub struct ChannelNode {
    pub slot: Option<usize>,
    pub volume: f32,
    pub pan: f32,
    pub audible: bool,
    pub insert: usize,
    /// The gains the last chunk ended on, glided from to the new ones.
    pub applied: Gains,
    pub events: Vec<Event>,
    /// The keys down, and those to be let go of first thing.
    pub held: u128,
    pub release: u128,
    pub meter: usize,
}

impl ChannelNode {
    /// A fader from 0 to 1 is a gain up to +3.9 dB, unity at 0.8.
    pub fn gains(&self) -> Gains {
        if self.audible {
            Gains::new(self.volume * self.volume / 0.64, self.pan)
        } else {
            Gains::SILENT
        }
    }
}

pub struct InsertNode {
    /// Each effect's slot, and the mixer track it listens to if it does.
    pub effects: Vec<(usize, Option<usize>)>,
    pub volume: f32,
    pub pan: f32,
    pub audible: bool,
    pub sends: Vec<(usize, f32)>,
    /// None for the master, which goes to the speakers.
    pub output: Option<usize>,
    pub applied: Gains,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub meter: usize,
}

impl InsertNode {
    pub fn gains(&self) -> Gains {
        if self.audible {
            Gains::new(self.volume, self.pan)
        } else {
            Gains::SILENT
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lever {
    ChannelVolume(usize),
    ChannelPan(usize),
    InsertVolume(usize),
    InsertPan(usize),
    Param { slot: usize, index: usize, param: Param },
    Tempo,
}

/// The lowest and highest tempo an automation goes between.
pub const TEMPO_RANGE: (f64, f64) = (60.0, 240.0);

impl Lever {
    /// What a point's value from 0 to 1 is for this lever.
    pub fn value(&self, normal: f64) -> f64 {
        match self {
            Lever::ChannelVolume(_) => normal,
            Lever::InsertVolume(_) => normal * 1.25,
            Lever::ChannelPan(_) | Lever::InsertPan(_) => normal * 2.0 - 1.0,
            Lever::Param { param, .. } => param.from_normal(normal),
            Lever::Tempo => TEMPO_RANGE.0 + normal * (TEMPO_RANGE.1 - TEMPO_RANGE.0),
        }
    }
}

pub struct AutomationClip {
    pub start: i64,
    pub end: i64,
    pub offset: i64,
    pub points: Vec<Point>,
}

impl AutomationClip {
    pub fn at(&self, tick: f64) -> f64 {
        let at = tick - self.start as f64 + self.offset as f64;
        let points = &self.points;
        let Some(first) = points.first() else { return 0.0 };
        let next = points.partition_point(|p| (p.at as f64) <= at);
        if next == 0 {
            return first.value;
        }
        let a = points[next - 1];
        let Some(b) = points.get(next) else { return a.value };
        let x = ((at - a.at as f64) / (b.at - a.at).max(1) as f64).clamp(0.0, 1.0);
        let bent = if a.curve == 0.0 { x } else { x.powf(4f64.powf(f64::from(a.curve))) };
        a.value + (b.value - a.value) * bent
    }
}

pub struct Lane {
    pub lever: Lever,
    /// In time order, none overlapping.
    pub clips: Vec<AutomationClip>,
    pub last: f64,
}

pub struct AudioClip {
    pub start: i64,
    pub end: i64,
    pub offset: i64,
    pub sample: Arc<Sample>,
    pub gain: f32,
}

pub struct Graph {
    pub slots: Vec<Option<Box<dyn Processor>>>,
    /// The processors the graph before had and this one keeps: (here, there).
    pub keep: Vec<(usize, usize)>,
    /// Settings for the processors kept, to be made as the graph comes in.
    pub settings: Vec<(usize, usize, f64)>,
    /// Sounds for the processors kept; once given, the sounds they had, to be dropped with
    /// the graph.
    pub samples: Vec<(usize, Option<Arc<Sample>>)>,
    pub channels: Vec<ChannelNode>,
    /// The mixer tracks, the master last.
    pub inserts: Vec<InsertNode>,
    /// The order the mixer tracks are played in, every one before those it goes to.
    pub order: Vec<usize>,
    pub events: Vec<NoteEvent>,
    pub audio: Vec<AudioClip>,
    pub lanes: Vec<Lane>,
    pub tempo: f64,
    /// Beats to a bar, for the metronome.
    pub beats: i64,
    pub loop_start: i64,
    pub loop_end: i64,
    /// Where in `events` playing is.
    pub cursor: usize,
    pub scratch_left: Vec<f32>,
    pub scratch_right: Vec<f32>,
}

impl Graph {
    pub fn master(&self) -> usize {
        self.inserts.len() - 1
    }

    /// Puts the cursor on the first event at or after `tick`.
    pub fn seek(&mut self, tick: f64) {
        self.cursor = self.events.partition_point(|e| (e.tick as f64) < tick);
    }

    /// Every key down let go of, first thing in the next chunk.
    pub fn release_all(&mut self) {
        for c in &mut self.channels {
            c.release |= c.held;
        }
    }
}
