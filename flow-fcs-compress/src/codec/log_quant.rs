//! Mode C: lossy log-domain quantization.
//!
//! Pipeline (encode):
//! 1. `y = asinh(x / cofactor)` per [`crate::transform::asinh`].
//! 2. Compute `y_min` and `y_max` for the chunk.
//! 3. Quantize: `q = round((y - y_min) / step)` where `step = (y_max - y_min) / (2^bits - 1)`.
//! 4. Bitpack `q` into `bits` per value (same packing primitive as Mode B).
//!
//! Decode reverses: unpack `q`, reconstruct `y = y_min + q * step`, then `x = cofactor * sinh(y)`.
//!
//! ## Loss model
//!
//! Quantization step in `y`-space translates to a roughly *relative* error in
//! `x`-space for large `|x|` (where asinh ≈ log) and a *near-absolute* error
//! near zero (where asinh ≈ identity / cofactor). The codec is therefore
//! "log-relative" — a property practitioners want for fluorescence data
//! displayed on log axes.
//!
//! ## Configuration
//!
//! [`LogQuantizationConfig`] exposes two knobs:
//! - `cofactor` — passed straight to `asinh`. Default 150 (Cytek-style).
//! - `bits` — 4..=24. Lower = smaller / lossier. Default 16.
//!
//! ## Per-chunk header (24 bytes)
//!
//! ```text
//! [cofactor   f32 4B]
//! [y_min      f32 4B]
//! [y_step     f32 4B]   (0.0 if all values quantize to the same level)
//! [n_values   u32 4B]
//! [bits       u8  1B]
//! [reserved   u8  1B]
//! [pad        u16 2B]
//! [packed bytes...]    when bits > 0
//! ```

use byteorder::{ByteOrder, LittleEndian};

use crate::codec::{ChannelParams, CodecId, ColumnCodec, EncodeStats};
use crate::error::{Error, Result};
use crate::transform::asinh::{DEFAULT_COFACTOR, forward, inverse};

const HEADER_BYTES: usize = 4 + 4 + 4 + 4 + 1 + 1 + 2;

/// Tunable parameters for the lossy log-quant codec.
#[derive(Debug, Clone, Copy)]
pub struct LogQuantizationConfig {
    pub cofactor: f32,
    pub bits: u8,
}

