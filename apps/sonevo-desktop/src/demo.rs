//! The song Sonevo opens with the first time: sixteen bars of house, every part of the
//! studio used once, to be played, taken apart and changed.

use sonevo_project::{
    builtin, Channel, Clip, ClipContent, Device, Id, Insert, Note, Pattern, Point, Send, Signature, Song,
    Target, Ticks, COLORS, PPQ,
};
use std::collections::BTreeMap;

pub(crate) const STEP: Ticks = PPQ / 4;
pub(crate) const BAR: Ticks = PPQ * 4;

pub(crate) fn channel(name: &str, color: usize, instrument: Device, insert: Id) -> Channel {
    Channel {
        id: Id::new(),
        name: name.into(),
        color: COLORS[color].into(),
        instrument,
        volume: 0.8,
        pan: 0.0,
        mute: false,
        insert: Some(insert),
        arp: None,
    }
}

fn drum(sound: f64) -> Device {
    Device::builtin(builtin::DRUMS).with("sound", sound)
}

/// Notes on the sixteenth steps given, of a bar.
pub(crate) fn steps(steps: &[i64], key: u8, velocity: f32) -> Vec<Note> {
    steps.iter().map(|&s| Note { start: s * STEP, length: STEP, key, velocity }).collect()
}

pub(crate) fn pattern(name: &str, color: usize, bars: i64, notes: Vec<(Id, Vec<Note>)>) -> Pattern {
    Pattern {
        id: Id::new(),
        name: name.into(),
        color: COLORS[color].into(),
        length: bars * BAR,
        swing: 0.0,
        notes: notes.into_iter().collect::<BTreeMap<_, _>>(),
    }
}

