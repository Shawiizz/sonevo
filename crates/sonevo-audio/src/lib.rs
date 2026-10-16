//! The sound card: the outputs there are, and one opened with the engine's renderer playing
//! out of it on a thread of its own.

mod input;

pub use input::{inputs, Input};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use sonevo_engine::Renderer;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Driver {
    /// Windows' own: every card, a little more latency.
    Wasapi,
    /// The card maker's: the lowest latency.
    Asio,
}

impl Driver {
    fn host(self) -> Result<cpal::Host, AudioError> {
        let id = match self {
            Driver::Wasapi => cpal::HostId::Wasapi,
            Driver::Asio => cpal::HostId::Asio,
        };
        cpal::host_from_id(id).map_err(|e| AudioError::Driver(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("the driver could not be opened: {0}")]
    Driver(String),
    #[error("no output named {0}")]
    NoDevice(String),
    #[error("no output to play to")]
    NoDefault,
    #[error("the output could not be opened: {0}")]
    Open(String),
    #[error("{0} samples are not played yet")]
    Format(String),
}

/// An output there is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputInfo {
    pub driver: Driver,
    pub name: String,
    pub default: bool,
}

fn name_of(device: &cpal::Device) -> String {
    device.description().map(|d| d.name().to_owned()).unwrap_or_default()
}

/// The outputs of `driver`; none when the driver is missing.
pub fn outputs(driver: Driver) -> Vec<OutputInfo> {
    let Ok(host) = driver.host() else { return Vec::new() };
    let default = host.default_output_device().map(|d| name_of(&d));
    let Ok(devices) = host.output_devices() else { return Vec::new() };
    devices
        .map(|d| {
            let name = name_of(&d);
            OutputInfo { driver, default: default.as_ref() == Some(&name), name }
        })
        .collect()
}

/// The output wanted: a driver, a device by its name (the driver's default if none), a
/// block length in frames (the device's own if none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub driver: Driver,
    pub device: Option<String>,
    pub buffer: Option<u32>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { driver: Driver::Wasapi, device: None, buffer: None }
    }
}

/// What the output thread lets know.
struct Status {
    /// The time from a block played to it heard, in nanoseconds.
    latency: AtomicU64,
    failed: AtomicBool,
}

/// An output playing; stopped when dropped.
pub struct Output {
    pub name: String,
    pub sample_rate: u32,
    pub channels: usize,
    /// The block length asked for, if one was.
    pub buffer: Option<u32>,
    status: Arc<Status>,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Output {
    /// Opens the output `settings` names, then plays out of it the renderer `make` gives for
    /// its sample rate.
    pub fn start(
        settings: &Settings,
        make: impl FnOnce(u32) -> Renderer + Send + 'static,
    ) -> Result<Output, AudioError> {
        let (opened, opened_in) = mpsc::channel();
        let (stop, stop_in) = mpsc::channel::<()>();
        let status = Arc::new(Status { latency: AtomicU64::new(0), failed: AtomicBool::new(false) });
        let settings = settings.clone();
        let thread_status = status.clone();
        // the stream is held by a thread of its own: some drivers want it never to move
        let thread = std::thread::Builder::new()
            .name("sonevo-output".into())
            .spawn(move || {
                let stream = match open(&settings, make, thread_status) {
                    Ok((stream, info)) => {
                        let _ = opened.send(Ok(info));
                        stream
                    }
                    Err(e) => {
                        let _ = opened.send(Err(e));
                        return;
                    }
                };
                let _ = stop_in.recv();
                drop(stream);
            })
            .map_err(|e| AudioError::Open(e.to_string()))?;
        let (name, sample_rate, channels, buffer) =
            opened_in.recv().map_err(|_| AudioError::Open("the output thread ended".into()))??;
        Ok(Output { name, sample_rate, channels, buffer, status, stop: Some(stop), thread: Some(thread) })
    }

    /// From a block played to it heard, in seconds.
    pub fn latency(&self) -> f64 {
        self.status.latency.load(Ordering::Relaxed) as f64 / 1e9
    }

    /// Whether the output stopped by itself: the card unplugged, the driver taken.
    pub fn failed(&self) -> bool {
        self.status.failed.load(Ordering::Relaxed)
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type Opened = (String, u32, usize, Option<u32>);

fn open(
    settings: &Settings,
    make: impl FnOnce(u32) -> Renderer,
    status: Arc<Status>,
) -> Result<(cpal::Stream, Opened), AudioError> {
    let host = settings.driver.host()?;
    let device = match &settings.device {
        Some(wanted) => host
            .output_devices()
            .map_err(|e| AudioError::Driver(e.to_string()))?
            .find(|d| name_of(d) == *wanted)
            .ok_or_else(|| AudioError::NoDevice(wanted.clone()))?,
        None => host.default_output_device().ok_or(AudioError::NoDefault)?,
    };
    let supported = device.default_output_config().map_err(|e| AudioError::Open(e.to_string()))?;
    let sample_rate = supported.sample_rate();
    let channels = usize::from(supported.channels());
    let mut config = supported.config();
    if let Some(frames) = settings.buffer {
        config.buffer_size = cpal::BufferSize::Fixed(frames);
    }
    let renderer = make(sample_rate);
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, renderer, channels, status),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, renderer, channels, status),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, renderer, channels, status),
        other => return Err(AudioError::Format(format!("{other:?}"))),
    }?;
    stream.play().map_err(|e| AudioError::Open(e.to_string()))?;
    Ok((stream, (name_of(&device), sample_rate, channels, settings.buffer)))
}

/// The most frames converted at once, for a card that does not take floats.
const SCRATCH: usize = 4096;

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut renderer: Renderer,
    channels: usize,
    status: Arc<Status>,
) -> Result<cpal::Stream, AudioError> {
    let mut scratch = vec![0.0f32; SCRATCH * channels];
    let errors = status.clone();
    let play = move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
        let stamp = info.timestamp();
        let latency = stamp.playback.duration_since(stamp.callback);
        status.latency.store(latency.as_nanos() as u64, Ordering::Relaxed);
        for chunk in data.chunks_mut(SCRATCH * channels) {
            let floats = &mut scratch[..chunk.len()];
            renderer.render_interleaved(floats, channels);
            for (out, &s) in chunk.iter_mut().zip(floats.iter()) {
                *out = T::from_sample(s);
            }
        }
    };
    device
        .build_output_stream(config, play, move |_| errors.failed.store(true, Ordering::Relaxed), None)
        .map_err(|e| AudioError::Open(e.to_string()))
}
