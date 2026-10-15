//! Sonevo's engine tried: a pattern played into a plugin, out through WASAPI or ASIO, the
//! plugin in a process of its own (or, to compare, in the engine's), and what it costs
//! measured each second. With `--kill-every`, the plugin's process is ended now and then,
//! as a crash would, to hear the engine play on.
//!
//! ```text
//! engine-spike [--asio <driver>] [--device <name>] [--buffer <frames>] [--here]
//!              [--seconds <n>] [--kill-every <seconds>] [--tempo <bpm>]
//!              [--plugin <bundle.clap>] [--id <plugin id>]
//! ```

mod player;
mod sequencer;
mod stats;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use player::{Player, Plugin, Watcher};
use sequencer::Sequencer;
use sonevo_plugin_ipc::MAX_FRAMES;
use stats::{percentile, Stats};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// The share of a block's own length the plugin's process has to play it.
const BUDGET: f64 = 0.6;
/// How loud the synth plays: its notes stay well under full scale.
const VOLUME: f32 = 0.7;

struct Options {
    asio: Option<String>,
    device: Option<String>,
    buffer: Option<u32>,
    here: bool,
    seconds: u64,
    kill_every: Option<Duration>,
    tempo: f64,
    plugin: Plugin,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let exe_dir =
        std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)).unwrap_or_default();
    let mut options = Options {
        asio: None,
        device: None,
        buffer: None,
        here: false,
        seconds: 20,
        kill_every: None,
        tempo: 120.0,
        plugin: Plugin {
            bundle: exe_dir.join(format!(
                "{}sonevo_test_synth{}",
                std::env::consts::DLL_PREFIX,
                std::env::consts::DLL_SUFFIX
            )),
            id: "dev.sonevo.test-synth".into(),
        },
    };
    let number = |value: Option<String>, what: &str| -> Result<f64, String> {
        value.and_then(|v| v.parse().ok()).ok_or_else(|| format!("{what} needs a number"))
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--asio" => options.asio = Some(args.next().ok_or("--asio needs a driver's name")?),
            "--device" => options.device = Some(args.next().ok_or("--device needs a name")?),
            "--buffer" => options.buffer = Some(number(args.next(), "--buffer")? as u32),
            "--here" => options.here = true,
            "--seconds" => options.seconds = number(args.next(), "--seconds")? as u64,
            "--kill-every" => {
                options.kill_every = Some(Duration::from_secs_f64(number(args.next(), "--kill-every")?))
            }
            "--tempo" => options.tempo = number(args.next(), "--tempo")?,
            "--plugin" => options.plugin.bundle = PathBuf::from(args.next().ok_or("--plugin needs a path")?),
            "--id" => options.plugin.id = args.next().ok_or("--id needs a plugin id")?,
            other => return Err(format!("{other}: not an option")),
        }
    }
    Ok(options)
}

