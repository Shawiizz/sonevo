//! The link on Windows: a named file mapping, and two auto-reset events.

use crate::{Note, ParamChange, MAX_FRAMES, MAX_NOTES, MAX_PARAMS};
use std::cell::UnsafeCell;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, SYNCHRONIZATION_SYNCHRONIZE,
};

/// How long the engine looks at the memory before sleeping on the signal: a plugin often
/// answers within it, and waking from a wait costs about as much.
const SPIN: Duration = Duration::from_micros(30);

/// One block's place: its notes and settings, and the sound made of them.
#[repr(C)]
struct Slot {
    frames: AtomicU32,
    count: AtomicU32,
    notes: UnsafeCell<[Note; MAX_NOTES]>,
    changed: AtomicU32,
    changes: UnsafeCell<[ParamChange; MAX_PARAMS]>,
    left: UnsafeCell<[f32; MAX_FRAMES]>,
    right: UnsafeCell<[f32; MAX_FRAMES]>,
}

/// What both processes see.
#[repr(C)]
struct Shared {
    /// The last block the engine asked for, and the last one the plugin played: 0 is none.
    asked: AtomicU64,
    played: AtomicU64,
    /// The engine is closing: the plugin's process ends.
    stop: AtomicBool,
    slots: [Slot; 2],
}

fn slot(shared: &Shared, block: u64) -> &Slot {
    &shared.slots[(block % 2) as usize]
}

/// The mapping and the events, by their names, closed when dropped.
struct Mapping {
    file: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    ask: HANDLE,
    done: HANDLE,
}

// Each end is used by one thread at a time; what they share goes through atomics, and the
// slots alternate.
unsafe impl Send for Mapping {}

impl Mapping {
    fn shared(&self) -> &Shared {
        // SAFETY: the view is a `Shared` for as long as the mapping lives
        unsafe { &*(self.view.Value as *const Shared) }
    }

    fn make(name: &str, create: bool) -> io::Result<Mapping> {
        let wide = |suffix: &str| -> Vec<u16> {
            format!("Local\\{name}-{suffix}").encode_utf16().chain([0]).collect()
        };
        let size = std::mem::size_of::<Shared>();
        let checked = |handle: HANDLE| {
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                Err(io::Error::last_os_error())
            } else {
                Ok(handle)
            }
        };
        // SAFETY: plain Win32 calls, with names that live through them
        unsafe {
            let file = if create {
                let file = checked(CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    std::ptr::null(),
                    PAGE_READWRITE,
                    (size as u64 >> 32) as u32,
                    size as u32,
                    wide("memory").as_ptr(),
                ))?;
                // another engine's link by that name would mix two songs
                const ERROR_ALREADY_EXISTS: u32 = 183;
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    CloseHandle(file);
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{name} is taken")));
                }
                file
            } else {
                checked(OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide("memory").as_ptr()))?
            };
            let view = MapViewOfFile(file, FILE_MAP_ALL_ACCESS, 0, 0, size);
            if view.Value.is_null() {
                let e = io::Error::last_os_error();
                CloseHandle(file);
                return Err(e);
            }
            let event = |suffix: &str| {
                let name = wide(suffix);
                if create {
                    checked(CreateEventW(std::ptr::null(), 0, 0, name.as_ptr()))
                } else {
                    checked(OpenEventW(EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()))
                }
            };
            // the mapping is closed with what was made, should an event fail
            let mut mapping = Mapping { file, view, ask: std::ptr::null_mut(), done: std::ptr::null_mut() };
            mapping.ask = event("ask")?;
            mapping.done = event("done")?;
            Ok(mapping)
        }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: each handle is ours, closed once
        unsafe {
            for handle in [self.ask, self.done] {
                if !handle.is_null() {
                    CloseHandle(handle);
                }
            }
            UnmapViewOfFile(self.view);
            CloseHandle(self.file);
        }
    }
}

/// Waits on `event` for at most `timeout`, whole milliseconds as Windows counts them.
fn wait(event: HANDLE, timeout: Duration) -> bool {
    let ms = timeout.as_millis().clamp(1, u128::from(u32::MAX - 1)) as u32;
    // SAFETY: the event is open
    unsafe { WaitForSingleObject(event, ms) == WAIT_OBJECT_0 }
}

/// The engine's end: it asks for blocks and takes the sound of those played in time.
pub struct EngineEnd {
    mapping: Mapping,
    next: u64,
}

impl EngineEnd {
    pub fn create(name: &str) -> io::Result<EngineEnd> {
        Ok(EngineEnd { mapping: Mapping::make(name, true)?, next: 1 })
    }

