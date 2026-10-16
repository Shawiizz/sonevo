//! What makes or shapes the sound: an instrument or an effect, built into Sonevo or a
//! plugin installed on the computer; its settings by name, so that any can be automated.

use crate::id::Id;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kinds of devices built into Sonevo.
pub mod builtin {
    pub const SYNTH: &str = "sonevo.synth";
    pub const DRUMS: &str = "sonevo.drums";
    pub const SAMPLER: &str = "sonevo.sampler";
    pub const EQ: &str = "sonevo.eq";
    pub const COMPRESSOR: &str = "sonevo.compressor";
    pub const DELAY: &str = "sonevo.delay";
    pub const REVERB: &str = "sonevo.reverb";
    pub const DISTORTION: &str = "sonevo.distortion";
    pub const FILTER: &str = "sonevo.filter";
    pub const LIMITER: &str = "sonevo.limiter";
    pub const CHORUS: &str = "sonevo.chorus";
    pub const PHASER: &str = "sonevo.phaser";
    pub const FM: &str = "sonevo.fm";
    pub const CRUSHER: &str = "sonevo.crusher";
    pub const AUTOPAN: &str = "sonevo.autopan";
    pub const STRINGS: &str = "sonevo.strings";

    pub const ALL: [&str; 16] = [
        SYNTH, DRUMS, SAMPLER, EQ, COMPRESSOR, DELAY, REVERB, DISTORTION, FILTER, LIMITER, CHORUS, PHASER,
        FM, CRUSHER, AUTOPAN, STRINGS,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DeviceKind {
    /// One of [`builtin`]'s.
    Builtin { name: String },
    /// A CLAP plugin: the bundle it is in, and its id within it.
    Clap { bundle: String, plugin: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Id,
    pub kind: DeviceKind,
    /// Its settings, by name: those left out are at their defaults.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f64>,
    /// Off, it lets the sound through as it is (an effect) or is silent (an instrument).
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub enabled: bool,
    /// The sample it plays, as the song's folder holds it (`samples/<name>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
    /// For an effect: the mixer track it listens to instead of the sound it changes, as a
    /// compressor ducking a bass under the kick (a sidechain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<Id>,
}

fn on() -> bool {
    true
}

fn is_on(enabled: &bool) -> bool {
    *enabled
}

impl Device {
    pub fn builtin(name: &str) -> Device {
        Device {
            id: Id::new(),
            kind: DeviceKind::Builtin { name: name.to_owned() },
            params: BTreeMap::new(),
            enabled: true,
            sample: None,
            key: None,
        }
    }

    /// The built-in kind it is, if it is one.
    pub fn builtin_name(&self) -> Option<&str> {
        match &self.kind {
            DeviceKind::Builtin { name } => Some(name),
            DeviceKind::Clap { .. } => None,
        }
    }

    pub fn with(mut self, param: &str, value: f64) -> Device {
        self.params.insert(param.to_owned(), value);
        self
    }
}
