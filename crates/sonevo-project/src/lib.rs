//! A song as Sonevo holds it: its channels and their instruments, its patterns of notes, the
//! playlist that arranges them, the mixer they play through; and the open folder it is
//! saved as, plain files written the same way every time, so that a change shows as a
//! small diff.

mod device;
mod edit;
mod format;
mod id;
mod song;
mod time;

pub use device::{builtin, Device, DeviceKind};
pub use edit::{Edit, EditError, History};
pub use format::{import_sample, load, save, FormatError, FORMAT, SONG_FILE};
pub use id::Id;
pub use song::{
    Channel, Clip, ClipContent, Color, Insert, Note, Pattern, Point, Region, Send, Signature, Song, Target,
    Track,
};
pub use time::{Ticks, PPQ};
