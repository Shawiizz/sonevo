//! Songs to start from, a style each: the parts already there, made of Sonevo's presets, to
//! be changed into a song of one's own.

use crate::demo::{channel, pattern, steps, BAR, STEP};
use sonevo_project::{
    builtin, Arp, ArpOrder, Clip, ClipContent, Device, Id, Insert, Marker, Note, Pattern, Point, Send, Song,
    Target, Ticks, COLORS,
};

/// The template called `name`.
pub fn song(name: &str) -> Option<Song> {
    match name {
        "trap" => Some(trap()),
        "lofi" => Some(lofi()),
        "techno" => Some(techno()),
        "schranz" => Some(schranz()),
        _ => None,
    }
}

/// The built-in device `device` set as its preset `name`.
fn preset(device: &str, name: &str) -> Device {
    let preset = sonevo_dsp::presets(device).find(|p| p.name == name).expect("a preset Sonevo has");
    preset.values.iter().fold(Device::builtin(device), |made, &(id, value)| made.with(id, value))
}

fn note(start: Ticks, length: Ticks, key: u8, velocity: f32) -> Note {
    Note { start, length, key, velocity }
}

/// Chords a bar each, held as long as `length` steps from `at` in each bar.
fn chords(chords: &[&[u8]], at: Ticks, length: Ticks, velocity: f32) -> Vec<Note> {
    let mut notes: Vec<Note> = chords
        .iter()
        .enumerate()
        .flat_map(|(bar, keys)| {
            keys.iter().map(move |&key| note(bar as Ticks * BAR + at * STEP, length * STEP, key, velocity))
        })
        .collect();
    notes.sort_by_key(|n| (n.start, n.key));
    notes
}

/// `pattern` placed on track `track` from bar `from` for `bars` bars.
fn clip(song: &Song, track: usize, pattern: &Pattern, from: i64, bars: i64) -> Clip {
    Clip {
        id: Id::new(),
        track: song.tracks[track].id,
        start: from * BAR,
        length: bars * BAR,
        offset: 0,
        content: ClipContent::Pattern { pattern: pattern.id },
    }
}

fn name_tracks(song: &mut Song, names: &[&str]) {
    for (track, name) in song.tracks.iter_mut().zip(names) {
        track.name = (*name).into();
    }
}