fn main() {
    if let Err(e) = run() {
        eprintln!("engine-spike: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let options = options()?;
    let host = match options.asio {
        Some(_) => cpal::host_from_id(cpal::HostId::Asio)?,
        None => cpal::host_from_id(cpal::HostId::Wasapi)?,
    };
    let wanted = options.asio.as_deref().or(options.device.as_deref()).map(str::to_lowercase);
    let name_of = |d: &cpal::Device| d.description().ok().map(|d| d.name().to_string()).unwrap_or_default();
    let device = match &wanted {
        Some(wanted) => host
            .output_devices()?
            .find(|d| name_of(d).to_lowercase().contains(wanted))
            .ok_or_else(|| format!("no output device named {wanted}"))?,
        None => host.default_output_device().ok_or("no output device")?,
    };
    let supported = device.default_output_config()?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let mut config = supported.config();
    if let Some(frames) = options.buffer {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }

    let (player, watcher) = if options.here {
        (options.plugin.here(sample_rate, MAX_FRAMES as u32)?, None)
    } else {
        let (player, watcher) =
            Watcher::start(&options.plugin, sample_rate, MAX_FRAMES as u32, options.kill_every)?;
        (player, Some(watcher))
    };
    println!(
        "{} · {} · {} Hz, {} channels, {:?} · plugin {}",
        if options.asio.is_some() { "ASIO" } else { "WASAPI" },
        name_of(&device),
        sample_rate,
        channels,
        supported.sample_format(),
        if options.here { "in the engine's process" } else { "in a process of its own" },
    );

    let stats = Arc::new(Stats::default());
    let engine = Engine {
        player,
        sequencer: Sequencer::new(sample_rate, options.tempo),
        left: vec![0.0; MAX_FRAMES],
        right: vec![0.0; MAX_FRAMES],
        channels,
        sample_rate,
        stats: Arc::clone(&stats),
    };
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => output::<f32>(&device, &config, engine)?,
        cpal::SampleFormat::I32 => output::<i32>(&device, &config, engine)?,
        cpal::SampleFormat::I16 => output::<i16>(&device, &config, engine)?,
        other => return Err(format!("{other:?} samples are not played yet").into()),
    };
    stream.play()?;
    report(&stats, watcher.as_ref(), sample_rate, options.seconds);
    if let Some(watcher) = &watcher {
        watcher.finish();
    }
    drop(stream);
    drop(watcher);
    Ok(())
}

/// What the audio thread holds: everything allocated before it starts.
struct Engine {
    player: Player,
    sequencer: Sequencer,
    left: Vec<f32>,
    right: Vec<f32>,
    channels: usize,
    sample_rate: u32,
    stats: Arc<Stats>,
}

impl Engine {
    fn render<T: SizedSample + FromSample<f32>>(&mut self, data: &mut [T], info: &cpal::OutputCallbackInfo) {
        let stamp = info.timestamp();
        let latency = stamp.playback.duration_since(stamp.callback);
        self.stats.output_latency_ns.store(latency.as_nanos() as u64, Ordering::Relaxed);
        for chunk in data.chunks_mut(MAX_FRAMES * self.channels) {
            let frames = chunk.len() / self.channels;
            let notes = self.sequencer.next(frames);
            let budget = Duration::from_secs_f64(frames as f64 / f64::from(self.sample_rate) * BUDGET);
            let (left, right) = (&mut self.left[..frames], &mut self.right[..frames]);
            if !self.player.play(notes, left, right, budget, &self.stats) {
                left.fill(0.0);
                right.fill(0.0);
                self.stats.late.fetch_add(1, Ordering::Relaxed);
            }
            for (i, frame) in chunk.chunks_mut(self.channels).enumerate() {
                let (l, r) = (left[i] * VOLUME, right[i] * VOLUME);
                for (c, sample) in frame.iter_mut().enumerate() {
                    *sample = T::from_sample(if c % 2 == 0 { l } else { r });
                }
            }
            self.stats.blocks.fetch_add(1, Ordering::Relaxed);
            self.stats.frames.fetch_add(frames as u64, Ordering::Relaxed);
        }
    }
}

fn output<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: Engine,
) -> Result<cpal::Stream, cpal::Error> {
    device.build_output_stream(
        *config,
        move |data: &mut [T], info: &cpal::OutputCallbackInfo| engine.render(data, info),
        |e| eprintln!("audio: {e}"),
        None,
    )
}

/// A line a second: the blocks, how long the plugin took for them, those late, the restarts.
fn report(stats: &Stats, watcher: Option<&Watcher>, sample_rate: u32, seconds: u64) {
    let mut before = stats.trips();
    let (mut blocks_before, mut frames_before, mut late_before) = (0, 0, 0);
    for second in 1..=seconds {
        std::thread::sleep(Duration::from_secs(1));
        let now = stats.trips();
        let trips: Vec<u64> = now.iter().zip(&before).map(|(a, b)| a - b).collect();
        before = now;
        let (blocks, frames, late) = (
            stats.blocks.load(Ordering::Relaxed),
            stats.frames.load(Ordering::Relaxed),
            stats.late.load(Ordering::Relaxed),
        );
        let block_frames = (frames - frames_before) as f64 / (blocks - blocks_before).max(1) as f64;
        let longest = stats.longest_trip_ns.swap(0, Ordering::Relaxed) / 1000;
        let shown = |us: Option<u64>| us.map_or("-".into(), |us| format!("{us} µs"));
        println!(
            "{second:>3} s · blocks of {block_frames:.0} frames ({:.2} ms) · out {:.1} ms · plugin p50 {} p99 {} max {longest} µs · late {} · restarts {}",
            block_frames / f64::from(sample_rate) * 1000.0,
            stats.output_latency_ns.load(Ordering::Relaxed) as f64 / 1e6,
            shown(percentile(&trips, 0.5)),
            shown(percentile(&trips, 0.99)),
            late - late_before,
            watcher.map_or(0, |w| w.restarts.load(Ordering::Relaxed)),
        );
        (blocks_before, frames_before, late_before) = (blocks, frames, late);
    }
}
