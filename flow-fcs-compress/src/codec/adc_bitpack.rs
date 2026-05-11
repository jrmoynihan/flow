//! Mode B: ADC-bit lossless codec.
//!
//! For raw spectral data, an FCS file's `$PnB` / `$PnR` keywords describe a
//! finite-resolution ADC (typically 14–22 bits on modern instruments). The
//! float values stored in the data segment are exact integer-valued multiples
//! of `scale = $PnR / 2^bits`. Storing them as full f32 wastes ~10 bits per
//! value on noise; quantizing back to the ADC integer is lossless w.r.t. the
//! physical signal.
//!
//! ## Per-chunk encoding
//!
//! 1. Determine `bits` (caller-supplied via [`ChannelParams::adc_bits`], else
//!    `ceil(log2(range))`).
//! 2. Compute `scale = range as f64 / (1 << bits) as f64` (kept in `f64` for
//!    precision when packing back into an `f32`).
//! 3. For every value `x`: `q = round(x / scale) as i64`, accumulate `min` and
//!    `max`.
//! 4. Choose `bits_used = max(1, ceil(log2(max - min + 1)))`. Bias each `q` by
//!    `min` to get a non-negative `u` that fits in `bits_used`.
//! 5. Pack the `u`-stream little-endian-bit-first with width `bits_used`.
//!
//! ## Per-chunk header
//!
//! ```text
//! [scale_f32: f32  4B] — the per-channel decode scale (lossy stash of f64 → f32)
//! [offset:    i64  8B] — the bias subtracted at encode time
//! [n_values:  u32  4B] — number of f32 samples encoded
//! [bits_used: u8   1B] — packing width in bits (0..=32; 0 = constant chunk)
//! [reserved:  u8   1B]
//! [packed bytes...]    — present only when bits_used > 0
//! ```
//!
//! ## Negatives
//!
//! Post-compensation channels routinely take negative values. The `offset`
//! field is `i64` so we can carry an arbitrary positive or negative bias
//! without changing the wire layout.

use byteorder::{ByteOrder, LittleEndian};

use crate::codec::{ChannelParams, CodecId, ColumnCodec, EncodeStats};
use crate::error::{Error, Result};

const HEADER_BYTES: usize = 4 + 8 + 4 + 1 + 1; // 18

/// Mode B codec.
#[derive(Debug, Clone, Default)]
pub struct AdcBitpack;

