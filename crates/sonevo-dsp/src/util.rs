//! What the devices are made of: gains, smoothing, envelopes, oscillators, filters, noise.

use std::f32::consts::{PI, TAU};

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.max(1e-9).log10()
}

/// A value gliding to where it is set within a few milliseconds: no click when it moves.
#[derive(Clone, Copy)]
pub struct Smooth {
    value: f32,
    target: f32,
    coeff: f32,
}

impl Smooth {
    pub fn new(value: f32, sample_rate: f32, seconds: f32) -> Smooth {
        Smooth { value, target: value, coeff: (-1.0 / (seconds * sample_rate)).exp() }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    pub fn next(&mut self) -> f32 {
        self.value = self.target + (self.value - self.target) * self.coeff;
        self.value
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Attack, decay, sustain, release: rising straight, falling as sounds do.
#[derive(Clone, Copy)]
pub struct Adsr {
    pub stage: Stage,
    pub level: f32,
    attack_step: f32,
    decay_coeff: f32,
    sustain: f32,
    release_coeff: f32,
}

/// Where a falling level stops counting: silence.
const FLOOR: f32 = 1e-4;

/// What a level is multiplied by each frame to fall to a thousandth in `seconds`.
fn fall(seconds: f32, sample_rate: f32) -> f32 {
    (1e-3f32).powf(1.0 / (seconds.max(1e-4) * sample_rate))
}

impl Adsr {
    pub fn new() -> Adsr {
        Adsr {
            stage: Stage::Idle,
            level: 0.0,
            attack_step: 1.0,
            decay_coeff: 0.0,
            sustain: 1.0,
            release_coeff: 0.0,
        }
    }

    /// Times in seconds, sustain from 0 to 1.
    pub fn set(&mut self, sample_rate: f32, attack: f32, decay: f32, sustain: f32, release: f32) {
        self.attack_step = 1.0 / (attack.max(1e-4) * sample_rate);
        self.decay_coeff = fall(decay, sample_rate);
        self.sustain = sustain.clamp(0.0, 1.0);
        self.release_coeff = fall(release, sample_rate);
    }

    pub fn start(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn release(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn next(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += self.attack_step;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coeff;
                if self.level - self.sustain < FLOOR {
                    self.level = self.sustain;
                    self.stage = if self.sustain < FLOOR { Stage::Idle } else { Stage::Sustain };
                }
            }
            Stage::Sustain => self.level = self.sustain,
            Stage::Release => {
                self.level *= self.release_coeff;
                if self.level < FLOOR {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }

    pub fn idle(&self) -> bool {
        self.stage == Stage::Idle
    }
}

/// The correction that takes the click out of a wave's jump at `t`, a phase stepping by `dt`.
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave {
    Saw,
    Square,
    Triangle,
    Sine,
}

impl Wave {
    pub fn from_index(index: f64) -> Wave {
        match index.round() as i32 {
            1 => Wave::Square,
            2 => Wave::Triangle,
            3 => Wave::Sine,
            _ => Wave::Saw,
        }
    }

    /// The wave at `phase` (0 to 1), stepping by `dt` a frame: band-limited, no aliasing heard.
    pub fn at(self, phase: f32, dt: f32) -> f32 {
        match self {
            Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
            Wave::Square => {
                let naive = if phase < 0.5 { 1.0 } else { -1.0 };
                naive + poly_blep(phase, dt) - poly_blep((phase + 0.5) % 1.0, dt)
            }
            Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
            Wave::Sine => (phase * TAU).sin(),
        }
    }
}

/// A state-variable filter (Zavalishin's, trapezoidal): steady when its cutoff moves fast.
#[derive(Clone, Copy, Default)]
pub struct Svf {
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: f32,
    ic2: f32,
}

pub struct SvfOut {
    pub low: f32,
    pub band: f32,
    pub high: f32,
}

impl Svf {
    /// `resonance` from 0 (none) to 1 (just short of ringing on its own).
    pub fn set(&mut self, cutoff: f32, resonance: f32, sample_rate: f32) {
        let cutoff = cutoff.clamp(10.0, sample_rate * 0.49);
        self.g = (PI * cutoff / sample_rate).tan();
        self.k = 2.0 - 1.98 * resonance.clamp(0.0, 1.0);
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    pub fn process(&mut self, input: f32) -> SvfOut {
        let v3 = input - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut { low: v2, band: v1, high: input - self.k * v1 - v2 }
    }

    pub fn clear(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
}

/// A biquad from the Audio EQ Cookbook (Robert Bristow-Johnson's).
#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

#[derive(Clone, Copy)]
pub enum Shape {
    Peak,
    LowShelf,
    HighShelf,
}

impl Biquad {
    pub fn flat() -> Biquad {
        Biquad { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z1: 0.0, z2: 0.0 }
    }

    pub fn set(&mut self, shape: Shape, frequency: f32, gain_db: f32, q: f32, sample_rate: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w = TAU * frequency.clamp(10.0, sample_rate * 0.49) / sample_rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q.max(0.05));
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::Peak => {
                (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
            }
            Shape::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - s),
                    (a + 1.0) + (a - 1.0) * cos + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - s,
                )
            }
            Shape::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - s),
                    (a + 1.0) - (a - 1.0) * cos + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - s,
                )
            }
        };
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    pub fn clear(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

/// White noise, from -1 to 1: a few operations a sample, the same sequence from the same seed.
#[derive(Clone, Copy)]
pub struct Noise(u32);

impl Noise {
    pub fn new(seed: u32) -> Noise {
        Noise(seed.max(1))
    }

    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// The frequency of MIDI key `key`, A4 (69) at 440 Hz.
pub fn key_frequency(key: f32) -> f32 {
    440.0 * 2f32.powf((key - 69.0) / 12.0)
}

/// A soft clip: straight near zero, rounding off towards ±1.
pub fn soft_clip(x: f32) -> f32 {
    if x.abs() < 3.0 {
        x * (27.0 + x * x) / (27.0 + 9.0 * x * x)
    } else {
        x.signum()
    }
}
