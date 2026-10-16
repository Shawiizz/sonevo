//! A plugin played in a process of its own, as one of the engine's processors.

use crate::host_program;
use sonevo_dsp::{Context, Event, Param, Processor};
use sonevo_plugin_ipc::{EngineEnd, Note, ParamChange, MAX_NOTES, MAX_PARAMS};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The part of a block's time a plugin is given before its block is played as silence.
const BUDGET: f64 = 0.6;
/// The most frames the engine asks for at once.
const MAX_FRAMES: u32 = 256;

static LINKS: AtomicUsize = AtomicUsize::new(0);

/// A plugin in its own process; the process started again whenever it ends.
pub struct Remote {
    end: Option<EngineEnd>,
    sample_rate: f32,
    restarts: Arc<AtomicU64>,
    /// The restarts seen: one more, and the keys held are pressed again.
    seen: u64,
    held: u128,
    /// Keys let go of in blocks the plugin did not play.
    released: u128,
    /// The last block went unplayed: its keys are to be pressed or let go of again.
    resend: bool,
    notes: [Note; MAX_NOTES],
    /// The plugin's settings by index, as the engine knows them: their CLAP ids, and the
    /// value each was set to.
    ids: Vec<u32>,
    values: Vec<Option<f64>>,
    /// The settings changed since the last block, by index.
    changed: Vec<usize>,
    changes: [ParamChange; MAX_PARAMS],
    stop: Arc<AtomicBool>,
    watcher: Option<std::thread::JoinHandle<()>>,
}

impl Remote {
    /// Starts the plugin `id` of the bundle at `bundle`, for blocks at `sample_rate`; its
    /// settings, as [`crate::params`] gave them, set by index; its window opened when the
    /// event `window` is set, if named.
    pub fn start(
        bundle: &Path,
        id: &str,
        sample_rate: f32,
        params: &[Param],
        window: Option<&str>,
    ) -> std::io::Result<Remote> {
        let host = host_program()?;
        let name = format!("sonevo-{}-{}", std::process::id(), LINKS.fetch_add(1, Ordering::Relaxed));
        let end = EngineEnd::create(&name)?;
        let mut args: Vec<String> = vec![
            name,
            bundle.to_string_lossy().into_owned(),
            id.to_owned(),
            sample_rate.to_string(),
            MAX_FRAMES.to_string(),
            std::process::id().to_string(),
        ];
        args.extend(window.map(str::to_owned));
        let child = spawn(&host, &args)?;
        let restarts = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let watcher = watch(child, host, args, restarts.clone(), stop.clone());
        Ok(Remote {
            end: Some(end),
            sample_rate,
            restarts,
            seen: 0,
            held: 0,
            released: 0,
            resend: false,
            notes: [Note { at: 0, key: 0, velocity: 0.0, on: false }; MAX_NOTES],
            ids: params.iter().map(|p| p.id.parse().unwrap_or(u32::MAX)).collect(),
            values: vec![None; params.len()],
            changed: Vec::with_capacity(params.len()),
            changes: [ParamChange { at: 0, id: 0, value: 0.0 }; MAX_PARAMS],
            stop,
            watcher: Some(watcher),
        })
    }

    /// How many times the process was started again.
    pub fn restarts(&self) -> u64 {
        self.restarts.load(Ordering::Relaxed)
    }
}

fn spawn(host: &PathBuf, args: &[String]) -> std::io::Result<Child> {
    Command::new(host).args(args).stdout(std::process::Stdio::null()).spawn()
}

/// Starts the process again whenever it ends, until `stop`.
fn watch(
    mut child: Child,
    host: PathBuf,
    args: Vec<String>,
    restarts: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(20));
            if matches!(child.try_wait(), Ok(Some(_))) && !stop.load(Ordering::Acquire) {
                match spawn(&host, &args) {
                    Ok(again) => {
                        child = again;
                        restarts.fetch_add(1, Ordering::Release);
                    }
                    Err(_) => return,
                }
            }
        }
        // the link closed, the process ends by itself; if not, it is ended
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(1) {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
    })
}

impl Processor for Remote {
    fn set(&mut self, index: usize, value: f64) {
        let Some(known) = self.values.get_mut(index) else { return };
        *known = Some(value);
        if !self.changed.contains(&index) && self.changed.len() < self.changed.capacity() {
            self.changed.push(index);
        }
    }

    fn process(&mut self, _context: &Context, events: &[Event], left: &mut [f32], right: &mut [f32]) {
        let Some(end) = &mut self.end else { return };
        let mut count = 0;
        // a process started again, or a block it did not play, and it may not know of the keys
        // pressed and let go of: told again
        let restarts = self.restarts.load(Ordering::Acquire);
        if restarts != self.seen {
            self.seen = restarts;
            self.resend = true;
        }
        if self.resend {
            for (keys, on) in [(self.released, false), (self.held, true)] {
                let mut keys = keys;
                while keys != 0 && count < MAX_NOTES {
                    let key = keys.trailing_zeros() as u8;
                    keys &= keys - 1;
                    self.notes[count] = Note { at: 0, key, velocity: if on { 0.8 } else { 0.0 }, on };
                    count += 1;
                }
            }
        }
        for e in events {
            if count == MAX_NOTES {
                break;
            }
            let bit = 1u128 << (e.key & 127);
            if e.on {
                self.held |= bit;
                self.released &= !bit;
            } else {
                self.held &= !bit;
                self.released |= bit;
            }
            self.notes[count] = Note { at: e.at, key: e.key, velocity: e.velocity, on: e.on };
            count += 1;
        }
        // the settings changed, or, the plugin perhaps not knowing them, all those set
        let mut changed = 0;
        if self.resend {
            for (index, value) in self.values.iter().enumerate() {
                if let (Some(value), true) = (value, changed < MAX_PARAMS) {
                    self.changes[changed] = ParamChange { at: 0, id: self.ids[index], value: *value };
                    changed += 1;
                }
            }
        } else {
            for &index in self.changed.iter().take(MAX_PARAMS) {
                if let Some(value) = self.values[index] {
                    self.changes[changed] = ParamChange { at: 0, id: self.ids[index], value };
                    changed += 1;
                }
            }
        }
        let budget = Duration::from_secs_f64(left.len() as f64 / f64::from(self.sample_rate) * BUDGET);
        let block = end.ask(left.len(), &self.notes[..count], &self.changes[..changed]);
        if end.take(block, budget, left, right) {
            self.resend = false;
            self.released = 0;
            self.changed.clear();
        } else {
            self.resend = true;
            left.fill(0.0);
            right.fill(0.0);
        }
    }

    fn reset(&mut self) {
        // let go of every key in the next block
        let mut held = std::mem::take(&mut self.held);
        let mut events = [Event { at: 0, key: 0, velocity: 0.0, on: false }; 128];
        let mut count = 0;
        while held != 0 {
            events[count].key = held.trailing_zeros() as u8;
            held &= held - 1;
            count += 1;
        }
        if count > 0 {
            let mut left = [0.0f32; 1];
            let mut right = [0.0f32; 1];
            let context = Context { sample_rate: self.sample_rate, tempo: 120.0 };
            self.process(&context, &events[..count], &mut left, &mut right);
        }
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // the link closed tells the process to end
        drop(self.end.take());
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}
