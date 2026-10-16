//! A song played through, looked at as a whole: how loud, where its energy is, how its two
//! sides go together; for advice on the mix in plain words.

use crate::loudness::loudness;
use std::f64::consts::PI;

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    /// In LUFS, none when too short or silent.
    pub loudness: Option<f64>,
    /// The loudest sample, in dB.
    pub peak: f64,
    /// Of the energy, how much is under 40 Hz, and how much over 8 kHz: from 0 to 1.
    pub rumble: f64,
    pub air: f64,
    /// How alike the two sides are: 1 the same, 0 unrelated, -1 opposite.
    pub correlation: f64,
}

/// The energy of `samples` through a second-order filter, low-pass or high-pass at `cut`.
fn energy_through(samples: &[f64], rate: f64, cut: f64, high: bool) -> f64 {
    let w = 2.0 * PI * cut / rate;
    let (sin, cos) = w.sin_cos();
    let alpha = sin / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = if high {
        ((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0)
    } else {
        ((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0)
    };
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, -2.0 * cos / a0, (1.0 - alpha) / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    samples
        .iter()
        .map(|&x| {
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, x, y1, y);
            y * y
        })
        .sum()
}

pub fn analyse(left: &[f32], right: &[f32], sample_rate: u32) -> Analysis {
    let rate = f64::from(sample_rate);
    let mono: Vec<f64> = left.iter().zip(right).map(|(&l, &r)| 0.5 * (f64::from(l) + f64::from(r))).collect();
    let total: f64 = mono.iter().map(|s| s * s).sum();
    let share = |energy: f64| if total > 0.0 { (energy / total).clamp(0.0, 1.0) } else { 0.0 };
    let (lr, ll, rr) = left.iter().zip(right).fold((0.0, 0.0, 0.0), |(lr, ll, rr), (&l, &r)| {
        let (l, r) = (f64::from(l), f64::from(r));
        (lr + l * r, ll + l * l, rr + r * r)
    });
    let loudest = left.iter().chain(right).fold(0.0f32, |a, s| a.max(s.abs()));
    Analysis {
        loudness: loudness(left, right, sample_rate),
        peak: if loudest > 0.0 { 20.0 * f64::from(loudest).log10() } else { f64::NEG_INFINITY },
        rumble: share(energy_through(&mono, rate, 40.0, false)),
        air: share(energy_through(&mono, rate, 8000.0, true)),
        correlation: if ll > 0.0 && rr > 0.0 { lr / (ll * rr).sqrt() } else { 1.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, amplitude: f32) -> Vec<f32> {
        (0..96_000).map(|i| (std::f32::consts::TAU * hz * i as f32 / 48_000.0).sin() * amplitude).collect()
    }

    #[test]
    fn a_low_rumble_and_a_bright_hiss_are_told_apart() {
        let low = tone(30.0, 0.5);
        let analysis = analyse(&low, &low, 48_000);
        assert!(analysis.rumble > 0.6 && analysis.air < 0.01, "{analysis:?}");
        let high = tone(12_000.0, 0.5);
        let analysis = analyse(&high, &high, 48_000);
        assert!(analysis.air > 0.6 && analysis.rumble < 0.01, "{analysis:?}");
        assert!((analysis.peak - 20.0 * 0.5f64.log10()).abs() < 0.1);
    }

    #[test]
    fn sides_opposite_are_told() {
        let left = tone(440.0, 0.5);
        let right: Vec<f32> = left.iter().map(|s| -s).collect();
        assert!(analyse(&left, &right, 48_000).correlation < -0.99);
        assert!(analyse(&left, &left, 48_000).correlation > 0.99);
    }
}
