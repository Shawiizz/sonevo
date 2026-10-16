//! The sounds a song plays, decoded once and kept: WAV, FLAC, MP3, Ogg Vorbis.

use sonevo_dsp::Sample;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as Symphonia;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("{0} could not be opened: {1}")]
    Open(PathBuf, std::io::Error),
    #[error("{0} is not a sound Sonevo reads: {1}")]
    Format(PathBuf, Symphonia),
    #[error("{0} holds no sound")]
    Empty(PathBuf),
}

/// Decodes the sound in the file at `path`, both channels (a mono sound in both).
pub fn decode(path: &Path) -> Result<Sample, DecodeError> {
    let file = File::open(path).map_err(|e| DecodeError::Open(path.to_owned(), e))?;
    let format_error = |e| DecodeError::Format(path.to_owned(), e);
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(format_error)?;
    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| DecodeError::Empty(path.to_owned()))?;
    let track_id = track.id;
    let mut sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(format_error)?;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Symphonia::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Symphonia::ResetRequired) => break,
            Err(e) => return Err(format_error(e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // a damaged packet is skipped, as players do
            Err(Symphonia::DecodeError(_)) => continue,
            Err(e) => return Err(format_error(e)),
        };
        let spec = *decoded.spec();
        sample_rate = spec.rate;
        let channels = spec.channels.count().max(1);
        let buffer = match &mut buffer {
            Some(b) if b.capacity() >= decoded.capacity() * channels => b,
            _ => buffer.insert(SampleBuffer::new(decoded.capacity() as u64, spec)),
        };
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks_exact(channels) {
            left.push(frame[0]);
            right.push(frame[if channels > 1 { 1 } else { 0 }]);
        }
    }
    if left.is_empty() {
        return Err(DecodeError::Empty(path.to_owned()));
    }
    Ok(Sample { sample_rate: sample_rate as f32, left, right })
}

/// The sounds of a song's folder, each decoded the first time it is asked for.
#[derive(Default)]
pub struct Samples {
    folder: Option<PathBuf>,
    loaded: HashMap<String, Option<Arc<Sample>>>,
}

impl Samples {
    pub fn new(folder: Option<&Path>) -> Samples {
        Samples { folder: folder.map(Path::to_owned), loaded: HashMap::new() }
    }

    /// Where the song's sounds are, the ones of another folder forgotten.
    /// Whether `name` is a file's own name, and no way to another folder or drive.
    pub fn plain_name(name: &str) -> bool {
        plain(name)
    }

    pub fn set_folder(&mut self, folder: Option<&Path>) {
        if self.folder.as_deref() != folder {
            self.folder = folder.map(Path::to_owned);
            self.loaded.clear();
        }
    }

    pub fn insert(&mut self, name: &str, sample: Arc<Sample>) {
        self.loaded.insert(name.to_owned(), Some(sample));
    }

    /// The sound the song names `name`, kept in its folder's `samples`.
    pub fn get(&mut self, name: &str) -> Option<Arc<Sample>> {
        if let Some(loaded) = self.loaded.get(name) {
            return loaded.clone();
        }
        let decoded = self
            .folder
            .as_ref()
            .filter(|_| plain(name))
            .and_then(|folder| decode(&folder.join("samples").join(name)).ok())
            .map(Arc::new);
        self.loaded.insert(name.to_owned(), decoded.clone());
        decoded
    }
}

/// Whether `name` is a single plain file name: no folder, no drive, no `..`.
fn plain(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!((parts.next(), parts.next()), (Some(std::path::Component::Normal(_)), None))
        && !name.contains([':', '/', '\\'])
}
