//! A sound made longer or shorter without its pitch changing (WSOLA): short windows of it laid
//! one after the other further apart or closer together, each taken where it best continues
//! the one before, so that the waves line up.

/// The window, about 43 ms at 48 kHz, and how far either way a window is looked for.
const WINDOW: usize = 2048;
const SEARCH: usize = 512;
/// Every this many samples are compared, looking for where a window fits.
const COARSE: usize = 4;

/// `left` and `right` played over `factor` times their length, at their own pitch.
pub fn stretch(left: &[f32], right: &[f32], factor: f64) -> (Vec<f32>, Vec<f32>) {
    let frames = left.len().min(right.len());
    let length = (frames as f64 * factor).round() as usize;
    if frames < WINDOW || factor <= 0.0 || length == 0 {
        // too short to be stretched by windows: the nearest sample each
        let pick = |side: &[f32]| -> Vec<f32> {
            (0..length).map(|i| side[((i as f64 / factor) as usize).min(frames.saturating_sub(1))]).collect()
        };
        return if frames == 0 { (vec![0.0; length], vec![0.0; length]) } else { (pick(left), pick(right)) };
    }
    let hop = WINDOW / 2;
    let window: Vec<f32> =
        (0..WINDOW).map(|n| 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / WINDOW as f32).cos()).collect();
    let mono: Vec<f32> = left.iter().zip(right).map(|(l, r)| 0.5 * (l + r)).collect();
    let (mut out_left, mut out_right) = (vec![0.0; length + WINDOW], vec![0.0; length + WINDOW]);
    let mut weight = vec![0.0f32; length + WINDOW];
    let last = frames - WINDOW;
    // where the window before was taken: the one now best continues from there
    let mut previous: Option<usize> = None;
    for k in 0..length.div_ceil(hop) + 1 {
        let natural = ((k * hop) as f64 / factor) as usize;
        let at = match previous {
            None => natural.min(last),
            Some(before) => {
                let follow = (before + hop).min(last);
                let from = natural.saturating_sub(SEARCH).min(last);
                let to = (natural + SEARCH).min(last);
                let score = |candidate: usize| -> f32 {
                    (0..WINDOW).step_by(COARSE).map(|n| mono[candidate + n] * mono[follow + n]).sum()
                };
                (from..=to)
                    .step_by(2)
                    .max_by(|&a, &b| score(a).total_cmp(&score(b)))
                    .unwrap_or(natural.min(last))
            }
        };
        previous = Some(at);
        let place = k * hop;
        for n in 0..WINDOW {
            if place + n >= out_left.len() {
                break;
            }
            out_left[place + n] += left[at + n] * window[n];
            out_right[place + n] += right[at + n] * window[n];
            weight[place + n] += window[n];
        }
    }
    for ((l, r), w) in out_left.iter_mut().zip(out_right.iter_mut()).zip(&weight) {
        if *w > 1e-3 {
            *l /= w;
            *r /= w;
        }
    }
    out_left.truncate(length);
    out_right.truncate(length);
    (out_left, out_right)
}

/// `left` and `right` `semitones` higher or lower, as long as they were: stretched by the
/// change of pitch, then played that much faster or slower.
pub fn transpose(left: &[f32], right: &[f32], semitones: f64) -> (Vec<f32>, Vec<f32>) {
    let ratio = 2f64.powf(semitones / 12.0);
    let frames = left.len().min(right.len());
    let (long_left, long_right) = stretch(left, right, ratio);
    let last = long_left.len().saturating_sub(1);
    let read = |side: &[f32], i: usize| -> f32 {
        let at = i as f64 * ratio;
        let whole = (at as usize).min(last);
        let part = (at - whole as f64) as f32;
        let next = (whole + 1).min(last);
        side[whole] + (side[next] - side[whole]) * part
    };
    if long_left.is_empty() {
        return (vec![0.0; frames], vec![0.0; frames]);
    }
    ((0..frames).map(|i| read(&long_left, i)).collect(), (0..frames).map(|i| read(&long_right, i)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pitch(samples: &[f32], rate: f32) -> f32 {
        let ups = samples.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        ups as f32 / (samples.len() as f32 / rate)
    }

    #[test]
    fn a_tone_made_longer_keeps_its_pitch() {
        let tone: Vec<f32> =
            (0..48_000).map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        for factor in [1.5, 0.75] {
            let (left, right) = stretch(&tone, &tone, factor);
            assert_eq!(left.len(), (48_000.0 * factor) as usize);
            assert_eq!(left, right);
            let middle = &left[left.len() / 4..left.len() * 3 / 4];
            assert!((pitch(middle, 48_000.0) - 440.0).abs() < 5.0, "{factor}: {}", pitch(middle, 48_000.0));
            let loudest = middle.iter().fold(0.0f32, |a, s| a.max(s.abs()));
            assert!(loudest > 0.45 && loudest < 0.55, "{factor}: {loudest}");
        }
    }

    #[test]
    fn a_tone_transposed_an_octave_up_keeps_its_length() {
        let tone: Vec<f32> =
            (0..48_000).map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        let (left, _) = transpose(&tone, &tone, 12.0);
        assert_eq!(left.len(), tone.len());
        let middle = &left[12_000..36_000];
        assert!((pitch(middle, 48_000.0) - 440.0).abs() < 6.0, "{}", pitch(middle, 48_000.0));
        let (down, _) = transpose(&tone, &tone, -12.0);
        assert!((pitch(&down[12_000..36_000], 48_000.0) - 110.0).abs() < 4.0);
    }

    #[test]
    fn a_sound_too_short_for_windows_is_stretched_all_the_same() {
        let (left, _) = stretch(&[0.1, 0.2, 0.3], &[0.1, 0.2, 0.3], 2.0);
        assert_eq!(left, [0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
        assert_eq!(stretch(&[], &[], 2.0).0.len(), 0);
    }
}
