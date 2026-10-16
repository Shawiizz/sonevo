#![allow(dead_code)]

use sonevo_engine::{Engine, Mode, Renderer};
use sonevo_project::{builtin, Clip, ClipContent, Device, Id, Insert, Note, Song, PPQ};

pub const RATE: f32 = 48_000.0;

/// A song of one synth playing `notes` in its first pattern, the pattern once in the
/// playlist; the synth's channel and the pattern's ids.
pub fn song(notes: &[Note]) -> (Song, Id, Id) {
    let mut song = Song::new("Test");
    song.tempo = 120.0;
    let channel = song.channels[0].id;
    song.channels[0].instrument = Device::builtin(builtin::SYNTH);
    let pattern = song.patterns[0].id;
    song.patterns[0].notes.insert(channel, notes.to_vec());
    let track = song.tracks[0].id;
    let length = song.patterns[0].length;
    song.clips.push(Clip {
        id: Id::new(),
        track,
        start: 0,
        length,
        offset: 0,
        content: ClipContent::Pattern { pattern },
    });
    (song, channel, pattern)
}

pub fn note(beat: i64, beats: i64, key: u8) -> Note {
    Note { start: beat * PPQ, length: beats * PPQ, key, velocity: 0.8 }
}

pub fn started(song: &Song, mode: Mode) -> (Engine, Renderer) {
    let (mut engine, renderer) = Engine::new(RATE);
    let missing = engine.load(song, mode, None);
    assert!(missing.is_empty(), "{missing:?}");
    (engine, renderer)
}

/// Plays `frames` frames, a block of 480 at a time.
pub fn play(renderer: &mut Renderer, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = (vec![0.0; frames], vec![0.0; frames]);
    for (l, r) in left.chunks_mut(480).zip(right.chunks_mut(480)) {
        renderer.render(l, r);
    }
    (left, right)
}

pub fn loudest(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |a, s| a.max(s.abs()))
}

/// The frame a beat falls on at 120 beats a minute.
pub fn beat(n: f64) -> usize {
    (n * 0.5 * f64::from(RATE)) as usize
}

pub fn insert(song: &mut Song, name: &str) -> Id {
    let insert = Insert::new(name, "#ffffff");
    let id = insert.id;
    song.inserts.push(insert);
    id
}
