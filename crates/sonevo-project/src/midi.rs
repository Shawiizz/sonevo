//! Standard MIDI files read into notes, and notes written as one: a track a channel.

use crate::song::Note;
use crate::time::{Ticks, PPQ};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MidiError {
    #[error("this is not a MIDI file")]
    NotMidi,
    #[error("the MIDI file ends too soon")]
    Short,
    #[error("MIDI files timed in frames are not read yet")]
    Frames,
}

/// A track of a MIDI file: its name if it has one, and its notes in Sonevo's ticks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MidiTrack {
    pub name: String,
    pub notes: Vec<Note>,
}

/// What a MIDI file holds: its first tempo, and its tracks that play notes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Midi {
    pub tempo: Option<f64>,
    pub tracks: Vec<MidiTrack>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, MidiError> {
        let b = *self.bytes.get(self.at).ok_or(MidiError::Short)?;
        self.at += 1;
        Ok(b)
    }

    fn take(&mut self, count: usize) -> Result<&[u8], MidiError> {
        let end = self.at.checked_add(count).filter(|&e| e <= self.bytes.len()).ok_or(MidiError::Short)?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u32(&mut self) -> Result<u32, MidiError> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u16(&mut self) -> Result<u16, MidiError> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// A number written seven bits a byte, the last byte's high bit clear.
    fn variable(&mut self) -> Result<u32, MidiError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let b = self.byte()?;
            value = (value << 7) | u32::from(b & 0x7F);
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        Ok(value)
    }
}

/// Reads the MIDI file `bytes`.
pub fn read_midi(bytes: &[u8]) -> Result<Midi, MidiError> {
    let mut r = Reader { bytes, at: 0 };
    if r.take(4)? != b"MThd" {
        return Err(MidiError::NotMidi);
    }
    let header = (r.u32()? as usize).max(6);
    let header_end = r.at.saturating_add(header);
    let _format = r.u16()?;
    let count = r.u16()?;
    let division = r.u16()?;
    if division & 0x8000 != 0 {
        return Err(MidiError::Frames);
    }
    let per_quarter = i64::from(division.max(1));
    r.at = header_end;

    let mut tempo = None;
    let mut tracks = Vec::new();
    for _ in 0..count {
        if bytes.len().saturating_sub(r.at) < 8 {
            break;
        }
        let is_track = r.take(4)? == b"MTrk";
        let length = r.u32()? as usize;
        let end = r.at.saturating_add(length).min(bytes.len());
        if !is_track {
            r.at = end;
            continue;
        }
        let mut track = MidiTrack { name: String::new(), notes: Vec::new() };
        // the keys down, by channel and key: where each went down, and how hard
        let mut down: Vec<(u8, u8, Ticks, f32)> = Vec::new();
        let (mut tick, mut status) = (0i64, 0u8);
        while r.at < end {
            tick += i64::from(r.variable()?);
            let at = tick * PPQ / per_quarter;
            let first = r.byte()?;
            match first {
                0xFF => {
                    let meta = r.byte()?;
                    let length = r.variable()? as usize;
                    let data = r.take(length)?;
                    match meta {
                        0x03 if track.name.is_empty() => {
                            track.name = String::from_utf8_lossy(data).trim().to_owned()
                        }
                        0x51 if tempo.is_none() && data.len() == 3 => {
                            let micros = u32::from_be_bytes([0, data[0], data[1], data[2]]);
                            // to the hundredth: the microseconds a quarter lasts are rounded
                            tempo = Some((6_000_000_000.0 / f64::from(micros.max(1))).round() / 100.0);
                        }
                        _ => {}
                    }
                    continue;
                }
                0xF0 | 0xF7 => {
                    let length = r.variable()? as usize;
                    r.take(length)?;
                    status = 0;
                    continue;
                }
                0xF1..=0xFE => return Err(MidiError::NotMidi),
                _ => {}
            }
            // a channel message, its status byte left out when it repeats the last one's
            let data = if first & 0x80 != 0 {
                status = first;
                r.byte()?
            } else if status != 0 {
                first
            } else {
                return Err(MidiError::NotMidi);
            };
            match status & 0xF0 {
                0x80 | 0x90 => {
                    let (channel, key) = (status & 0x0F, data & 0x7F);
                    let velocity = r.byte()? & 0x7F;
                    if status & 0xF0 == 0x90 && velocity > 0 {
                        down.push((channel, key, at, f32::from(velocity) / 127.0));
                    } else if let Some(i) = down.iter().position(|&(c, k, _, _)| c == channel && k == key) {
                        let (_, _, start, velocity) = down.remove(i);
                        track.notes.push(Note { start, length: (at - start).max(1), key, velocity });
                    }
                }
                0xC0 | 0xD0 => {}
                _ => {
                    r.byte()?;
                }
            }
        }
        r.at = end;
        if !track.notes.is_empty() {
            track.notes.sort_by_key(|n| (n.start, n.key));
            tracks.push(track);
        }
    }
    Ok(Midi { tempo, tracks })
}

