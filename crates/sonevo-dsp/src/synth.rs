//! A subtractive synth: two oscillators, up to four copies of each spread around the note
//! and across the stereo field, a resonant low-pass with its own envelope, sixteen voices.

use crate::param::Param;
use crate::util::{db_to_gain, key_frequency, Adsr, Svf, Wave};
use crate::{Context, Event, Processor};

const WAVES: &[&str] = &["Saw", "Square", "Triangle", "Sine"];
const UNISON: &[&str] = &["1", "2", "3", "4"];

pub const PARAMS: &[Param] = &[
    Param::choice("wave1", "Oscillator 1", WAVES, 0),
    Param::choice("wave2", "Oscillator 2", WAVES, 1),
    Param::linear("semitones2", "Oscillator 2 pitch", -24.0, 24.0, 0.0, "st"),
    Param::linear("detune2", "Oscillator 2 fine", -50.0, 50.0, 7.0, "ct"),
    Param::linear("mix", "Oscillator mix", 0.0, 1.0, 0.5, ""),
    Param::choice("unison", "Unison", UNISON, 0),
    Param::linear("spread", "Unison spread", 0.0, 50.0, 15.0, "ct"),
    Param::log("cutoff", "Cutoff", 20.0, 20000.0, 2500.0, "Hz"),
    Param::linear("resonance", "Resonance", 0.0, 1.0, 0.25, ""),
    Param::linear("envAmount", "Filter envelope", -1.0, 1.0, 0.4, ""),
    Param::log("filterAttack", "Filter attack", 0.001, 5.0, 0.005, "s"),
    Param::log("filterDecay", "Filter decay", 0.005, 10.0, 0.4, "s"),
    Param::linear("filterSustain", "Filter sustain", 0.0, 1.0, 0.2, ""),
    Param::log("filterRelease", "Filter release", 0.005, 10.0, 0.3, "s"),
    Param::log("attack", "Attack", 0.001, 5.0, 0.005, "s"),
    Param::log("decay", "Decay", 0.005, 10.0, 0.6, "s"),
    Param::linear("sustain", "Sustain", 0.0, 1.0, 0.7, ""),
    Param::log("release", "Release", 0.005, 10.0, 0.25, "s"),
    Param::linear("volume", "Volume", -36.0, 6.0, -6.0, "dB"),
    Param::choice("lfoTarget", "LFO moves", LFO_TARGETS, 0),
    Param::log("lfoRate", "LFO speed", 0.05, 20.0, 4.0, "Hz"),
    Param::linear("lfoAmount", "LFO depth", 0.0, 1.0, 0.0, ""),
];

/// What the LFO moves: the filter (a wah), the pitch (a vibrato), the level (a tremolo).
const LFO_TARGETS: &[&str] = &["Cutoff", "Pitch", "Volume"];
/// How far the LFO moves the cutoff, and the pitch, at its full depth.
const LFO_OCTAVES: f32 = 3.0;
const LFO_SEMITONES: f32 = 2.0;

const VOICES: usize = 16;
const MAX_UNISON: usize = 4;
/// Frames between two settings of a voice's filter: its envelope moves smoothly enough.
const FILTER_EVERY: u32 = 16;
/// How far the filter envelope moves the cutoff at its full amount.
const ENV_OCTAVES: f32 = 4.0;

#[derive(Clone, Copy)]
struct Voice {
    key: u8,
    velocity: f32,
    frequency: f32,
    phase1: [f32; MAX_UNISON],
    phase2: [f32; MAX_UNISON],
    amp: Adsr,
    env: Adsr,
    left: Svf,
    right: Svf,
    /// Frames until the filter is set again.
    filter_in: u32,
    age: u32,
}

impl Voice {
    fn idle() -> Voice {
        Voice {
            key: 0,
            velocity: 0.0,
            frequency: 0.0,
            phase1: [0.0; MAX_UNISON],
            phase2: [0.0; MAX_UNISON],
            amp: Adsr::new(),
            env: Adsr::new(),
            left: Svf::default(),
            right: Svf::default(),
            filter_in: 0,
            age: 0,
        }
    }
}

pub struct Synth {
    sample_rate: f32,
    values: [f64; 22],
    /// Where the LFO is in its cycle, from 0 to 1: the same for every voice.
    lfo: f32,
    voices: [Voice; VOICES],
    /// Each unison copy's detune (cents at full spread) and pan, for the count chosen.
    layout: [(f32, f32, f32); MAX_UNISON],
    unison: usize,
    gain: f32,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Synth {
        let mut synth = Synth {
            sample_rate,
            values: [0.0; 22],
            lfo: 0.0,
            voices: [Voice::idle(); VOICES],
            layout: [(0.0, 1.0, 1.0); MAX_UNISON],
            unison: 1,
            gain: 1.0,
        };
        for (i, p) in PARAMS.iter().enumerate() {
            synth.values[i] = p.default;
        }
        synth.apply();
        synth
    }

    fn v(&self, index: usize) -> f32 {
        self.values[index] as f32
    }

    /// What follows from the settings: the envelopes of every voice, the unison's layout.
    fn apply(&mut self) {
        self.unison = (self.values[5].round() as usize + 1).clamp(1, MAX_UNISON);
        let n = self.unison;
        for (u, slot) in self.layout.iter_mut().enumerate() {
            let place = if n == 1 { 0.0 } else { (u as f32 / (n - 1) as f32) * 2.0 - 1.0 };
            let angle = (place * 0.8 + 1.0) * std::f32::consts::FRAC_PI_4;
            *slot = (place, angle.cos(), angle.sin());
        }
        self.gain = db_to_gain(self.v(18)) / (n as f32).sqrt();
        let (rate, s) = (self.sample_rate, &self.values);
        for voice in &mut self.voices {
            voice.amp.set(rate, s[14] as f32, s[15] as f32, s[16] as f32, s[17] as f32);
            voice.env.set(rate, s[10] as f32, s[11] as f32, s[12] as f32, s[13] as f32);
        }
    }

