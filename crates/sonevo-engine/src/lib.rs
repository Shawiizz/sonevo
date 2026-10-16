//! Plays a song: its patterns' notes through the channels' instruments, the instruments
//! through the mixer, the mixer to the speakers or to a file.
//!
//! The [`Engine`] is held by the interface; the [`Renderer`] it comes with, by the audio
//! thread. A song changed is compiled again, on the engine's side, into a graph the
//! renderer swaps in between two blocks: instruments and effects kept from one graph to the
//! next, so that a sound playing goes on. Old graphs come back to the engine to be freed, so
//! that the audio thread never frees nor allocates.

mod compile;
mod graph;
mod meters;
mod renderer;
mod samples;
mod wav;

pub use compile::{Mode, Plugins};
pub use renderer::Renderer;
pub use samples::{decode, DecodeError, Samples};
pub use wav::{write_wav, Depth};

use compile::{compile, Mirror};
use graph::Graph;
use renderer::{Command, Shared};
use rtrb::{Consumer, Producer, RingBuffer};
use sonevo_dsp::Sample;
use sonevo_project::{Id, Region, Song, Ticks};
use std::path::Path;
use std::sync::Arc;

/// Whose loudness to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meter {
    Master,
    Insert(Id),
    Channel(Id),
}

pub struct Engine {
    commands: Producer<Command>,
    garbage: Consumer<Box<Graph>>,
    shared: Arc<Shared>,
    sample_rate: f32,
    mirror: Mirror,
    samples: Samples,
    plugins: Option<Arc<dyn Plugins>>,
    /// The sounds listened to last: kept here so that the audio thread, letting go of
    /// them, never frees them.
    listened: [Option<Arc<Sample>>; 2],
}

