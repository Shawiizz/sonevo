//! What the master played last, kept for the interface to see as a spectrum: the audio
//! thread writes, another reads, neither waits for the other.

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// The samples kept, and looked at together.
const SIZE: usize = 4096;
pub const BANDS: usize = 48;
const LOWEST: f32 = 30.0;
const HIGHEST: f32 = 16_000.0;
/// The quietest a band shows, at 0.
const FLOOR_DB: f32 = -90.0;

pub struct Scope {
    samples: Box<[AtomicU32]>,
    written: AtomicUsize,
}

impl Scope {
    pub fn new() -> Scope {
        Scope { samples: (0..SIZE).map(|_| AtomicU32::new(0)).collect(), written: AtomicUsize::new(0) }
    }

    /// Keeps `left` and `right` together, in place of the oldest.
    pub fn keep(&self, left: &[f32], right: &[f32]) {
        let mut at = self.written.load(Ordering::Relaxed);
        for (l, r) in left.iter().zip(right) {
            self.samples[at % SIZE].store(((l + r) * 0.5).to_bits(), Ordering::Relaxed);
            at = at.wrapping_add(1);
        }
        self.written.store(at, Ordering::Release);
    }

    /// How loud each band of what was kept is, from 0 (silence) to 1 (as loud as can be),
    /// the bands as wide as each other in octaves from the lowest to the highest.
    pub fn spectrum(&self, sample_rate: f32) -> [f32; BANDS] {
        let oldest = self.written.load(Ordering::Acquire);
        let mut re: Vec<f32> = (0..SIZE)
            .map(|i| {
                let sample = f32::from_bits(self.samples[(oldest + i) % SIZE].load(Ordering::Relaxed));
                sample * (0.5 - 0.5 * (TAU * i as f32 / SIZE as f32).cos())
            })
            .collect();
        let mut im = vec![0.0; SIZE];
        fft(&mut re, &mut im);
        let per_bin = sample_rate / SIZE as f32;
        let mut bands = [0.0; BANDS];
        for (b, band) in bands.iter_mut().enumerate() {
            let edge = |b: usize| LOWEST * (HIGHEST / LOWEST).powf(b as f32 / BANDS as f32) / per_bin;
            let magnitude = |k: usize| re[k].hypot(im[k]);
            let (from, to) = (edge(b).round() as usize, edge(b + 1).round() as usize);
            let loudest = if to > from + 1 {
                (from.max(1)..to.min(SIZE / 2)).map(magnitude).fold(0.0f32, f32::max)
            } else {
                // a band narrower than the bins: between the two around its middle
                let middle = ((edge(b) + edge(b + 1)) * 0.5).clamp(1.0, (SIZE / 2 - 2) as f32);
                let (k, part) = (middle as usize, middle.fract());
                magnitude(k) * (1.0 - part) + magnitude(k + 1) * part
            };
            // a sine of amplitude 1 comes out as SIZE / 4 through the window
            let db = 20.0 * (loudest * 4.0 / SIZE as f32).max(1e-9).log10();
            *band = ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
        }
        bands
    }
}

/// The discrete Fourier transform of `re` and `im` in place; their length a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut length = 2;
    while length <= n {
        let angle = -TAU / length as f32;
        for start in (0..n).step_by(length) {
            for k in 0..length / 2 {
                let (sin, cos) = (angle * k as f32).sin_cos();
                let (a, b) = (start + k, start + k + length / 2);
                let (tr, ti) = (re[b] * cos - im[b] * sin, re[b] * sin + im[b] * cos);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
        length <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_shows_in_its_band_only() {
        let scope = Scope::new();
        let tone: Vec<f32> = (0..SIZE * 2).map(|i| (TAU * 1000.0 * i as f32 / 48_000.0).sin()).collect();
        scope.keep(&tone, &tone);
        let bands = scope.spectrum(48_000.0);
        let loudest = (0..BANDS).max_by(|&a, &b| bands[a].total_cmp(&bands[b])).unwrap();
        let edge = |b: usize| LOWEST * (HIGHEST / LOWEST).powf(b as f32 / BANDS as f32);
        assert!(edge(loudest) <= 1000.0 && 1000.0 <= edge(loudest + 1), "band {loudest}");
        assert!(bands[loudest] > 0.95, "{}", bands[loudest]);
        assert!(bands[0] < 0.3 && bands[BANDS - 1] < 0.3, "{bands:?}");
    }

    #[test]
    fn silence_shows_nothing() {
        assert!(Scope::new().spectrum(48_000.0).iter().all(|&b| b == 0.0));
    }
}
