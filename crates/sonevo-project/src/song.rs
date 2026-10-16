//! The song's parts.

use crate::arp::Arp;
use crate::device::{builtin, Device};
use crate::id::Id;
use crate::time::{Ticks, PPQ};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A colour as the interface shows it: `#rrggbb`.
pub type Color = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    /// Beats to a bar.
    pub beats: u8,
    /// The note that is a beat: 4 for a quarter note.
    pub unit: u8,
}

impl Signature {
    pub fn bar(&self) -> Ticks {
        Ticks::from(self.beats) * PPQ * 4 / Ticks::from(self.unit.max(1))
    }
}

/// What plays: an instrument, the notes the patterns give it, and the mixer track it goes to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: Id,
    pub name: String,
    pub color: Color,
    pub instrument: Device,
    /// From 0 to 1; 0.8 is where it starts.
    pub volume: f32,
    /// From -1 (left) to 1 (right).
    pub pan: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
    /// The mixer track it plays into; none is the master.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insert: Option<Id>,
    /// Its notes played one after the other instead of together, when on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arp: Option<Arp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub start: Ticks,
    pub length: Ticks,
    /// MIDI's numbering: 60 is middle C.
    pub key: u8,
    /// From 0 to 1.
    pub velocity: f32,
}

/// Notes for some of the channels, a few bars long, placed in the playlist as often as wanted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    pub id: Id,
    pub name: String,
    pub color: Color,
    pub length: Ticks,
    /// From 0 to 1: how late every second sixteenth plays, up to half a sixteenth.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub swing: f32,
    /// Each channel's notes, in time order. Saved in a file of the pattern's own.
    #[serde(skip)]
    pub notes: BTreeMap<Id, Vec<Note>>,
}

impl Pattern {
    /// Its notes for `channel`, in time order.
    pub fn notes_of(&self, channel: Id) -> &[Note] {
        self.notes.get(&channel).map_or(&[], Vec::as_slice)
    }
}

/// A lane of the playlist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: Id,
    pub name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
}

/// What an automation clip moves.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Target {
    ChannelVolume { channel: Id },
    ChannelPan { channel: Id },
    InsertVolume { insert: Id },
    InsertPan { insert: Id },
    Param { device: Id, param: String },
    Tempo,
}

/// A point of an automation clip: from its start, a value from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub at: Ticks,
    pub value: f64,
    /// How the line bends on to the next point: 0 is straight, -1 and 1 the most curved.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub curve: f32,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClipContent {
    Pattern {
        pattern: Id,
    },
    /// A sample as the song's folder holds it, played from the clip's offset.
    Audio {
        sample: String,
        gain: f32,
    },
    Automation {
        target: Target,
        points: Vec<Point>,
    },
}

/// Something placed in the playlist: a pattern, a sample or an automation, on a track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: Id,
    pub track: Id,
    pub start: Ticks,
    pub length: Ticks,
    /// Where in its content it starts: a pattern cut from its second bar starts a bar in.
    #[serde(default, skip_serializing_if = "is_zero_ticks")]
    pub offset: Ticks,
    pub content: ClipContent,
}

fn is_zero_ticks(value: &Ticks) -> bool {
    *value == 0
}

/// A send from one mixer track to another, as loud as `amount`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Send {
    pub to: Id,
    pub amount: f32,
}

/// A mixer track: the effects the sound goes through, its level, where it goes on to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Insert {
    pub id: Id,
    pub name: String,
    pub color: Color,
    /// From 0 to 1.25 (+2 dB); 1 leaves the sound as it is.
    pub volume: f32,
    pub pan: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solo: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Device>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sends: Vec<Send>,
    /// The track it goes on to; none is the master (or, for the master, the speakers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Id>,
}

impl Insert {
    pub fn new(name: &str, color: &str) -> Insert {
        Insert {
            id: Id::new(),
            name: name.to_owned(),
            color: color.to_owned(),
            volume: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            effects: Vec::new(),
            sends: Vec::new(),
            output: None,
        }
    }
}

/// An arrangement kept beside the one played: the same patterns and sounds, placed otherwise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Arrangement {
    pub name: String,
    pub clips: Vec<Clip>,
}

fn first_arrangement() -> String {
    "Arrangement 1".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub start: Ticks,
    pub end: Ticks,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub name: String,
    /// Beats a minute.
    pub tempo: f64,
    pub signature: Signature,
    pub channels: Vec<Channel>,
    pub patterns: Vec<Pattern>,
    pub tracks: Vec<Track>,
    /// The clips of the arrangement played, named `arrangement`.
    pub clips: Vec<Clip>,
    #[serde(default = "first_arrangement")]
    pub arrangement: String,
    /// The other arrangements, to be played in its place.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrangements: Vec<Arrangement>,
    pub inserts: Vec<Insert>,
    pub master: Insert,
    /// The part played over and over, when looping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_region: Option<Region>,
}

