//! Plucked strings, as Karplus and Strong made them: a burst of noise going round a loop as
//! long as one period of the note, softened a little each time round, rings as a string does.
//! Sixteen voices.

use crate::param::Param;
use crate::util::{db_to_gain, key_frequency};
use crate::{Context, Event, Processor};

pub const PARAMS: &[Param] = &[
    Param::linear("tone", "Pluck brightness", 0.0, 1.0, 0.6, ""),
    Param::log("decay", "Ring", 0.1, 20.0, 3.0, "s"),
    Param::linear("damping", "Softening", 0.0, 1.0, 0.5, ""),
    Param::log("release", "Release", 0.01, 2.0, 0.15, "s"),
    Param::linear("volume", "Volume", -36.0, 6.0, -6.0, "dB"),
];

const VOICES: usize = 16;
/// The longest loop, a period of the lowest note played and a little more: 20 Hz at 192 kHz.
const LOOP: usize = 9604;

#[derive(Clone)]
struct Voice {
    key: u8,
    on: bool,
    held: bool,
    /// The loop, where it is written, and how far back it is read, in samples.
    line: Box<[f32]>,
    write: usize,
    length: f32,
    /// The softening's last output, and what is kept of the sound each time round.
    last: f32,
    keep: f32,
    /// Frames played almost silent in a row: as many as the loop is long, and it is over.
    quiet: usize,
    age: u32,
}

impl Voice {
    fn new() -> Voice {
        Voice {
            key: 0,
            on: false,
            held: false,
            line: vec![0.0; LOOP].into_boxed_slice(),
            write: 0,
            length: 1.0,
            last: 0.0,
            keep: 0.0,
            quiet: 0,
            age: 0,
        }
    }
}

/// The next of a noise from -1 to 1, `seed` its state.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed as f32 / u32::MAX as f32 * 2.0 - 1.0
}

/// How much the softening takes of the newer sample: half and half is the softest.
fn lean(damping: f32) -> f32 {
    0.5 + 0.5 * (1.0 - damping)
}

pub struct Strings {
    sample_rate: f32,
    values: [f32; 5],
    voices: Vec<Voice>,
    gain: f32,
    /// The noise's state: the same pluck each time for the same key and age.
    seed: u32,
}

impl Strings {
    pub fn new(sample_rate: f32) -> Strings {
        let mut values = [0.0; 5];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default as f32;
        }
        Strings {
            sample_rate,
            values,
            voices: (0..VOICES).map(|_| Voice::new()).collect(),
            gain: db_to_gain(values[4]),
            seed: 0x9E37_79B9,
        }
    }

    /// What is kept each time round so that the sound falls to a thousandth in `seconds`.
    fn keep(frequency: f32, seconds: f32) -> f32 {
        0.001f32.powf(1.0 / (seconds * frequency).max(1.0))
    }

    fn press(&mut self, key: u8, velocity: f32) {
        for voice in &mut self.voices {
            voice.age = voice.age.saturating_add(1);
        }
        let at = self
            .voices
            .iter()
            .position(|v| v.on && v.key == key)
            .or_else(|| self.voices.iter().position(|v| !v.on))
            .unwrap_or_else(|| (0..VOICES).max_by_key(|&i| self.voices[i].age).unwrap_or(0));
        let frequency = key_frequency(f32::from(key));
        // the softening makes the loop longer, by as much as it leans on the older sample
        let length =
            (self.sample_rate / frequency - (1.0 - lean(self.values[2]))).clamp(2.0, (LOOP - 4) as f32);
        let keep = Strings::keep(frequency, self.values[1]);
        // the pluck: noise, darker the lower its setting, written straight into the loop
        let bright = 0.05 + 0.95 * self.values[0];
        let mut smooth = 0.0;
        let span = (length.ceil() as usize + 2).min(LOOP - 1);
        let voice = &mut self.voices[at];
        for sample in voice.line.iter_mut().take(span) {
            smooth += (noise(&mut self.seed) - smooth) * bright;
            *sample = smooth * velocity.clamp(0.0, 1.0);
        }
        voice.line[span..].fill(0.0);
        voice.write = span % LOOP;
        voice.length = length;
        voice.keep = keep;
        voice.last = 0.0;
        voice.quiet = 0;
        voice.key = key;
        voice.on = true;
        voice.held = true;
        voice.age = 0;
    }

    fn release(&mut self, key: u8) {
        let (rate, release) = (self.sample_rate, self.values[3]);
        for voice in self.voices.iter_mut().filter(|v| v.on && v.held && v.key == key) {
            voice.held = false;
            let frequency = rate / voice.length;
            voice.keep = voice.keep.min(Strings::keep(frequency, release));
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let (lean, gain) = (lean(self.values[2]), self.gain);
        for voice in self.voices.iter_mut().filter(|v| v.on) {
            let size = voice.line.len();
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                let back = voice.write as f32 + size as f32 - voice.length;
                let i = back as usize;
                let part = back - i as f32;
                let a = voice.line[i % size];
                let b = voice.line[(i + 1) % size];
                let out = a + (b - a) * part;
                let y = (lean * out + (1.0 - lean) * voice.last) * voice.keep;
                voice.last = out;
                voice.line[voice.write] = y;
                voice.write = (voice.write + 1) % size;
                *l += y * gain;
                *r += y * gain;
                voice.quiet = if y.abs() < 1e-5 { voice.quiet + 1 } else { 0 };
                if voice.quiet > voice.length as usize + 2 {
                    voice.on = false;
                    break;
                }
            }
        }
    }
}

impl Processor for Strings {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value) as f32;
            self.gain = db_to_gain(self.values[4]);
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
        for voice in &mut self.voices {
            voice.on = false;
            voice.line.fill(0.0);
        }
    }
}
