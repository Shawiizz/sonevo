//! Musical time: ticks, so many to a quarter note, whatever the tempo.

/// A position or a length in a song.
pub type Ticks = i64;

/// Ticks to a quarter note: a sixteenth is 240, a triplet's eighth 320, both whole.
pub const PPQ: Ticks = 960;
