use std::sync::Arc;

use sonevo_dsp::{builtin, Context, Event, Processor, Sample, Scale, BUILTINS};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 480;
const CONTEXT: Context = Context { sample_rate: RATE, tempo: 128.0 };

fn note(at: u32, key: u8, on: bool) -> Event {
    Event { at, key, velocity: 0.8, on }
}

/// Plays `blocks` blocks, the events given for the first; the loudest sample heard.
fn play(device: &mut dyn Processor, events: &[Event], blocks: usize) -> f32 {
    let mut loudest = 0.0f32;
    for i in 0..blocks {
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        device.process(&CONTEXT, if i == 0 { events } else { &[] }, &mut left, &mut right);
        for s in left.iter().chain(&right) {
            assert!(s.is_finite(), "{s} heard");
            loudest = loudest.max(s.abs());
        }
    }
    loudest
}

fn instrument(name: &str) -> Box<dyn Processor> {
    let mut made = builtin(name).unwrap().make(RATE);
    let tone = Sample {
        sample_rate: 44_100.0,
        left: (0..44_100).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
        right: (0..44_100).map(|i| (i as f32 * 0.05).sin() * 0.5).collect(),
    };
    made.set_sample(Some(Arc::new(tone)));
    made
}

#[test]
fn every_instrument_sounds_then_ends() {
    for b in BUILTINS.iter().filter(|b| b.instrument) {
        let mut device = instrument(b.name);
        let loudest =
            play(device.as_mut(), &[note(0, 60, true), note(100, 64, true), note(200, 67, true)], 20);
        assert!(loudest > 0.01 && loudest < 4.0, "{}: {loudest}", b.name);
        let ending = [note(0, 60, false), note(0, 64, false), note(0, 67, false)];
        play(device.as_mut(), &ending, 400);
        assert_eq!(play(device.as_mut(), &[], 2), 0.0, "{} still sounding", b.name);
    }
}

#[test]
fn reset_ends_every_voice() {
    for b in BUILTINS.iter().filter(|b| b.instrument) {
        let mut device = instrument(b.name);
        play(device.as_mut(), &[note(0, 48, true)], 4);
        device.reset();
        assert_eq!(play(device.as_mut(), &[], 1), 0.0, "{}", b.name);
    }
}

#[test]
fn every_setting_at_its_ends_stays_bounded() {
    for b in BUILTINS {
        for end in [0.0, 1.0] {
            let mut device = instrument(b.name);
            for (i, p) in b.params.iter().enumerate() {
                device.set(i, p.from_normal(end));
            }
            let loud = |frame: usize| ((frame as f32) * 0.031).sin() * 0.9;
            let mut loudest = 0.0f32;
            for i in 0..100 {
                let (mut left, mut right): (Vec<f32>, Vec<f32>) = if b.instrument {
                    (vec![0.0; BLOCK], vec![0.0; BLOCK])
                } else {
                    ((0..BLOCK).map(loud).collect(), (0..BLOCK).map(loud).collect())
                };
                let events: &[Event] =
                    if i == 0 { &[Event { at: 0, key: 36, velocity: 1.0, on: true }] } else { &[] };
                device.process(&CONTEXT, events, &mut left, &mut right);
                for s in left.iter().chain(&right) {
                    assert!(s.is_finite(), "{} at {end}: {s}", b.name);
                    loudest = loudest.max(s.abs());
                }
            }
            assert!(loudest < 16.0, "{} at {end}: {loudest}", b.name);
        }
    }
}

#[test]
fn effects_at_their_defaults_let_the_sound_through() {
    for b in BUILTINS.iter().filter(|b| !b.instrument) {
        let mut device = b.make(RATE);
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        let mut heard = 0.0f32;
        for _ in 0..20 {
            left.iter_mut()
                .chain(right.iter_mut())
                .enumerate()
                .for_each(|(i, s)| *s = (i as f32 * 0.02).sin() * 0.5);
            device.process(&CONTEXT, &[], &mut left, &mut right);
            heard = heard.max(left.iter().fold(0.0, |a, s| a.max(s.abs())));
        }
        assert!(heard > 0.1 && heard < 2.0, "{}: {heard}", b.name);
    }
}

#[test]
fn a_flat_eq_changes_nothing() {
    let mut eq = builtin("sonevo.eq").unwrap().make(RATE);
    let original: Vec<f32> = (0..BLOCK).map(|i| (i as f32 * 0.1).sin()).collect();
    let (mut left, mut right) = (original.clone(), original.clone());
    eq.process(&CONTEXT, &[], &mut left, &mut right);
    assert_eq!(left, original);
    assert_eq!(right, original);
}

