//! These tests play Sonevo's own test synth and test effect, built beside them by
//! `cargo build -p sonevo-test-synth -p sonevo-test-effect -p sonevo-plugin-host`.

use sonevo_dsp::{Context, Event, Processor};
use sonevo_plugins::{scan, Remote};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RATE: f32 = 48_000.0;

fn built() -> PathBuf {
    built_as("sonevo_test_synth.dll")
}

fn built_as(name: &str) -> PathBuf {
    let deps = std::env::current_exe().unwrap();
    deps.parent().unwrap().parent().unwrap().join(name)
}

/// A folder with the test synth installed in it, as a CLAP bundle.
fn installed() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let vendor = dir.path().join("Sonevo");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::copy(built(), vendor.join("Test Synth.clap")).expect("the test synth is built");
    dir
}

fn play(remote: &mut Remote, events: &[Event], blocks: usize) -> f32 {
    let context = Context { sample_rate: RATE, tempo: 120.0, key: None };
    let mut loudest = 0.0f32;
    for i in 0..blocks {
        let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
        remote.process(&context, if i == 0 { events } else { &[] }, &mut left, &mut right);
        loudest = left.iter().chain(&right).fold(loudest, |a, s| a.max(s.abs()));
    }
    loudest
}

fn note(on: bool) -> Event {
    Event { at: 0, key: 60, velocity: 0.9, on }
}

#[test]
fn an_effect_changes_the_sound_it_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("Test Gain.clap");
    std::fs::copy(built_as("sonevo_test_effect.dll"), &bundle).expect("the test effect is built");
    let found = scan(&[dir.path().to_owned()]);
    assert!(!found[0].instrument);
    let mut remote = Remote::start(&bundle, "dev.sonevo.test-effect", RATE, &[], None).unwrap();
    let context = Context { sample_rate: RATE, tempo: 120.0, key: None };
    let tone = |i: usize| (i as f32 * 0.05).sin() * 0.8;
    // until its process answers, the sound goes through as it is; then at its gain of a half
    let start = Instant::now();
    let mut ratio = 1.0;
    while (ratio - 1.0f32).abs() < 1e-3 && start.elapsed() < Duration::from_secs(10) {
        let mut left: Vec<f32> = (0..256).map(tone).collect();
        let mut right = left.clone();
        remote.process(&context, &[], &mut left, &mut right);
        ratio = left[100] / tone(100);
    }
    assert!((ratio - 0.5).abs() < 1e-3, "{ratio}");
}

#[test]
fn bundles_in_subfolders_are_found_and_what_is_not_a_plugin_is_left_out() {
    let dir = installed();
    std::fs::write(dir.path().join("Broken.clap"), b"not a plugin").unwrap();
    let found = scan(&[dir.path().to_owned(), Path::new("Z:/nowhere").to_owned()]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].id, "dev.sonevo.test-synth");
    assert_eq!(found[0].name, "Sonevo Test Synth");
    assert!(found[0].instrument);
}

#[test]
fn a_plugin_plays_in_its_own_process_and_plays_on_after_a_crash() {
    let dir = installed();
    let bundle = dir.path().join("Sonevo").join("Test Synth.clap");
    let mut remote = Remote::start(&bundle, "dev.sonevo.test-synth", RATE, &[], None).unwrap();
    // the process starting: blocks are silent until it answers
    let start = Instant::now();
    let mut heard = play(&mut remote, &[note(true)], 1);
    while heard == 0.0 && start.elapsed() < Duration::from_secs(10) {
        heard = play(&mut remote, &[], 1);
    }
    assert!(heard > 0.01, "the plugin never answered");

    // its process killed, as a crash would: started again, the key held pressed again
    kill_hosts(&bundle);
    let start = Instant::now();
    while remote.restarts() == 0 && start.elapsed() < Duration::from_secs(10) {
        play(&mut remote, &[], 1);
    }
    assert_eq!(remote.restarts(), 1);
    let start = Instant::now();
    let mut again = 0.0;
    while again == 0.0 && start.elapsed() < Duration::from_secs(10) {
        again = play(&mut remote, &[], 4);
    }
    assert!(again > 0.01, "the key held was not pressed again");
    play(&mut remote, &[note(false)], 200);
    assert!(play(&mut remote, &[], 2) < 1e-4);
}

