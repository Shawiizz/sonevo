//! A sound told to be a loop by its length alone: a loop cut cleanly lasts a round number of
//! bars at a round tempo, whatever its file is named.

/// A loop's length in bars of four beats, and the tempo it was played at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loop {
    pub bars: u32,
    pub tempo: f64,
}

const BARS: [u32; 6] = [1, 2, 4, 8, 16, 32];
/// The tempos a loop may be at, and those music is most often made at, preferred among them.
const SLOWEST: f64 = 70.0;
const FASTEST: f64 = 200.0;
const LIKELY: std::ops::Range<f64> = 100.0..180.0;
/// How far from a round tempo (to half a beat a minute) a loop's may be, as a share of it:
/// a few milliseconds over a loop of eight bars.
const NEAR: f64 = 0.0005;
/// Shorter than this, a sound is a hit, not a loop.
const SHORTEST: f64 = 1.0;

/// The loop a sound of `frames` frames at `sample_rate` is, if its length is one's.
pub fn loop_of(frames: u64, sample_rate: f64) -> Option<Loop> {
    let seconds = frames as f64 / sample_rate;
    if !seconds.is_finite() || seconds < SHORTEST {
        return None;
    }
    let found = BARS.iter().filter_map(|&bars| {
        let tempo = f64::from(bars) * 4.0 * 60.0 / seconds;
        let round = (tempo * 2.0).round() / 2.0;
        let fits = (SLOWEST..=FASTEST).contains(&tempo) && (tempo - round).abs() <= tempo * NEAR;
        fits.then_some(Loop { bars, tempo: round })
    });
    found.min_by_key(|l| !LIKELY.contains(&l.tempo))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 44_100.0;

    fn frames(bars: f64, tempo: f64) -> u64 {
        (bars * 4.0 * 60.0 / tempo * RATE).round() as u64
    }

    #[test]
    fn a_loop_is_told_by_its_length() {
        assert_eq!(loop_of(frames(8.0, 140.0), RATE), Some(Loop { bars: 8, tempo: 140.0 }));
        assert_eq!(loop_of(frames(4.0, 142.0), RATE), Some(Loop { bars: 4, tempo: 142.0 }));
        assert_eq!(loop_of(frames(16.0, 172.5), RATE), Some(Loop { bars: 16, tempo: 172.5 }));
    }

    #[test]
    fn of_two_readings_the_usual_tempo_is_taken() {
        // as long as four bars at 70 as eight at 140
        assert_eq!(loop_of(frames(8.0, 140.0), RATE).map(|l| l.tempo), Some(140.0));
        assert_eq!(loop_of(frames(2.0, 90.0), RATE).map(|l| l.tempo), Some(90.0));
    }

    #[test]
    fn a_hit_or_a_sound_cut_anyhow_is_no_loop() {
        assert_eq!(loop_of(frames(1.0, 480.0), RATE), None);
        assert_eq!(loop_of((6.5 * RATE) as u64, RATE), None);
        assert_eq!(loop_of(frames(7.97, 170.0), RATE), None);
        assert_eq!(loop_of(0, RATE), None);
    }
}
