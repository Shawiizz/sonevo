//! A three-band equalizer: a low shelf, a bell in the middle, a high shelf.

use crate::param::Param;
use crate::util::{Biquad, Shape};
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::log("lowFrequency", "Low", 20.0, 1000.0, 100.0, "Hz"),
    Param::linear("lowGain", "Low gain", -18.0, 18.0, 0.0, "dB"),
    Param::log("midFrequency", "Mid", 100.0, 10000.0, 1000.0, "Hz"),
    Param::linear("midGain", "Mid gain", -18.0, 18.0, 0.0, "dB"),
    Param::log("midQ", "Mid width", 0.3, 8.0, 1.0, ""),
    Param::log("highFrequency", "High", 1000.0, 20000.0, 8000.0, "Hz"),
    Param::linear("highGain", "High gain", -18.0, 18.0, 0.0, "dB"),
];

pub struct Eq {
    sample_rate: f32,
    values: [f32; 7],
    bands: [[Biquad; 3]; 2],
}

impl Eq {
    pub fn new(sample_rate: f32) -> Eq {
        let mut values = [0.0; 7];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        let mut eq = Eq { sample_rate, values, bands: [[Biquad::flat(); 3]; 2] };
        eq.apply();
        eq
    }

    fn apply(&mut self) {
        let v = self.values;
        for channel in &mut self.bands {
            channel[0].set(Shape::LowShelf, v[0], v[1], 0.707, self.sample_rate);
            channel[1].set(Shape::Peak, v[2], v[3], v[4], self.sample_rate);
            channel[2].set(Shape::HighShelf, v[5], v[6], 0.707, self.sample_rate);
        }
    }
}

impl Processor for Eq {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
            self.apply();
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        // all bands flat, the sound goes through untouched
        if self.values[1] == 0.0 && self.values[3] == 0.0 && self.values[6] == 0.0 {
            return;
        }
        for (samples, bands) in [left, right].into_iter().zip(self.bands.iter_mut()) {
            for s in samples.iter_mut() {
                *s = bands.iter_mut().fold(*s, |x, band| band.process(x));
            }
        }
    }

    fn reset(&mut self) {
        self.bands.iter_mut().flatten().for_each(Biquad::clear);
    }
}
