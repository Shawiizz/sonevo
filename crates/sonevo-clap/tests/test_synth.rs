//! The test synth played through the host: built first (`cargo build -p sonevo-test-synth`).

use sonevo_clap::{Bundle, Note, ParamChange};
use std::path::PathBuf;

fn synth_path() -> PathBuf {
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    target.join(format!("{}sonevo_test_synth{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX))
}

#[test]
fn a_note_played_sounds_from_its_frame_on_and_dies_away_once_released() {
    let bundle = Bundle::open(&synth_path()).expect("the test synth is built");
    let plugins = bundle.plugins();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].id, "dev.sonevo.test-synth");
    assert_eq!(plugins[0].features, ["instrument", "synthesizer"]);

    let mut synth = bundle.instantiate(&plugins[0].id).unwrap();
    synth.activate(48_000.0, 512).unwrap();
    let (mut left, mut right) = (vec![0.0f32; 256], vec![0.0f32; 256]);

    let on = Note { at: 100, key: 69, velocity: 1.0, on: true };
    assert!(synth.process(&[on], &[], &mut left, &mut right));
    assert!(left[..100].iter().all(|&s| s == 0.0), "silent before the note");
    assert!(left[100..].iter().any(|&s| s.abs() > 0.01), "sounding after it");
    assert_eq!(left, right);

    let off = Note { at: 0, key: 69, velocity: 0.0, on: false };
    assert!(synth.process(&[off], &[], &mut left, &mut right));
    // a quarter of a second later, the release has faded it out
    for _ in 0..60 {
        assert!(synth.process(&[], &[], &mut left, &mut right));
    }
    assert!(left.iter().all(|&s| s.abs() < 1e-3), "silent once released");
}

#[test]
fn a_state_saved_is_given_back() {
    let bundle = Bundle::open(&synth_path()).expect("the test synth is built");
    let synth = bundle.instantiate("dev.sonevo.test-synth").unwrap();
    let saved = synth.keeper().expect("the synth keeps a state").save().unwrap();
    assert_eq!(saved, 0.8f64.to_le_bytes());
    // another synth, given a state where its volume is nothing: silent
    let mut other = bundle.instantiate("dev.sonevo.test-synth").unwrap();
    assert!(other.load_state(&0.0f64.to_le_bytes()));
    assert!(!other.load_state(b"short"));
    other.activate(48_000.0, 512).unwrap();
    let (mut left, mut right) = (vec![0.0f32; 256], vec![0.0f32; 256]);
    let on = Note { at: 0, key: 69, velocity: 1.0, on: true };
    assert!(other.process(&[on], &[], &mut left, &mut right));
    assert!(left.iter().all(|&s| s == 0.0));
}

#[test]
fn a_setting_is_listed_and_moved_from_the_frame_asked() {
    let bundle = Bundle::open(&synth_path()).expect("the test synth is built");
    let mut synth = bundle.instantiate("dev.sonevo.test-synth").unwrap();
    let params = synth.params();
    assert_eq!(params.len(), 1);
    assert_eq!((params[0].id, params[0].name.as_str(), params[0].default), (0, "Volume", 0.8));
    synth.activate(48_000.0, 512).unwrap();
    let (mut left, mut right) = (vec![0.0f32; 256], vec![0.0f32; 256]);
    let on = Note { at: 0, key: 69, velocity: 1.0, on: true };
    assert!(synth.process(&[on], &[], &mut left, &mut right));
    let loud = left[200..].iter().fold(0.0f32, |a, s| a.max(s.abs()));
    // silenced from the block's frame 0
    let silenced = ParamChange { at: 0, id: 0, value: 0.0 };
    assert!(synth.process(&[], &[silenced], &mut left, &mut right));
    assert!(loud > 0.01);
    assert!(left.iter().all(|&s| s == 0.0));
}
