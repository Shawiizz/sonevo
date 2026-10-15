//! A pattern looped: four bars of chords (C, A minor, F, G), each with its bass note and an
//! arpeggio in eighths, cut into the notes of each block on their very frame.

use sonevo_clap::Note;

const BEATS_PER_BAR: u32 = 4;
/// Root, third and fifth of each bar's chord.
const CHORDS: [[u8; 3]; 4] = [[60, 64, 67], [57, 60, 64], [53, 57, 60], [55, 59, 62]];
/// Which note of the chord each eighth plays, an octave up for the last ones.
const ARPEGGIO: [(usize, u8); 8] = [(0, 0), (1, 0), (2, 0), (0, 12), (1, 12), (2, 0), (1, 0), (2, 0)];

/// A note's start or end, in frames from the start of the loop.
#[derive(Clone, Copy)]
struct Event {
    frame: u64,
    key: u8,
    velocity: f32,
    on: bool,
}

pub struct Sequencer {
    events: Vec<Event>,
    length: u64,
    /// Where the next block starts, in frames from the start of the loop.
    position: u64,
    /// The notes of the block being made, kept to allocate nothing while playing.
    notes: Vec<Note>,
}

impl Sequencer {
    pub fn new(sample_rate: u32, tempo: f64) -> Sequencer {
        let beat = f64::from(sample_rate) * 60.0 / tempo;
        let frame = |beats: f64| (beats * beat).round() as u64;
        let mut events = Vec::new();
        let mut note = |start: f64, length: f64, key: u8, velocity: f32| {
            events.push(Event { frame: frame(start), key, velocity, on: true });
            events.push(Event { frame: frame(start + length), key, velocity: 0.0, on: false });
        };
        for (bar, chord) in CHORDS.iter().enumerate() {
            let start = (bar as u32 * BEATS_PER_BAR) as f64;
            note(start, 3.8, chord[0] - 24, 0.8);
            for (eighth, &(degree, up)) in ARPEGGIO.iter().enumerate() {
                note(start + eighth as f64 * 0.5, 0.45, chord[degree] + up, 0.55 + 0.1 * (eighth % 2) as f32);
            }
        }
        // in time order, a note's end before another's start on the same frame
        events.sort_by_key(|e| (e.frame, e.on));
        let length = frame((CHORDS.len() as u32 * BEATS_PER_BAR) as f64);
        Sequencer { events, length, position: 0, notes: Vec::with_capacity(sonevo_clap::MAX_NOTES) }
    }

    /// The notes of the next `frames`, the loop going round within them if it ends there.
    pub fn next(&mut self, frames: usize) -> &[Note] {
        self.notes.clear();
        let mut done = 0u64;
        let mut from = self.position;
        let mut left = frames as u64;
        while left > 0 {
            let take = left.min(self.length - from);
            let to = from + take;
            for event in self.events.iter().filter(|e| e.frame >= from && e.frame < to) {
                if self.notes.len() < self.notes.capacity() {
                    self.notes.push(Note {
                        at: (done + event.frame - from) as u32,
                        key: event.key,
                        velocity: event.velocity,
                        on: event.on,
                    });
                }
            }
            done += take;
            left -= take;
            from = to % self.length;
        }
        self.position = from;
        &self.notes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_note_of_the_loop_comes_once_on_its_frame_whatever_the_blocks() {
        let mut whole = Sequencer::new(48_000, 120.0);
        let all: Vec<Note> = whole.next(whole.length as usize).to_vec();
        assert_eq!(all.len(), 4 * 9 * 2);
        let mut cut = Sequencer::new(48_000, 120.0);
        let mut seen = Vec::new();
        let mut at = 0u32;
        // blocks of an odd size, so that notes fall anywhere in them
        for _ in 0..(cut.length / 300) {
            seen.extend(cut.next(300).iter().map(|n| Note { at: n.at + at, ..*n }));
            at += 300;
        }
        let whole_ones: Vec<Note> = all.into_iter().filter(|n| n.at < at).collect();
        assert_eq!(seen, whole_ones);
    }

    #[test]
    fn the_loop_goes_round_within_a_block() {
        let mut sequencer = Sequencer::new(48_000, 120.0);
        sequencer.next(sequencer.length as usize - 100);
        let notes = sequencer.next(200);
        // the first notes of the loop again, a hundred frames in
        assert!(notes.iter().any(|n| n.on && n.at == 100));
    }
}
