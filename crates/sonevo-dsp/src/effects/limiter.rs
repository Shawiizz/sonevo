//! A limiter, for the master: nothing past the ceiling. It looks 1.5 ms ahead, so that the
//! gain is already down when a peak comes.

use crate::param::Param;
use crate::util::db_to_gain;
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::linear("ceiling", "Ceiling", -12.0, 0.0, -0.3, "dB"),
    Param::log("release", "Release", 10.0, 1000.0, 120.0, "ms"),
];

const AHEAD_SECONDS: f32 = 0.0015;

pub struct Limiter {
    sample_rate: f32,
    ceiling: f32,
    release: f32,
    gain: f32,
    /// The last frames, played `ahead` frames late; and each one's peak, for the gain to see.
    left: Vec<f32>,
    right: Vec<f32>,
    peaks: Vec<f32>,
    at: usize,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Limiter {
        let ahead = ((sample_rate * AHEAD_SECONDS) as usize).max(1);
        Limiter {
            sample_rate,
            ceiling: db_to_gain(-0.3),
            release: 0.0,
            gain: 1.0,
            left: vec![0.0; ahead],
            right: vec![0.0; ahead],
            peaks: vec![0.0; ahead],
            at: 0,
        }
    }
}

impl Processor for Limiter {
    fn set(&mut self, index: usize, value: f64) {
        let Some(p) = PARAMS.get(index) else { return };
        let v = p.clamp(value) as f32;
        match index {
            0 => self.ceiling = db_to_gain(v),
            _ => self.release = (-1.0 / (v / 1000.0 * self.sample_rate)).exp(),
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let ahead = self.left.len();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.peaks[self.at] = l.abs().max(r.abs());
            let (out_l, out_r) = (self.left[self.at], self.right[self.at]);
            self.left[self.at] = *l;
            self.right[self.at] = *r;
            self.at = (self.at + 1) % ahead;
            // the loudest of what comes next sets how low the gain must be already
            let peak = self.peaks.iter().fold(out_l.abs().max(out_r.abs()), |a, &b| a.max(b));
            let wanted = if peak > self.ceiling { self.ceiling / peak } else { 1.0 };
            self.gain =
                if wanted < self.gain { wanted } else { wanted + (self.gain - wanted) * self.release };
            *l = out_l * self.gain;
            *r = out_r * self.gain;
        }
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
        self.peaks.fill(0.0);
        self.gain = 1.0;
    }
}
