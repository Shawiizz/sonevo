//! The engine and a plugin's own process playing together, block by block: the notes of a
//! block and the sound the plugin made of them in memory both see, a signal each way.
//!
//! The engine asks for block `n` and waits for it until its deadline; a block late is
//! silence, never a wait. Blocks take two slots in turn, so that a plugin late with one
//! never writes over the next. A plugin that falls behind skips to the last block asked.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{EngineEnd, PluginEnd};

pub use sonevo_clap::{Note, MAX_NOTES};

/// The frames a block holds at most.
pub const MAX_FRAMES: usize = 8192;
