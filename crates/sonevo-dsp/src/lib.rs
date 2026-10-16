//! Sonevo's own instruments and effects, each a [`Processor`]: settings described by
//! [`Param`]s and set by index, blocks played without allocating. An instrument writes its
//! sound; an effect changes the sound in place.

mod drums;
pub mod effects;
mod fm;
mod param;
mod presets;
mod sampler;
mod stretch;
mod synth;
mod util;

pub use param::{Param, Scale};
pub use presets::{presets, Preset, PRESETS};
pub use sampler::Sample;
pub use stretch::stretch;

use std::sync::Arc;

/// A note starting or ending on a frame of the block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Event {
    pub at: u32,
    pub key: u8,
    /// From 0 to 1.
    pub velocity: f32,
    pub on: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Context<'a> {
    pub sample_rate: f32,
    /// Beats a minute.
    pub tempo: f64,
    /// For an effect listening to another mixer track (a sidechain): that track's sound, as
    /// long as the block.
    pub key: Option<(&'a [f32], &'a [f32])>,
}

pub trait Processor: Send {
    /// Sets the setting `index` of [`Builtin::params`], in its own units.
    fn set(&mut self, index: usize, value: f64);

    /// Plays a block: `events` in time order, `left` and `right` as long as each other.
    fn process(&mut self, context: &Context, events: &[Event], left: &mut [f32], right: &mut [f32]);

    /// Silence: every voice ended, every tail cut.
    fn reset(&mut self);

    /// Gives the sampler its sound: the one it had back, to be dropped off the audio thread.
    fn set_sample(&mut self, sample: Option<Arc<Sample>>) -> Option<Arc<Sample>> {
        sample
    }
}

pub struct Builtin {
    /// The name songs know it by (`sonevo.synth`).
    pub name: &'static str,
    /// Its name as shown.
    pub label: &'static str,
    pub instrument: bool,
    pub params: &'static [Param],
    make: fn(sample_rate: f32) -> Box<dyn Processor>,
}

impl Builtin {
    /// A new one, at its default settings.
    pub fn make(&self, sample_rate: f32) -> Box<dyn Processor> {
        let mut made = (self.make)(sample_rate);
        for (i, param) in self.params.iter().enumerate() {
            made.set(i, param.default);
        }
        made
    }

    pub fn param(&self, id: &str) -> Option<usize> {
        self.params.iter().position(|p| p.id == id)
    }
}

pub const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "sonevo.synth",
        label: "Synth",
        instrument: true,
        params: synth::PARAMS,
        make: |rate| Box::new(synth::Synth::new(rate)),
    },
    Builtin {
        name: "sonevo.drums",
        label: "Drums",
        instrument: true,
        params: drums::PARAMS,
        make: |rate| Box::new(drums::Drums::new(rate)),
    },
    Builtin {
        name: "sonevo.sampler",
        label: "Sampler",
        instrument: true,
        params: sampler::PARAMS,
        make: |rate| Box::new(sampler::Sampler::new(rate)),
    },
    Builtin {
        name: "sonevo.fm",
        label: "FM synth",
        instrument: true,
        params: fm::PARAMS,
        make: |rate| Box::new(fm::Fm::new(rate)),
    },
    Builtin {
        name: "sonevo.eq",
        label: "EQ",
        instrument: false,
        params: effects::eq::PARAMS,
        make: |rate| Box::new(effects::eq::Eq::new(rate)),
    },
    Builtin {
        name: "sonevo.compressor",
        label: "Compressor",
        instrument: false,
        params: effects::compressor::PARAMS,
        make: |rate| Box::new(effects::compressor::Compressor::new(rate)),
    },
    Builtin {
        name: "sonevo.delay",
        label: "Delay",
        instrument: false,
        params: effects::delay::PARAMS,
        make: |rate| Box::new(effects::delay::Delay::new(rate)),
    },
    Builtin {
        name: "sonevo.reverb",
        label: "Reverb",
        instrument: false,
        params: effects::reverb::PARAMS,
        make: |rate| Box::new(effects::reverb::Reverb::new(rate)),
    },
    Builtin {
        name: "sonevo.distortion",
        label: "Distortion",
        instrument: false,
        params: effects::distortion::PARAMS,
        make: |rate| Box::new(effects::distortion::Distortion::new(rate)),
    },
    Builtin {
        name: "sonevo.filter",
        label: "Filter",
        instrument: false,
        params: effects::filter::PARAMS,
        make: |rate| Box::new(effects::filter::Filter::new(rate)),
    },
    Builtin {
        name: "sonevo.limiter",
        label: "Limiter",
        instrument: false,
        params: effects::limiter::PARAMS,
        make: |rate| Box::new(effects::limiter::Limiter::new(rate)),
    },
    Builtin {
        name: "sonevo.chorus",
        label: "Chorus",
        instrument: false,
        params: effects::chorus::PARAMS,
        make: |rate| Box::new(effects::chorus::Chorus::new(rate)),
    },
    Builtin {
        name: "sonevo.phaser",
        label: "Phaser",
        instrument: false,
        params: effects::phaser::PARAMS,
        make: |rate| Box::new(effects::phaser::Phaser::new(rate)),
    },
];

pub fn builtin(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}