#[test]
fn the_limiter_keeps_under_its_ceiling() {
    let limiter = builtin("sonevo.limiter").unwrap();
    let mut device = limiter.make(RATE);
    device.set(limiter.param("ceiling").unwrap(), -6.0);
    let ceiling = 10f32.powf(-6.0 / 20.0);
    for block in 0..50 {
        let mut left: Vec<f32> =
            (0..BLOCK).map(|i| ((i + block * BLOCK) as f32 * 0.05).sin() * 3.0).collect();
        let mut right = left.clone();
        device.process(&CONTEXT, &[], &mut left, &mut right);
        for s in left.iter().chain(&right) {
            assert!(s.abs() <= ceiling * 1.001, "{s}");
        }
    }
}

#[test]
fn the_compressor_brings_loud_sounds_down() {
    let compressor = builtin("sonevo.compressor").unwrap();
    let mut device = compressor.make(RATE);
    let mut last = 0.0f32;
    for block in 0..100 {
        let mut left: Vec<f32> = (0..BLOCK).map(|i| ((i + block * BLOCK) as f32 * 0.05).sin()).collect();
        let mut right = left.clone();
        device.process(&CONTEXT, &[], &mut left, &mut right);
        last = left.iter().fold(0.0, |a, s| a.max(s.abs()));
    }
    // 0 dB in, 18 over the threshold at 4:1: about 13 dB down
    assert!(last > 0.15 && last < 0.35, "{last}");
}

#[test]
fn the_delay_echoes_on_the_beat() {
    let delay = builtin("sonevo.delay").unwrap();
    let mut device = delay.make(RATE);
    device.set(delay.param("time").unwrap(), 3.0);
    device.set(delay.param("mix").unwrap(), 1.0);
    let beat = (60.0 / CONTEXT.tempo * f64::from(RATE)) as usize;
    let mut heard = Vec::new();
    for block in 0..(beat * 2 / BLOCK + 1) {
        let mut left = vec![0.0; BLOCK];
        if block == 0 {
            left[0] = 1.0;
        }
        let mut right = left.clone();
        device.process(&CONTEXT, &[], &mut left, &mut right);
        heard.extend(left);
    }
    let loudest_after =
        heard[1..]
            .iter()
            .enumerate()
            .fold((0, 0.0f32), |a, (i, s)| if s.abs() > a.1 { (i + 1, s.abs()) } else { a });
    assert!(loudest_after.0.abs_diff(beat) < 8, "echo at {} for a beat of {beat}", loudest_after.0);
}

#[test]
fn settings_go_to_and_from_the_knob() {
    for b in BUILTINS {
        for p in b.params {
            for position in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let value = p.from_normal(position);
                assert!(value >= p.min && value <= p.max, "{}.{}", b.name, p.id);
                if !matches!(p.scale, Scale::Choice(_)) {
                    assert!((p.to_normal(value) - position).abs() < 1e-9, "{}.{}", b.name, p.id);
                }
            }
            assert!(p.default >= p.min && p.default <= p.max, "{}.{} default", b.name, p.id);
        }
        let mut ids: Vec<_> = b.params.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), b.params.len(), "{} has a setting twice", b.name);
    }
}

#[test]
fn every_builtin_the_song_names_exists() {
    for name in sonevo_project::builtin::ALL {
        assert!(builtin(name).is_some(), "{name}");
    }
}

#[test]
fn the_synth_lfo_makes_a_tremolo() {
    let synth = builtin("sonevo.synth").unwrap();
    let mut device = synth.make(RATE);
    device.set(synth.param("sustain").unwrap(), 1.0);
    device.set(synth.param("lfoTarget").unwrap(), 2.0);
    device.set(synth.param("lfoRate").unwrap(), 2.0);
    device.set(synth.param("lfoAmount").unwrap(), 1.0);
    play(device.as_mut(), &[note(0, 57, true)], 10);
    // a quarter of a second apart, half a cycle at 2 Hz: one loud, one quiet
    let a = play(device.as_mut(), &[], 1);
    play(device.as_mut(), &[], 24);
    let b = play(device.as_mut(), &[], 1);
    assert!(a.max(b) > 3.0 * a.min(b), "{a} then {b}");
}