pub fn song() -> Song {
    let mut song = Song::new("Demo");
    song.tempo = 124.0;
    song.signature = Signature { beats: 4, unit: 4 };

    let mut drums = Insert::new("Drums", COLORS[0]);
    let mut synths = Insert::new("Synths", COLORS[2]);
    let mut space = Insert::new("Space", COLORS[4]);
    drums.effects.push(Device::builtin(builtin::COMPRESSOR).with("threshold", -14.0).with("makeup", 3.0));
    synths.effects.push(Device::builtin(builtin::EQ).with("lowGain", -4.0).with("highGain", 2.0));
    space
        .effects
        .push(Device::builtin(builtin::DELAY).with("mix", 0.35).with("feedback", 0.35).with("mode", 1.0));
    space.effects.push(Device::builtin(builtin::REVERB).with("mix", 1.0).with("size", 0.75));
    synths.sends.push(Send { to: space.id, amount: 0.35 });
    song.master.effects.push(Device::builtin(builtin::LIMITER).with("ceiling", -0.5));

    let kick = channel("Kick", 0, drum(0.0), drums.id);
    let clap = channel("Clap", 1, drum(3.0), drums.id);
    let hat = channel("Hat", 2, drum(2.0).with("level", -6.0), drums.id);
    let open = channel("Open hat", 3, drum(4.0).with("level", -9.0), drums.id);
    let bass = channel(
        "Bass",
        4,
        Device::builtin(builtin::SYNTH)
            .with("wave1", 0.0)
            .with("wave2", 1.0)
            .with("semitones2", -12.0)
            .with("cutoff", 380.0)
            .with("resonance", 0.35)
            .with("envAmount", 0.55)
            .with("filterDecay", 0.18)
            .with("decay", 0.25)
            .with("sustain", 0.5)
            .with("release", 0.08)
            .with("volume", -4.0),
        synths.id,
    );
    let chords = channel(
        "Chords",
        5,
        Device::builtin(builtin::SYNTH)
            .with("unison", 3.0)
            .with("spread", 22.0)
            .with("cutoff", 1400.0)
            .with("envAmount", 0.2)
            .with("attack", 0.02)
            .with("decay", 0.9)
            .with("sustain", 0.45)
            .with("release", 0.6)
            .with("volume", -14.0),
        synths.id,
    );
    let pluck = channel(
        "Pluck",
        6,
        Device::builtin(builtin::SYNTH)
            .with("wave1", 1.0)
            .with("wave2", 2.0)
            .with("semitones2", 12.0)
            .with("cutoff", 900.0)
            .with("envAmount", 0.7)
            .with("filterDecay", 0.15)
            .with("filterSustain", 0.0)
            .with("decay", 0.3)
            .with("sustain", 0.0)
            .with("release", 0.2)
            .with("volume", -16.0),
        synths.id,
    );

    let beat = pattern(
        "Beat",
        0,
        1,
        vec![
            (kick.id, steps(&[0, 4, 8, 12], 60, 0.9)),
            (clap.id, steps(&[4, 12], 60, 0.8)),
            (hat.id, steps(&[2, 6, 10], 60, 0.7)),
            (open.id, steps(&[14], 60, 0.7)),
        ],
    );

    // A minor, F, C, G: a bar each
    let roots = [45u8, 41, 48, 43];
    let triads: [[u8; 3]; 4] = [[57, 60, 64], [57, 60, 65], [55, 60, 64], [55, 59, 62]];
    let mut chord_notes = Vec::new();
    let mut bass_notes = Vec::new();
    for (bar, (root, triad)) in roots.iter().zip(triads).enumerate() {
        let at = bar as Ticks * BAR;
        for key in triad {
            for hit in [0, 6, 10] {
                let length = if hit == 0 { 5 * STEP } else { 3 * STEP };
                chord_notes.push(Note { start: at + hit * STEP, length, key, velocity: 0.7 });
            }
        }
        for step in [2, 6, 10, 14] {
            bass_notes.push(Note { start: at + step * STEP, length: 2 * STEP, key: *root, velocity: 0.85 });
        }
    }
    let melody = [69u8, 72, 76, 74, 72, 69, 67, 64];
    let pluck_notes: Vec<Note> = (0..32)
        .filter(|i| i % 3 != 2)
        .map(|i| Note {
            start: i * 2 * STEP,
            length: STEP,
            key: melody[(i as usize * 5) % melody.len()],
            velocity: 0.75,
        })
        .collect();
    chord_notes.sort_by_key(|n| (n.start, n.key));
    let harmony = pattern("Chords", 2, 4, vec![(chords.id, chord_notes)]);
    let low = pattern("Bass", 4, 4, vec![(bass.id, bass_notes)]);
    let lead = pattern("Lead", 6, 4, vec![(pluck.id, pluck_notes)]);

    let clip = |track: usize, pattern: &Pattern, from: i64, bars: i64| Clip {
        id: Id::new(),
        track: song.tracks[track].id,
        start: from * BAR,
        length: bars * BAR,
        offset: 0,
        content: ClipContent::Pattern { pattern: pattern.id },
    };
    let mut clips =
        vec![clip(0, &harmony, 0, 16), clip(1, &beat, 4, 12), clip(2, &low, 8, 8), clip(3, &lead, 12, 4)];
    // the chords' filter opening over the first four bars
    let device = chords.instrument.id;
    let points =
        vec![Point { at: 0, value: 0.25, curve: 0.0 }, Point { at: 4 * BAR, value: 0.75, curve: -0.4 }];
    clips.push(Clip {
        id: Id::new(),
        track: song.tracks[5].id,
        start: 0,
        length: 4 * BAR,
        offset: 0,
        content: ClipContent::Automation { target: Target::Param { device, param: "cutoff".into() }, points },
    });

    song.channels = vec![kick, clap, hat, open, bass, chords, pluck];
    song.patterns = vec![beat, harmony, low, lead];
    song.clips = clips;
    song.inserts = vec![drums, synths, space];
    for (track, name) in song.tracks.iter_mut().zip(["Chords", "Beat", "Bass", "Lead", "", "Filter"]) {
        if !name.is_empty() {
            track.name = name.into();
        }
    }
    song
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_demo_plays_its_sixteen_bars() {
        let mut song = super::song();
        assert!(song.mend().is_empty());
        let rendered = sonevo_engine::render(&song, None, 48_000);
        assert!(rendered.missing.is_empty(), "{:?}", rendered.missing);
        let seconds = rendered.left.len() as f64 / 48_000.0;
        let length = 16.0 * 4.0 * 60.0 / 124.0;
        assert!(seconds > length && seconds < length + 10.0, "{seconds}");
        let loudest = rendered.left.iter().chain(&rendered.right).fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(loudest > 0.3 && loudest <= 1.0, "{loudest}");
    }
}
