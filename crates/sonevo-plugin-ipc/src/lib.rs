//! The engine and a plugin's process playing together, block by block, through shared memory
//! and a signal each way. A block late is silence, never a wait; blocks take two slots in turn
//! so that a plugin late with one never writes over the next.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{EngineEnd, PluginEnd};

pub use sonevo_clap::{Note, ParamChange, MAX_NOTES, MAX_PARAMS};

/// The frames a block holds at most.
pub const MAX_FRAMES: usize = 8192;