/// 140 beats a minute, half time: a sliding 808 under the kick, hats rolling, keys and bells
/// arpeggiated over C minor.
fn trap() -> Song {
    let mut song = Song::new("Trap");
    song.tempo = 140.0;
    let mut drums = Insert::new("Drums", COLORS[0]);
    drums.effects.push(preset(builtin::COMPRESSOR, "Punchy drums"));
    let mut low = Insert::new("808", COLORS[4]);
    low.effects.push(preset(builtin::DISTORTION, "Warm drive"));
    let mut keys = Insert::new("Keys", COLORS[2]);
    let mut space = Insert::new("Space", COLORS[5]);
    space.effects.push(preset(builtin::REVERB, "Big hall").with("mix", 1.0));
    keys.sends.push(Send { to: space.id, amount: 0.3 });
    song.master.effects.push(preset(builtin::LIMITER, "Loud master"));

    let kick = channel("Kick", 0, preset(builtin::DRUMS, "Punchy kick"), drums.id);
    let clap = channel("Clap", 1, preset(builtin::DRUMS, "Big clap"), drums.id);
    let hat = channel("Hat", 2, preset(builtin::DRUMS, "Crisp hat"), drums.id);
    let eight = channel("808", 4, preset(builtin::SYNTH, "808 glide"), low.id);
    let piano = channel("Keys", 5, preset(builtin::SYNTH, "Keys"), keys.id);
    let mut bells = channel("Bells", 6, preset(builtin::SYNTH, "Pluck").with("volume", -15.0), keys.id);
    bells.arp = Some(Arp { order: ArpOrder::UpDown, step: STEP, octaves: 2, gate: 0.6 });

    // eighths, and rolls of thirty-seconds and of triplets into the second beat three
    let mut hats: Vec<Note> =
        (0..32).step_by(2).filter(|s| *s != 14 && *s != 28).map(|s| note(s * STEP, STEP, 60, 0.7)).collect();
    hats.extend((0..4).map(|i| note(14 * STEP + i * STEP / 2, STEP / 2, 60, 0.55 + 0.05 * i as f32)));
    hats.extend((0..6).map(|i| note(28 * STEP + i * 4 * STEP / 6, STEP / 2, 60, 0.5 + 0.05 * i as f32)));
    hats.sort_by_key(|n| n.start);
    let beat = pattern(
        "Beat",
        0,
        2,
        vec![
            (kick.id, steps(&[0, 10, 16, 23, 26], 60, 0.9)),
            (clap.id, steps(&[8, 24], 60, 0.85)),
            (hat.id, hats),
        ],
    );
    // notes into the next one glide there
    let slides = [(0, 11, 36), (10, 6, 43), (16, 8, 39), (23, 4, 41), (26, 6, 36)];
    let bass = slides.iter().map(|&(at, length, key)| note(at * STEP, length * STEP, key, 0.9)).collect();
    let bass = pattern("808", 4, 2, vec![(eight.id, bass)]);
    let progression: [&[u8]; 4] = [&[48, 51, 55], &[44, 48, 51], &[51, 55, 58], &[46, 50, 53]];
    let harmony = pattern("Keys", 2, 4, vec![(piano.id, chords(&progression, 0, 16, 0.6))]);
    let high: [&[u8]; 4] = [&[72, 75, 79], &[68, 72, 75], &[75, 79, 82], &[70, 74, 77]];
    let melody = pattern("Bells", 6, 4, vec![(bells.id, chords(&high, 0, 16, 0.7))]);

    song.clips = vec![
        clip(&song, 0, &harmony, 0, 16),
        clip(&song, 1, &beat, 4, 12),
        clip(&song, 2, &bass, 4, 12),
        clip(&song, 3, &melody, 8, 8),
    ];
    name_tracks(&mut song, &["Keys", "Beat", "808", "Bells"]);
    song.channels = vec![kick, clap, hat, eight, piano, bells];
    song.patterns = vec![beat, bass, harmony, melody];
    song.inserts = vec![drums, low, keys, space];
    song
}

