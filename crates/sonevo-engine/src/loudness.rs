//! How loud a song sounds as a whole, in LUFS, the way streaming services measure it
//! (ITU-R BS.1770): the sound weighted as the ear hears it, in blocks of 400 ms, the silences
//! and the quiet moments left out.

use std::f64::consts::PI;

/// A biquad's coefficients, its first output one already divided out.
#[derive(Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
}

impl Biquad {
    /// The next output for `x`, `state` its last inputs and outputs.
    fn next(&self, x: f64, state: &mut [f64; 4]) -> f64 {
        let [x1, x2, y1, y2] = *state;
        let y = self.b[0] * x + self.b[1] * x1 + self.b[2] * x2 - self.a[0] * y1 - self.a[1] * y2;
        *state = [x, x1, y, y1];
        y
    }
}

/// The two filters of the K-weighting at `rate`: a shelf lifting the highs, as the head does,
/// and a high-pass leaving out the lowest lows.
fn k_weighting(rate: f64) -> [Biquad; 2] {
    let (f0, gain, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
    let k = (PI * f0 / rate).tan();
    let vh = 10f64.powf(gain / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b: [(vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0],
        a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    };
    let (f0, q) = (38.13547087602444, 0.5003270373238773);
    let k = (PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let high_pass = Biquad { b: [1.0, -2.0, 1.0], a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0] };
    [shelf, high_pass]
}

/// The integrated loudness of `left` and `right` played at `rate`, in LUFS: none for a
/// sound too short or too quiet to be measured.
pub fn loudness(left: &[f32], right: &[f32], rate: u32) -> Option<f64> {
    let rate = f64::from(rate);
    let [shelf, high_pass] = k_weighting(rate);
    let hop = (0.1 * rate) as usize;
    if hop == 0 {
        return None;
    }
    // the power of each tenth of a second, weighted; a block is four of them
    let mut states = [[0.0; 4]; 4];
    let mut tenths = Vec::with_capacity(left.len() / hop + 1);
    let mut sum = 0.0;
    for (i, (&l, &r)) in left.iter().zip(right).enumerate() {
        for (side, x) in [f64::from(l), f64::from(r)].into_iter().enumerate() {
            let shelved = shelf.next(x, &mut states[side * 2]);
            let y = high_pass.next(shelved, &mut states[side * 2 + 1]);
            sum += y * y;
        }
        if (i + 1) % hop == 0 {
            tenths.push(sum);
            sum = 0.0;
        }
    }
    let powers: Vec<f64> = tenths.windows(4).map(|w| w.iter().sum::<f64>() / (4 * hop) as f64).collect();
    let lufs = |power: f64| -0.691 + 10.0 * power.log10();
    let mean = |powers: &mut dyn Iterator<Item = f64>| {
        let (sum, count) = powers.fold((0.0, 0usize), |(s, c), p| (s + p, c + 1));
        (count > 0).then(|| sum / count as f64)
    };
    let loud_enough: Vec<f64> = powers.into_iter().filter(|&p| p > 0.0 && lufs(p) > -70.0).collect();
    let ungated = mean(&mut loud_enough.iter().copied())?;
    let floor = lufs(ungated) - 10.0;
    mean(&mut loud_enough.into_iter().filter(|&p| lufs(p) > floor)).map(lufs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amplitude: f32, hz: f32, seconds: f32) -> Vec<f32> {
        (0..(48_000.0 * seconds) as usize)
            .map(|i| amplitude * (std::f32::consts::TAU * hz * i as f32 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn a_full_scale_tone_on_both_sides_is_zero_lufs() {
        let tone = sine(1.0, 997.0, 3.0);
        let lufs = loudness(&tone, &tone, 48_000).unwrap();
        assert!(lufs.abs() < 0.2, "{lufs}");
        let quiet = sine(0.1, 997.0, 3.0);
        let lufs = loudness(&quiet, &quiet, 48_000).unwrap();
        assert!((lufs + 20.0).abs() < 0.2, "{lufs}");
    }

    #[test]
    fn silence_and_the_quiet_moments_are_left_out() {
        let mut tone = sine(0.1, 997.0, 3.0);
        tone.extend(vec![0.0; 48_000 * 3]);
        let lufs = loudness(&tone, &tone, 48_000).unwrap();
        assert!((lufs + 20.0).abs() < 0.3, "the silence counted: {lufs}");
        assert_eq!(loudness(&[0.0; 48_000], &[0.0; 48_000], 48_000), None);
        assert_eq!(loudness(&[0.5; 100], &[0.5; 100], 48_000), None);
    }
}
