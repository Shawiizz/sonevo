//! A compressor: what goes over the threshold brought down by the ratio, both channels
//! together, a soft knee, the level made up after.

use crate::param::Param;
use crate::util::{db_to_gain, gain_to_db};
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::linear("threshold", "Threshold", -60.0, 0.0, -18.0, "dB"),
    Param::log("ratio", "Ratio", 1.0, 20.0, 4.0, ":1"),
    Param::log("attack", "Attack", 0.1, 100.0, 10.0, "ms"),
    Param::log("release", "Release", 10.0, 1000.0, 120.0, "ms"),
    Param::linear("knee", "Knee", 0.0, 12.0, 6.0, "dB"),
    Param::linear("makeup", "Makeup", 0.0, 24.0, 0.0, "dB"),
    Param::linear("mix", "Mix", 0.0, 1.0, 1.0, ""),
];

pub struct Compressor {
    sample_rate: f32,
    values: [f32; 7],
    attack: f32,
    release: f32,
    /// How much the level is brought down now, in dB (0 or less).
    reduction: f32,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Compressor {
        let mut values = [0.0; 7];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        let mut compressor = Compressor { sample_rate, values, attack: 0.0, release: 0.0, reduction: 0.0 };
        compressor.apply();
        compressor
    }

    fn apply(&mut self) {
        let coeff = |ms: f32| (-1.0 / (ms / 1000.0 * self.sample_rate)).exp();
        self.attack = coeff(self.values[2]);
        self.release = coeff(self.values[3]);
    }

    /// The reduction a level of `db` calls for.
    fn wanted(&self, db: f32) -> f32 {
        let (threshold, ratio, knee) = (self.values[0], self.values[1], self.values[4]);
        let over = db - threshold;
        let slope = 1.0 / ratio - 1.0;
        if knee > 0.0 && over.abs() < knee / 2.0 {
            slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
        } else if over > 0.0 {
            slope * over
        } else {
            0.0
        }
    }
}

impl Processor for Compressor {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
            self.apply();
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let makeup = db_to_gain(self.values[5]);
        let mix = self.values[6];
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let wanted = self.wanted(gain_to_db(l.abs().max(r.abs())));
            // coming down at the attack's pace, going back up at the release's
            let coeff = if wanted < self.reduction { self.attack } else { self.release };
            self.reduction = wanted + (self.reduction - wanted) * coeff;
            let gain = db_to_gain(self.reduction) * makeup;
            *l = *l * (1.0 - mix) + *l * gain * mix;
            *r = *r * (1.0 - mix) + *r * gain * mix;
        }
    }

    fn reset(&mut self) {
        self.reduction = 0.0;
    }
}