/// 82 beats a minute: swung, dusty drums, warm seventh chords, a round bass.
fn lofi() -> Song {
    let mut song = Song::new("Lo-fi");
    song.tempo = 82.0;
    let mut drums = Insert::new("Drums", COLORS[0]);
    drums.effects.push(preset(builtin::FILTER, "Dark").with("cutoff", 5000.0));
    drums.effects.push(preset(builtin::CRUSHER, "Dusty"));
    drums.effects.push(preset(builtin::COMPRESSOR, "Gentle glue"));
    let mut keys = Insert::new("Keys", COLORS[2]);
    keys.effects.push(preset(builtin::EQ, "Warmer"));
    keys.effects.push(preset(builtin::CHORUS, "Subtle thickener"));
    let mut space = Insert::new("Space", COLORS[5]);
    space.effects.push(preset(builtin::REVERB, "Small room").with("mix", 1.0));
    keys.sends.push(Send { to: space.id, amount: 0.25 });
    song.master.effects.push(preset(builtin::EQ, "Warmer"));
    song.master.effects.push(preset(builtin::LIMITER, "Safe"));

    let kick = channel("Kick", 0, preset(builtin::DRUMS, "Punchy kick").with("tone", 0.4), drums.id);
    let snare = channel("Snare", 1, preset(builtin::DRUMS, "Tight snare").with("tone", 0.4), drums.id);
    let hat = channel("Hat", 2, preset(builtin::DRUMS, "Crisp hat").with("level", -10.0), drums.id);
    let piano = channel("Keys", 5, preset(builtin::SYNTH, "Keys").with("release", 0.8), keys.id);
    let sub = channel("Bass", 4, preset(builtin::SYNTH, "Sub bass"), keys.id);

    let hats = (0..8).map(|i| note(i * 2 * STEP, STEP, 60, if i % 2 == 0 { 0.6 } else { 0.4 })).collect();
    let mut beat = pattern(
        "Beat",
        0,
        1,
        vec![(kick.id, steps(&[0, 7, 10], 60, 0.85)), (snare.id, steps(&[4, 12], 60, 0.8)), (hat.id, hats)],
    );
    beat.swing = 0.55;
    let sevenths: [&[u8]; 4] = [&[53, 57, 60, 64], &[52, 55, 59, 62], &[50, 53, 57, 60], &[48, 52, 55, 59]];
    let mut played = chords(&sevenths, 0, 9, 0.55);
    played.extend(chords(&sevenths, 10, 6, 0.45));
    played.sort_by_key(|n| (n.start, n.key));
    let harmony = pattern("Chords", 2, 4, vec![(piano.id, played)]);
    let roots = [41u8, 40, 38, 36];
    let bass: Vec<Note> = roots
        .iter()
        .enumerate()
        .flat_map(|(bar, &key)| {
            let at = bar as Ticks * BAR;
            [note(at, 6 * STEP, key, 0.8), note(at + 10 * STEP, 4 * STEP, key, 0.7)]
        })
        .collect();
    let low = pattern("Bass", 4, 4, vec![(sub.id, bass)]);

    song.clips =
        vec![clip(&song, 0, &harmony, 0, 16), clip(&song, 1, &beat, 2, 14), clip(&song, 2, &low, 4, 12)];
    name_tracks(&mut song, &["Chords", "Beat", "Bass"]);
    song.channels = vec![kick, snare, hat, piano, sub];
    song.patterns = vec![beat, harmony, low];
    song.inserts = vec![drums, keys, space];
    song
}

