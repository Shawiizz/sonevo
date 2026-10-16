//! Every change the interface makes to a song, one of a few kinds: a part put back as it now
//! is, a part taken out, a setting set. Each is applied whole, the song mended after it, and
//! undone by going back to the song as it was before.

use crate::device::Device;
use crate::id::Id;
use crate::song::{Channel, Clip, ClipContent, Insert, Note, Pattern, Region, Signature, Song, Track};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Edit {
    SetSong {
        name: String,
        tempo: f64,
        signature: Signature,
        loop_region: Option<Region>,
    },
    /// A channel put back as it now is, or added; at `index` if given.
    PutChannel {
        channel: Channel,
        index: Option<usize>,
    },
    RemoveChannel {
        id: Id,
    },
    /// A pattern's name, colour and length; its notes are kept.
    PutPattern {
        pattern: Pattern,
        index: Option<usize>,
    },
    RemovePattern {
        id: Id,
    },
    /// A channel's notes in a pattern, all of them.
    SetNotes {
        pattern: Id,
        channel: Id,
        notes: Vec<Note>,
    },
    PutTrack {
        track: Track,
        index: Option<usize>,
    },
    /// A track and its clips taken out.
    RemoveTrack {
        id: Id,
    },
    PutClip {
        clip: Clip,
    },
    RemoveClips {
        ids: Vec<Id>,
    },
    PutInsert {
        insert: Insert,
        index: Option<usize>,
    },
    RemoveInsert {
        id: Id,
    },
    SetMaster {
        insert: Insert,
    },
    SetParam {
        device: Id,
        param: String,
        value: f64,
    },
    /// Several edits undone as one: clips moved together.
    Batch {
        edits: Vec<Edit>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("nothing in the song has the id {0}")]
    Missing(Id),
    #[error("a song keeps at least one {0}")]
    Last(&'static str),
}

/// Puts `value` back in place of the one of the same id, or adds it at `index`.
fn put<T>(items: &mut Vec<T>, value: T, index: Option<usize>, id: impl Fn(&T) -> Id) {
    let at = items.iter().position(|i| id(i) == id(&value));
    match (at, index) {
        (Some(at), Some(index)) if at != index => {
            items.remove(at);
            items.insert(index.min(items.len()), value);
        }
        (Some(at), _) => items[at] = value,
        (None, Some(index)) => items.insert(index.min(items.len()), value),
        (None, None) => items.push(value),
    }
}

fn remove<T>(items: &mut Vec<T>, id: Id, of: impl Fn(&T) -> Id) -> Result<(), EditError> {
    let before = items.len();
    items.retain(|i| of(i) != id);
    if items.len() == before {
        return Err(EditError::Missing(id));
    }
    Ok(())
}

impl Song {
    /// The device of id `id`: an instrument, or an effect of the mixer.
    pub fn device_mut(&mut self, id: Id) -> Option<&mut Device> {
        let instruments = self.channels.iter_mut().map(|c| &mut c.instrument);
        let effects = self.inserts.iter_mut().chain([&mut self.master]).flat_map(|i| i.effects.iter_mut());
        instruments.chain(effects).find(|d| d.id == id)
    }

    /// Applies `edit`, then mends what it left pointing nowhere.
    pub fn apply(&mut self, edit: Edit) -> Result<(), EditError> {
        self.apply_one(edit)?;
        self.mend();
        Ok(())
    }

    fn apply_one(&mut self, edit: Edit) -> Result<(), EditError> {
        match edit {
            Edit::SetSong { name, tempo, signature, loop_region } => {
                self.name = name;
                self.tempo = tempo;
                self.signature =
                    Signature { beats: signature.beats.clamp(1, 32), unit: signature.unit.clamp(1, 32) };
                self.loop_region = loop_region.filter(|r| r.end > r.start);
            }
            Edit::PutChannel { channel, index } => put(&mut self.channels, channel, index, |c| c.id),
            Edit::RemoveChannel { id } => remove(&mut self.channels, id, |c| c.id)?,
            Edit::PutPattern { mut pattern, index } => {
                if let Some(old) = self.patterns.iter_mut().find(|p| p.id == pattern.id) {
                    pattern.notes = std::mem::take(&mut old.notes);
                }
                put(&mut self.patterns, pattern, index, |p| p.id);
            }
            Edit::RemovePattern { id } => {
                if self.patterns.len() == 1 {
                    return Err(EditError::Last("pattern"));
                }
                remove(&mut self.patterns, id, |p| p.id)?;
            }
            Edit::SetNotes { pattern, channel, mut notes } => {
                if self.channel(channel).is_none() {
                    return Err(EditError::Missing(channel));
                }
                let pattern =
                    self.patterns.iter_mut().find(|p| p.id == pattern).ok_or(EditError::Missing(pattern))?;
                notes.iter_mut().for_each(|n| n.velocity = n.velocity.clamp(0.0, 1.0));
                if notes.is_empty() {
                    pattern.notes.remove(&channel);
                } else {
                    pattern.notes.insert(channel, notes);
                }
            }
            Edit::PutTrack { track, index } => put(&mut self.tracks, track, index, |t| t.id),
            Edit::RemoveTrack { id } => {
                if self.tracks.len() == 1 {
                    return Err(EditError::Last("track"));
                }
                remove(&mut self.tracks, id, |t| t.id)?;
                self.clips.retain(|c| c.track != id);
            }
            Edit::PutClip { mut clip } => {
                clip.start = clip.start.max(0);
                clip.offset = clip.offset.max(0);
                if let ClipContent::Automation { points, .. } = &mut clip.content {
                    points.sort_by_key(|p| p.at);
                }
                put(&mut self.clips, clip, None, |c| c.id);
            }
            Edit::RemoveClips { ids } => self.clips.retain(|c| !ids.contains(&c.id)),
            Edit::PutInsert { insert, index } => {
                if insert.id == self.master.id {
                    self.master = insert;
                } else {
                    put(&mut self.inserts, insert, index, |i| i.id);
                }
            }
            Edit::RemoveInsert { id } => remove(&mut self.inserts, id, |i| i.id)?,
            Edit::SetMaster { mut insert } => {
                insert.id = self.master.id;
                insert.output = None;
                insert.sends.clear();
                self.master = insert;
            }
            Edit::SetParam { device, param, value } => {
                let device = self.device_mut(device).ok_or(EditError::Missing(device))?;
                device.params.insert(param, value);
            }
            Edit::Batch { edits } => {
                // all of them or none
                let mut after = self.clone();
                for edit in edits {
                    after.apply_one(edit)?;
                }
                *self = after;
            }
        }
        Ok(())
    }
}

impl Edit {
    /// Edits of the same key made close together are undone as one: a knob turned, a fader
    /// moved.
    fn key(&self) -> Option<String> {
        match self {
            Edit::SetParam { device, param, .. } => Some(format!("param {device} {param}")),
            Edit::PutChannel { channel, .. } => Some(format!("channel {}", channel.id)),
            Edit::PutInsert { insert, .. } => Some(format!("insert {}", insert.id)),
            Edit::SetMaster { .. } => Some("master".into()),
            Edit::SetSong { .. } => Some("song".into()),
            _ => None,
        }
    }
}

/// How close together edits of the same key are to be undone as one.
const TOGETHER: Duration = Duration::from_millis(1500);
/// How many steps back undo goes.
const STEPS: usize = 200;

/// The songs as they were before each edit, and after each undo.
#[derive(Default)]
pub struct History {
    undo: Vec<Song>,
    redo: Vec<Song>,
    last: Option<(String, Instant)>,
}

impl History {
    /// Applies `edit` to `song`, remembering the song as it was.
    pub fn apply(&mut self, song: &mut Song, edit: Edit) -> Result<(), EditError> {
        let key = edit.key();
        let before = song.clone();
        song.apply(edit)?;
        if *song == before {
            return Ok(());
        }
        let now = Instant::now();
        let together = matches!((&self.last, &key), (Some((last, at)), Some(key)) if last == key && now - *at < TOGETHER);
        if !together {
            self.undo.push(before);
            if self.undo.len() > STEPS {
                self.undo.remove(0);
            }
        }
        self.last = key.map(|k| (k, now));
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self, song: &mut Song) -> bool {
        let Some(before) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(song, before));
        self.last = None;
        true
    }

    pub fn redo(&mut self, song: &mut Song) -> bool {
        let Some(after) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(song, after));
        self.last = None;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        *self = History::default();
    }
}