impl Default for LogQuantizationConfig {
    fn default() -> Self {
        Self {
            cofactor: DEFAULT_COFACTOR,
            bits: 16,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LogQuantization {
    pub cfg: LogQuantizationConfig,
}

impl Default for LogQuantization {
    fn default() -> Self {
        Self {
            cfg: LogQuantizationConfig::default(),
        }
    }
}

impl LogQuantization {
    pub fn new(cfg: LogQuantizationConfig) -> Self {
        Self { cfg }
    }
}

impl ColumnCodec for LogQuantization {
    fn id(&self) -> CodecId {
        CodecId::LogQuantization
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        _params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        if input.is_empty() {
            return Err(Error::InvalidParams("LogQuantization: empty chunk"));
        }
        if !(4..=24).contains(&self.cfg.bits) {
            return Err(Error::InvalidParams("LogQuantization: bits must be in 4..=24"));
        }
        if !self.cfg.cofactor.is_finite() || self.cfg.cofactor <= 0.0 {
            return Err(Error::InvalidParams(
                "LogQuantization: cofactor must be finite and > 0",
            ));
        }

        // Pass 1: compute y values + extrema.
        let mut ys: Vec<f32> = Vec::with_capacity(input.len());
        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;
        for &x in input {
            if !x.is_finite() {
                return Err(Error::InvalidParams(
                    "LogQuantization: encountered NaN or infinite input",
                ));
            }
            let y = forward(x, self.cfg.cofactor);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
            ys.push(y);
        }

        let bits = self.cfg.bits;
        let levels = (1u32 << bits) - 1;
        let span = y_max - y_min;
        let step = if span <= 0.0 {
            0.0
        } else {
            span / levels as f32
        };

        // Header
        let header_start = out.len();
        out.resize(header_start + HEADER_BYTES, 0);
        {
            let h = &mut out[header_start..header_start + HEADER_BYTES];
            LittleEndian::write_f32(&mut h[0..4], self.cfg.cofactor);
            LittleEndian::write_f32(&mut h[4..8], y_min);
            LittleEndian::write_f32(&mut h[8..12], step);
            LittleEndian::write_u32(&mut h[12..16], input.len() as u32);
            h[16] = bits;
            h[17] = 0;
            // pad bytes already zero
        }

        // Payload
        if step > 0.0 {
            let mask = if bits == 32 {
                u32::MAX
            } else {
                (1u32 << bits) - 1
            };
            let mut staged: Vec<u32> = Vec::with_capacity(ys.len());
            for &y in &ys {
                let q = ((y - y_min) / step).round();
                let q_clamped = q.clamp(0.0, levels as f32) as u32 & mask;
                staged.push(q_clamped);
            }
            pack_bits_fast(&staged, bits, out);
        }

        let written = out.len() - header_start;
        Ok(EncodeStats {
            input_events: input.len() as u32,
            input_bytes: (input.len() * 4) as u64,
            output_bytes: written as u64,
        })
    }

    fn decode_chunk(
        &self,
        payload: &[u8],
        _params: &ChannelParams,
        out: &mut [f32],
    ) -> Result<()> {
        if payload.len() < HEADER_BYTES {
            return Err(Error::Truncated {
                needed: HEADER_BYTES,
                have: payload.len(),
            });
        }
        let cofactor = LittleEndian::read_f32(&payload[0..4]);
        let y_min = LittleEndian::read_f32(&payload[4..8]);
        let step = LittleEndian::read_f32(&payload[8..12]);
        let n_values = LittleEndian::read_u32(&payload[12..16]) as usize;
        let bits = payload[16];

        if !cofactor.is_finite() || cofactor <= 0.0 {
            return Err(Error::InvalidParams(
                "LogQuantization: payload cofactor invalid",
            ));
        }
        if !(4..=24).contains(&bits) {
            return Err(Error::InvalidParams("LogQuantization: payload bits out of range"));
        }
        if out.len() != n_values {
            return Err(Error::LengthMismatch {
                expected: n_values,
                actual: out.len(),
            });
        }

        if step <= 0.0 {
            // Constant chunk
            let x = inverse(y_min, cofactor);
            for slot in out.iter_mut() {
                *slot = x;
            }
            return Ok(());
        }

        let total_bits = n_values * bits as usize;
        let needed = HEADER_BYTES + total_bits.div_ceil(8);
        if payload.len() < needed {
            return Err(Error::Truncated {
                needed,
                have: payload.len(),
            });
        }
        let packed = &payload[HEADER_BYTES..];

        // SIMD-style optimization: bulk-unpack quantization codes into a u32
        // staging buffer, then dequantize. Two-pass form lets the compiler
        // auto-vectorize the dequant arithmetic. For small bit widths (≤14)
        // we additionally precompute a `cofactor * sinh(y_min + i*step)` LUT,
        // skipping per-value sinh calls and yielding a 4–10× decode speedup.
        let mut staging: Vec<u32> = vec![0; n_values];
        unpack_bits_fast(packed, bits, n_values, &mut staging);

        if bits <= 14 {
            let levels = (1usize << bits).min(1 << 14);
            let mut lut: Vec<f32> = Vec::with_capacity(levels);
            for i in 0..levels {
                let y = y_min + (i as f32) * step;
                lut.push(inverse(y, cofactor));
            }
            for (slot, &q) in out.iter_mut().zip(staging.iter()) {
                let idx = (q as usize).min(levels - 1);
                *slot = lut[idx];
            }
        } else {
            for (slot, &q) in out.iter_mut().zip(staging.iter()) {
                let y = y_min + (q as f32) * step;
                *slot = inverse(y, cofactor);
            }
        }
        Ok(())
    }
}

/// Pack u32 values into a contiguous LE bit-stream of `width`-bit fields.
/// Bit-reservoir form, mirrors the packer in `adc_bitpack.rs`.
fn pack_bits_fast(values: &[u32], width: u8, dst: &mut Vec<u8>) {
    if width == 0 {
        return;
    }
    let mask = if width >= 32 { u32::MAX } else { (1u32 << width) - 1 };
    let mut buf: u64 = 0;
    let mut buf_bits: u32 = 0;
    for &v in values {
        let masked = (v & mask) as u64;
        buf |= masked << buf_bits;
        buf_bits += width as u32;
        if buf_bits >= 32 {
            let four = (buf & 0xFFFF_FFFF) as u32;
            dst.extend_from_slice(&four.to_le_bytes());
            buf >>= 32;
            buf_bits -= 32;
        }
    }
    while buf_bits >= 8 {
        dst.push((buf & 0xFF) as u8);
        buf >>= 8;
        buf_bits -= 8;
    }
    if buf_bits > 0 {
        dst.push((buf & 0xFF) as u8);
    }
}

/// Inverse of [`pack_bits_fast`].
#[inline]
fn unpack_bits_fast(src: &[u8], width: u8, n: usize, out: &mut [u32]) {
    if width == 0 {
        for slot in out.iter_mut().take(n) {
            *slot = 0;
        }
        return;
    }
    let mask = if width >= 32 {
        u32::MAX as u64
    } else {
        (1u64 << width) - 1
    };
    let mut buf: u64 = 0;
    let mut buf_bits: u32 = 0;
    let mut src_pos = 0usize;
    let bytes_avail = src.len();
    for slot in out.iter_mut().take(n) {
        while buf_bits < width as u32 {
            if src_pos + 4 <= bytes_avail && buf_bits + 32 <= 64 {
                let four = u32::from_le_bytes([
                    src[src_pos],
                    src[src_pos + 1],
                    src[src_pos + 2],
                    src[src_pos + 3],
                ]);
                buf |= (four as u64) << buf_bits;
                buf_bits += 32;
                src_pos += 4;
            } else if src_pos < bytes_avail {
                buf |= (src[src_pos] as u64) << buf_bits;
                buf_bits += 8;
                src_pos += 1;
            } else {
                break;
            }
        }
        *slot = (buf & mask) as u32;
        buf >>= width;
        buf_bits = buf_bits.saturating_sub(width as u32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_log_channel(n: usize, seed: u64) -> Vec<f32> {
        // Mix of small noise around zero and decade-spread positive values —
        // shaped like a typical fluorescence channel.
        let mut s = seed;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
            let base = if i % 5 == 0 {
                (u - 0.5) * 50.0
            } else {
                10f32.powf(u * 5.0)
            };
            v.push(base);
        }
        v
    }

    fn params() -> ChannelParams {
        ChannelParams {
            name: "fluo".into(),
            stored_bits: 32,
            range: 262_144,
            log_decades: (5.0, 0.0),
            adc_bits: None,
            signed: true,
        }
    }

    #[test]
    fn round_trip_within_tolerance_at_16_bits() {
        let codec = LogQuantization::default();
        let p = params();
        let input = synth_log_channel(8192, 42);

        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut payload).unwrap();
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p, &mut out).unwrap();

        let mut max_rel = 0f32;
        for (a, b) in input.iter().zip(out.iter()) {
            if a.abs() > 100.0 {
                max_rel = max_rel.max(((a - b).abs()) / a.abs());
            }
        }
        // 16-bit log-quant should give well under 0.1% relative error on log-scale data.
        assert!(max_rel < 1e-3, "max rel err = {max_rel}");
    }

    #[test]
    fn smaller_bits_smaller_payload() {
        let p = params();
        let input = synth_log_channel(4096, 7);

        let mut p16 = Vec::new();
        LogQuantization::new(LogQuantizationConfig {
            cofactor: 150.0,
            bits: 16,
        })
        .encode_chunk(&input, &p, &mut p16)
        .unwrap();

        let mut p8 = Vec::new();
        LogQuantization::new(LogQuantizationConfig {
            cofactor: 150.0,
            bits: 8,
        })
        .encode_chunk(&input, &p, &mut p8)
        .unwrap();

        assert!(
            p8.len() < p16.len(),
            "8-bit payload ({}) should be smaller than 16-bit ({})",
            p8.len(),
            p16.len()
        );
    }

    #[test]
    fn beats_raw_f32_on_log_data() {
        let p = params();
        let input = synth_log_channel(65_536, 1);
        let raw_bytes = input.len() * 4;

        // 16-bit Mode C: ~16 bits/value = 50% of raw f32 (header overhead pushes
        // it just over 50%, so test with a looser bound).
        let mut p16 = Vec::new();
        LogQuantization::default().encode_chunk(&input, &p, &mut p16).unwrap();
        assert!(
            p16.len() < raw_bytes,
            "16-bit Mode C ({}) should be smaller than raw f32 ({})",
            p16.len(),
            raw_bytes
        );

        // 12-bit Mode C: 12/32 = 37.5% of raw f32, decisively smaller.
        let mut p12 = Vec::new();
        LogQuantization::new(LogQuantizationConfig {
            cofactor: 150.0,
            bits: 12,
        })
        .encode_chunk(&input, &p, &mut p12)
        .unwrap();
        assert!(
            p12.len() * 2 < raw_bytes,
            "12-bit Mode C ({}) should be < 50% of raw f32 ({})",
            p12.len(),
            raw_bytes
        );
    }

    #[test]
    fn rejects_invalid_bits() {
        let codec = LogQuantization::new(LogQuantizationConfig {
            cofactor: 150.0,
            bits: 2,
        });
        let p = params();
        let input = vec![1.0f32; 64];
        let mut payload = Vec::new();
        let err = codec.encode_chunk(&input, &p, &mut payload).unwrap_err();
        assert!(matches!(err, Error::InvalidParams(_)));
    }

    #[test]
    fn rejects_nan() {
        let codec = LogQuantization::default();
        let p = params();
        let mut input = synth_log_channel(64, 1);
        input[5] = f32::NAN;
        let mut payload = Vec::new();
        let err = codec.encode_chunk(&input, &p, &mut payload).unwrap_err();
        assert!(matches!(err, Error::InvalidParams(_)));
    }

    #[test]
    fn constant_chunk_roundtrips() {
        let codec = LogQuantization::default();
        let p = params();
        let input = vec![137.5f32; 256];
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut payload).unwrap();
        // Constant chunk → header only, step = 0.
        assert_eq!(payload.len(), HEADER_BYTES);

        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p, &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-3, "{} vs {}", a, b);
        }
    }
}