/// 130 beats a minute: four on the floor, an acid line ducking under the kick as its filter
/// opens, stabs echoing.
fn techno() -> Song {
    let mut song = Song::new("Techno");
    song.tempo = 130.0;
    let thump = Insert::new("Kick", COLORS[0]);
    let mut synths = Insert::new("Synths", COLORS[4]);
    let mut pump = preset(builtin::COMPRESSOR, "Pump under the kick");
    pump.key = Some(thump.id);
    synths.effects.push(pump);
    let mut perc = Insert::new("Perc", COLORS[1]);
    perc.effects.push(preset(builtin::EQ, "Cut the lows"));
    let mut space = Insert::new("Space", COLORS[5]);
    space.effects.push(preset(builtin::DELAY, "Dotted eighth").with("mix", 1.0));
    space.effects.push(preset(builtin::REVERB, "Big hall").with("mix", 1.0));
    synths.sends.push(Send { to: space.id, amount: 0.2 });
    song.master.effects.push(preset(builtin::LIMITER, "Loud master"));

    let kick = channel("Kick", 0, preset(builtin::DRUMS, "Punchy kick").with("decay", 1.2), thump.id);
    let hat = channel(
        "Open hat",
        3,
        Device::builtin(builtin::DRUMS).with("sound", 4.0).with("level", -8.0),
        perc.id,
    );
    let clap = channel("Clap", 1, preset(builtin::DRUMS, "Big clap").with("level", -4.0), perc.id);
    let acid = channel("Acid", 4, preset(builtin::SYNTH, "Acid bass"), synths.id);
    let stab = channel(
        "Stab",
        6,
        preset(builtin::SYNTH, "Pluck").with("unison", 1.0).with("volume", -10.0),
        synths.id,
    );

    let four = pattern("Kick", 0, 1, vec![(kick.id, steps(&[0, 4, 8, 12], 60, 0.95))]);
    let offbeats = pattern(
        "Perc",
        1,
        1,
        vec![(hat.id, steps(&[2, 6, 10, 14], 60, 0.7)), (clap.id, steps(&[4, 12], 60, 0.8))],
    );
    let line = [36u8, 36, 48, 36, 39, 36, 46, 36, 36, 48, 36, 43, 36, 41, 36, 48];
    let acid_notes = line
        .iter()
        .enumerate()
        .map(|(i, &key)| note(i as Ticks * STEP, STEP, key, if i % 4 == 2 { 1.0 } else { 0.7 }));
    let acid_line = pattern("Acid", 4, 1, vec![(acid.id, acid_notes.collect())]);
    let stabs: [&[u8]; 2] = [&[60, 63, 67], &[58, 62, 65]];
    let mut stab_notes = chords(&stabs, 3, 1, 0.8);
    stab_notes.extend(chords(&stabs, 11, 1, 0.7));
    stab_notes.sort_by_key(|n| (n.start, n.key));
    let stab_line = pattern("Stab", 6, 2, vec![(stab.id, stab_notes)]);

    let mut clips = vec![
        clip(&song, 0, &four, 0, 16),
        clip(&song, 1, &offbeats, 4, 12),
        clip(&song, 2, &acid_line, 4, 12),
        clip(&song, 3, &stab_line, 8, 8),
    ];
    // the acid's filter opening over eight bars
    let points =
        vec![Point { at: 0, value: 0.35, curve: 0.0 }, Point { at: 8 * BAR, value: 0.65, curve: 0.0 }];
    let target = Target::Param { device: acid.instrument.id, param: "cutoff".into() };
    clips.push(Clip {
        id: Id::new(),
        track: song.tracks[5].id,
        start: 4 * BAR,
        length: 8 * BAR,
        offset: 0,
        content: ClipContent::Automation { target, points },
    });
    song.clips = clips;
    name_tracks(&mut song, &["Kick", "Perc", "Acid", "Stab", "Track 5", "Filter"]);
    song.channels = vec![kick, hat, clap, acid, stab];
    song.patterns = vec![four, offbeats, acid_line, stab_line];
    song.inserts = vec![thump, synths, perc, space];
    song
}

