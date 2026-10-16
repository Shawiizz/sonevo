//! How long a big song takes to compile, and to be sent to the interface as text.

use sonevo_engine::{Engine, Mode};
use sonevo_project::{builtin, Channel, Clip, ClipContent, Device, Id, Note, Pattern, Song, PPQ};
use std::time::Instant;

fn main() {
    let mut song = Song::new("Big");
    song.channels.clear();
    song.patterns.clear();
    for c in 0..48 {
        song.channels.push(Channel {
            id: Id::new(),
            name: format!("Channel {c}"),
            color: "#ffffff".into(),
            instrument: Device::builtin(if c % 2 == 0 { builtin::SYNTH } else { builtin::DRUMS }),
            volume: 0.8,
            pan: 0.0,
            mute: false,
            insert: None,
        });
    }
    for p in 0..64 {
        let mut pattern = Pattern {
            id: Id::new(),
            name: format!("P{p}"),
            color: "#fff".into(),
            length: PPQ * 16,
            notes: Default::default(),
        };
        for channel in song.channels.iter().skip(p % 8).step_by(8) {
            let notes = (0..128)
                .map(|i| Note {
                    start: i * PPQ / 8,
                    length: PPQ / 8,
                    key: 48 + (i % 24) as u8,
                    velocity: 0.8,
                })
                .collect();
            pattern.notes.insert(channel.id, notes);
        }
        song.patterns.push(pattern);
    }
    let track = song.tracks[0].id;
    for i in 0..400 {
        let pattern = song.patterns[i % 64].id;
        song.clips.push(Clip {
            id: Id::new(),
            track,
            start: i as i64 * PPQ * 16,
            length: PPQ * 16,
            offset: 0,
            content: ClipContent::Pattern { pattern },
        });
    }
    let notes: usize = song.patterns.iter().map(|p| p.notes.values().map(Vec::len).sum::<usize>()).sum();
    let (mut engine, _renderer) = Engine::new(48_000.0);
    let start = Instant::now();
    engine.load(&song, Mode::Song, None);
    let first = start.elapsed();
    let start = Instant::now();
    engine.load(&song, Mode::Song, None);
    let again = start.elapsed();
    let start = Instant::now();
    let text = serde_json::to_string(&song).unwrap();
    let notes_text =
        serde_json::to_string(&song.patterns.iter().map(|p| &p.notes).collect::<Vec<_>>()).unwrap();
    let json = start.elapsed();
    println!(
        "{notes} notes in patterns, {} clips: first compile {first:?}, again {again:?}; json {json:?}, {} KB",
        song.clips.len(),
        (text.len() + notes_text.len()) / 1024
    );
}
