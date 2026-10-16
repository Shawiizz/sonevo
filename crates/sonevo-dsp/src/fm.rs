//! An FM synth: a sine whose phase another sine moves, the modulator, at a ratio of the note's
//! pitch. How far it moves it makes the tone bright or dull, and falls as the note goes on, as
//! an electric piano's or a bell's brightness does. Sixteen voices.

use crate::param::Param;
use crate::util::{db_to_gain, key_frequency, Adsr};
use crate::{Context, Event, Processor};
use std::f32::consts::TAU;

pub const PARAMS: &[Param] = &[
    Param::log("ratio", "Ratio", 0.5, 16.0, 1.0, "×"),
    Param::linear("index", "Brightness", 0.0, 10.0, 2.0, ""),
    Param::linear("bloom", "Brightness at the start", 0.0, 10.0, 3.0, ""),
    Param::log("fade", "Brightness fall", 0.01, 5.0, 0.4, "s"),
    Param::linear("feedback", "Grit", 0.0, 1.0, 0.0, ""),
    Param::log("attack", "Attack", 0.001, 5.0, 0.002, "s"),
    Param::log("decay", "Decay", 0.005, 10.0, 1.2, "s"),
    Param::linear("sustain", "Sustain", 0.0, 1.0, 0.3, ""),
    Param::log("release", "Release", 0.005, 10.0, 0.4, "s"),
    Param::linear("volume", "Volume", -36.0, 6.0, -8.0, "dB"),
];

const VOICES: usize = 16;

#[derive(Clone, Copy)]
struct Voice {
    key: u8,
    velocity: f32,
    frequency: f32,
    carrier: f32,
    modulator: f32,
    /// The modulator's last two outputs, fed back into it.
    last: [f32; 2],
    /// The brightness added at the start, falling to nothing.
    bloom: f32,
    amp: Adsr,
    age: u32,
}

impl Voice {
    fn idle() -> Voice {
        Voice {
            key: 0,
            velocity: 0.0,
            frequency: 0.0,
            carrier: 0.0,
            modulator: 0.0,
            last: [0.0; 2],
            bloom: 0.0,
            amp: Adsr::new(),
            age: 0,
        }
    }
}

pub struct Fm {
    sample_rate: f32,
    values: [f32; 10],
    voices: [Voice; VOICES],
    /// How much of the bloom is left after each frame.
    fall: f32,
    gain: f32,
}

impl Fm {
    pub fn new(sample_rate: f32) -> Fm {
        let mut values = [0.0; 10];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        let mut fm = Fm { sample_rate, values, voices: [Voice::idle(); VOICES], fall: 0.0, gain: 1.0 };
        fm.apply();
        fm
    }

    fn apply(&mut self) {
        let v = self.values;
        self.fall = (-1.0 / (v[3] * self.sample_rate)).exp();
        self.gain = db_to_gain(v[9]);
        for voice in &mut self.voices {
            voice.amp.set(self.sample_rate, v[5], v[6], v[7], v[8]);
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
            voice.carrier = 0.0;
            voice.modulator = 0.0;
            voice.last = [0.0; 2];
        }
        voice.key = key;
        voice.velocity = velocity.clamp(0.0, 1.0);
        voice.frequency = key_frequency(f32::from(key));
        // harder, brighter
        voice.bloom = self.values[2] * (0.4 + 0.6 * voice.velocity);
        voice.age = 0;
        voice.amp.start();
    }

    fn release(&mut self, key: u8) {
        for voice in self.voices.iter_mut().filter(|v| v.key == key) {
            voice.amp.release();
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let (ratio, index, feedback) = (self.values[0], self.values[1], self.values[4]);
        let (rate, fall, gain) = (self.sample_rate, self.fall, self.gain);
        for voice in self.voices.iter_mut().filter(|v| !v.amp.idle()) {
            let carrier_step = voice.frequency / rate;
            let modulator_step = carrier_step * ratio;
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                let fed = feedback * 0.5 * (voice.last[0] + voice.last[1]);
                let modulation = (TAU * voice.modulator + fed * std::f32::consts::PI).sin();
                voice.last = [modulation, voice.last[0]];
                let depth = index + voice.bloom;
                let sample = (TAU * voice.carrier + depth * modulation).sin();
                voice.carrier = (voice.carrier + carrier_step).fract();
                voice.modulator = (voice.modulator + modulator_step).fract();
                voice.bloom *= fall;
                let level = voice.amp.next() * voice.velocity * gain;
                *l += sample * level;
                *r += sample * level;
                if voice.amp.idle() {
                    break;
                }
            }
        }
    }
}

impl Processor for Fm {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
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
    }

    fn reset(&mut self) {
        self.voices = [Voice::idle(); VOICES];
        self.apply();
    }
}