impl ColumnCodec for AdcBitpack {
    fn id(&self) -> CodecId {
        CodecId::AdcBitpack
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        if input.is_empty() {
            return Err(Error::InvalidParams("AdcBitpack: empty chunk"));
        }
        if params.range == 0 {
            return Err(Error::InvalidParams(
                "AdcBitpack: $PnR must be > 0 (use Mode A for unbounded data)",
            ));
        }

        let adc_bits = effective_adc_bits(params)?;
        let scale = scale_from(params.range, adc_bits);
        if !scale.is_finite() || scale == 0.0 {
            return Err(Error::InvalidParams(
                "AdcBitpack: derived scale is non-finite or zero",
            ));
        }

        // Pass 1: quantize, track min/max.
        let mut quantized: Vec<i64> = Vec::with_capacity(input.len());
        let mut q_min = i64::MAX;
        let mut q_max = i64::MIN;
        for &x in input {
            if !x.is_finite() {
                return Err(Error::InvalidParams(
                    "AdcBitpack: encountered NaN or infinite value",
                ));
            }
            let q = (x as f64 / scale).round() as i64;
            q_min = q_min.min(q);
            q_max = q_max.max(q);
            quantized.push(q);
        }

        // Pass 2: pack with bits_used wide enough for the range.
        let span = (q_max - q_min) as u128;
        let bits_used = if span == 0 {
            0u8
        } else {
            // ceil(log2(span + 1))
            ((128 - (span).leading_zeros()) as u8).clamp(1, 32)
        };

        let header_start = out.len();
        out.resize(header_start + HEADER_BYTES, 0);
        {
            let h = &mut out[header_start..header_start + HEADER_BYTES];
            LittleEndian::write_f32(&mut h[0..4], scale as f32);
            LittleEndian::write_i64(&mut h[4..12], q_min);
            LittleEndian::write_u32(&mut h[12..16], input.len() as u32);
            h[16] = bits_used;
            h[17] = 0; // reserved
        }

        if bits_used > 0 {
            let mask = if bits_used == 32 {
                u32::MAX
            } else {
                (1u32 << bits_used) - 1
            };
            // Stage biased u32 values, then bulk-pack via the bit-reservoir
            // packer. Reusing the buffer would matter at the edges (millions
            // of chunks/sec); not the M5 bottleneck.
            let mut staged: Vec<u32> = Vec::with_capacity(quantized.len());
            for q in &quantized {
                staged.push((*q - q_min) as u32 & mask);
            }
            pack_bits_fast(&staged, bits_used, out);
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
        let scale = LittleEndian::read_f32(&payload[0..4]) as f64;
        let offset = LittleEndian::read_i64(&payload[4..12]);
        let n_values = LittleEndian::read_u32(&payload[12..16]) as usize;
        let bits_used = payload[16];

        if out.len() != n_values {
            return Err(Error::LengthMismatch {
                expected: n_values,
                actual: out.len(),
            });
        }

        if bits_used == 0 {
            // Constant chunk: every value decodes to offset * scale.
            let v = (offset as f64 * scale) as f32;
            for slot in out.iter_mut() {
                *slot = v;
            }
            return Ok(());
        }
        if bits_used > 32 {
            return Err(Error::InvalidParams(
                "AdcBitpack: bits_used > 32 is invalid",
            ));
        }

        let total_bits = n_values * bits_used as usize;
        let needed = HEADER_BYTES + total_bits.div_ceil(8);
        if payload.len() < needed {
            return Err(Error::Truncated {
                needed,
                have: payload.len(),
            });
        }
        let packed = &payload[HEADER_BYTES..];
        // Two-pass: bulk bit-unpack into a u32 staging buffer, then dequantize.
        // Splitting the loops gives the compiler a clean shot at vectorizing the
        // dequant pass (purely arithmetic, no data-dependent branches).
        let mut staging: Vec<u32> = vec![0; n_values];
        unpack_bits_fast(packed, bits_used, n_values, &mut staging);
        for (slot, q_biased) in out.iter_mut().zip(staging.iter()) {
            let q = *q_biased as i64 + offset;
            *slot = (q as f64 * scale) as f32;
        }
        Ok(())
    }
}

fn effective_adc_bits(params: &ChannelParams) -> Result<u8> {
    if let Some(b) = params.adc_bits {
        if !(1..=32).contains(&b) {
            return Err(Error::InvalidParams(
                "AdcBitpack: adc_bits must be in 1..=32",
            ));
        }
        return Ok(b);
    }
    // Fallback: derive from $PnR. range = max value + 1, so bits = ceil(log2(range)).
    let range = params.range as u64;
    if range <= 1 {
        return Ok(1);
    }
    let bits = 64 - (range - 1).leading_zeros();
    Ok(bits.clamp(1, 32) as u8)
}

fn scale_from(range: u32, adc_bits: u8) -> f64 {
    // FCS convention: $PnR = max + 1, ADC integer in [0, 2^bits).
    // scale = range / 2^bits.
    let denom = if adc_bits >= 64 {
        f64::INFINITY
    } else {
        (1u64 << adc_bits) as f64
    };
    range as f64 / denom
}

/// Pack `values` into a contiguous LE bit-stream of `width`-bit fields.
///
/// Bit-reservoir form: a `u64` accumulator gathers up to 64 bits, then flushes
/// 8 bytes at a time using an unaligned LE store. For typical widths (16..24)
/// this is roughly an order of magnitude faster than the per-bit loop because
/// the inner loop body has no data-dependent branches and amortizes bookkeeping
/// across 4–8 values per iteration.
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
            // Flush 4 bytes (we know we have ≥ 32 bits buffered).
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

/// Inverse of [`pack_bits_fast`]. Reads `n` values of `width` bits each and
/// writes them into `out`. `src` may be padded with arbitrary garbage past the
/// last meaningful byte.
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
        // Refill: pull as much as we can in one u64 read.
        while buf_bits < width as u32 {
            // Try a 4-byte refill first; fall back to byte-at-a-time near the
            // tail. The tail loop matters because the encoder only emits whole
            // bytes once the reservoir has ≥ 8 bits remaining.
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

    fn integer_channel(range: u32, adc_bits: u8, n: usize, seed: u64) -> Vec<f32> {
        // Generate exact integer-valued floats in [0, range) at ADC granularity.
        let scale = scale_from(range, adc_bits);
        let mut v = Vec::with_capacity(n);
        let mut s = seed;
        for _ in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let q = (s as u64) % (1u64 << adc_bits);
            v.push((q as f64 * scale) as f32);
        }
        v
    }

