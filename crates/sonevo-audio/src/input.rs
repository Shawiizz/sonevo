//! A sound card's input, recorded: what it hears kept as it comes, on a thread of its own
//! holding the stream, to be taken from the interface's side.

use crate::{name_of, AudioError, Driver};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

/// What the inputs there are can be listened to through.
pub fn inputs(driver: Driver) -> Vec<String> {
    let Ok(host) = driver.host() else { return Vec::new() };
    let Ok(devices) = host.input_devices() else { return Vec::new() };
    devices.map(|d| name_of(&d)).collect()
}

/// Seconds of sound kept between two takes, at most.
const KEPT_SECONDS: usize = 30;

/// An input recording; stopped when dropped.
pub struct Input {
    pub name: String,
    pub sample_rate: u32,
    /// The frames heard, two samples each: left, right.
    heard: Consumer<f32>,
    /// The loudest sample since the last look, as bits of a float.
    peak: Arc<AtomicU32>,
    overflowed: Arc<AtomicBool>,
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Input {
    /// Opens the input `device` of `driver` (its default if none), recording at once.
    pub fn start(driver: Driver, device: Option<&str>) -> Result<Input, AudioError> {
        let (opened, opened_in) = mpsc::channel();
        let (stop, stop_in) = mpsc::channel::<()>();
        let device = device.map(str::to_owned);
        let thread = std::thread::Builder::new()
            .name("sonevo-input".into())
            .spawn(move || {
                let stream = match open(driver, device.as_deref()) {
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
        let (name, sample_rate, heard, peak, overflowed) =
            opened_in.recv().map_err(|_| AudioError::Open("the input thread ended".into()))??;
        Ok(Input { name, sample_rate, heard, peak, overflowed, stop: Some(stop), thread: Some(thread) })
    }

    /// The frames heard since the last take, as left and right.
    pub fn take(&mut self, left: &mut Vec<f32>, right: &mut Vec<f32>) {
        let count = self.heard.slots() / 2 * 2;
        if let Ok(chunk) = self.heard.read_chunk(count) {
            let (a, b) = chunk.as_slices();
            for pair in a.iter().chain(b).copied().collect::<Vec<f32>>().chunks_exact(2) {
                left.push(pair[0]);
                right.push(pair[1]);
            }
            chunk.commit_all();
        }
    }

    /// The loudest sample heard since the last look.
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }

    /// Whether some of what was heard was lost, not taken in time.
    pub fn overflowed(&self) -> bool {
        self.overflowed.load(Ordering::Relaxed)
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

type Opened = (String, u32, Consumer<f32>, Arc<AtomicU32>, Arc<AtomicBool>);

fn open(driver: Driver, wanted: Option<&str>) -> Result<(cpal::Stream, Opened), AudioError> {
    let host = driver.host()?;
    let device = match wanted {
        Some(wanted) => host
            .input_devices()
            .map_err(|e| AudioError::Driver(e.to_string()))?
            .find(|d| name_of(d) == wanted)
            .ok_or_else(|| AudioError::NoDevice(wanted.to_owned()))?,
        None => host.default_input_device().ok_or(AudioError::NoDefault)?,
    };
    let supported = device.default_input_config().map_err(|e| AudioError::Open(e.to_string()))?;
    let sample_rate = supported.sample_rate();
    let channels = usize::from(supported.channels()).max(1);
    let config = supported.config();
    let (heard_in, heard) = RingBuffer::new(sample_rate as usize * 2 * KEPT_SECONDS);
    let peak = Arc::new(AtomicU32::new(0));
    let overflowed = Arc::new(AtomicBool::new(false));
    let shared = (peak.clone(), overflowed.clone());
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => build::<f32>(&device, config, channels, heard_in, shared),
        cpal::SampleFormat::I32 => build::<i32>(&device, config, channels, heard_in, shared),
        cpal::SampleFormat::I16 => build::<i16>(&device, config, channels, heard_in, shared),
        other => return Err(AudioError::Format(format!("{other:?}"))),
    }?;
    stream.play().map_err(|e| AudioError::Open(e.to_string()))?;
    Ok((stream, (name_of(&device), sample_rate, heard, peak, overflowed)))
}

fn build<T: SizedSample>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    channels: usize,
    mut heard: Producer<f32>,
    (peak, overflowed): (Arc<AtomicU32>, Arc<AtomicBool>),
) -> Result<cpal::Stream, AudioError>
where
    f32: FromSample<T>,
{
    let listen = move |data: &[T], _: &cpal::InputCallbackInfo| {
        let mut loudest = 0.0f32;
        for frame in data.chunks_exact(channels) {
            // a mono input heard on both sides
            let left: f32 = frame[0].to_sample();
            let right: f32 = if channels > 1 { frame[1].to_sample() } else { left };
            loudest = loudest.max(left.abs()).max(right.abs());
            // a frame kept whole or not at all
            if heard.slots() < 2 {
                overflowed.store(true, Ordering::Relaxed);
                continue;
            }
            let _ = heard.push(left);
            let _ = heard.push(right);
        }
        peak.fetch_max(loudest.to_bits(), Ordering::Relaxed);
    };
    device.build_input_stream(config, listen, |_| {}, None).map_err(|e| AudioError::Open(e.to_string()))
}
