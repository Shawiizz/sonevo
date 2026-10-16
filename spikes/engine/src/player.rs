//! The plugin played: in the engine's own process, or in one of its own that a watcher
//! starts again whenever it ends.

use crate::stats::Stats;
use sonevo_clap::{Bundle, Instance, Note};
use sonevo_plugin_ipc::EngineEnd;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Plays a block on the audio thread: the sound into `left` and `right`, false when there is
/// none within `budget`.
pub enum Player {
    Here(Instance),
    Apart(EngineEnd),
}

impl Player {
    pub fn play(
        &mut self,
        notes: &[Note],
        left: &mut [f32],
        right: &mut [f32],
        budget: Duration,
        stats: &Stats,
    ) -> bool {
        let start = Instant::now();
        let played = match self {
            Player::Here(instance) => instance.process(notes, &[], left, right),
            Player::Apart(end) => {
                let block = end.ask(notes, &[], left, right);
                end.take(block, budget, left, right)
            }
        };
        if played {
            stats.trip(start.elapsed());
        }
        played
    }
}

/// What the plugin is, and how to play it.
pub struct Plugin {
    pub bundle: PathBuf,
    pub id: String,
}

impl Plugin {
    pub fn here(&self, sample_rate: u32, max_frames: u32) -> Result<Player, Box<dyn std::error::Error>> {
        let bundle = Bundle::open(&self.bundle)?;
        let mut instance = bundle.instantiate(&self.id)?;
        instance.activate(f64::from(sample_rate), max_frames)?;
        Ok(Player::Here(instance))
    }
}

/// The process the plugin plays in, started again when it ends, and ended on demand to see
/// the engine play on.
pub struct Watcher {
    pub restarts: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watcher {
    /// The link made, its process started; `kill_every` ends it that often, as a crash would.
    pub fn start(
        plugin: &Plugin,
        sample_rate: u32,
        max_frames: u32,
        kill_every: Option<Duration>,
    ) -> Result<(Player, Watcher), Box<dyn std::error::Error>> {
        let name = format!("sonevo-{}-0", std::process::id());
        let end = EngineEnd::create(&name)?;
        let host = host_program()?;
        let args = vec![
            name,
            plugin.bundle.to_string_lossy().into_owned(),
            plugin.id.clone(),
            sample_rate.to_string(),
            max_frames.to_string(),
            std::process::id().to_string(),
        ];
        let spawn = move || Command::new(&host).args(&args).spawn();
        let mut child = spawn()?;
        let restarts = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (restarts, stop) = (Arc::clone(&restarts), Arc::clone(&stop));
            std::thread::spawn(move || {
                let mut killed_at = Instant::now();
                while !stop.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(20));
                    if kill_every.is_some_and(|every| killed_at.elapsed() >= every) {
                        let _ = child.kill();
                        killed_at = Instant::now();
                    }
                    // ended once the engine is closing, the process stays ended
                    if matches!(child.try_wait(), Ok(Some(_))) && !stop.load(Ordering::Acquire) {
                        match spawn() {
                            Ok(again) => {
                                child = again;
                                restarts.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => {
                                eprintln!("the plugin's process would not start again: {e}");
                                return;
                            }
                        }
                    }
                }
                end_child(&mut child);
            })
        };
        Ok((Player::Apart(end), Watcher { restarts, stop, thread: Some(thread) }))
    }
}

impl Watcher {
    /// Starts the process no more: to call before the engine closes the link, which ends it.
    pub fn finish(&self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.finish();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Leaves the process a moment to end by itself, the engine gone, then ends it.
fn end_child(child: &mut Child) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// `sonevo-plugin-host`, built beside this program.
fn host_program() -> std::io::Result<PathBuf> {
    let here = std::env::current_exe()?;
    let dir = here.parent().unwrap_or(Path::new("."));
    let host = dir.join(format!("sonevo-plugin-host{}", std::env::consts::EXE_SUFFIX));
    if host.is_file() {
        Ok(host)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} is not built: cargo build -p sonevo-plugin-host", host.display()),
        ))
    }
}
