//! Mode A: lossless f32 codecs.
//!
//! The default backend is byte-stream-split followed by zstd. Rationale: unmixed
//! flow cytometry channels carry full f32 entropy in the mantissa (so dictionary
//! coders like ALP underperform), but the exponent and high-mantissa byte planes
//! have very narrow distributions across a single channel. Splitting into byte
//! planes lets zstd's match-finder exploit that narrowness directly.

use std::io::{Cursor, Write};

use crate::codec::{ChannelParams, CodecId, ColumnCodec, EncodeStats};
use crate::error::Result;
use crate::transform::byte_stream_split::{split_f32_le, unsplit_f32_le};

/// Mode A default codec: byte-stream-split + zstd.
#[derive(Debug, Clone)]
pub struct BssZstd {
    pub level: i32,
}

impl Default for BssZstd {
    fn default() -> Self {
        Self { level: 3 }
    }
}

impl BssZstd {
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl ColumnCodec for BssZstd {
    fn id(&self) -> CodecId {
        CodecId::LosslessF32BssZstd
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        _params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        let mut planes = Vec::with_capacity(input.len() * 4);
        split_f32_le(input, &mut planes);

        let start = out.len();
        let mut encoder = zstd::Encoder::new(out, self.level)?;
        encoder.write_all(&planes)?;
        let out = encoder.finish()?;

        Ok(EncodeStats {
            input_events: input.len() as u32,
            input_bytes: (input.len() * 4) as u64,
            output_bytes: (out.len() - start) as u64,
        })
    }

    fn decode_chunk(
        &self,
        payload: &[u8],
        _params: &ChannelParams,
        out: &mut [f32],
    ) -> Result<()> {
        let n = out.len();
        let mut planes = Vec::with_capacity(n * 4);
        let mut decoder = zstd::Decoder::new(Cursor::new(payload))?;
        std::io::copy(&mut decoder, &mut planes)?;
        unsplit_f32_le(&planes, out);
        Ok(())
    }
}

/// Baseline: raw f32 LE bytes through zstd. Useful for ratio comparisons and as
/// a fallback when a channel turns out to compress better without BSS.
#[derive(Debug, Clone)]
pub struct RawZstd {
    pub level: i32,
}

impl Default for RawZstd {
    fn default() -> Self {
        Self { level: 3 }
    }
}

impl ColumnCodec for RawZstd {
    fn id(&self) -> CodecId {
        CodecId::RawZstd
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        _params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        let bytes = bytemuck::cast_slice::<f32, u8>(input);
        let start = out.len();
        let mut encoder = zstd::Encoder::new(out, self.level)?;
        encoder.write_all(bytes)?;
        let out = encoder.finish()?;
        Ok(EncodeStats {
            input_events: input.len() as u32,
            input_bytes: bytes.len() as u64,
            output_bytes: (out.len() - start) as u64,
        })
    }

    fn decode_chunk(
        &self,
        payload: &[u8],
        _params: &ChannelParams,
        out: &mut [f32],
    ) -> Result<()> {
        let dst = bytemuck::cast_slice_mut::<f32, u8>(out);
        let mut decoder = zstd::Decoder::new(Cursor::new(payload))?;
        let mut written = 0;
        let mut tmp = [0u8; 4096];
        loop {
            let n = std::io::Read::read(&mut decoder, &mut tmp)?;
            if n == 0 {
                break;
            }
            dst[written..written + n].copy_from_slice(&tmp[..n]);
            written += n;
        }
        Ok(())
    }
}

/// Baseline: raw f32 LE bytes, no compression. Useful for round-trip sanity tests.
#[derive(Debug, Clone, Default)]
pub struct RawNone;

impl ColumnCodec for RawNone {
    fn id(&self) -> CodecId {
        CodecId::RawNone
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        _params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        let bytes = bytemuck::cast_slice::<f32, u8>(input);
        out.extend_from_slice(bytes);
        Ok(EncodeStats {
            input_events: input.len() as u32,
            input_bytes: bytes.len() as u64,
            output_bytes: bytes.len() as u64,
        })
    }

    fn decode_chunk(
        &self,
        payload: &[u8],
        _params: &ChannelParams,
        out: &mut [f32],
    ) -> Result<()> {
        let dst = bytemuck::cast_slice_mut::<f32, u8>(out);
        dst.copy_from_slice(payload);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_params() -> ChannelParams {
        ChannelParams::linear_unsigned("FSC-A", 262_144)
    }

    fn synthesize_channel(n: usize, seed: u64) -> Vec<f32> {
        // mix of small positives, near-zero noise, and a few outliers — typical
        // post-compensation channel shape.
        let mut x = Vec::with_capacity(n);
        let mut s = seed;
        for i in 0..n {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
            let base = (i as f32) * 0.001;
            let noise = (u - 0.5) * 50.0;
            let outlier = if i % 997 == 0 { 100_000.0 } else { 0.0 };
            x.push(base + noise + outlier);
        }
        x
    }

    #[test]
    fn bss_zstd_roundtrips() {
        let codec = BssZstd::default();
        let params = linear_params();
        let input = synthesize_channel(8192, 0xCAFEBABE);

        let mut payload = Vec::new();
        let stats = codec.encode_chunk(&input, &params, &mut payload).unwrap();
        assert_eq!(stats.input_events, 8192);
        assert!(stats.output_bytes > 0);

        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &params, &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "lossless requirement violated");
        }
    }

    #[test]
    fn raw_zstd_roundtrips() {
        let codec = RawZstd::default();
        let params = linear_params();
        let input = synthesize_channel(4096, 0xBADF00D);

        let mut payload = Vec::new();
        codec.encode_chunk(&input, &params, &mut payload).unwrap();
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &params, &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn raw_none_roundtrips() {
        let codec = RawNone;
        let params = linear_params();
        let input = synthesize_channel(1024, 1);
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &params, &mut payload).unwrap();
        assert_eq!(payload.len(), input.len() * 4);
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &params, &mut out).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn bss_beats_raw_on_smooth_data() {
        let params = linear_params();
        // smooth ramp — BSS should compress dramatically better than zstd-on-interleaved
        let input: Vec<f32> = (0..8192).map(|i| (i as f32) * 0.25).collect();

        let mut bss = Vec::new();
        BssZstd::default().encode_chunk(&input, &params, &mut bss).unwrap();
        let mut raw = Vec::new();
        RawZstd::default().encode_chunk(&input, &params, &mut raw).unwrap();

        assert!(
            bss.len() < raw.len(),
            "BSS+zstd ({} bytes) should beat raw+zstd ({} bytes) on smooth data",
            bss.len(),
            raw.len(),
        );
    }
}
