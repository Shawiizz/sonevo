//! A chorus: the sound with a copy of itself a few milliseconds late, the lateness swaying,
//! so that it sounds wider and thicker. Short and fed back, it is a flanger.

use crate::param::Param;
use crate::{Context, Event, Processor};
use std::f32::consts::TAU;

/// The most lateness the sway adds, in milliseconds.
const SWAY_MS: f32 = 5.0;
const LONGEST_MS: f32 = 30.0 + SWAY_MS + 1.0;

pub const PARAMS: &[Param] = &[
    Param::log("rate", "Speed", 0.05, 5.0, 0.8, "Hz"),
    Param::linear("depth", "Depth", 0.0, 1.0, 0.5, ""),
    Param::log("delay", "Delay", 1.0, 30.0, 12.0, "ms"),
    Param::linear("feedback", "Feedback", -0.9, 0.9, 0.0, ""),
    Param::linear("width", "Width", 0.0, 1.0, 1.0, ""),
    Param::linear("mix", "Mix", 0.0, 1.0, 0.5, ""),
];

pub struct Chorus {
    sample_rate: f32,
    values: [f32; 6],
    lines: [Vec<f32>; 2],
    write: usize,
    /// Where the sway is, in turns.
    phase: f32,
}

impl Chorus {
    pub fn new(sample_rate: f32) -> Chorus {
        let length = (LONGEST_MS / 1000.0 * sample_rate) as usize + 2;
        let mut values = [0.0; 6];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        Chorus { sample_rate, values, lines: [vec![0.0; length], vec![0.0; length]], write: 0, phase: 0.0 }
    }
}

/// What `line` held `late` samples before `write`, between two samples.
fn read(line: &[f32], write: usize, late: f32) -> f32 {
    let length = line.len();
    let whole = late as usize;
    let part = late - whole as f32;
    let a = line[(write + length - whole) % length];
    let b = line[(write + 2 * length - whole - 1) % length];
    a + (b - a) * part
}

impl Processor for Chorus {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let [rate, depth, delay, feedback, width, mix] = self.values;
        let per_ms = self.sample_rate / 1000.0;
        let length = self.lines[0].len();
        let longest = (length - 2) as f32;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            for (side, sample) in [l, r].into_iter().enumerate() {
                // the right sways a quarter turn after the left, as wide as asked
                let phase = self.phase - side as f32 * 0.25 * width;
                let sway = 0.5 + 0.5 * (TAU * phase).sin();
                let late = ((delay + depth * SWAY_MS * sway) * per_ms).clamp(1.0, longest);
                let line = &mut self.lines[side];
                let echo = read(line, self.write, late);
                line[self.write] = *sample + echo * feedback;
                *sample = *sample * (1.0 - mix) + echo * mix;
            }
            self.write = (self.write + 1) % length;
            self.phase = (self.phase + rate / self.sample_rate).fract();
        }
    }

    fn reset(&mut self) {
        for line in &mut self.lines {
            line.fill(0.0);
        }
        self.phase = 0.0;
    }
}
