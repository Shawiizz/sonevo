//! The synth's voices: a sine and its octave, softly bright on the strike and mellowing as
//! it dies away, an envelope that clicks neither in nor out.

use std::f32::consts::TAU;

const VOICES: usize = 16;
/// How loud a voice is at full velocity: sixteen at once stay under full scale.
const GAIN: f32 = 0.18;
const ATTACK_SECONDS: f32 = 0.004;
/// How long a held note takes to fall to a tenth.
const DECAY_SECONDS: f32 = 1.6;
const RELEASE_SECONDS: f32 = 0.25;

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Idle,
    Attack,
    Held,
    Released,
}

#[derive(Clone, Copy)]
struct Voice {
    stage: Stage,
    key: i16,
    velocity: f32,
    /// Where the wave is, from 0 to 1.
    phase: f32,
    step: f32,
    level: f32,
    /// How bright the voice still is: its octave fades faster than it does.
    bright: f32,
    /// Notes played since: the oldest voice is the one taken when all are busy.
    age: u32,
}

const IDLE: Voice = Voice {
    stage: Stage::Idle,
    key: 0,
    velocity: 0.0,
    phase: 0.0,
    step: 0.0,
    level: 0.0,
    bright: 0.0,
    age: 0,
};

pub struct Voices {
    voices: [Voice; VOICES],
    attack: f32,
    /// Per frame, what the level is multiplied by while held, and once released.
    decay: f32,
    release: f32,
    sample_rate: f32,
}

impl Voices {
    pub fn new(sample_rate: f32) -> Voices {
        let fall = |seconds: f32| (0.1f32).powf(1.0 / (seconds * sample_rate));
        Voices {
            voices: [IDLE; VOICES],
            attack: 1.0 / (ATTACK_SECONDS * sample_rate),
            decay: fall(DECAY_SECONDS),
            release: fall(RELEASE_SECONDS / 3.0),
            sample_rate,
        }
    }

    pub fn silence(&mut self) {
        self.voices = [IDLE; VOICES];
    }

    /// Starts `key`, in a free voice or the oldest one.
    pub fn press(&mut self, key: i16, velocity: f32) {
        for voice in &mut self.voices {
            voice.age = voice.age.saturating_add(1);
        }
        let free = self.voices.iter().position(|v| v.stage == Stage::Idle);
        let oldest = || (0..VOICES).max_by_key(|&i| self.voices[i].age).unwrap_or(0);
        let at = free.unwrap_or_else(oldest);
        let frequency = 440.0 * 2f32.powf((f32::from(key) - 69.0) / 12.0);
        self.voices[at] = Voice {
            stage: Stage::Attack,
            key,
            velocity: velocity.clamp(0.0, 1.0),
            phase: 0.0,
            step: frequency / self.sample_rate,
            level: 0.0,
            bright: 1.0,
            age: 0,
        };
    }

    pub fn release(&mut self, key: i16) {
        for voice in &mut self.voices {
            if voice.key == key && matches!(voice.stage, Stage::Attack | Stage::Held) {
                voice.stage = Stage::Released;
            }
        }
    }

    /// Adds every voice to `left`, and to `right` when there is one: silence first.
    pub fn render(&mut self, left: &mut [f32], mut right: Option<&mut [f32]>) {
        left.fill(0.0);
        if let Some(right) = right.as_deref_mut() {
            right.fill(0.0);
        }
        for voice in &mut self.voices {
            if voice.stage == Stage::Idle {
                continue;
            }
            for i in 0..left.len() {
                match voice.stage {
                    Stage::Attack => {
                        voice.level += self.attack;
                        if voice.level >= 1.0 {
                            voice.level = 1.0;
                            voice.stage = Stage::Held;
                        }
                    }
                    Stage::Held => voice.level *= self.decay,
                    Stage::Released => voice.level *= self.release,
                    Stage::Idle => break,
                }
                voice.bright *= self.decay * self.decay;
                if voice.level < 1e-4 && voice.stage != Stage::Attack {
                    voice.stage = Stage::Idle;
                    break;
                }
                let wave = (voice.phase * TAU).sin() + 0.35 * voice.bright * (voice.phase * 2.0 * TAU).sin();
                let sample = wave * voice.level * voice.velocity * GAIN;
                left[i] += sample;
                if let Some(right) = right.as_deref_mut() {
                    right[i] += sample;
                }
                voice.phase += voice.step;
                voice.phase -= voice.phase.floor();
            }
        }
    }
}
