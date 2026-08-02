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
//!
//! # Configuration
//!
//! Encoding uses pco's full [`ChunkConfig`]. Defaults match pco's own defaults
//! (compression level 8, auto mode/delta detection, equal pages up to
//! [`pco::DEFAULT_MAX_PAGE_N`]). Callers can override any field; see the
//! re-exported [`ModeSpec`], [`DeltaSpec`], and [`PagingSpec`] types.
//!
//! `enable_8_bit` is irrelevant for this f32 codec and should be left at the
//! default (`false`).

use crate::codec::{ChannelParams, CodecId, ColumnCodec, EncodeStats};
use crate::error::{Error, Result};

pub use pco::{ChunkConfig, DeltaSpec, ModeSpec, PagingSpec};

/// Mode A alternate codec: lossless f32 via pco.
///
/// Holds a full [`ChunkConfig`] so every pco encode knob is available. Prefer
/// [`LosslessF32Pco::default`] or [`LosslessF32Pco::with_compression_level`]
/// unless you need a specific [`ModeSpec`] / [`DeltaSpec`] / [`PagingSpec`].
#[derive(Debug, Clone)]
pub struct LosslessF32Pco {
    pub config: ChunkConfig,
}

impl Default for LosslessF32Pco {
    fn default() -> Self {
        Self {
            config: ChunkConfig::default(),
        }
    }
}

impl LosslessF32Pco {
    /// Build from an explicit pco [`ChunkConfig`].
    pub fn new(config: ChunkConfig) -> Self {
        Self { config }
    }

    /// Convenience: pco defaults with only `compression_level` overridden
    /// (valid range 0..=12; library default is 8).
    pub fn with_compression_level(level: usize) -> Self {
        Self {
            config: ChunkConfig::default().with_compression_level(level),
        }
    }

    /// Convenience: pco defaults with only `mode_spec` overridden.
    pub fn with_mode_spec(mode_spec: ModeSpec) -> Self {
        Self {
            config: ChunkConfig::default().with_mode_spec(mode_spec),
        }
    }

    /// Convenience: pco defaults with only `delta_spec` overridden.
    pub fn with_delta_spec(delta_spec: DeltaSpec) -> Self {
        Self {
            config: ChunkConfig::default().with_delta_spec(delta_spec),
        }
    }

    /// Convenience: pco defaults with only `paging_spec` overridden.
    pub fn with_paging_spec(paging_spec: PagingSpec) -> Self {
        Self {
            config: ChunkConfig::default().with_paging_spec(paging_spec),
        }
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
        let bytes = pco::standalone::simple_compress(input, &self.config)
            .map_err(|e| Error::InvalidParams(pco_err_static(&e)))?;
        let written = bytes.len();
        out.extend_from_slice(&bytes);
        Ok(EncodeStats {
            input_events: input.len() as u32,
            input_bytes: (input.len() * 4) as u64,
            output_bytes: written as u64,
        })
    }

    fn decode_chunk(&self, payload: &[u8], _params: &ChannelParams, out: &mut [f32]) -> Result<()> {
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
        ErrorKind::Corruption => "pco: corruption",
        ErrorKind::InsufficientData => "pco: insufficient data",
        ErrorKind::Io(_) => "pco: IO error",
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

    fn assert_bit_exact_round_trip(codec: &LosslessF32Pco, input: &[f32]) {
        let mut payload = Vec::new();
        codec.encode_chunk(input, &p(), &mut payload).unwrap();
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p(), &mut out).unwrap();
        for (a, b) in input.iter().zip(out.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "pco lossless violated");
        }
    }

    #[test]
    fn round_trips_log_channel_lossless() {
        assert_bit_exact_round_trip(&LosslessF32Pco::default(), &log_channel(8192, 42));
    }

    #[test]
    fn round_trips_with_custom_chunk_config() {
        let codec = LosslessF32Pco::new(
            ChunkConfig::default()
                .with_compression_level(4)
                .with_mode_spec(ModeSpec::Classic)
                .with_delta_spec(DeltaSpec::NoOp),
        );
        assert_bit_exact_round_trip(&codec, &log_channel(4096, 7));
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