    fn press(&mut self, key: u8, velocity: f32) {
        for voice in &mut self.voices {
            voice.age = voice.age.saturating_add(1);
        }
        let same = self.voices.iter().position(|v| v.key == key && !v.amp.idle());
        let free = || self.voices.iter().position(|v| v.amp.idle());
        let oldest = || (0..VOICES).max_by_key(|&i| self.voices[i].age).unwrap_or(0);
        let at = same.or_else(free).unwrap_or_else(oldest);
        let voice = &mut self.voices[at];
        if voice.amp.idle() {
            // a voice started from silence starts its waves at random places: the copies of
            // the unison do not cancel each other
            for (u, (p1, p2)) in voice.phase1.iter_mut().zip(voice.phase2.iter_mut()).enumerate() {
                let seed = (key as u32 * 31 + u as u32 * 17 + voice.age) as f32;
                *p1 = (seed * 0.618_034) % 1.0;
                *p2 = (seed * 0.414_214) % 1.0;
            }
            voice.left.clear();
            voice.right.clear();
        }
        voice.key = key;
        voice.velocity = velocity.clamp(0.0, 1.0);
        voice.frequency = key_frequency(f32::from(key));
        voice.age = 0;
        voice.filter_in = 0;
        voice.amp.start();
        voice.env.start();
    }

    fn release(&mut self, key: u8) {
        for voice in self.voices.iter_mut().filter(|v| v.key == key) {
            voice.amp.release();
            voice.env.release();
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let rate = self.sample_rate;
        let (wave1, wave2) = (Wave::from_index(self.values[0]), Wave::from_index(self.values[1]));
        let ratio2 = 2f32.powf((self.v(2) * 100.0 + self.v(3)) / 1200.0);
        let (mix, spread, cutoff, resonance, amount) =
            (self.v(4), self.v(6), self.v(7), self.v(8), self.v(9));
        let unison = self.unison;
        let layout = self.layout;
        let gain = self.gain;
        let (lfo_target, lfo_step, depth) = (self.values[19].round() as i32, self.v(20) / rate, self.v(21));
        let lfo_start = self.lfo;
        let lfo_at =
            |frame: usize| (std::f32::consts::TAU * (lfo_start + frame as f32 * lfo_step)).sin() * depth;
        for voice in self.voices.iter_mut().filter(|v| !v.amp.idle()) {
            let mut steps = [(0.0f32, 0.0f32); MAX_UNISON];
            for (u, step) in steps.iter_mut().enumerate().take(unison) {
                let detune = 2f32.powf(layout[u].0 * spread / 1200.0);
                let f = voice.frequency * detune / rate;
                *step = (f, f * ratio2);
            }
            for (frame, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
                let env = voice.env.next();
                let lfo = if depth > 0.0 { lfo_at(frame) } else { 0.0 };
                if voice.filter_in == 0 {
                    let wah = if lfo_target == 0 { LFO_OCTAVES * lfo } else { 0.0 };
                    let moved = (cutoff * 2f32.powf(amount * ENV_OCTAVES * env + wah)).min(rate * 0.45);
                    voice.left.set(moved, resonance, rate);
                    voice.right.set(moved, resonance, rate);
                    voice.filter_in = FILTER_EVERY;
                }
                voice.filter_in -= 1;
                let (mut sum_l, mut sum_r) = (0.0, 0.0);
                let vibrato = if lfo_target == 1 { 2f32.powf(lfo * LFO_SEMITONES / 12.0) } else { 1.0 };
                for u in 0..unison {
                    let (d1, d2) = (steps[u].0 * vibrato, steps[u].1 * vibrato);
                    let s = (1.0 - mix) * wave1.at(voice.phase1[u], d1) + mix * wave2.at(voice.phase2[u], d2);
                    sum_l += s * layout[u].1;
                    sum_r += s * layout[u].2;
                    voice.phase1[u] = (voice.phase1[u] + d1) % 1.0;
                    voice.phase2[u] = (voice.phase2[u] + d2) % 1.0;
                }
                let tremolo = if lfo_target == 2 { 1.0 - (depth - lfo) * 0.5 } else { 1.0 };
                let level = voice.amp.next() * voice.velocity * gain * tremolo;
                *l += voice.left.process(sum_l).low * level;
                *r += voice.right.process(sum_r).low * level;
                if voice.amp.idle() {
                    break;
                }
            }
        }
    }
}

impl Processor for Synth {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value);
            self.apply();
        }
    }

    fn process(&mut self, _context: &Context, events: &[Event], left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        let frames = left.len();
        let mut done = 0;
        for event in events {
            let at = (event.at as usize).min(frames);
            if at > done {
                self.render(&mut left[done..at], &mut right[done..at]);
                done = at;
            }
            if event.on {
                self.press(event.key, event.velocity);
            } else {
                self.release(event.key);
            }
        }
        self.render(&mut left[done..], &mut right[done..]);
        self.lfo = (self.lfo + frames as f32 * self.v(20) / self.sample_rate).fract();
    }

    fn reset(&mut self) {
        self.voices = [Voice::idle(); VOICES];
        self.apply();
    }
}