    /// Asks for the next block, `frames` long with `notes` and the settings `changes` in it:
    /// its number.
    pub fn ask(&mut self, frames: usize, notes: &[Note], changes: &[ParamChange]) -> u64 {
        let block = self.next;
        self.next += 1;
        let shared = self.mapping.shared();
        let slot = slot(shared, block);
        let count = notes.len().min(MAX_NOTES);
        // SAFETY: the plugin writes this slot only for the block before last, long given up
        unsafe { (&mut *slot.notes.get())[..count].copy_from_slice(&notes[..count]) };
        slot.count.store(count as u32, Ordering::Relaxed);
        let changed = changes.len().min(MAX_PARAMS);
        // SAFETY: as the notes
        unsafe { (&mut *slot.changes.get())[..changed].copy_from_slice(&changes[..changed]) };
        slot.changed.store(changed as u32, Ordering::Relaxed);
        slot.frames.store(frames.min(MAX_FRAMES) as u32, Ordering::Relaxed);
        shared.asked.store(block, Ordering::Release);
        // SAFETY: the event is open
        unsafe { SetEvent(self.mapping.ask) };
        block
    }

    /// Waits for `block` for at most `timeout`, and copies its sound into `left` and
    /// `right`: false when it came too late, the two left as they were.
    pub fn take(&self, block: u64, timeout: Duration, left: &mut [f32], right: &mut [f32]) -> bool {
        let shared = self.mapping.shared();
        let start = Instant::now();
        let played = || shared.played.load(Ordering::Acquire) >= block;
        while !played() {
            let spent = start.elapsed();
            if spent >= timeout {
                return false;
            }
            if spent < SPIN {
                std::hint::spin_loop();
            } else {
                wait(self.mapping.done, timeout - spent);
            }
        }
        let slot = slot(shared, block);
        let frames = left.len().min(right.len()).min(MAX_FRAMES);
        // SAFETY: played, the slot is the engine's until it asks for the block after next
        unsafe {
            left[..frames].copy_from_slice(&(&*slot.left.get())[..frames]);
            right[..frames].copy_from_slice(&(&*slot.right.get())[..frames]);
        }
        true
    }
}

impl Drop for EngineEnd {
    fn drop(&mut self) {
        self.mapping.shared().stop.store(true, Ordering::Release);
        // SAFETY: the event is open
        unsafe { SetEvent(self.mapping.ask) };
    }
}

/// A block asked for, as the plugin's end reads it.
pub struct Asked<'a> {
    pub block: u64,
    pub notes: &'a [Note],
    pub changes: &'a [ParamChange],
    pub left: &'a mut [f32],
    pub right: &'a mut [f32],
}

/// The plugin's end: it plays the blocks asked for, the last one first.
pub struct PluginEnd {
    mapping: Mapping,
    last: u64,
}

impl PluginEnd {
    pub fn open(name: &str) -> io::Result<PluginEnd> {
        let mapping = Mapping::make(name, false)?;
        // a process started again goes on from where the engine is
        let last = mapping.shared().asked.load(Ordering::Acquire);
        Ok(PluginEnd { mapping, last })
    }

    pub fn stopping(&self) -> bool {
        self.mapping.shared().stop.load(Ordering::Acquire)
    }

    /// Waits for at most `timeout` for a block to play, and plays it with `play`: false when
    /// none came.
    pub fn serve(&mut self, timeout: Duration, play: impl FnOnce(Asked)) -> bool {
        let shared = self.mapping.shared();
        let mut block = shared.asked.load(Ordering::Acquire);
        if block <= self.last {
            wait(self.mapping.ask, timeout);
            block = shared.asked.load(Ordering::Acquire);
            if block <= self.last || self.stopping() {
                return false;
            }
        }
        self.last = block;
        let slot = slot(shared, block);
        let frames = slot.frames.load(Ordering::Relaxed) as usize;
        let count = slot.count.load(Ordering::Relaxed) as usize;
        let changed = slot.changed.load(Ordering::Relaxed) as usize;
        // SAFETY: the engine is done writing this slot until it asks for the block after next
        let asked = unsafe {
            Asked {
                block,
                notes: &(&*slot.notes.get())[..count],
                changes: &(&*slot.changes.get())[..changed],
                left: &mut (&mut *slot.left.get())[..frames],
                right: &mut (&mut *slot.right.get())[..frames],
            }
        };
        play(asked);
        shared.played.store(block, Ordering::Release);
        // SAFETY: the event is open
        unsafe { SetEvent(self.mapping.done) };
        true
    }
}