    fn signed_integer_channel(range: u32, adc_bits: u8, n: usize, seed: u64) -> Vec<f32> {
        // Like integer_channel but biased to include negatives (post-comp).
        let scale = scale_from(range, adc_bits);
        let mut v = Vec::with_capacity(n);
        let mut s = seed;
        for _ in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let q = ((s as i64) % (1i64 << adc_bits)) - (1i64 << (adc_bits - 1));
            v.push((q as f64 * scale) as f32);
        }
        v
    }

    fn params(range: u32, adc_bits: u8, signed: bool) -> ChannelParams {
        ChannelParams {
            name: "test".into(),
            stored_bits: 32,
            range,
            log_decades: (0.0, 0.0),
            adc_bits: Some(adc_bits),
            signed,
        }
    }

    #[test]
    fn round_trips_22_bit_unsigned() {
        let p = params(1 << 22, 22, false);
        let input = integer_channel(p.range, 22, 4096, 42);
        let codec = AdcBitpack::default();

        let mut payload = Vec::new();
        let stats = codec.encode_chunk(&input, &p, &mut payload).unwrap();
        assert_eq!(stats.input_events, 4096);

        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p, &mut out).unwrap();
        // Lossless w.r.t. ADC: round-trip should be bit-exact for integer values.
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "lossless ADC round-trip violated");
        }
    }

    #[test]
    fn round_trips_18_bit_signed_with_negatives() {
        let p = params(1 << 18, 18, true);
        let input = signed_integer_channel(p.range, 18, 2048, 7);
        // Sanity: at least one negative present.
        assert!(input.iter().any(|x| *x < 0.0));

        let codec = AdcBitpack::default();
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut payload).unwrap();
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p, &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn constant_chunk_uses_zero_bits() {
        let p = params(262_144, 18, false);
        let input = vec![1024.0f32; 1024];

        let codec = AdcBitpack::default();
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut payload).unwrap();
        // Header only: bits_used = 0.
        assert_eq!(payload.len(), HEADER_BYTES);
        assert_eq!(payload[16], 0);

        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p, &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn ratio_beats_raw_on_22_bit_data() {
        let p = params(1 << 22, 22, false);
        let input = integer_channel(p.range, 22, 65536, 99);

        let codec = AdcBitpack::default();
        let mut adc_payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut adc_payload).unwrap();

        // Mode B should beat raw f32 (4 bytes/event) by close to 22/32 = 31%.
        let raw_bytes = input.len() * 4;
        assert!(
            adc_payload.len() < raw_bytes,
            "Mode B ({}) failed to beat raw f32 ({})",
            adc_payload.len(),
            raw_bytes
        );
        // Loose lower bound: should be less than ~75% of raw.
        assert!(
            adc_payload.len() * 4 < raw_bytes * 3,
            "Mode B compression unexpectedly poor: {} vs raw {}",
            adc_payload.len(),
            raw_bytes
        );
    }

    #[test]
    fn rejects_nan() {
        let p = params(1 << 22, 22, false);
        let mut input = integer_channel(p.range, 22, 16, 1);
        input[3] = f32::NAN;
        let codec = AdcBitpack::default();
        let mut out = Vec::new();
        let err = codec.encode_chunk(&input, &p, &mut out).unwrap_err();
        assert!(matches!(err, Error::InvalidParams(_)));
    }

    #[test]
    fn truncated_payload_detected() {
        let p = params(1 << 22, 22, false);
        let input = integer_channel(p.range, 22, 256, 5);
        let codec = AdcBitpack::default();
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut payload).unwrap();
        payload.truncate(HEADER_BYTES + 1);
        let mut out = vec![0.0f32; input.len()];
        let err = codec.decode_chunk(&payload, &p, &mut out).unwrap_err();
        assert!(matches!(err, Error::Truncated { .. }));
    }
}
