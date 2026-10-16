//! How loud each part played, for the interface to show: the loudest sample since it last
//! looked, master, mixer tracks and channels each in a slot of their own.

use std::sync::atomic::{AtomicU32, Ordering};

pub const MASTER: usize = 0;
const INSERTS: usize = 1;
const CHANNELS: usize = 256;
pub const SLOTS: usize = 512;

pub fn insert(index: usize) -> usize {
    (INSERTS + index).min(CHANNELS - 1)
}

pub fn channel(index: usize) -> usize {
    (CHANNELS + index).min(SLOTS - 1)
}

pub struct Meters {
    peaks: Box<[AtomicU32]>,
}

impl Meters {
    pub fn new() -> Meters {
        Meters { peaks: (0..SLOTS).map(|_| AtomicU32::new(0)).collect() }
    }

    /// Lets `slot` know of a sample as loud as `peak`, 0 or more.
    pub fn hear(&self, slot: usize, peak: f32) {
        // bits of floats 0 or more are in the order of the floats
        if peak > 0.0 {
            self.peaks[slot].fetch_max(peak.to_bits(), Ordering::Relaxed);
        }
    }

    /// The loudest sample since the last look.
    pub fn take(&self, slot: usize) -> f32 {
        f32::from_bits(self.peaks[slot].swap(0, Ordering::Relaxed))
    }
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |a, s| a.max(s.abs()))
}
