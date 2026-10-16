mod common;

use common::*;
use sonevo_dsp::Sample;
use sonevo_engine::{decode, render, write_wav, Depth, Engine, Meter, Mode};
use sonevo_project::{builtin, Clip, ClipContent, Device, Id, Point, Region, Send, Target, PPQ};
use std::sync::Arc;

#[test]
fn notes_sound_where_they_are() {
    let (song, _, _) = song(&[note(1, 1, 60)]);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    let (left, _) = play(&mut renderer, beat(3.0));
    assert_eq!(loudest(&left[..beat(1.0)]), 0.0, "sound before the note");
    assert!(loudest(&left[beat(1.0)..beat(2.0)]) > 0.05, "no sound for the note");
}

#[test]
fn nothing_plays_until_play() {
    let (song, _, _) = song(&[note(0, 1, 60)]);
    let (_engine, mut renderer) = started(&song, Mode::Song);
    let (left, right) = play(&mut renderer, beat(2.0));
    assert_eq!(loudest(&left).max(loudest(&right)), 0.0);
}

#[test]
fn a_pattern_goes_round() {
    let (song, _, pattern) = song(&[note(0, 1, 60)]);
    let (mut engine, mut renderer) = started(&song, Mode::Pattern(pattern));
    engine.play();
    // a bar of four beats, played twice: the note at the start of each
    let (left, _) = play(&mut renderer, beat(8.0));
    assert!(loudest(&left[..beat(0.5)]) > 0.05);
    assert!(loudest(&left[beat(3.5)..beat(4.0)]) < 0.01, "the note did not end");
    assert!(loudest(&left[beat(4.0)..beat(4.5)]) > 0.05, "the pattern did not go round");
}

#[test]
fn stop_goes_back_where_playing_started() {
    let (song, _, _) = song(&[note(0, 4, 60)]);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.seek(PPQ);
    engine.play();
    play(&mut renderer, beat(1.0));
    assert!((engine.position() - 2.0 * PPQ as f64).abs() < 50.0, "{}", engine.position());
    engine.stop();
    play(&mut renderer, 480);
    assert_eq!(engine.position(), PPQ as f64);
    assert!(!engine.playing());
    // the note let go of: the synth's release dies away
    play(&mut renderer, beat(2.0));
    let (left, _) = play(&mut renderer, 4800);
    assert_eq!(loudest(&left), 0.0);
}

#[test]
fn a_muted_channel_and_a_muted_track_are_silent() {
    let (mut song, _, _) = song(&[note(0, 1, 60)]);
    song.channels[0].mute = true;
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    assert_eq!(loudest(&play(&mut renderer, beat(1.0)).0), 0.0);

    song.channels[0].mute = false;
    song.tracks[0].mute = true;
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    assert_eq!(loudest(&play(&mut renderer, beat(1.0)).0), 0.0);
}

#[test]
fn solo_leaves_only_its_track_heard() {
    let (mut song, _, _) = song(&[note(0, 1, 60)]);
    let synth = insert(&mut song, "Synth");
    let other = insert(&mut song, "Other");
    song.channels[0].insert = Some(synth);
    song.inserts[1].solo = true;
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    assert_eq!(loudest(&play(&mut renderer, beat(1.0)).0), 0.0, "heard though another is soloed");

    song.inserts[1].solo = false;
    song.inserts[0].solo = true;
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    assert!(loudest(&play(&mut renderer, beat(1.0)).0) > 0.05);
    let _ = other;
}

#[test]
fn routes_going_round_in_a_loop_are_cut() {
    let (mut song, _, _) = song(&[note(0, 1, 60)]);
    let a = insert(&mut song, "A");
    let b = insert(&mut song, "B");
    song.inserts[0].output = Some(b);
    song.inserts[1].output = Some(a);
    song.inserts[1].sends.push(Send { to: a, amount: 1.0 });
    song.channels[0].insert = Some(a);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    let heard = loudest(&play(&mut renderer, beat(1.0)).0);
    assert!(heard > 0.05 && heard < 2.0, "{heard}");
}

