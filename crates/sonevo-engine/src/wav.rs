//! A song played to a WAV file.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// CD quality, dithered.
    Pcm16,
    Pcm24,
    /// Every bit kept, past 0 dB too.
    Float32,
}

/// Writes `left` and `right` as a stereo WAV file.
pub fn write_wav(
    path: &Path,
    sample_rate: u32,
    left: &[f32],
    right: &[f32],
    depth: Depth,
) -> std::io::Result<()> {
    let frames = left.len().min(right.len());
    let (bytes, format) = match depth {
        Depth::Pcm16 => (2u16, 1u16),
        Depth::Pcm24 => (3, 1),
        Depth::Float32 => (4, 3),
    };
    let block = 2 * bytes;
    let data = u32::try_from(frames * usize::from(block))
        .map_err(|_| std::io::Error::other("too long for a WAV file"))?;
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + data).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&format.to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&sample_rate.to_le_bytes())?;
    out.write_all(&(sample_rate * u32::from(block)).to_le_bytes())?;
    out.write_all(&block.to_le_bytes())?;
    out.write_all(&(bytes * 8).to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data.to_le_bytes())?;
    let mut noise = 0x9e37_79b9u32;
    let mut dither = move || {
        // two uniform draws: a triangle a bit wide, the least noise that hides the steps
        let mut draw = || {
            noise ^= noise << 13;
            noise ^= noise >> 17;
            noise ^= noise << 5;
            noise as f32 / u32::MAX as f32
        };
        draw() - draw()
    };
    for i in 0..frames {
        for s in [left[i], right[i]] {
            match depth {
                Depth::Pcm16 => {
                    let v = (s * 32767.0 + dither()).round().clamp(-32768.0, 32767.0) as i16;
                    out.write_all(&v.to_le_bytes())?;
                }
                Depth::Pcm24 => {
                    let v = (s * 8_388_607.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
                    out.write_all(&v.to_le_bytes()[..3])?;
                }
                Depth::Float32 => out.write_all(&s.to_le_bytes())?,
            }
        }
    }
    out.flush()
}