fn variable(out: &mut Vec<u8>, value: u32) {
    let mut bytes = vec![(value & 0x7F) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        bytes.push(0x80 | (rest & 0x7F) as u8);
        rest >>= 7;
    }
    out.extend(bytes.iter().rev());
}

/// A MIDI file of `tracks`, each its name and its notes, at `tempo`.
pub fn write_midi(tempo: f64, tracks: &[(&str, &[Note])]) -> Vec<u8> {
    let mut out = b"MThd".to_vec();
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend((tracks.len() as u16 + 1).to_be_bytes());
    out.extend((PPQ as u16).to_be_bytes());

    let mut chunk = |events: Vec<u8>| {
        out.extend(b"MTrk");
        out.extend((events.len() as u32).to_be_bytes());
        out.extend(events);
    };
    let micros = (60_000_000.0 / tempo.max(1.0)).round() as u32;
    let mut first = vec![0, 0xFF, 0x51, 3];
    first.extend(&micros.to_be_bytes()[1..]);
    first.extend([0, 0xFF, 0x2F, 0]);
    chunk(first);

    for (name, notes) in tracks {
        let mut events = vec![0, 0xFF, 0x03];
        variable(&mut events, name.len() as u32);
        events.extend(name.as_bytes());
        // each key let go of before one pressed on the same tick
        let mut moments: Vec<(Ticks, bool, u8, u8)> = notes
            .iter()
            .flat_map(|n| {
                let velocity = (n.velocity.clamp(0.0, 1.0) * 127.0).round().max(1.0) as u8;
                [(n.start, true, n.key, velocity), (n.start + n.length, false, n.key, 0)]
            })
            .collect();
        moments.sort_by_key(|&(at, on, key, _)| (at, on, key));
        let mut last = 0;
        for (at, on, key, velocity) in moments {
            variable(&mut events, (at - last).max(0) as u32);
            last = at;
            events.extend([if on { 0x90 } else { 0x80 }, key & 0x7F, velocity]);
        }
        events.extend([0, 0xFF, 0x2F, 0]);
        chunk(events);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: Ticks, length: Ticks, key: u8) -> Note {
        Note { start, length, key, velocity: 100.0 / 127.0 }
    }

    #[test]
    fn notes_written_read_back_the_same() {
        let bass = [note(0, PPQ, 36), note(PPQ, PPQ / 2, 38), note(PPQ, PPQ / 2, 43)];
        let lead = [note(PPQ / 4, 3 * PPQ, 72)];
        let bytes = write_midi(128.0, &[("Bass", &bass), ("Lead", &lead)]);
        let midi = read_midi(&bytes).unwrap();
        assert_eq!(midi.tempo.map(|t| t.round()), Some(128.0));
        assert_eq!(midi.tracks.len(), 2);
        assert_eq!(midi.tracks[0].name, "Bass");
        assert_eq!(midi.tracks[0].notes, bass);
        assert_eq!(midi.tracks[1].notes, lead);
    }

    #[test]
    fn running_status_and_other_resolutions_are_read() {
        // 96 ticks to a quarter; a note on, its off as a note on of velocity 0 in running status
        let mut bytes = b"MThd".to_vec();
        bytes.extend(6u32.to_be_bytes());
        bytes.extend([0, 0, 0, 1, 0, 96]);
        let events = [0x00, 0x90, 60, 100, 0x60, 60, 0, 0x00, 0xFF, 0x2F, 0x00];
        bytes.extend(b"MTrk");
        bytes.extend((events.len() as u32).to_be_bytes());
        bytes.extend(events);
        let midi = read_midi(&bytes).unwrap();
        assert_eq!(midi.tracks[0].notes, [Note { start: 0, length: PPQ, key: 60, velocity: 100.0 / 127.0 }]);
    }

    #[test]
    fn what_is_not_midi_is_said() {
        assert_eq!(read_midi(b"RIFF....WAVE"), Err(MidiError::NotMidi));
        assert_eq!(read_midi(b"MThd"), Err(MidiError::Short));
    }
}