#[test]
fn sends_add_the_sound_to_another_track() {
    let (mut song, _, _) = song(&[note(0, 1, 60)]);
    let dry = insert(&mut song, "Dry");
    let wet = insert(&mut song, "Wet");
    song.channels[0].insert = Some(dry);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    let alone = loudest(&play(&mut renderer, beat(0.5)).0);

    song.inserts[0].sends.push(Send { to: wet, amount: 1.0 });
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    let sent = loudest(&play(&mut renderer, beat(0.5)).0);
    assert!((sent / alone - 2.0).abs() < 0.05, "{alone} then {sent}");
}

#[test]
fn a_song_changed_while_playing_goes_on() {
    let (mut song, _, _) = song(&[note(0, 4, 60)]);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    play(&mut renderer, beat(1.0));
    // a setting changed: the synth is kept, its note goes on sounding
    let device = song.channels[0].instrument.id;
    song.channels[0].instrument.params.insert("cutoff".into(), 2000.0);
    assert!(engine.load(&song, Mode::Song, None).is_empty());
    let (left, _) = play(&mut renderer, beat(1.0));
    assert!(loudest(&left[..480]) > 0.05, "the note was cut");
    assert!(engine.set_param(device, "volume", -6.0));
    assert!(!engine.set_param(device, "nothing", 1.0));
    let (left, _) = play(&mut renderer, beat(1.0));
    assert!(loudest(&left) > 0.02);
}

#[test]
fn a_sound_listened_to_plays_through_once() {
    let (song, _, _) = song(&[]);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    let tone: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    engine.listen(Some(Arc::new(Sample { sample_rate: 48_000.0, left: tone.clone(), right: tone })));
    assert!(loudest(&play(&mut renderer, 4800).0) > 0.3);
    play(&mut renderer, 24_000);
    assert_eq!(loudest(&play(&mut renderer, 4800).0), 0.0, "it went on past its end");
    engine.listen(None);
    play(&mut renderer, 480);
}

#[test]
fn a_key_pressed_plays_while_stopped() {
    let (song, channel, _) = song(&[]);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.note(channel, 64, 0.9, true);
    assert!(loudest(&play(&mut renderer, 4800).0) > 0.05);
    engine.note(channel, 64, 0.0, false);
    play(&mut renderer, beat(2.0));
    assert_eq!(loudest(&play(&mut renderer, 4800).0), 0.0);
}

#[test]
fn automation_moves_the_volume() {
    let (mut song, channel, _) = song(&[note(0, 4, 60)]);
    let track = song.tracks[1].id;
    let points = vec![Point { at: 0, value: 0.0, curve: 0.0 }, Point { at: 4 * PPQ, value: 1.0, curve: 0.0 }];
    song.clips.push(Clip {
        id: Id::new(),
        track,
        start: 0,
        length: 4 * PPQ,
        offset: 0,
        content: ClipContent::Automation { target: Target::ChannelVolume { channel }, points },
    });
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    let (left, _) = play(&mut renderer, beat(4.0));
    let early = loudest(&left[beat(0.25)..beat(0.5)]);
    let late = loudest(&left[beat(3.25)..beat(3.5)]);
    assert!(late > early * 4.0, "{early} then {late}");
}

#[test]
fn tempo_automation_speeds_the_song_up() {
    let (mut song, _, _) = song(&[]);
    let track = song.tracks[1].id;
    // all the way up: 240 beats a minute
    let points = vec![Point { at: 0, value: 1.0, curve: 0.0 }];
    let length = 4 * PPQ;
    let content = ClipContent::Automation { target: Target::Tempo, points };
    song.clips.push(Clip { id: Id::new(), track, start: 0, length, offset: 0, content });
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    play(&mut renderer, beat(1.0));
    assert!((engine.position() - 2.0 * PPQ as f64).abs() < 100.0, "{}", engine.position());
}

#[test]
fn the_loop_region_goes_round() {
    let (mut song, _, _) = song(&[note(0, 1, 60), note(2, 1, 67)]);
    song.loop_region = Some(Region { start: 2 * PPQ, end: 3 * PPQ });
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.seek(2 * PPQ);
    engine.play();
    play(&mut renderer, beat(2.5));
    let at = engine.position();
    assert!(at >= 2.0 * PPQ as f64 && at < 3.0 * PPQ as f64, "{at}");
}

