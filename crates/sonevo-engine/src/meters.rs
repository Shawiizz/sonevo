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
        Meters::with(SLOTS)
    }

    /// Meters for `slots` things, each of the most heard of it since the last look.
    pub fn with(slots: usize) -> Meters {
        Meters { peaks: (0..slots).map(|_| AtomicU32::new(0)).collect() }
    }

    /// Lets `slot` know of a sample as loud as `peak`, 0 or more; a slot past the last is
    /// not heard.
    pub fn hear(&self, slot: usize, peak: f32) {
        // bits of floats 0 or more are in the order of the floats
        if let (true, Some(at)) = (peak > 0.0, self.peaks.get(slot)) {
            at.fetch_max(peak.to_bits(), Ordering::Relaxed);
        }
    }

    /// The loudest sample since the last look.
    pub fn take(&self, slot: usize) -> f32 {
        self.peaks.get(slot).map_or(0.0, |at| f32::from_bits(at.swap(0, Ordering::Relaxed)))
    }
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |a, s| a.max(s.abs()))
}
