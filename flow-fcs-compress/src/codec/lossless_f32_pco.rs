//! Mode A alternate backend: pco (Pcodec).
//!
//! pco is a numeric-aware compressor that auto-detects integer-multiple,
//! float-mantissa, and delta patterns. On flow-cytometry channels with
//! biexp/log shape it routinely wins 1.5–2× over BSS+zstd because it can
//! quantize to a discovered base + multiplier per chunk.
//!
//! Tradeoffs vs the always-on `BssZstd`:
//! - Slower decode (~400 MB/s vs 1+ GB/s) — there's an inner Huffman-style step.
//! - Better ratio on noisier / log-shaped data.
//! - Bigger encode dependency surface.
//!
//! Behind the `pco-backend` feature.

use crate::codec::{ChannelParams, CodecId, ColumnCodec, EncodeStats};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct LosslessF32Pco {
    /// pco "compression level" — 0..=12. 8 is the library default.
    pub level: usize,
}

impl Default for LosslessF32Pco {
    fn default() -> Self {
        Self { level: 8 }
    }
}

impl ColumnCodec for LosslessF32Pco {
    fn id(&self) -> CodecId {
        CodecId::LosslessF32Pco
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        _params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        if input.is_empty() {
            return Err(Error::InvalidParams("LosslessF32Pco: empty chunk"));
        }
        let bytes = pco::standalone::simpler_compress(input, self.level)
            .map_err(|e| Error::InvalidParams(pco_err_static(&e)))?;
        let written = bytes.len();
        out.extend_from_slice(&bytes);
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
        let progress = pco::standalone::simple_decompress_into::<f32>(payload, out)
            .map_err(|e| Error::InvalidParams(pco_err_static(&e)))?;
        if progress.n_processed != out.len() {
            return Err(Error::LengthMismatch {
                expected: out.len(),
                actual: progress.n_processed,
            });
        }
        Ok(())
    }
}

// pco's PcoError is non-static; we surface a category string for our error type.
fn pco_err_static(e: &pco::errors::PcoError) -> &'static str {
    use pco::errors::ErrorKind;
    match e.kind {
        ErrorKind::InvalidArgument => "pco: invalid argument",
        ErrorKind::Compatibility => "pco: compatibility / version mismatch",
        ErrorKind::Corruption => "pco: corruption",
        ErrorKind::InsufficientData => "pco: insufficient data",
        ErrorKind::Io(_) => "pco: io",
        _ => "pco: unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_channel(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
            let base = if i % 7 == 0 {
                (u - 0.5) * 50.0
            } else {
                10f32.powf(u * 5.0)
            };
            v.push(base);
        }
        v
    }

    fn p() -> ChannelParams {
        ChannelParams::linear_unsigned("ch", 262_144)
    }

    #[test]
    fn round_trips_log_channel_lossless() {
        let codec = LosslessF32Pco::default();
        let input = log_channel(8192, 42);
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p(), &mut payload).unwrap();
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p(), &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "pco lossless violated");
        }
    }

    #[test]
    fn beats_raw_on_log_data() {
        let codec = LosslessF32Pco::default();
        let input = log_channel(65_536, 1);
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p(), &mut payload).unwrap();
        let raw_bytes = input.len() * 4;
        assert!(
            payload.len() < raw_bytes,
            "pco ({}) failed to beat raw ({})",
            payload.len(),
            raw_bytes
        );
    }
}
