//! FLAC files written: the sound kept whole in about half a WAV's size. Each block of frames
//! is foreseen from the samples before it by the best of FLAC's fixed predictors, and what
//! the prediction missed is Rice-coded.

use std::io::{self, Write};
use std::path::Path;

const BLOCK: usize = 4096;
/// The Rice parameters tried, five bits each as FLAC's second coding writes them.
const MAX_RICE: u32 = 30;

/// Bits written one after another, the first the highest.
struct Bits {
    bytes: Vec<u8>,
    pending: u64,
    count: u32,
}

impl Bits {
    fn new() -> Bits {
        Bits { bytes: Vec::new(), pending: 0, count: 0 }
    }

    fn put(&mut self, value: u64, bits: u32) {
        for i in (0..bits).rev() {
            self.pending = (self.pending << 1) | ((value >> i) & 1);
            self.count += 1;
            if self.count == 8 {
                self.bytes.push(self.pending as u8);
                (self.pending, self.count) = (0, 0);
            }
        }
    }

    fn signed(&mut self, value: i64, bits: u32) {
        self.put(value as u64 & ((1u64 << bits) - 1), bits);
    }

    /// `value` as Rice codes it with `k`: its high part in unary, then its `k` low bits.
    fn rice(&mut self, value: u64, k: u32) {
        let high = value >> k;
        for _ in 0..high {
            self.put(0, 1);
        }
        self.put(1, 1);
        self.put(value & ((1u64 << k) - 1), k);
    }

    fn align(&mut self) {
        if self.count > 0 {
            self.put(0, 8 - self.count);
        }
    }
}

fn crc8(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |mut crc, &b| {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0x07 } else { crc << 1 };
        }
        crc
    })
}

fn crc16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0u16, |mut crc, &b| {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x8005 } else { crc << 1 };
        }
        crc
    })
}

/// `n` as FLAC numbers its frames: UTF-8's way, up to 36 bits.
fn utf8(bits: &mut Bits, n: u64) {
    if n < 0x80 {
        return bits.put(n, 8);
    }
    let follow = (1..=6).find(|&f| n < 1u64 << (6 * f + 6 - f)).unwrap_or(6);
    let lead = (0xFF00u64 >> (follow + 1)) & 0xFF;
    bits.put(lead | (n >> (6 * follow)), 8);
    for i in (0..follow).rev() {
        bits.put(0x80 | ((n >> (6 * i)) & 0x3F), 8);
    }
}

/// What a fixed predictor of `order` misses of each sample from the `order`-th on.
fn residual(samples: &[i64], order: usize) -> Vec<i64> {
    (order..samples.len())
        .map(|i| {
            let s = |back: usize| samples[i - back];
            s(0) - match order {
                0 => 0,
                1 => s(1),
                2 => 2 * s(1) - s(2),
                3 => 3 * s(1) - 3 * s(2) + s(3),
                _ => 4 * s(1) - 6 * s(2) + 4 * s(3) - s(4),
            }
        })
        .collect()
}

/// The Rice parameter `residual` takes fewest bits with, and how many.
fn best_rice(residual: &[i64]) -> (u32, u64) {
    let folded: Vec<u64> = residual.iter().map(|&r| ((r << 1) ^ (r >> 63)) as u64).collect();
    (0..=MAX_RICE)
        .map(|k| (k, folded.iter().map(|&u| (u >> k) + 1 + u64::from(k)).sum::<u64>()))
        .min_by_key(|&(_, cost)| cost)
        .unwrap_or((0, 0))
}

/// How a channel of a block is best written: constant, or by a fixed predictor of an order
/// with a Rice parameter; and how many bits it takes.
struct Plan {
    constant: bool,
    order: usize,
    k: u32,
    cost: u64,
}

fn plan(samples: &[i64], depth: u32) -> Plan {
    if samples.iter().all(|&s| s == samples[0]) {
        return Plan { constant: true, order: 0, k: 0, cost: 8 + u64::from(depth) };
    }
    (0..=4.min(samples.len() - 1))
        .map(|order| {
            let (k, cost) = best_rice(&residual(samples, order));
            Plan { constant: false, order, k, cost: 8 + 11 + cost + u64::from(depth) * order as u64 }
        })
        .min_by_key(|p| p.cost)
        .unwrap_or(Plan { constant: false, order: 0, k: 0, cost: u64::MAX })
}

fn subframe(bits: &mut Bits, samples: &[i64], depth: u32, plan: &Plan) {
    if plan.constant {
        bits.put(0, 8);
        bits.signed(samples[0], depth);
        return;
    }
    // fixed, of its order; no wasted bits
    bits.put(0b0001_0000 | ((plan.order as u64) << 1), 8);
    for &warm in &samples[..plan.order] {
        bits.signed(warm, depth);
    }
    // Rice with five-bit parameters, one partition
    bits.put(0b01, 2);
    bits.put(0, 4);
    bits.put(u64::from(plan.k), 5);
    for r in residual(samples, plan.order) {
        bits.rice(((r << 1) ^ (r >> 63)) as u64, plan.k);
    }
}

