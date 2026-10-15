//! A delay in time with the song: echoes a fraction of a beat apart, darker each time,
//! straight or from one side to the other.

use crate::param::Param;
use crate::util::Svf;
use crate::{Context, Event, Processor};

const TIMES: &[&str] =
    &["1/16", "1/8", "1/8 dotted", "1/4", "1/4 dotted", "1/2", "1/8 triplet", "1/4 triplet"];
/// Each choice, in beats.
const BEATS: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 1.0 / 3.0, 2.0 / 3.0];
const MODES: &[&str] = &["Stereo", "Ping-pong"];
/// The longest echo held: half a note at 30 beats a minute.
const MAX_SECONDS: f32 = 4.0;

pub const PARAMS: &[Param] = &[
    Param::choice("time", "Time", TIMES, 2),
    Param::linear("feedback", "Feedback", 0.0, 0.95, 0.4, ""),
    Param::linear("mix", "Mix", 0.0, 1.0, 0.25, ""),
    Param::choice("mode", "Mode", MODES, 0),
    Param::log("tone", "Tone", 500.0, 20000.0, 6000.0, "Hz"),
];

pub struct Delay {
    sample_rate: f32,
    values: [f64; 5],
    left: Vec<f32>,
    right: Vec<f32>,
    write: usize,
    filters: [Svf; 2],
}

impl Delay {
    pub fn new(sample_rate: f32) -> Delay {
        let length = (sample_rate * MAX_SECONDS) as usize + 1;
        let mut values = [0.0; 5];
        for (v, p) in values.iter_mut().zip(PARAMS) {
            *v = p.default;
        }
        let mut delay = Delay {
            sample_rate,
            values,
            left: vec![0.0; length],
            right: vec![0.0; length],
            write: 0,
            filters: [Svf::default(); 2],
        };
        delay.apply();
        delay
    }

    fn apply(&mut self) {
        for filter in &mut self.filters {
            filter.set(self.values[4] as f32, 0.0, self.sample_rate);
        }
    }
}

impl Processor for Delay {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value);
            self.apply();
        }
    }

    fn process(&mut self, context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let length = self.left.len();
        let beats = BEATS[(self.values[0].round() as usize).min(BEATS.len() - 1)];
        let delay = ((beats * 60.0 / context.tempo.max(1.0)) * f64::from(self.sample_rate)) as usize;
        let delay = delay.clamp(1, length - 1);
        let (feedback, mix) = (self.values[1] as f32, self.values[2] as f32);
        let ping_pong = self.values[3].round() as i32 == 1;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let read = (self.write + length - delay) % length;
            let (echo_l, echo_r) = (self.left[read], self.right[read]);
            let (in_l, in_r) = (*l, *r);
            let (back_l, back_r) = if ping_pong {
                // what comes in goes left first; each echo crosses to the other side
                ((in_l + in_r) * 0.5 + echo_r * feedback, echo_l * feedback)
            } else {
                (in_l + echo_l * feedback, in_r + echo_r * feedback)
            };
            self.left[self.write] = self.filters[0].process(back_l).low;
            self.right[self.write] = self.filters[1].process(back_r).low;
            self.write = (self.write + 1) % length;
            *l = in_l + echo_l * mix;
            *r = in_r + echo_r * mix;
        }
    }

    fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
        self.filters.iter_mut().for_each(Svf::clear);
    }
}
