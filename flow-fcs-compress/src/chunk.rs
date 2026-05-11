//! Chunk header and on-disk framing primitives.
//!
//! ```text
//! [u16  codec_id    ]   stable wire id from CodecId::to_wire()
//! [u32  payload_len ]   length of the codec payload that follows the header
//! [u32  decoded_len ]   number of f32 events the payload decodes to
//! [u64  checksum    ]   xxh3-64 of the payload bytes
//! ```
//!
//! The header is little-endian. Total size: [`CHUNK_HEADER_BYTES`] (= 18) bytes.

use byteorder::{ByteOrder, LittleEndian};

use crate::codec::CodecId;
use crate::error::{Error, Result};

/// Default events per chunk. ~7.5 MB for 30 channels of f32, L2-friendly.
pub const DEFAULT_CHUNK_EVENTS: u32 = 65_536;

/// Size in bytes of [`ChunkHeader`] when serialized.
pub const CHUNK_HEADER_BYTES: usize = 2 + 4 + 4 + 8;

/// Frame describing a single (channel, chunk) payload.
#[derive(Debug, Clone, Copy)]
pub struct ChunkHeader {
    pub codec_id: u16,
    pub payload_len: u32,
    pub decoded_len: u32,
    pub checksum: u64,
}

impl ChunkHeader {
    pub fn new(codec: CodecId, payload_len: u32, decoded_len: u32, checksum: u64) -> Self {
        Self {
            codec_id: codec.to_wire(),
            payload_len,
            decoded_len,
            checksum,
        }
    }

    pub fn write_to(&self, out: &mut Vec<u8>) {
        let start = out.len();
        out.resize(start + CHUNK_HEADER_BYTES, 0);
        let buf = &mut out[start..start + CHUNK_HEADER_BYTES];
        LittleEndian::write_u16(&mut buf[0..2], self.codec_id);
        LittleEndian::write_u32(&mut buf[2..6], self.payload_len);
        LittleEndian::write_u32(&mut buf[6..10], self.decoded_len);
        LittleEndian::write_u64(&mut buf[10..18], self.checksum);
    }

    pub fn read_from(buf: &[u8]) -> Result<Self> {
        if buf.len() < CHUNK_HEADER_BYTES {
            return Err(Error::Truncated {
                needed: CHUNK_HEADER_BYTES,
                have: buf.len(),
            });
        }
        Ok(Self {
            codec_id: LittleEndian::read_u16(&buf[0..2]),
            payload_len: LittleEndian::read_u32(&buf[2..6]),
            decoded_len: LittleEndian::read_u32(&buf[6..10]),
            checksum: LittleEndian::read_u64(&buf[10..18]),
        })
    }
}

/// Per-chunk encode result, kept here (rather than in `codec`) so consumers
/// don't need to import the codec module just to talk about a finished chunk.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChunkStats {
    pub input_events: u32,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrips() {
        let h = ChunkHeader::new(CodecId::LosslessF32BssZstd, 1234, 65536, 0xDEAD_BEEF_F00D_BABE);
        let mut buf = Vec::new();
        h.write_to(&mut buf);
        assert_eq!(buf.len(), CHUNK_HEADER_BYTES);
        let got = ChunkHeader::read_from(&buf).unwrap();
        assert_eq!(got.codec_id, h.codec_id);
        assert_eq!(got.payload_len, h.payload_len);
        assert_eq!(got.decoded_len, h.decoded_len);
        assert_eq!(got.checksum, h.checksum);
    }

    #[test]
    fn header_truncation_detected() {
        let buf = vec![0u8; CHUNK_HEADER_BYTES - 1];
        match ChunkHeader::read_from(&buf) {
            Err(Error::Truncated { needed, have }) => {
                assert_eq!(needed, CHUNK_HEADER_BYTES);
                assert_eq!(have, CHUNK_HEADER_BYTES - 1);
            }
            other => panic!("expected Truncated, got {other:?}"),
        }
    }
}
