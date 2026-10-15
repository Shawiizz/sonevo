//! What the audio thread measures, counted without locks or allocations, read each second.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Round trips are counted in buckets of this many microseconds, the last one for all
/// those longer.
const BUCKET_US: u64 = 5;
const BUCKETS: usize = 2000;

#[derive(Default)]
pub struct Stats {
    pub blocks: AtomicU64,
    /// Blocks the plugin had not played by their deadline: silence instead.
    pub late: AtomicU64,
    pub frames: AtomicU64,
    /// The time from asking a block to having it, its longest.
    pub longest_trip_ns: AtomicU64,
    /// How long the sound takes from the callback to the speakers, as the driver says.
    pub output_latency_ns: AtomicU64,
    trips: Trips,
}

struct Trips([AtomicU64; BUCKETS]);

impl Default for Trips {
    fn default() -> Trips {
        Trips(std::array::from_fn(|_| AtomicU64::new(0)))
    }
}

impl Stats {
    pub fn trip(&self, took: Duration) {
        let ns = took.as_nanos() as u64;
        self.longest_trip_ns.fetch_max(ns, Ordering::Relaxed);
        let bucket = ((ns / 1000 / BUCKET_US) as usize).min(BUCKETS - 1);
        self.trips.0[bucket].fetch_add(1, Ordering::Relaxed);
    }

    /// The round trips counted, bucket by bucket.
    pub fn trips(&self) -> Vec<u64> {
        self.trips.0.iter().map(|b| b.load(Ordering::Relaxed)).collect()
    }
}

/// The round trip that `share` of those counted in `buckets` stayed under, in microseconds.
pub fn percentile(buckets: &[u64], share: f64) -> Option<u64> {
    let total: u64 = buckets.iter().sum();
    if total == 0 {
        return None;
    }
    let wanted = (total as f64 * share).ceil() as u64;
    let mut seen = 0;
    buckets
        .iter()
        .position(|&b| {
            seen += b;
            seen >= wanted
        })
        .map(|i| (i as u64 + 1) * BUCKET_US)
}
