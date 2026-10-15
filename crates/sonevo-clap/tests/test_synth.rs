//! The test synth played through the host: built first (`cargo build -p sonevo-test-synth`).

use sonevo_clap::{Bundle, Note};
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
    assert!(synth.process(&[on], &mut left, &mut right));
    assert!(left[..100].iter().all(|&s| s == 0.0), "silent before the note");
    assert!(left[100..].iter().any(|&s| s.abs() > 0.01), "sounding after it");
    assert_eq!(left, right);

    let off = Note { at: 0, key: 69, velocity: 0.0, on: false };
    assert!(synth.process(&[off], &mut left, &mut right));
    // a quarter of a second later, the release has faded it out
    for _ in 0..60 {
        assert!(synth.process(&[], &mut left, &mut right));
    }
    assert!(left.iter().all(|&s| s.abs() < 1e-3), "silent once released");
}