/// Ends the plugin host processes playing `bundle`, as a crash would.
fn kill_hosts(bundle: &Path) {
    let name = bundle.file_name().unwrap().to_string_lossy();
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='sonevo-plugin-host.exe'\" | \
         Where-Object {{ $_.CommandLine -like '*{name}*' }} | \
         ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}"
    );
    let status =
        std::process::Command::new("powershell").args(["-NoProfile", "-Command", &script]).status().unwrap();
    assert!(status.success());
}

#[test]
fn a_song_plays_a_plugin_through_the_engine() {
    use sonevo_project::{Device, DeviceKind, Song};
    let dir = installed();
    let bundle = dir.path().join("Sonevo").join("Test Synth.clap");
    let mut song = Song::new("Plugin");
    song.channels[0].instrument = Device {
        kind: DeviceKind::Clap {
            bundle: bundle.to_string_lossy().into_owned(),
            plugin: "dev.sonevo.test-synth".into(),
        },
        ..Device::builtin("")
    };
    let channel = song.channels[0].id;
    let (mut engine, mut renderer) = sonevo_engine::Engine::new(RATE);
    engine.set_plugins(std::sync::Arc::new(sonevo_plugins::Clap::new(std::sync::Arc::default())));
    assert!(engine.load(&song, sonevo_engine::Mode::Song, None).is_empty());
    engine.note(channel, 64, 0.9, true);
    let (mut left, mut right) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    let start = Instant::now();
    let mut heard = 0.0f32;
    while heard < 0.01 && start.elapsed() < Duration::from_secs(10) {
        renderer.render(&mut left, &mut right);
        heard = left.iter().fold(0.0, |a, s| a.max(s.abs()));
    }
    assert!(heard >= 0.01, "the plugin was not heard");

    // without a way to start plugins, the engine says so
    let (mut plain, _renderer) = sonevo_engine::Engine::new(RATE);
    let missing = plain.load(&song, sonevo_engine::Mode::Song, None);
    assert_eq!(missing, ["dev.sonevo.test-synth: plugins are not played here"]);
}

#[test]
fn a_song_played_to_a_file_plays_its_plugins_from_the_first_block() {
    use sonevo_project::{Clip, ClipContent, Device, DeviceKind, Id, Note, Song, PPQ};
    let dir = installed();
    let bundle = dir.path().join("Sonevo").join("Test Synth.clap");
    let mut song = Song::new("Plugin");
    song.tempo = 120.0;
    song.channels[0].instrument = Device {
        kind: DeviceKind::Clap {
            bundle: bundle.to_string_lossy().into_owned(),
            plugin: "dev.sonevo.test-synth".into(),
        },
        ..Device::builtin("")
    };
    let pattern = song.patterns[0].id;
    song.patterns[0]
        .notes
        .insert(song.channels[0].id, vec![Note { start: 0, length: 2 * PPQ, key: 60, velocity: 0.9 }]);
    let length = song.patterns[0].length;
    let track = song.tracks[0].id;
    song.clips.push(Clip {
        id: Id::new(),
        track,
        start: 0,
        length,
        offset: 0,
        content: ClipContent::Pattern { pattern },
    });
    let plugins = std::sync::Arc::new(sonevo_plugins::Offline::new([song.channels[0].instrument.id], None));
    let start = Instant::now();
    let rendered = sonevo_engine::bounce(&song, None, 48_000, None, sonevo_engine::Mode::Song, Some(plugins));
    assert!(rendered.missing.is_empty(), "{:?}", rendered.missing);
    // the plugin waited for as it loads, not given up on
    assert!(start.elapsed() < Duration::from_secs(10), "{:?}", start.elapsed());
    let first = rendered.left[..2400].iter().fold(0.0f32, |a, s| a.max(s.abs()));
    assert!(first > 0.01, "the first blocks were silent");
}