/// `left` and `right` played at `sample_rate`, written to `path` as a FLAC file of `depth`
/// bits, 16 or 24.
pub fn write_flac(path: &Path, sample_rate: u32, left: &[f32], right: &[f32], depth: u32) -> io::Result<()> {
    let depth = if depth >= 24 { 24 } else { 16 };
    let frames = left.len().min(right.len());
    // scaled as decoders scale back, the very top kept a step under
    let scale = (1i64 << (depth - 1)) as f32;
    let top = (1i64 << (depth - 1)) - 1;
    let quantized = |side: &[f32]| -> Vec<i64> {
        side[..frames].iter().map(|&s| ((s.clamp(-1.0, 1.0) * scale).round() as i64).min(top)).collect()
    };
    let (left, right) = (quantized(left), quantized(right));

    let mut out = Bits::new();
    out.bytes.extend(b"fLaC");
    // the one metadata block, last: the stream's information
    out.put(0x80, 8);
    out.put(34, 24);
    out.put(BLOCK as u64, 16);
    out.put(BLOCK as u64, 16);
    out.put(0, 24);
    out.put(0, 24);
    out.put(u64::from(sample_rate), 20);
    out.put(1, 3);
    out.put(u64::from(depth - 1), 5);
    out.put(frames as u64, 36);
    out.put(0, 64);
    out.put(0, 64);

    for (number, start) in (0..frames).step_by(BLOCK).enumerate() {
        let end = (start + BLOCK).min(frames);
        let mut frame = Bits::new();
        frame.put(0b1111_1111_1111_1000, 16);
        let whole = end - start == BLOCK;
        frame.put(if whole { 0b1100 } else { 0b0111 }, 4);
        // the two sides as they are, or one and their difference, or their middle and difference:
        // whichever takes fewest bits
        let (l, r) = (&left[start..end], &right[start..end]);
        let side: Vec<i64> = l.iter().zip(r).map(|(a, b)| a - b).collect();
        let middle: Vec<i64> = l.iter().zip(r).map(|(a, b)| (a + b) >> 1).collect();
        let (pl, pr, ps, pm) = (plan(l, depth), plan(r, depth), plan(&side, depth + 1), plan(&middle, depth));
        let ways = [
            (0b0001, pl.cost + pr.cost),
            (0b1000, pl.cost + ps.cost),
            (0b1001, pr.cost + ps.cost),
            (0b1010, pm.cost + ps.cost),
        ];
        let way = ways.iter().min_by_key(|w| w.1).map_or(0b0001, |w| w.0);
        frame.put(0, 4);
        frame.put(way, 4);
        frame.put(if depth == 24 { 0b110 } else { 0b100 }, 3);
        frame.put(0, 1);
        utf8(&mut frame, number as u64);
        if !whole {
            frame.put((end - start - 1) as u64, 16);
        }
        let header = crc8(&frame.bytes);
        frame.put(u64::from(header), 8);
        match way {
            0b1000 => {
                subframe(&mut frame, l, depth, &pl);
                subframe(&mut frame, &side, depth + 1, &ps);
            }
            0b1001 => {
                subframe(&mut frame, &side, depth + 1, &ps);
                subframe(&mut frame, r, depth, &pr);
            }
            0b1010 => {
                subframe(&mut frame, &middle, depth, &pm);
                subframe(&mut frame, &side, depth + 1, &ps);
            }
            _ => {
                subframe(&mut frame, l, depth, &pl);
                subframe(&mut frame, r, depth, &pr);
            }
        }
        frame.align();
        let footer = crc16(&frame.bytes);
        frame.put(u64::from(footer), 16);
        out.bytes.extend(frame.bytes);
    }
    let mut file = io::BufWriter::new(std::fs::File::create(path)?);
    file.write_all(&out.bytes)?;
    file.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flac_written_reads_back_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        // longer than a block, its last one short; a tone on one side, noise on the other
        let frames = BLOCK * 3 + 1000;
        let left: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.03).sin() * 0.7).collect();
        let mut seed = 1u32;
        let right: Vec<f32> = (0..frames)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect();
        for depth in [16, 24] {
            let path = dir.path().join(format!("sound {depth}.flac"));
            write_flac(&path, 44_100, &left, &right, depth).unwrap();
            let back = crate::decode(&path).unwrap();
            assert_eq!(back.sample_rate, 44_100.0);
            assert_eq!(back.left.len(), frames);
            let step = 1.0 / (1u32 << (depth - 1)) as f32;
            let off =
                |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max);
            assert!(off(&back.left, &left) <= step, "{depth}: {}", off(&back.left, &left));
            assert!(off(&back.right, &right) <= step, "{depth}: {}", off(&back.right, &right));
        }
        let size = std::fs::metadata(dir.path().join("sound 16.flac")).unwrap().len();
        assert!(size < (frames * 4) as u64, "no smaller than a WAV: {size}");
    }

    #[test]
    fn two_sides_alike_take_little_more_than_one() {
        let dir = tempfile::tempdir().unwrap();
        let tone: Vec<f32> =
            (0..BLOCK * 4).map(|i| (i as f32 * 0.021).sin() * 0.6 + (i as f32 * 0.37).sin() * 0.1).collect();
        let near: Vec<f32> = tone.iter().map(|s| s * 0.98).collect();
        let (one, both) = (dir.path().join("one.flac"), dir.path().join("both.flac"));
        write_flac(&one, 48_000, &tone, &vec![0.0; tone.len()], 16).unwrap();
        write_flac(&both, 48_000, &tone, &near, 16).unwrap();
        let size = |p: &Path| std::fs::metadata(p).unwrap().len() as f64;
        assert!(size(&both) < size(&one) * 1.6, "{} {}", size(&one), size(&both));
        let back = crate::decode(&both).unwrap();
        let off = back.right.iter().zip(&near).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(off <= 1.0 / 32768.0, "{off}");
    }

    #[test]
    fn silence_takes_almost_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("silence.flac");
        write_flac(&path, 48_000, &[0.0; 48_000], &[0.0; 48_000], 16).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() < 1000);
        assert_eq!(crate::decode(&path).unwrap().left, vec![0.0; 48_000]);
    }
}