/// The colours channels, patterns and mixer tracks take in turn.
pub const COLORS: [&str; 8] =
    ["#5ab0f0", "#f2b84b", "#8f8bf7", "#ef6f6c", "#5fd3a8", "#e88ad6", "#b4dc5a", "#4fd0d6"];

impl Song {
    /// A song to start from: four drums in the rack, one pattern, a playlist of a few tracks.
    pub fn new(name: &str) -> Song {
        let drum = |name: &str, sound: f64, color: usize| Channel {
            id: Id::new(),
            name: name.to_owned(),
            color: COLORS[color].to_owned(),
            instrument: Device::builtin(builtin::DRUMS).with("sound", sound),
            volume: 0.8,
            pan: 0.0,
            mute: false,
            insert: None,
            arp: None,
        };
        let channels =
            vec![drum("Kick", 0.0, 0), drum("Clap", 3.0, 1), drum("Hat", 2.0, 2), drum("Snare", 1.0, 3)];
        let signature = Signature { beats: 4, unit: 4 };
        let pattern = Pattern {
            id: Id::new(),
            name: "Pattern 1".into(),
            color: COLORS[0].to_owned(),
            length: signature.bar(),
            swing: 0.0,
            notes: BTreeMap::new(),
        };
        let tracks =
            (1..=8).map(|i| Track { id: Id::new(), name: format!("Track {i}"), mute: false }).collect();
        Song {
            name: name.to_owned(),
            tempo: 130.0,
            signature,
            channels,
            patterns: vec![pattern],
            tracks,
            clips: Vec::new(),
            arrangement: first_arrangement(),
            arrangements: Vec::new(),
            inserts: Vec::new(),
            master: Insert::new("Master", "#bcc3be"),
            loop_region: None,
        }
    }

    pub fn channel(&self, id: Id) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn pattern(&self, id: Id) -> Option<&Pattern> {
        self.patterns.iter().find(|p| p.id == id)
    }

    pub fn insert(&self, id: Id) -> Option<&Insert> {
        self.inserts.iter().find(|i| i.id == id).or((self.master.id == id).then_some(&self.master))
    }

    /// Where the song ends: the end of its last clip.
    pub fn length(&self) -> Ticks {
        self.clips.iter().map(|c| c.start + c.length).max().unwrap_or(0)
    }

    /// Mends what points nowhere, as a song edited by hand or by an older Sonevo may: notes
    /// of channels gone, clips of tracks or patterns gone, routes to mixer tracks gone. What
    /// was mended, said plainly.
    pub fn mend(&mut self) -> Vec<String> {
        let mut mended = Vec::new();
        let channels: Vec<Id> = self.channels.iter().map(|c| c.id).collect();
        for pattern in &mut self.patterns {
            let before = pattern.notes.len();
            pattern.notes.retain(|channel, _| channels.contains(channel));
            if pattern.notes.len() != before {
                mended.push(format!("notes of channels gone left out of {}", pattern.name));
            }
            for notes in pattern.notes.values_mut() {
                notes.retain(|n| n.length > 0 && n.key <= 127);
                notes.sort_by_key(|n| (n.start, n.key));
            }
            pattern.length = pattern.length.max(PPQ);
        }
        let tracks: Vec<Id> = self.tracks.iter().map(|t| t.id).collect();
        let patterns: Vec<Id> = self.patterns.iter().map(|p| p.id).collect();
        let fits = |clip: &Clip| {
            let content = match &clip.content {
                ClipContent::Pattern { pattern } => patterns.contains(pattern),
                _ => true,
            };
            content && tracks.contains(&clip.track) && clip.length > 0
        };
        let before = self.clips.len() + self.arrangements.iter().map(|a| a.clips.len()).sum::<usize>();
        self.clips.retain(fits);
        for arrangement in &mut self.arrangements {
            arrangement.clips.retain(fits);
        }
        let after = self.clips.len() + self.arrangements.iter().map(|a| a.clips.len()).sum::<usize>();
        if after != before {
            mended.push(format!("{} clips of tracks or patterns gone left out", before - after));
        }
        let inserts: Vec<Id> = self.inserts.iter().map(|i| i.id).collect();
        for channel in &mut self.channels {
            if channel.insert.is_some_and(|i| !inserts.contains(&i)) {
                channel.insert = None;
                mended.push(format!("{} goes to the master: its mixer track is gone", channel.name));
            }
        }
        for insert in &mut self.inserts {
            if insert.output.is_some_and(|o| !inserts.contains(&o) || o == insert.id) {
                insert.output = None;
            }
            let own = insert.id;
            insert.sends.retain(|s| inserts.contains(&s.to) && s.to != own);
        }
        self.tempo = self.tempo.clamp(10.0, 999.0);
        mended
    }
}