#[test]
fn audio_clips_play_their_sound() {
    let (mut song, _, _) = song(&[]);
    let tone: Vec<f32> = (0..24_000).map(|i| (i as f32 * 0.06).sin() * 0.5).collect();
    let sample = Arc::new(Sample { sample_rate: 24_000.0, left: tone.clone(), right: tone });
    let track = song.tracks[2].id;
    let content = ClipContent::Audio { sample: "tone.wav".into(), gain: 1.0 };
    song.clips.push(Clip { id: Id::new(), track, start: PPQ, length: 2 * PPQ, offset: 0, content });
    let (mut engine, mut renderer) = Engine::new(RATE);
    engine.samples().insert("tone.wav", sample);
    assert!(engine.load(&song, Mode::Song, None).is_empty());
    engine.play();
    let (left, _) = play(&mut renderer, beat(4.0));
    assert_eq!(loudest(&left[..beat(1.0)]), 0.0);
    assert!(loudest(&left[beat(1.0)..beat(2.0)]) > 0.4);
    // the sound is a second long: it ends before the clip does
    assert_eq!(loudest(&left[beat(3.05)..beat(4.0)]), 0.0);
}

#[test]
fn the_sampler_plays_its_sound() {
    let (mut song, channel, _) = song(&[]);
    let tone: Vec<f32> = (0..48_000).map(|i| (i as f32 * 0.03).sin() * 0.5).collect();
    let mut device = Device::builtin(builtin::SAMPLER);
    device.sample = Some("tone.wav".into());
    song.channels[0].instrument = device;
    let (mut engine, mut renderer) = Engine::new(RATE);
    engine
        .samples()
        .insert("tone.wav", Arc::new(Sample { sample_rate: RATE, left: tone.clone(), right: tone }));
    assert!(engine.load(&song, Mode::Song, None).is_empty());
    engine.note(channel, 60, 1.0, true);
    assert!(loudest(&play(&mut renderer, 4800).0) > 0.1);
}

#[test]
fn what_is_missing_is_said() {
    let (mut song, _, _) = song(&[]);
    let mut device = Device::builtin(builtin::SAMPLER);
    device.sample = Some("gone.wav".into());
    song.channels[1].instrument = device;
    let (mut engine, _renderer) = Engine::new(RATE);
    let missing = engine.load(&song, Mode::Song, None);
    assert_eq!(missing.len(), 1, "{missing:?}");
}

#[test]
fn meters_hear_what_plays() {
    let (song, channel, _) = song(&[note(0, 2, 60)]);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    play(&mut renderer, beat(1.0));
    assert!(engine.meter(Meter::Master) > 0.05);
    assert!(engine.meter(Meter::Channel(channel)) > 0.05);
    // read, then nothing since
    assert_eq!(engine.meter(Meter::Master), 0.0);
    assert_eq!(engine.meter(Meter::Channel(Id::new())), 0.0);
}

#[test]
fn a_song_renders_to_its_end_and_its_tails() {
    let (mut song, _, _) = song(&[note(0, 1, 60), note(3, 1, 64)]);
    song.master.effects.push(Device::builtin(builtin::REVERB));
    let rendered = render(&song, None, 48_000);
    assert!(rendered.missing.is_empty());
    let bar = beat(4.0);
    // the bar, then the reverb's tail; never all of the next time round
    assert!(rendered.left.len() > bar && rendered.left.len() < bar + 48_000 * 11, "{}", rendered.left.len());
    assert!(loudest(&rendered.left[bar..]) > 0.0, "no tail");
    assert!(loudest(&rendered.left[..beat(0.5)]) > 0.05);
}

