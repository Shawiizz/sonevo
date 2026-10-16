//! A song as Sonevo holds it, and the folder it is saved as: plain files written the same way
//! every time, so that a change shows as a small diff.

mod arp;
mod device;
mod edit;
mod format;
mod id;
mod midi;
mod song;
mod time;

pub use arp::{Arp, ArpOrder};
pub use device::{builtin, Device, DeviceKind};
pub use edit::{Edit, EditError, History};
pub use format::{import_sample, load, save, FormatError, FORMAT, SONG_FILE};
pub use id::Id;
pub use midi::{read_midi, write_midi, Midi, MidiError, MidiTrack};
pub use song::{
    Arrangement, Channel, Clip, ClipContent, Color, Insert, Note, Pattern, Point, Region, Send, Signature,
    Song, Target, Track, COLORS,
};
pub use time::{Ticks, PPQ};