impl Engine {
    pub fn new(sample_rate: f32) -> (Engine, Renderer) {
        let (commands, commands_in) = RingBuffer::new(1024);
        let (garbage_out, garbage) = RingBuffer::new(256);
        let shared = Arc::new(Shared::new());
        let renderer = Renderer::new(commands_in, garbage_out, shared.clone(), sample_rate);
        let engine = Engine {
            commands,
            garbage,
            shared,
            sample_rate,
            mirror: Mirror::default(),
            samples: Samples::default(),
            plugins: None,
            listened: [None, None],
        };
        (engine, renderer)
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// How the plugins songs name are started.
    pub fn set_plugins(&mut self, plugins: Arc<dyn Plugins>) {
        self.plugins = Some(plugins);
    }

    /// The sounds the songs play, for one decoded already to be given.
    pub fn samples(&mut self) -> &mut Samples {
        &mut self.samples
    }

    /// Plays `song` from now on, its sounds in `folder`: what it could not play, said
    /// plainly.
    pub fn load(&mut self, song: &Song, mode: Mode, folder: Option<&Path>) -> Vec<String> {
        self.collect();
        if folder.is_some() {
            self.samples.set_folder(folder);
        }
        let compiled =
            compile(song, mode, &self.mirror, self.sample_rate, &mut self.samples, self.plugins.as_deref());
        let mut missing = compiled.missing;
        match self.commands.push(Command::Swap(compiled.graph)) {
            Ok(()) => self.mirror = compiled.mirror,
            Err(_) => missing.push("the engine is not keeping up: the song was not changed".into()),
        }
        missing
    }

    fn send(&mut self, command: Command) {
        self.collect();
        // full only when nothing has played for a thousand commands: nothing is listening
        let _ = self.commands.push(command);
    }

    /// Frees the graphs the audio thread is done with.
    pub fn collect(&mut self) {
        while self.garbage.pop().is_ok() {}
    }

    pub fn play(&mut self) {
        self.send(Command::Play { once: false });
    }

    /// Plays to the end of the song, or of the loop, and stops there.
    pub fn play_once(&mut self) {
        self.send(Command::Play { once: true });
    }

    pub fn pause(&mut self) {
        self.send(Command::Pause);
    }

    /// Stops, back to where playing started.
    pub fn stop(&mut self) {
        self.send(Command::Stop);
    }

    pub fn seek(&mut self, tick: Ticks) {
        self.send(Command::Seek(tick as f64));
    }

    /// Plays `sample` as it is over the song, or stops the one playing.
    pub fn listen(&mut self, sample: Option<Arc<Sample>>) {
        self.listened.rotate_left(1);
        self.listened[1] = sample.clone();
        self.send(Command::Listen(sample));
    }

    pub fn set_metronome(&mut self, on: bool) {
        self.send(Command::Metronome(on));
    }

    /// Every sound cut at once.
    pub fn panic(&mut self) {
        self.send(Command::Panic);
    }

    /// A key pressed or let go of on `channel`'s instrument, from a keyboard or a click.
    pub fn note(&mut self, channel: Id, key: u8, velocity: f32, on: bool) {
        if let Some(channel) = self.mirror.channels.iter().position(|&c| c == channel) {
            self.send(Command::Note { channel, key: key.min(127), velocity: velocity.clamp(0.0, 1.0), on });
        }
    }

    /// Sets a device's setting at once, as a knob turns; the song is to be changed as well.
    pub fn set_param(&mut self, device: Id, param: &str, value: f64) -> bool {
        let Some(held) = self.mirror.devices.get_mut(&device) else { return false };
        let Some(index) = held.params.iter().position(|p| p.id == param) else { return false };
        let value = held.params[index].clamp(value);
        held.set(index, value);
        let slot = held.slot;
        self.send(Command::Param { slot, index, value });
        true
    }

    pub fn set_channel_level(&mut self, channel: Id, volume: f32, pan: f32) {
        if let Some(channel) = self.mirror.channels.iter().position(|&c| c == channel) {
            let (volume, pan) = (volume.clamp(0.0, 1.0), pan.clamp(-1.0, 1.0));
            self.send(Command::ChannelLevel { channel, volume, pan });
        }
    }

    pub fn set_insert_level(&mut self, insert: Id, volume: f32, pan: f32) {
        if let Some(insert) = self.mirror.inserts.iter().position(|&i| i == insert) {
            let (volume, pan) = (volume.clamp(0.0, 1.25), pan.clamp(-1.0, 1.0));
            self.send(Command::InsertLevel { insert, volume, pan });
        }
    }

    /// Where playing is, in ticks.
    pub fn position(&self) -> f64 {
        self.shared.position()
    }

    pub fn playing(&self) -> bool {
        self.shared.playing()
    }

    /// The loudest sample `meter` played since it was last read.
    pub fn meter(&self, meter: Meter) -> f32 {
        let slot = match meter {
            Meter::Master => Some(meters::MASTER),
            Meter::Insert(id) => {
                let last = self.mirror.inserts.len().saturating_sub(1);
                self.mirror.inserts.iter().position(|&i| i == id).map(|i| {
                    if i == last {
                        meters::MASTER
                    } else {
                        meters::insert(i)
                    }
                })
            }
            Meter::Channel(id) => self.mirror.channels.iter().position(|&c| c == id).map(meters::channel),
        };
        slot.map_or(0.0, |s| self.shared.meters.take(s))
    }
}

/// A song played through, as a file would hold it.
pub struct Rendered {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    /// What could not be played, said plainly.
    pub missing: Vec<String>,
}

/// The longest song played to a file, in seconds; and the longest its tails ring after.
const LONGEST: usize = 60 * 60;
const TAIL: usize = 10;

/// Plays `song` from its start to its end, then its tails (a reverb, a delay) until they
/// die away.
pub fn render(song: &Song, folder: Option<&Path>, sample_rate: u32) -> Rendered {
    render_part(song, folder, sample_rate, None)
}

/// Plays `song` as [`render`] does, only from `region`'s start to its end if given.
pub fn render_part(song: &Song, folder: Option<&Path>, sample_rate: u32, region: Option<Region>) -> Rendered {
    render_with(song, folder, sample_rate, region, Mode::Song)
}

/// Plays `song` as [`render`] does, as the mixer track `insert` alone sends it to the
/// master; the master's id gives what goes to the master straight, through no track.
pub fn render_stem(song: &Song, folder: Option<&Path>, sample_rate: u32, insert: Id) -> Rendered {
    render_with(song, folder, sample_rate, None, Mode::Stem(insert))
}

fn render_with(
    song: &Song,
    folder: Option<&Path>,
    sample_rate: u32,
    region: Option<Region>,
    mode: Mode,
) -> Rendered {
    let mut song = song.clone();
    song.loop_region = region.filter(|r| r.end > r.start);
    let (mut engine, mut renderer) = Engine::new(sample_rate as f32);
    let missing = engine.load(&song, mode, folder);
    if let Some(region) = song.loop_region {
        engine.seek(region.start);
    }
    engine.play_once();
    let block = 1024;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    let rate = sample_rate as usize;
    loop {
        renderer.render(&mut l, &mut r);
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
        engine.collect();
        if !renderer.playing() || left.len() > LONGEST * rate {
            break;
        }
    }
    // the tails, until half a second under -80 dB
    let (mut quiet, mut tail) = (0, 0);
    while quiet < rate / 2 && tail < TAIL * rate {
        renderer.render(&mut l, &mut r);
        let loud = l.iter().chain(&r).any(|s| s.abs() > 1e-4);
        quiet = if loud { 0 } else { quiet + block };
        tail += block;
        left.extend_from_slice(&l);
        right.extend_from_slice(&r);
    }
    let end = left.len() - quiet.min(left.len());
    left.truncate(end);
    right.truncate(end);
    Rendered { sample_rate, left, right, missing }
}
