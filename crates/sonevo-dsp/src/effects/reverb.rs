//! A reverb after Jezar's Freeverb: eight damped combs and four all-passes a side, the right
//! side's a little longer than the left's so that the space is wide.

use crate::param::Param;
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::linear("size", "Size", 0.0, 1.0, 0.6, ""),
    Param::linear("damping", "Damping", 0.0, 1.0, 0.4, ""),
    Param::linear("width", "Width", 0.0, 1.0, 1.0, ""),
    Param::linear("mix", "Mix", 0.0, 1.0, 0.25, ""),
];

/// Freeverb's lengths at 44.1 kHz, scaled to the sample rate.
const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;
/// What goes in is brought down this much, the combs adding up.
const INPUT: f32 = 0.015;

struct Comb {
    buffer: Vec<f32>,
    at: usize,
    store: f32,
}

impl Comb {
    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let out = self.buffer[self.at];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buffer[self.at] = input + self.store * feedback;
        self.at = (self.at + 1) % self.buffer.len();
        out
    }
}

struct Allpass {
    buffer: Vec<f32>,
    at: usize,
}

impl Allpass {
    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.buffer[self.at];
        self.buffer[self.at] = input + delayed * 0.5;
        self.at = (self.at + 1) % self.buffer.len();
        delayed - input
    }
}

struct Side {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
}

impl Side {
    fn new(sample_rate: f32, extra: usize) -> Side {
        let scale = |n: usize| (((n + extra) as f32) * sample_rate / 44_100.0) as usize + 1;
        Side {
            combs: COMBS.iter().map(|&n| Comb { buffer: vec![0.0; scale(n)], at: 0, store: 0.0 }).collect(),
            allpasses: ALLPASSES.iter().map(|&n| Allpass { buffer: vec![0.0; scale(n)], at: 0 }).collect(),
        }
    }

    fn process(&mut self, input: f32, feedback: f32, damp: f32) -> f32 {
        let wet: f32 = self.combs.iter_mut().map(|c| c.process(input, feedback, damp)).sum();
        self.allpasses.iter_mut().fold(wet, |x, a| a.process(x))
    }

    fn clear(&mut self) {
        for c in &mut self.combs {
            c.buffer.fill(0.0);
            c.store = 0.0;
        }
        for a in &mut self.allpasses {
            a.buffer.fill(0.0);
        }
    }
}

pub struct Reverb {
    values: [f32; 4],
    left: Side,
    right: Side,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Reverb {
        let mut values = [0.0; 4];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        Reverb { values, left: Side::new(sample_rate, 0), right: Side::new(sample_rate, SPREAD) }
    }
}

impl Processor for Reverb {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let [size, damping, width, mix] = self.values;
        let feedback = 0.7 + size * 0.28;
        let damp = damping * 0.4;
        let (wet1, wet2) = (mix * 3.0 * (width / 2.0 + 0.5), mix * 3.0 * ((1.0 - width) / 2.0));
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let input = (*l + *r) * INPUT;
            let out_l = self.left.process(input, feedback, damp);
            let out_r = self.right.process(input, feedback, damp);
            *l += out_l * wet1 + out_r * wet2;
            *r += out_r * wet1 + out_l * wet2;
        }
    }

    fn reset(&mut self) {
        self.left.clear();
        self.right.clear();
    }
}
