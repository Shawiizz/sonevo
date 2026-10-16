//! An arpeggiator: the keys held played one after the other, a step apart, instead of
//! together.

use crate::song::Note;
use crate::time::{Ticks, PPQ};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArpOrder {
    Up,
    Down,
    UpDown,
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Arp {
    pub order: ArpOrder,
    /// How far apart its notes are: a sixteenth is `PPQ / 4`.
    pub step: Ticks,
    /// Over how many octaves it climbs, from 1.
    pub octaves: u8,
    /// How much of its step each note lasts, from 0.1 to 1.
    pub gate: f32,
}

impl Arp {
    /// What `notes` play as, arpeggiated, within `length`.
    pub fn play(&self, notes: &[Note], length: Ticks) -> Vec<Note> {
        let step = self.step.max(PPQ / 16);
        let lasting = ((step as f32 * self.gate.clamp(0.1, 1.0)) as Ticks).max(1);
        let end = notes.iter().map(|n| n.start + n.length).max().unwrap_or(0).min(length);
        let mut played = Vec::new();
        let mut keys: Vec<(u8, f32)> = Vec::new();
        let mut count = 0usize;
        let mut at = notes.iter().map(|n| n.start).min().unwrap_or(end);
        while at < end {
            keys.clear();
            keys.extend(
                notes
                    .iter()
                    .filter(|n| n.start <= at && at < n.start + n.length)
                    .map(|n| (n.key, n.velocity)),
            );
            if keys.is_empty() {
                // nothing held: on to the next key pressed, the arpeggio started again there
                match notes.iter().map(|n| n.start).filter(|&s| s > at).min() {
                    Some(next) => at = next,
                    None => break,
                }
                count = 0;
                continue;
            }
            keys.sort_by_key(|k| k.0);
            keys.dedup_by_key(|k| k.0);
            let (key, velocity) = self.nth(&keys, count, at);
            played.push(Note { start: at, length: lasting.min(length - at), key, velocity });
            count += 1;
            at += step;
        }
        played
    }

    /// The `count`th note of the arpeggio over `keys`, in order of pitch, held at `at`.
    pub fn nth(&self, keys: &[(u8, f32)], count: usize, at: Ticks) -> (u8, f32) {
        let octaves = usize::from(self.octaves.clamp(1, 4));
        let climb = keys.len() * octaves;
        let index = match self.order {
            ArpOrder::Up => count % climb,
            ArpOrder::Down => climb - 1 - count % climb,
            ArpOrder::UpDown if climb > 2 => {
                let turn = count % (2 * climb - 2);
                if turn < climb {
                    turn
                } else {
                    2 * climb - 2 - turn
                }
            }
            ArpOrder::UpDown => count % climb,
            ArpOrder::Random => {
                // the same every time the song plays: a hash of where it is
                let mut x = (at as u64) ^ ((count as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
                x ^= x >> 33;
                x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
                x ^= x >> 33;
                (x % climb as u64) as usize
            }
        };
        let (key, velocity) = keys[index % keys.len()];
        let octave = (index / keys.len()) as u8;
        (key.saturating_add(12 * octave).min(127), velocity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIXTEENTH: Ticks = PPQ / 4;

    fn held(start: Ticks, length: Ticks, keys: &[u8]) -> Vec<Note> {
        keys.iter().map(|&key| Note { start, length, key, velocity: 0.8 }).collect()
    }

    fn arp(order: ArpOrder, octaves: u8) -> Arp {
        Arp { order, step: SIXTEENTH, octaves, gate: 0.5 }
    }

    fn keys(notes: &[Note]) -> Vec<u8> {
        notes.iter().map(|n| n.key).collect()
    }

    #[test]
    fn a_chord_held_goes_up_a_step_at_a_time() {
        let played = arp(ArpOrder::Up, 1).play(&held(0, PPQ, &[67, 60, 64]), 4 * PPQ);
        assert_eq!(keys(&played), [60, 64, 67, 60]);
        assert_eq!(played.iter().map(|n| n.start).collect::<Vec<_>>(), [0, 240, 480, 720]);
        assert!(played.iter().all(|n| n.length == SIXTEENTH / 2));
    }

    #[test]
    fn down_over_two_octaves_and_up_and_down() {
        let chord = held(0, 6 * SIXTEENTH, &[60, 64, 67]);
        assert_eq!(keys(&arp(ArpOrder::Down, 2).play(&chord, 4 * PPQ)), [79, 76, 72, 67, 64, 60]);
        assert_eq!(keys(&arp(ArpOrder::UpDown, 1).play(&chord, 4 * PPQ)), [60, 64, 67, 64, 60, 64]);
    }

    #[test]
    fn after_a_silence_it_starts_again_where_the_next_chord_does() {
        let mut notes = held(0, SIXTEENTH * 2, &[60, 64]);
        notes.extend(held(PPQ + 10, SIXTEENTH * 2, &[62, 65]));
        let played = arp(ArpOrder::Up, 1).play(&notes, 4 * PPQ);
        assert_eq!(keys(&played), [60, 64, 62, 65]);
        assert_eq!(played[2].start, PPQ + 10);
    }

    #[test]
    fn random_stays_among_the_keys_and_is_the_same_each_time() {
        let chord = held(0, 4 * PPQ, &[60, 64, 67]);
        let random = arp(ArpOrder::Random, 1);
        let played = random.play(&chord, 4 * PPQ);
        assert_eq!(played.len(), 16);
        assert!(played.iter().all(|n| [60, 64, 67].contains(&n.key)));
        assert_eq!(played, random.play(&chord, 4 * PPQ));
    }

    #[test]
    fn nothing_plays_past_the_pattern() {
        let played = arp(ArpOrder::Up, 1).play(&held(0, 8 * PPQ, &[60]), PPQ);
        assert_eq!(played.len(), 4);
    }
}