#[test]
fn a_plugins_settings_are_read_and_set_from_the_song() {
    use sonevo_project::{Device, DeviceKind, Song};
    let dir = installed();
    let bundle = dir.path().join("Sonevo").join("Test Synth.clap");
    let bundle_text = bundle.to_string_lossy().into_owned();
    let params = sonevo_plugins::params(&bundle_text, "dev.sonevo.test-synth");
    assert_eq!(params.len(), 1);
    assert_eq!((params[0].id, params[0].name, params[0].default), ("0", "Volume", 0.8));

    // the song turns the plugin's volume down to nothing
    let mut song = Song::new("Plugin");
    song.channels[0].instrument = Device {
        kind: DeviceKind::Clap { bundle: bundle_text, plugin: "dev.sonevo.test-synth".into() },
        ..Device::builtin("")
    }
    .with("0", 0.0);
    let channel = song.channels[0].id;
    let (mut engine, mut renderer) = sonevo_engine::Engine::new(RATE);
    engine.set_plugins(std::sync::Arc::new(sonevo_plugins::Clap::new(std::sync::Arc::default())));
    assert!(engine.load(&song, sonevo_engine::Mode::Song, None).is_empty());
    engine.note(channel, 64, 0.9, true);
    let (mut left, mut right) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    let start = Instant::now();
    let mut answered = false;
    while start.elapsed() < Duration::from_secs(3) {
        renderer.render(&mut left, &mut right);
        answered |= left.iter().any(|s| s.abs() > 0.0);
    }
    assert!(!answered, "the plugin was heard though its volume is 0");
    // turned up as a knob would: heard
    let device = song.channels[0].instrument.id;
    assert!(engine.set_param(device, "0", 1.0));
    // the piano's first note has faded away by now: another one
    engine.note(channel, 64, 0.0, false);
    engine.note(channel, 67, 0.9, true);
    let start = Instant::now();
    while !answered && start.elapsed() < Duration::from_secs(5) {
        renderer.render(&mut left, &mut right);
        answered = left.iter().any(|s| s.abs() > 0.01);
    }
    assert!(answered, "the plugin was not heard once turned up");
}

#[test]
fn a_plugins_state_is_saved_and_given_back_from_the_songs_folder() {
    use sonevo_project::{Device, DeviceKind, Song};
    let dir = installed();
    let bundle = dir.path().join("Sonevo").join("Test Synth.clap").to_string_lossy().into_owned();
    let mut song = Song::new("State");
    song.channels[0].instrument = Device {
        kind: DeviceKind::Clap { bundle, plugin: "dev.sonevo.test-synth".into() },
        ..Device::builtin("")
    };
    let device = song.channels[0].instrument.id;
    let channel = song.channels[0].id;
    let folder = tempfile::tempdir().unwrap();
    let shared = std::sync::Arc::new(std::sync::Mutex::new(Some(folder.path().to_owned())));

    // played, its state saved: the volume it starts at
    let (mut engine, mut renderer) = sonevo_engine::Engine::new(RATE);
    engine.set_plugins(std::sync::Arc::new(sonevo_plugins::Clap::new(shared.clone())));
    assert!(engine.load(&song, sonevo_engine::Mode::Song, None).is_empty());
    let (mut left, mut right) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    renderer.render(&mut left, &mut right);
    let start = Instant::now();
    let mut saved = None;
    while saved.is_none() && start.elapsed() < Duration::from_secs(10) {
        saved = sonevo_plugins::save_state(device);
    }
    assert_eq!(saved.as_deref(), Some(&0.8f64.to_le_bytes()[..]));

    // a state where the volume is nothing, in the song's folder: the plugin started again is silent
    drop((engine, renderer));
    let file = sonevo_plugins::state_file(folder.path(), device);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, 0.0f64.to_le_bytes()).unwrap();
    let (mut engine, mut renderer) = sonevo_engine::Engine::new(RATE);
    engine.set_plugins(std::sync::Arc::new(sonevo_plugins::Clap::new(shared)));
    assert!(engine.load(&song, sonevo_engine::Mode::Song, None).is_empty());
    engine.note(channel, 64, 0.9, true);
    let start = Instant::now();
    let mut heard = false;
    while start.elapsed() < Duration::from_secs(3) {
        renderer.render(&mut left, &mut right);
        heard |= left.iter().any(|s| s.abs() > 0.0);
    }
    assert!(!heard, "the state was not given back");
}