/// 155 beats a minute: a driven kick and the rumble it leaves, sixteenths of hats, a loop of
/// toms and rims, a saw stab every three steps; a break without the kick, its filter opening,
/// a riser and a hit into each drop.
fn schranz() -> Song {
    let mut song = Song::new("Schranz");
    song.tempo = 155.0;
    let mut thump = Insert::new("Kick", COLORS[0]);
    thump.effects.push(
        Device::builtin(builtin::DISTORTION).with("drive", 18.0).with("tone", 5000.0).with("output", -8.0),
    );
    // the kick's tail, smeared, driven and kept under 180 Hz, ducking under the kick itself
    let mut rumble = Insert::new("Rumble", COLORS[3]);
    rumble.effects.push(
        Device::builtin(builtin::REVERB)
            .with("size", 0.85)
            .with("damping", 0.7)
            .with("width", 0.3)
            .with("mix", 1.0),
    );
    rumble.effects.push(preset(builtin::DISTORTION, "Fuzz").with("tone", 2500.0));
    rumble.effects.push(Device::builtin(builtin::FILTER).with("cutoff", 180.0).with("resonance", 0.25));
    let mut duck = Device::builtin(builtin::COMPRESSOR)
        .with("threshold", -32.0)
        .with("ratio", 12.0)
        .with("attack", 0.5)
        .with("release", 140.0);
    duck.key = Some(thump.id);
    rumble.effects.push(duck);
    rumble.volume = 0.9;
    thump.sends.push(Send { to: rumble.id, amount: 0.55 });
    let mut perc = Insert::new("Perc", COLORS[1]);
    perc.effects.push(preset(builtin::EQ, "Cut the lows"));
    perc.effects.push(preset(builtin::DISTORTION, "Parallel grit"));
    let mut space = Insert::new("Space", COLORS[5]);
    space.effects.push(preset(builtin::EQ, "Cut the lows"));
    space.effects.push(preset(builtin::DELAY, "Dotted eighth").with("mix", 1.0).with("feedback", 0.35));
    space.effects.push(preset(builtin::REVERB, "Big hall").with("mix", 1.0));
    let mut stabs = Insert::new("Stab", COLORS[2]);
    stabs.effects.push(preset(builtin::DISTORTION, "Crunch"));
    let mut pump = preset(builtin::COMPRESSOR, "Pump under the kick");
    pump.key = Some(thump.id);
    stabs.effects.push(pump);
    stabs.sends.push(Send { to: space.id, amount: 0.25 });
    perc.sends.push(Send { to: space.id, amount: 0.08 });
    // driven into the limiter, as loud as the style is played
    song.master.effects.push(
        preset(builtin::DISTORTION, "Warm drive")
            .with("drive", 6.0)
            .with("tone", 20000.0)
            .with("output", 0.0),
    );
    song.master.effects.push(preset(builtin::LIMITER, "Loud master"));

    let drum = |sound: f64| Device::builtin(builtin::DRUMS).with("sound", sound);
    let kick =
        channel("Kick", 0, drum(0.0).with("tune", -2.0).with("decay", 0.7).with("tone", 0.8), thump.id);
    let hat = channel("Hat", 4, drum(2.0).with("decay", 0.35).with("tone", 1.0).with("level", -2.0), perc.id);
    let open =
        channel("Open hat", 6, drum(4.0).with("decay", 0.7).with("tone", 0.9).with("level", -5.0), perc.id);
    let clap = channel("Clap", 1, preset(builtin::DRUMS, "Big clap").with("level", -6.0), perc.id);
    let tom = channel("Tom", 3, drum(5.0).with("tune", 7.0).with("decay", 0.5).with("level", -5.0), perc.id);
    let rim = channel("Rim", 7, drum(6.0).with("level", -8.0), perc.id);
    let stab = channel(
        "Stab",
        2,
        Device::builtin(builtin::SYNTH)
            .with("wave1", 0.0)
            .with("wave2", 0.0)
            .with("detune2", 18.0)
            .with("unison", 2.0)
            .with("spread", 25.0)
            .with("cutoff", 1600.0)
            .with("resonance", 0.35)
            .with("envAmount", 0.5)
            .with("filterDecay", 0.12)
            .with("filterSustain", 0.0)
            .with("decay", 0.18)
            .with("sustain", 0.0)
            .with("release", 0.08)
            .with("volume", -12.0),
        stabs.id,
    );
    let riser = channel(
        "Riser",
        5,
        preset(builtin::SYNTH, "Noise riser").with("filterAttack", 5.0).with("volume", -16.0),
        stabs.id,
    );
    let hit = channel("Hit", 5, preset(builtin::SYNTH, "Noise hit").with("volume", -6.0), perc.id);

    let four = pattern("Kick", 0, 1, vec![(kick.id, steps(&[0, 4, 8, 12], 60, 1.0))]);
    let sixteenths: Vec<Note> =
        (0..16).map(|s| note(s * STEP, STEP, 60, if s % 2 == 0 { 0.45 } else { 0.7 })).collect();
    let hats = pattern("Hats", 4, 1, vec![(hat.id, sixteenths), (open.id, steps(&[2, 6, 10, 14], 60, 0.8))]);
    let claps = pattern("Clap", 1, 1, vec![(clap.id, steps(&[4, 12], 60, 0.85))]);
    let perc_loop = pattern(
        "Perc loop",
        3,
        1,
        vec![(tom.id, steps(&[3, 7, 10, 13], 60, 0.8)), (rim.id, steps(&[1, 6, 9, 11, 15], 60, 0.7))],
    );
    // the stab every three sixteenths, against the kick's four
    let stab_notes: Vec<Note> = [2, 5, 8, 11, 14]
        .iter()
        .flat_map(|&s| [note(s * STEP, STEP, 57, 0.9), note(s * STEP, STEP, 64, 0.8)])
        .collect();
    let stab_line = pattern("Stab", 2, 1, vec![(stab.id, stab_notes)]);
    let rise = pattern("Riser", 5, 4, vec![(riser.id, vec![note(0, 4 * BAR, 60, 0.8)])]);
    let impact = pattern("Hit", 5, 1, vec![(hit.id, vec![note(0, BAR, 48, 1.0)])]);

    let mut clips = vec![
        clip(&song, 0, &four, 0, 16),
        clip(&song, 0, &four, 24, 24),
        clip(&song, 1, &hats, 4, 16),
        clip(&song, 1, &hats, 24, 24),
        clip(&song, 2, &claps, 8, 8),
        clip(&song, 2, &claps, 24, 24),
        clip(&song, 3, &perc_loop, 8, 8),
        clip(&song, 3, &perc_loop, 28, 16),
        clip(&song, 4, &stab_line, 12, 12),
        clip(&song, 4, &stab_line, 24, 24),
        clip(&song, 5, &rise, 12, 4),
        clip(&song, 5, &rise, 20, 4),
        clip(&song, 5, &impact, 16, 1),
        clip(&song, 5, &impact, 24, 1),
    ];
    // the stab's filter closed at the break, opening into the drop
    let points = vec![
        Point { at: 0, value: 0.45, curve: 0.0 },
        Point { at: 4 * BAR, value: 0.35, curve: 0.0 },
        Point { at: 12 * BAR, value: 0.75, curve: 0.0 },
    ];
    let target = Target::Param { device: stab.instrument.id, param: "cutoff".into() };
    clips.push(Clip {
        id: Id::new(),
        track: song.tracks[6].id,
        start: 12 * BAR,
        length: 12 * BAR,
        offset: 0,
        content: ClipContent::Automation { target, points },
    });
    song.clips = clips;
    song.markers = [(0, "Intro"), (12, "Build"), (16, "Break"), (24, "Drop")]
        .into_iter()
        .map(|(bar, name)| Marker { id: Id::new(), at: bar * BAR, name: name.into() })
        .collect();
    name_tracks(&mut song, &["Kick", "Hats", "Clap", "Perc", "Stab", "FX", "Filter"]);
    song.channels = vec![kick, hat, open, clap, tom, rim, stab, riser, hit];
    song.patterns = vec![four, hats, claps, perc_loop, stab_line, rise, impact];
    song.inserts = vec![thump, rumble, perc, stabs, space];
    song
}

#[cfg(test)]
mod tests {
    use crate::demo::BAR;

    #[test]
    fn every_template_plays_to_its_end() {
        for name in ["trap", "lofi", "techno", "schranz"] {
            let mut song = super::song(name).unwrap();
            assert!(song.mend().is_empty(), "{name}");
            let rendered = sonevo_engine::render(&song, None, 48_000);
            assert!(rendered.missing.is_empty(), "{name}: {:?}", rendered.missing);
            let seconds = rendered.left.len() as f64 / 48_000.0;
            let bars = song.clips.iter().map(|c| c.start + c.length).max().unwrap_or(0) / BAR;
            let length = bars as f64 * 4.0 * 60.0 / song.tempo;
            assert!(seconds > length && seconds < length + 10.0, "{name}: {seconds}");
            let loudest = rendered.left.iter().chain(&rendered.right).fold(0.0f32, |a, s| a.max(s.abs()));
            assert!(loudest > 0.3 && loudest <= 1.0, "{name}: {loudest}");
        }
    }
}
