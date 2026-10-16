//! A sampler: a sound played from a key, at its own pitch on its root key and higher or
//! lower on the others, or at its own pitch on every key; held with the note, or played
//! to its end whatever the note's length. Cut in slices, each key from the root up plays one.

use crate::param::Param;
use crate::util::{db_to_gain, Adsr};
use crate::{Context, Event, Processor};
use std::sync::Arc;

/// A sound decoded, as the sampler plays it: two channels (a mono sound in both).
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub sample_rate: f32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

impl Sample {
    pub fn frames(&self) -> usize {
        self.left.len().min(self.right.len())
    }
}

const MODES: &[&str] = &["One shot", "Held"];
const PITCH: &[&str] = &["Follows the key", "Same on every key"];
const DIRECTIONS: &[&str] = &["Forward", "Reverse"];
const SLICES: &[&str] = &["Off", "4", "8", "16", "32"];

pub const PARAMS: &[Param] = &[
    Param::linear("root", "Root key", 0.0, 127.0, 60.0, ""),
    Param::choice("mode", "Mode", MODES, 0),
    Param::choice("pitch", "Pitch", PITCH, 0),
    Param::log("attack", "Attack", 0.001, 5.0, 0.001, "s"),
    Param::log("release", "Release", 0.005, 10.0, 0.1, "s"),
    Param::linear("start", "Start", 0.0, 1.0, 0.0, ""),
    Param::choice("direction", "Direction", DIRECTIONS, 0),
    Param::linear("tune", "Tune", -24.0, 24.0, 0.0, "st"),
    Param::linear("gain", "Gain", -36.0, 12.0, 0.0, "dB"),
    Param::choice("slices", "Slices", SLICES, 0),
];

const VOICES: usize = 16;

#[derive(Clone, Copy)]
struct Voice {
    on: bool,
    key: u8,
    velocity: f32,
    /// Where it is in the sound, in its frames; and how far it moves each frame played.
    at: f64,
    step: f64,
    /// The part of the sound it plays, in frames: all of it, or its slice.
    from: f64,
    to: f64,
    amp: Adsr,
    age: u32,
}

impl Voice {
    fn idle() -> Voice {
        Voice {
            on: false,
            key: 0,
            velocity: 0.0,
            at: 0.0,
            step: 1.0,
            from: 0.0,
            to: 0.0,
            amp: Adsr::new(),
            age: 0,
        }
    }
}

pub struct Sampler {
    sample_rate: f32,
    sample: Option<Arc<Sample>>,
    values: [f64; 10],
    voices: [Voice; VOICES],
    gain: f32,
}

impl Sampler {
    pub fn new(sample_rate: f32) -> Sampler {
        let mut values = [0.0; 10];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default;
        }
        Sampler { sample_rate, sample: None, values, voices: [Voice::idle(); VOICES], gain: 1.0 }
    }

    fn held(&self) -> bool {
        self.values[1].round() as i32 == 1
    }

    fn reversed(&self) -> bool {
        self.values[6].round() as i32 == 1
    }

    /// How many slices the sound is cut in; none when it is played whole.
    fn slices(&self) -> usize {
        match self.values[9].round() as i32 {
            n @ 1..=4 => 2 << n,
            _ => 0,
        }
    }

    fn press(&mut self, key: u8, velocity: f32) {
        let Some(sample) = &self.sample else { return };
        let frames = sample.frames();
        if frames == 0 {
            return;
        }
        for voice in &mut self.voices {
            voice.age = voice.age.saturating_add(1);
        }
        let slices = self.slices();
        let (from, to, semitones) = if slices > 0 {
            let slice = i64::from(key) - self.values[0].round() as i64;
            if slice < 0 || slice >= slices as i64 {
                return;
            }
            let width = frames as f64 / slices as f64;
            (slice as f64 * width, (slice + 1) as f64 * width, self.values[7])
        } else {
            let follows = self.values[2].round() as i32 == 0;
            let semitones = self.values[7] + if follows { f64::from(key) - self.values[0] } else { 0.0 };
            // the start is as far from the end when played backwards
            let start = self.values[5] * frames as f64;
            if self.reversed() {
                (0.0, frames as f64 - start, semitones)
            } else {
                (start, frames as f64, semitones)
            }
        };
        let to = to.min((frames - 1) as f64);
        let rate = f64::from(sample.sample_rate) / f64::from(self.sample_rate) * 2f64.powf(semitones / 12.0);
        let (at, step) = if self.reversed() { (to - 1.0, -rate) } else { (from, rate) };
        let slot = self
            .voices
            .iter()
            .position(|v| !v.on)
            .unwrap_or_else(|| (0..VOICES).max_by_key(|&i| self.voices[i].age).unwrap_or(0));
        let mut amp = Adsr::new();
        amp.set(self.sample_rate, self.values[3] as f32, 1.0, 1.0, self.values[4] as f32);
        amp.start();
        self.voices[slot] =
            Voice { on: true, key, velocity: velocity.clamp(0.0, 1.0), at, step, from, to, amp, age: 0 };
    }

    fn release(&mut self, key: u8) {
        if !self.held() {
            return;
        }
        for voice in self.voices.iter_mut().filter(|v| v.on && v.key == key) {
            voice.amp.release();
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let Some(sample) = &self.sample else { return };
        let gain = self.gain;
        for voice in self.voices.iter_mut().filter(|v| v.on) {
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                if voice.at < voice.from.max(0.0) || voice.at >= voice.to {
                    voice.on = false;
                    break;
                }
                let i = voice.at as usize;
                let frac = (voice.at - i as f64) as f32;
                let level = voice.amp.next() * voice.velocity * gain;
                *l += (sample.left[i] + (sample.left[i + 1] - sample.left[i]) * frac) * level;
                *r += (sample.right[i] + (sample.right[i + 1] - sample.right[i]) * frac) * level;
                voice.at += voice.step;
                if voice.amp.idle() {
                    voice.on = false;
                    break;
                }
            }
        }
    }
}

impl Processor for Sampler {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value);
            self.gain = db_to_gain(self.values[8] as f32);
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
    }

    fn set_sample(&mut self, sample: Option<Arc<Sample>>) -> Option<Arc<Sample>> {
        self.reset();
        std::mem::replace(&mut self.sample, sample)
    }
}
