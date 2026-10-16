//! An auto-pan: the sound moved from one side to the other and back, in time with the song.

use crate::param::Param;
use crate::{Context, Event, Processor};
use std::f64::consts::TAU;

const TIMES: &[&str] = &["1 bar", "1/2", "1/4", "1/8", "1/16"];
/// Each choice, in beats to a sway there and back.
const BEATS: [f64; 5] = [4.0, 2.0, 1.0, 0.5, 0.25];

pub const PARAMS: &[Param] =
    &[Param::choice("time", "Time", TIMES, 2), Param::linear("depth", "Depth", 0.0, 1.0, 0.7, "")];

pub struct AutoPan {
    sample_rate: f32,
    values: [f64; 2],
    /// Where the sway is, in turns.
    phase: f64,
}

impl AutoPan {
    pub fn new(sample_rate: f32) -> AutoPan {
        AutoPan { sample_rate, values: [PARAMS[0].default, PARAMS[1].default], phase: 0.0 }
    }
}

impl Processor for AutoPan {
    fn set(&mut self, index: usize, value: f64) {
        if let Some(p) = PARAMS.get(index) {
            self.values[index] = p.clamp(value);
        }
    }

    fn process(&mut self, context: &Context, _events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let beats = BEATS[(self.values[0].round() as usize).min(BEATS.len() - 1)];
        let step = context.tempo.max(1.0) / 60.0 / beats / f64::from(self.sample_rate);
        let depth = self.values[1];
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            // equal power: as loud in the middle as at either side
            let pan = (TAU * self.phase).sin() * depth;
            let angle = (pan + 1.0) * std::f64::consts::FRAC_PI_4;
            let (to_left, to_right) =
                (angle.cos() * std::f64::consts::SQRT_2, angle.sin() * std::f64::consts::SQRT_2);
            *l *= to_left as f32;
            *r *= to_right as f32;
            self.phase = (self.phase + step).fract();
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }
}