#[test]
fn a_compressor_listening_to_the_kick_ducks_the_pad() {
    let (mut song, _, _) = song(&[note(0, 4, 48)]);
    let pad = insert(&mut song, "Pad");
    let kick = insert(&mut song, "Kick");
    song.channels[0].insert = Some(pad);
    let mut drums = song.channels[0].clone();
    drums.id = Id::new();
    drums.instrument = Device::builtin(builtin::DRUMS);
    drums.insert = Some(kick);
    song.patterns[0].notes.insert(drums.id, vec![note(2, 1, 60)]);
    song.channels.push(drums);
    let mut compressor =
        Device::builtin(builtin::COMPRESSOR).with("threshold", -40.0).with("ratio", 20.0).with("attack", 0.1);
    song.inserts[0].effects.push(compressor.clone());
    let level = |song: &sonevo_project::Song| {
        let stem = sonevo_engine::render_stem(song, None, 48_000, pad);
        (loudest(&stem.left[beat(1.5)..beat(1.9)]), loudest(&stem.left[beat(2.05)..beat(2.2)]))
    };

    let (before, during) = level(&song);
    assert!((during / before - 1.0).abs() < 0.2, "ducked listening to itself: {before} then {during}");

    compressor.key = Some(kick);
    song.inserts[0].effects[0] = compressor;
    let (before, during) = level(&song);
    assert!(during < before * 0.5, "not ducked under the kick: {before} then {during}");
}

#[test]
fn a_track_listening_to_one_after_it_is_left_alone() {
    let (mut song, _, _) = song(&[note(0, 1, 60)]);
    let a = insert(&mut song, "A");
    let b = insert(&mut song, "B");
    song.channels[0].insert = Some(a);
    // A goes to B, so B listening to A is fine; A listening to B would be a loop, and is let go
    song.inserts[0].output = Some(b);
    let mut listens = Device::builtin(builtin::COMPRESSOR);
    listens.key = Some(b);
    song.inserts[0].effects.push(listens);
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.play();
    assert!(loudest(&play(&mut renderer, beat(0.5)).0) > 0.05);
}

#[test]
fn a_stem_holds_its_track_only() {
    let (mut song, _, _) = song(&[note(0, 1, 60)]);
    let dry = insert(&mut song, "Dry");
    let wet = insert(&mut song, "Wet");
    song.channels[0].insert = Some(dry);
    song.inserts[0].sends.push(Send { to: wet, amount: 1.0 });
    song.inserts[1].effects.push(Device::builtin(builtin::REVERB).with("mix", 1.0));
    let dry_stem = sonevo_engine::render_stem(&song, None, 48_000, dry);
    let wet_stem = sonevo_engine::render_stem(&song, None, 48_000, wet);
    let master = sonevo_engine::render_stem(&song, None, 48_000, song.master.id);
    // the dry track ends with its note; the wet one rings on; nothing goes straight to the master
    let after = beat(1.5);
    assert!(loudest(&dry_stem.left[..beat(0.5)]) > 0.05);
    assert!(loudest(&dry_stem.left[after..]) < 0.01, "{}", loudest(&dry_stem.left[after..]));
    assert!(loudest(&wet_stem.left[after..]) > 0.01);
    assert_eq!(loudest(&master.left), 0.0);
}

#[test]
fn a_part_of_a_song_renders_alone() {
    let (song, _, _) = song(&[note(0, 1, 60), note(2, 1, 64)]);
    let part = sonevo_engine::render_part(&song, None, 48_000, Some(Region { start: 2 * PPQ, end: 3 * PPQ }));
    // the third beat's note from the start, then its tail
    assert!(loudest(&part.left[..beat(0.25)]) > 0.05);
    assert!(part.left.len() >= beat(1.0) && part.left.len() < beat(1.0) + 48_000 * 2, "{}", part.left.len());
}

#[test]
fn a_rendered_song_reads_back_from_its_file() {
    let (song, _, _) = song(&[note(0, 1, 60)]);
    let rendered = render(&song, None, 48_000);
    let folder = tempfile::tempdir().unwrap();
    for depth in [Depth::Pcm16, Depth::Pcm24, Depth::Float32] {
        let path = folder.path().join(format!("{depth:?}.wav"));
        write_wav(&path, 48_000, &rendered.left, &rendered.right, depth).unwrap();
        let read = decode(&path).unwrap();
        assert_eq!(read.sample_rate, 48_000.0);
        assert_eq!(read.frames(), rendered.left.len());
        let error = read.left.iter().zip(&rendered.left).fold(0.0f32, |a, (x, y)| a.max((x - y).abs()));
        assert!(error < 1e-3, "{depth:?}: {error}");
    }
}
