//! A lo-fi crusher: the sound held a few samples at a time, as an old sampler at a low rate
//! would, and its levels rounded to fewer steps, as with fewer bits.

use crate::param::Param;
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::linear("bits", "Bits", 2.0, 16.0, 10.0, ""),
    Param::log("rate", "Sample rate", 1000.0, 48000.0, 16000.0, "Hz"),
    Param::linear("mix", "Mix", 0.0, 1.0, 1.0, ""),
];

pub struct Crusher {
    sample_rate: f32,
    values: [f32; 3],
    /// How far into holding the sample it is, from 0 to 1, and the sample held.
    phase: f32,
    held: [f32; 2],
}

impl Crusher {
    pub fn new(sample_rate: f32) -> Crusher {
        let mut values = [0.0; 3];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        Crusher { sample_rate, values, phase: 1.0, held: [0.0; 2] }
    }
}

impl Processor for Crusher {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let [bits, rate, mix] = self.values;
        let steps = 2f32.powf(bits - 1.0);
        let step = (rate / self.sample_rate).min(1.0);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.phase += step;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                self.held = [(*l * steps).round() / steps, (*r * steps).round() / steps];
            }
            *l = *l * (1.0 - mix) + self.held[0] * mix;
            *r = *r * (1.0 - mix) + self.held[1] * mix;
        }
    }

    fn reset(&mut self) {
        self.phase = 1.0;
        self.held = [0.0; 2];
    }
}
