//! A filter: low-pass, high-pass or band-pass, resonant, its cutoff gliding where it is set.

use crate::param::Param;
use crate::util::{Smooth, Svf};
use crate::{Context, Event, Processor};

const MODES: &[&str] = &["Low-pass", "High-pass", "Band-pass"];

pub const PARAMS: &[Param] = &[
    Param::choice("mode", "Mode", MODES, 0),
    Param::log("cutoff", "Cutoff", 20.0, 20000.0, 20000.0, "Hz"),
    Param::linear("resonance", "Resonance", 0.0, 1.0, 0.2, ""),
];

/// Frames between two settings of the filter while its cutoff glides.
const EVERY: usize = 16;

pub struct Filter {
    sample_rate: f32,
    mode: i32,
    cutoff: Smooth,
    resonance: f32,
    filters: [Svf; 2],
}

impl Filter {
    pub fn new(sample_rate: f32) -> Filter {
        Filter {
            sample_rate,
            mode: 0,
            cutoff: Smooth::new(20000.0, sample_rate / EVERY as f32, 0.02),
            resonance: 0.2,
            filters: [Svf::default(); 2],
        }
    }
}

impl Processor for Filter {
    fn set(&mut self, index: usize, value: f64) {
        let Some(p) = PARAMS.get(index) else { return };
        let v = p.clamp(value);
        match index {
            0 => self.mode = v as i32,
            1 => self.cutoff.set(v as f32),
            _ => self.resonance = v as f32,
        }
    }

    fn process(&mut self, _context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let mode = self.mode;
        for (chunk_l, chunk_r) in left.chunks_mut(EVERY).zip(right.chunks_mut(EVERY)) {
            let cutoff = self.cutoff.next();
            for f in &mut self.filters {
                f.set(cutoff, self.resonance, self.sample_rate);
            }
            for (samples, filter) in [chunk_l, chunk_r].into_iter().zip(self.filters.iter_mut()) {
                for s in samples.iter_mut() {
                    let out = filter.process(*s);
                    *s = match mode {
                        1 => out.high,
                        2 => out.band,
                        _ => out.low,
                    };
                }
            }
        }
    }

    fn reset(&mut self) {
        self.filters.iter_mut().for_each(Svf::clear);
    }
}
