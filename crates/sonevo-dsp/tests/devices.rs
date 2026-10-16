use std::sync::Arc;

use sonevo_dsp::{builtin, Context, Event, Processor, Sample, Scale, BUILTINS};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 480;
const CONTEXT: Context = Context { sample_rate: RATE, tempo: 128.0, key: None };

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
fn every_preset_sets_what_its_device_has_within_its_range() {
    for preset in sonevo_dsp::PRESETS {
        let device = builtin(preset.device).unwrap_or_else(|| panic!("{}", preset.device));
        for &(id, value) in preset.values {
            let param = device.params.iter().find(|p| p.id == id);
            let param = param.unwrap_or_else(|| panic!("{} has no {id}", preset.device));
            assert_eq!(param.clamp(value), value, "{}: {id}", preset.name);
        }
    }
}

#[test]
fn every_instrument_preset_sounds() {
    for preset in sonevo_dsp::PRESETS.iter().filter(|p| builtin(p.device).is_some_and(|b| b.instrument)) {
        let device = builtin(preset.device).unwrap();
        let mut made = instrument(preset.device);
        for &(id, value) in preset.values {
            made.set(device.param(id).unwrap(), value);
        }
        let loudest = play(&mut *made, &[note(0, 48, true)], 40);
        assert!(loudest > 0.02 && loudest < 4.0, "{}: {loudest}", preset.name);
    }
}

#[test]
fn a_sampler_in_slices_plays_one_a_key() {
    let sampler = builtin("sonevo.sampler").unwrap();
    let mut device = sampler.make(RATE);
    // four parts as loud as 0.1, 0.2, 0.3 and 0.4
    let part = 4800;
    let level: Vec<f32> = (0..4 * part).map(|i| 0.1 * (1 + i / part) as f32).collect();
    device.set_sample(Some(Arc::new(Sample { sample_rate: RATE, left: level.clone(), right: level })));
    device.set(sampler.param("slices").unwrap(), 1.0);
    device.set(sampler.param("attack").unwrap(), 0.001);
    let mut heard = Vec::new();
    for block in 0..20 {
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        let events = [Event { at: 0, key: 62, velocity: 1.0, on: true }];
        device.process(&CONTEXT, if block == 0 { &events } else { &[] }, &mut left, &mut right);
        heard.extend(left);
    }
    assert!((heard[part / 2] - 0.3).abs() < 0.01, "{}", heard[part / 2]);
    assert!(heard[part + 10..].iter().all(|s| *s == 0.0), "the slice played on");
    let below = play(&mut *device, &[note(0, 59, true)], 4);
    assert_eq!(below, 0.0, "a key under the root plays no slice");
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
fn the_chorus_plays_its_copy_late() {
    let chorus = builtin("sonevo.chorus").unwrap();
    let mut device = chorus.make(RATE);
    device.set(chorus.param("depth").unwrap(), 0.0);
    device.set(chorus.param("mix").unwrap(), 1.0);
    let mut left = vec![0.0; BLOCK * 2];
    left[0] = 1.0;
    let mut right = left.clone();
    device.process(&CONTEXT, &[], &mut left, &mut right);
    let loudest =
        left.iter().enumerate().fold((0, 0.0f32), |a, (i, s)| if s.abs() > a.1 { (i, s.abs()) } else { a });
    // 12 milliseconds late
    assert_eq!(loudest.0, 576, "{loudest:?}");
}

#[test]
fn the_phaser_cuts_a_notch() {
    let phaser = builtin("sonevo.phaser").unwrap();
    let level = |hz: f32| {
        let mut device = phaser.make(RATE);
        device.set(phaser.param("depth").unwrap(), 0.0);
        device.set(phaser.param("feedback").unwrap(), 0.0);
        device.set(phaser.param("stages").unwrap(), 0.0);
        let mut last = 0.0f32;
        for block in 0..40 {
            let mut left: Vec<f32> = (0..BLOCK)
                .map(|i| (std::f32::consts::TAU * hz * (i + block * BLOCK) as f32 / RATE).sin())
                .collect();
            let mut right = left.clone();
            device.process(&CONTEXT, &[], &mut left, &mut right);
            last = left.iter().fold(0.0, |a, s| a.max(s.abs()));
        }
        last
    };
    // four stages at 900 Hz: each a quarter turn there, an eighth at about 373 Hz, where
    // the four together are half a turn and cancel what comes in
    assert!(level(373.0) < 0.05, "{}", level(373.0));
    assert!(level(900.0) > 0.95, "{}", level(900.0));
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

/// The synth as a plain sine, one voice, gliding for `glide` seconds.
fn sine_synth(glide: f64) -> Box<dyn Processor> {
    let synth = builtin("sonevo.synth").unwrap();
    let mut device = synth.make(RATE);
    for (id, value) in
        [("wave1", 3.0), ("mix", 0.0), ("cutoff", 20000.0), ("envAmount", 0.0), ("voices", 1.0)]
    {
        device.set(synth.param(id).unwrap(), value);
    }
    device.set(synth.param("glide").unwrap(), glide);
    device
}

/// How often the sound crosses zero going up over `blocks` blocks, in hertz.
fn pitch_of(device: &mut dyn Processor, events: &[Event], blocks: usize) -> f32 {
    let mut heard = Vec::new();
    for block in 0..blocks {
        let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
        device.process(&CONTEXT, if block == 0 { events } else { &[] }, &mut left, &mut right);
        heard.extend(left);
    }
    let ups = heard.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    ups as f32 / (heard.len() as f32 / RATE)
}

#[test]
fn one_voice_plays_the_last_key_and_goes_back_to_the_one_held() {
    let mut synth = sine_synth(0.0);
    pitch_of(&mut *synth, &[note(0, 48, true)], 5);
    let high = pitch_of(&mut *synth, &[note(0, 60, true)], 20);
    assert!((high - 261.6).abs() < 6.0, "{high}");
    let back = pitch_of(&mut *synth, &[note(0, 60, false)], 20);
    assert!((back - 130.8).abs() < 6.0, "{back}");
}

#[test]
fn glide_slides_from_one_key_to_the_next() {
    let mut synth = sine_synth(0.3);
    pitch_of(&mut *synth, &[note(0, 48, true)], 10);
    pitch_of(&mut *synth, &[note(0, 60, true)], 10);
    let sliding = pitch_of(&mut *synth, &[], 10);
    assert!(sliding > 160.0 && sliding < 245.0, "{sliding}");
    pitch_of(&mut *synth, &[], 30);
    let there = pitch_of(&mut *synth, &[], 20);
    assert!((there - 261.6).abs() < 6.0, "{there}");
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
