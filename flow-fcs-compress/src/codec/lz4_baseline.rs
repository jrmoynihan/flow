//! Baseline codec: raw f32 bytes through lz4_flex (block format, no frame).
//!
//! lz4 is the speed-of-light reference — it compresses as fast as memory
//! bandwidth and decompresses faster. Our purpose-built codecs should beat lz4
//! on ratio while staying within an order of magnitude on decode speed.
//!
//! Behind the `lz4-baseline` feature (off by default; pulled in by the bench).

use crate::codec::{ChannelParams, CodecId, ColumnCodec, EncodeStats};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Default)]
pub struct Lz4Block;

impl ColumnCodec for Lz4Block {
    fn id(&self) -> CodecId {
        // Reusing RawNone wire id for now — this codec is bench-only and
        // shouldn't be written to .fcz files. A dedicated id can be added in
        // v0.2 if we ever ship lz4 as a real codec choice.
        CodecId::RawNone
    }

    fn encode_chunk(
        &self,
        input: &[f32],
        _params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats> {
        let bytes = bytemuck::cast_slice::<f32, u8>(input);
        // Frame format carries a length prefix and content checksum — the right
        // shape for "swap in for zstd" comparisons.
        let compressed = lz4_flex::frame::FrameEncoder::new(Vec::new());
        let mut enc = compressed;
        std::io::Write::write_all(&mut enc, bytes)
            .map_err(|e| Error::Zstd(std::io::Error::other(e)))?;
        let payload = enc
            .finish()
            .map_err(|e| Error::Zstd(std::io::Error::other(e)))?;
        let written = payload.len();
        out.extend_from_slice(&payload);
        Ok(EncodeStats {
            input_events: input.len() as u32,
            input_bytes: bytes.len() as u64,
            output_bytes: written as u64,
        })
    }

    fn decode_chunk(
        &self,
        payload: &[u8],
        _params: &ChannelParams,
        out: &mut [f32],
    ) -> Result<()> {
        let mut decoder = lz4_flex::frame::FrameDecoder::new(payload);
        let dst = bytemuck::cast_slice_mut::<f32, u8>(out);
        let mut written = 0usize;
        let mut tmp = [0u8; 4096];
        loop {
            let n = std::io::Read::read(&mut decoder, &mut tmp)
                .map_err(|e| Error::Zstd(std::io::Error::other(e)))?;
            if n == 0 {
                break;
            }
            dst[written..written + n].copy_from_slice(&tmp[..n]);
            written += n;
        }
        if written != dst.len() {
            return Err(Error::LengthMismatch {
                expected: dst.len() / 4,
                actual: written / 4,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lz4_round_trips_smooth() {
        let codec = Lz4Block;
        let p = ChannelParams::linear_unsigned("x", 262_144);
        let input: Vec<f32> = (0..4096).map(|i| (i as f32) * 0.5).collect();
        let mut payload = Vec::new();
        codec.encode_chunk(&input, &p, &mut payload).unwrap();
        let mut out = vec![0.0f32; input.len()];
        codec.decode_chunk(&payload, &p, &mut out).unwrap();
        assert_eq!(out, input);
    }
}
