//! A phaser: the sound through a chain of filters that shift its phase, mixed back with
//! itself, so that notches sweep up and down the spectrum.

use crate::param::Param;
use crate::{Context, Event, Processor};
use std::f32::consts::{PI, TAU};

const STAGES: &[&str] = &["4", "6", "8"];

pub const PARAMS: &[Param] = &[
    Param::log("rate", "Speed", 0.05, 5.0, 0.4, "Hz"),
    Param::linear("depth", "Depth", 0.0, 1.0, 0.7, ""),
    Param::log("center", "Center", 200.0, 5000.0, 900.0, "Hz"),
    Param::linear("feedback", "Feedback", 0.0, 0.9, 0.5, ""),
    Param::choice("stages", "Stages", STAGES, 1),
    Param::linear("mix", "Mix", 0.0, 1.0, 0.5, ""),
];

pub struct Phaser {
    sample_rate: f32,
    values: [f32; 6],
    /// Each side's stages: what last came into each, and what last went out.
    stages: [[(f32, f32); 8]; 2],
    /// What each side's chain last gave, fed back in.
    last: [f32; 2],
    /// Where the sweep is, in turns.
    phase: f32,
}

impl Phaser {
    pub fn new(sample_rate: f32) -> Phaser {
        let mut values = [0.0; 6];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        Phaser { sample_rate, values, stages: [[(0.0, 0.0); 8]; 2], last: [0.0; 2], phase: 0.0 }
    }
}

impl Processor for Phaser {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let [rate, depth, center, feedback, stages, mix] = self.values;
        let count = 4 + 2 * (stages.round() as usize).min(2);
        let highest = self.sample_rate * 0.45;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            for (side, sample) in [l, r].into_iter().enumerate() {
                // two octaves either side of the center at full depth; the right a quarter turn on
                let sway = (TAU * (self.phase + side as f32 * 0.25)).sin();
                let at = (center * 4f32.powf(sway * depth)).min(highest);
                let t = (PI * at / self.sample_rate).tan();
                let a = (t - 1.0) / (t + 1.0);
                let mut x = *sample + self.last[side] * feedback;
                for (x1, y1) in &mut self.stages[side][..count] {
                    let y = a * x + *x1 - a * *y1;
                    *x1 = x;
                    *y1 = y;
                    x = y;
                }
                self.last[side] = x;
                *sample = *sample * (1.0 - mix) + x * mix;
            }
            self.phase = (self.phase + rate / self.sample_rate).fract();
        }
    }

    fn reset(&mut self) {
        self.stages = [[(0.0, 0.0); 8]; 2];
        self.last = [0.0; 2];
        self.phase = 0.0;
    }
}
