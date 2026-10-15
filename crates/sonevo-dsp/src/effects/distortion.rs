//! A distortion: the sound driven into a soft clip, its harshness tamed by a low-pass.

use crate::param::Param;
use crate::util::{db_to_gain, soft_clip, Svf};
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::linear("drive", "Drive", 0.0, 36.0, 12.0, "dB"),
    Param::log("tone", "Tone", 1000.0, 20000.0, 12000.0, "Hz"),
    Param::linear("mix", "Mix", 0.0, 1.0, 1.0, ""),
    Param::linear("output", "Output", -24.0, 6.0, -6.0, "dB"),
];

pub struct Distortion {
    sample_rate: f32,
    values: [f32; 4],
    filters: [Svf; 2],
}

impl Distortion {
    pub fn new(sample_rate: f32) -> Distortion {
        let mut values = [0.0; 4];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        let mut distortion = Distortion { sample_rate, values, filters: [Svf::default(); 2] };
        distortion.apply();
        distortion
    }

    fn apply(&mut self) {
        for f in &mut self.filters {
            f.set(self.values[1], 0.0, self.sample_rate);
        }
    }
}

impl Processor for Distortion {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
            self.apply();
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let drive = db_to_gain(self.values[0]);
        let (mix, output) = (self.values[2], db_to_gain(self.values[3]));
        for (samples, filter) in [left, right].into_iter().zip(self.filters.iter_mut()) {
            for s in samples.iter_mut() {
                let wet = filter.process(soft_clip(*s * drive)).low;
                *s = (*s * (1.0 - mix) + wet * mix) * output;
            }
        }
    }

    fn reset(&mut self) {
        self.filters.iter_mut().for_each(Svf::clear);
    }
}
