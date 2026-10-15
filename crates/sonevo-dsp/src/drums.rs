//! A drum kit made of nothing but oscillators and noise: kick, snare, hats, clap, tom, rim,
//! cowbell. A hit lasts as long as its sound, whatever the note's length; a note above or
//! below middle C tunes it up or down.

use crate::param::Param;
use crate::util::{db_to_gain, Noise, Svf};
use crate::{Context, Event, Processor};
use std::f32::consts::TAU;

const SOUNDS: &[&str] = &["Kick", "Snare", "Closed hat", "Clap", "Open hat", "Tom", "Rim", "Cowbell"];

pub const PARAMS: &[Param] = &[
    Param::choice("sound", "Sound", SOUNDS, 0),
    Param::linear("tune", "Tune", -24.0, 24.0, 0.0, "st"),
    Param::log("decay", "Decay", 0.25, 4.0, 1.0, "×"),
    Param::linear("tone", "Tone", 0.0, 1.0, 0.5, ""),
    Param::linear("level", "Level", -36.0, 6.0, 0.0, "dB"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sound {
    Kick,
    Snare,
    ClosedHat,
    Clap,
    OpenHat,
    Tom,
    Rim,
    Cowbell,
}

impl Sound {
    fn from_index(index: f64) -> Sound {
        match index.round() as i32 {
            1 => Sound::Snare,
            2 => Sound::ClosedHat,
            3 => Sound::Clap,
            4 => Sound::OpenHat,
            5 => Sound::Tom,
            6 => Sound::Rim,
            7 => Sound::Cowbell,
            _ => Sound::Kick,
        }
    }

    /// How long its body takes to fall to a thousandth, in seconds, at a decay of 1.
    fn length(self) -> f32 {
        match self {
            Sound::Kick => 0.45,
            Sound::Snare => 0.22,
            Sound::ClosedHat => 0.06,
            Sound::Clap => 0.3,
            Sound::OpenHat => 0.45,
            Sound::Tom => 0.5,
            Sound::Rim => 0.05,
            Sound::Cowbell => 0.35,
        }
    }
}

const HITS: usize = 8;

#[derive(Clone, Copy)]
struct Hit {
    on: bool,
    /// Frames since it was struck.
    t: u32,
    velocity: f32,
    /// The tune and the note together, as a ratio.
    pitch: f32,
    phase: f32,
    phase2: f32,
    body: f32,
    fall: f32,
    noise: Noise,
    filter: Svf,
    age: u32,
}

impl Hit {
    fn idle() -> Hit {
        Hit {
            on: false,
            t: 0,
            velocity: 0.0,
            pitch: 1.0,
            phase: 0.0,
            phase2: 0.0,
            body: 0.0,
            fall: 0.0,
            noise: Noise::new(1),
            filter: Svf::default(),
            age: 0,
        }
    }
}

pub struct Drums {
    sample_rate: f32,
    sound: Sound,
    tune: f32,
    decay: f32,
    tone: f32,
    gain: f32,
    hits: [Hit; HITS],
    strikes: u32,
}

impl Drums {
    pub fn new(sample_rate: f32) -> Drums {
        Drums {
            sample_rate,
            sound: Sound::Kick,
            tune: 0.0,
            decay: 1.0,
            tone: 0.5,
            gain: 1.0,
            hits: [Hit::idle(); HITS],
            strikes: 0,
        }
    }

    fn strike(&mut self, key: u8, velocity: f32) {
        self.strikes = self.strikes.wrapping_add(1);
        for hit in &mut self.hits {
            hit.age = hit.age.saturating_add(1);
        }
        // a closed hat stops an open one, as on a real hi-hat
        if self.sound == Sound::ClosedHat {
            for hit in self.hits.iter_mut().filter(|h| h.on) {
                hit.fall = hit.fall.min((1e-3f32).powf(1.0 / (0.01 * self.sample_rate)));
            }
        }
        let at = self
            .hits
            .iter()
            .position(|h| !h.on)
            .unwrap_or_else(|| (0..HITS).max_by_key(|&i| self.hits[i].age).unwrap_or(0));
        let rate = self.sample_rate;
        let pitch = 2f32.powf((self.tune + f32::from(key) - 60.0) / 12.0);
        let mut filter = Svf::default();
        let tone = self.tone;
        match self.sound {
            Sound::Snare => filter.set(1800.0 + 4000.0 * tone, 0.2, rate),
            Sound::ClosedHat | Sound::OpenHat => filter.set(6000.0 + 5000.0 * tone, 0.3, rate),
            Sound::Clap => filter.set(900.0 + 900.0 * tone, 0.5, rate),
            Sound::Rim => filter.set(1500.0 + 1500.0 * tone, 0.7, rate),
            Sound::Cowbell => filter.set(700.0 + 600.0 * tone, 0.6, rate),
            Sound::Kick | Sound::Tom => filter.set(400.0, 0.0, rate),
        }
        self.hits[at] = Hit {
            on: true,
            t: 0,
            velocity: velocity.clamp(0.0, 1.0),
            pitch,
            phase: 0.0,
            phase2: 0.0,
            body: 1.0,
            fall: (1e-3f32).powf(1.0 / (self.sound.length() * self.decay * rate)),
            noise: Noise::new(self.strikes.wrapping_mul(2_654_435_761)),
            filter,
            age: 0,
        };
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let rate = self.sample_rate;
        let (sound, tone, gain) = (self.sound, self.tone, self.gain);
        for hit in self.hits.iter_mut().filter(|h| h.on) {
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                let seconds = hit.t as f32 / rate;
                let s = match sound {
                    Sound::Kick | Sound::Tom => {
                        let (start, end, drop) =
                            if sound == Sound::Kick { (160.0, 48.0, 28.0) } else { (230.0, 110.0, 18.0) };
                        let f = (end + (start - end) * (-seconds * drop).exp()) * hit.pitch;
                        hit.phase = (hit.phase + f / rate) % 1.0;
                        let click = if seconds < 0.004 {
                            hit.noise.next() * (1.0 - seconds / 0.004) * 0.4 * tone
                        } else {
                            0.0
                        };
                        (hit.phase * TAU).sin() * hit.body + click
                    }
                    Sound::Snare => {
                        let f = 185.0 * hit.pitch;
                        hit.phase = (hit.phase + f / rate) % 1.0;
                        let tone_part = (hit.phase * TAU).sin() * (-seconds * 30.0).exp() * 0.6;
                        let noise = hit.filter.process(hit.noise.next()).high;
                        tone_part + noise * hit.body * 0.8
                    }
                    Sound::ClosedHat | Sound::OpenHat => {
                        hit.filter.process(hit.noise.next()).high * hit.body * 0.7
                    }
                    Sound::Clap => {
                        // three quick bursts, then the tail
                        let burst = seconds % 0.011 < 0.006 && seconds < 0.033;
                        let shape = if seconds < 0.033 {
                            if burst {
                                1.0
                            } else {
                                0.15
                            }
                        } else {
                            hit.body
                        };
                        hit.filter.process(hit.noise.next()).band * shape * 1.6
                    }
                    Sound::Rim => {
                        let f = 1700.0 * hit.pitch;
                        hit.phase = (hit.phase + f / rate) % 1.0;
                        let ping = (hit.phase * TAU).sin() * 0.5;
                        (ping + hit.filter.process(hit.noise.next()).band) * hit.body
                    }
                    Sound::Cowbell => {
                        hit.phase = (hit.phase + 540.0 * hit.pitch / rate) % 1.0;
                        hit.phase2 = (hit.phase2 + 800.0 * hit.pitch / rate) % 1.0;
                        let square = |p: f32| if p < 0.5 { 1.0 } else { -1.0 };
                        hit.filter.process(square(hit.phase) + square(hit.phase2)).band * hit.body * 0.6
                    }
                };
                let out = s * hit.velocity * gain;
                *l += out;
                *r += out;
                hit.body *= hit.fall;
                hit.t = hit.t.saturating_add(1);
                if hit.body < 1e-4 {
                    hit.on = false;
                    break;
                }
            }
        }
    }
}

impl Processor for Drums {
    fn set(&mut self, index: usize, value: f64) {
        let Some(p) = PARAMS.get(index) else { return };
        let v = p.clamp(value);
        match index {
            0 => self.sound = Sound::from_index(v),
            1 => self.tune = v as f32,
            2 => self.decay = v as f32,
            3 => self.tone = v as f32,
            _ => self.gain = db_to_gain(v as f32),
        }
    }

    fn process(&mut self, _context: &Context, events: &[Event], left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        let frames = left.len();
        let mut done = 0;
        for event in events.iter().filter(|e| e.on) {
            let at = (event.at as usize).min(frames);
            if at > done {
                self.render(&mut left[done..at], &mut right[done..at]);
                done = at;
            }
            self.strike(event.key, event.velocity);
        }
        self.render(&mut left[done..], &mut right[done..]);
    }

    fn reset(&mut self) {
        self.hits = [Hit::idle(); HITS];
    }
}
