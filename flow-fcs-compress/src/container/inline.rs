//! "Inline" payload format — the codec-payload bytes intended to live inside an
//! FCS DATA segment.
//!
//! Distinct from `.fcz`: there's no file-level magic or trailer. The layout is
//! self-describing so a host (e.g. an FCS file with `$COMPRESSION = FCZ1`) can
//! locate it via DATA-segment offsets and round-trip channels without any
//! file-level chrome.
//!
//! This module is what M5c (the FCS-inline pilot) embeds. It lives in the
//! container layer because its on-disk wire format is independent of any
//! particular file container.
//!
//! ## Wire format (little-endian throughout)
//!
//! ```text
//! [magic         "FCZIN"        5B]
//! [version u8                   1B]
//! [reserved u16                 2B]
//! [n_channels   u16             2B]
//! [events_per_chunk u32         4B]
//! [total_events u64             8B]
//! [channel descriptors          variable]   one per channel, see below
//! [n_chunks     u32             4B]
//! [chunk payloads               variable]   chunk-major then channel-major;
//!                                           each payload is 18B header + bytes
//! ```
//!
//! Channel descriptor:
//! `[name_len u16][name bytes][codec_id u16][stored_bits u8][adc_bits u8][range u32][log_a f32][log_b f32][signed u8][reserved u8]`.

use byteorder::{ByteOrder, LittleEndian};

use crate::chunk::{CHUNK_HEADER_BYTES, ChunkHeader};
use crate::codec::adc_bitpack::AdcBitpack;
use crate::codec::log_quant::LogQuantization;
use crate::codec::lossless_f32::{BssZstd, RawNone, RawZstd};
use crate::codec::{ChannelParams, CodecId, ColumnCodec};
use crate::error::{Error, Result};

const MAGIC: [u8; 5] = *b"FCZIN";
const FORMAT_VERSION: u8 = 1;
const PRELUDE_BYTES: usize = 5 + 1 + 2 + 2 + 4 + 8;

/// Encode every column of an in-memory event table into a single inline blob.
///
/// `columns[i].0` is the name, `columns[i].1` is `(params, events_slice)`.
pub fn encode_inline(
    columns: &[(String, ChannelParams, &[f32], CodecId)],
    events_per_chunk: u32,
) -> Result<Vec<u8>> {
    if columns.is_empty() {
        return Err(Error::InvalidParams("inline encode: no channels"));
    }
    if events_per_chunk == 0 {
        return Err(Error::InvalidParams(
            "inline encode: events_per_chunk must be > 0",
        ));
    }
    let total_events = columns[0].2.len();
    if !columns.iter().all(|c| c.2.len() == total_events) {
        return Err(Error::InvalidParams(
            "inline encode: all channels must have identical event count",
        ));
    }

    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.push(FORMAT_VERSION);
    write_u16(&mut out, 0);
    write_u16(&mut out, columns.len() as u16);
    write_u32(&mut out, events_per_chunk);
    write_u64(&mut out, total_events as u64);

    for (name, params, _, codec_id) in columns {
        let bytes = name.as_bytes();
        if bytes.len() > u16::MAX as usize {
            return Err(Error::InvalidParams("inline encode: channel name too long"));
        }
        write_u16(&mut out, bytes.len() as u16);
        out.extend_from_slice(bytes);
        write_u16(&mut out, codec_id.to_wire());
        out.push(params.stored_bits);
        out.push(params.adc_bits.unwrap_or(0));
        write_u32(&mut out, params.range);
        write_f32(&mut out, params.log_decades.0);
        write_f32(&mut out, params.log_decades.1);
        out.push(params.signed as u8);
        out.push(0);
    }

    let n_chunks = total_events.div_ceil(events_per_chunk as usize) as u32;
    write_u32(&mut out, n_chunks);

    for chunk_idx in 0..n_chunks {
        let start = (chunk_idx as usize) * (events_per_chunk as usize);
        let end = (start + events_per_chunk as usize).min(total_events);
        for (_name, params, events, codec_id) in columns {
            let codec = codec_from_id(codec_id.to_wire())?;
            let mut payload = Vec::new();
            codec.encode_chunk(&events[start..end], params, &mut payload)?;
            let checksum = xxhash_rust::xxh3::xxh3_64(&payload);
            let header = ChunkHeader::new(
                *codec_id,
                payload.len() as u32,
                (end - start) as u32,
                checksum,
            );
            header.write_to(&mut out);
            out.extend_from_slice(&payload);
        }
    }
    Ok(out)
}

/// Per-channel decoded view returned by [`decode_inline`].
pub struct DecodedChannel {
    pub name: String,
    pub params: ChannelParams,
    pub data: Vec<f32>,
}

/// Decode an inline blob into one `DecodedChannel` per channel.
pub fn decode_inline(buf: &[u8]) -> Result<Vec<DecodedChannel>> {
    if buf.len() < PRELUDE_BYTES {
        return Err(Error::Truncated {
            needed: PRELUDE_BYTES,
            have: buf.len(),
        });
    }
    if buf[..5] != MAGIC {
        return Err(Error::InvalidParams("inline: missing FCZIN magic"));
    }
    if buf[5] != FORMAT_VERSION {
        return Err(Error::InvalidParams("inline: unsupported version"));
    }
    let mut p = 5 + 1 + 2; // skip magic + version + reserved
    let n_channels = read_u16(buf, &mut p)? as usize;
    let _events_per_chunk = read_u32(buf, &mut p)?;
    let total_events = read_u64(buf, &mut p)? as usize;

    let mut channels: Vec<(String, ChannelParams, CodecId)> = Vec::with_capacity(n_channels);
    for _ in 0..n_channels {
        let name_len = read_u16(buf, &mut p)? as usize;
        if p + name_len > buf.len() {
            return Err(Error::Truncated {
                needed: p + name_len,
                have: buf.len(),
            });
        }
        let name = std::str::from_utf8(&buf[p..p + name_len])
            .map_err(|_| Error::InvalidParams("inline: channel name not UTF-8"))?
            .to_string();
        p += name_len;
        let codec_wire = read_u16(buf, &mut p)?;
        if p + 2 > buf.len() {
            return Err(Error::Truncated {
                needed: p + 2,
                have: buf.len(),
            });
        }
        let stored_bits = buf[p];
        let adc_bits_raw = buf[p + 1];
        p += 2;
        let range = read_u32(buf, &mut p)?;
        let log_a = read_f32(buf, &mut p)?;
        let log_b = read_f32(buf, &mut p)?;
        if p + 2 > buf.len() {
            return Err(Error::Truncated {
                needed: p + 2,
                have: buf.len(),
            });
        }
        let signed = buf[p] != 0;
        p += 2;

        let codec_id = CodecId::from_wire(codec_wire).ok_or(Error::UnknownCodec(codec_wire))?;
        channels.push((
            name,
            ChannelParams {
                name: String::new(),
                stored_bits,
                range,
                log_decades: (log_a, log_b),
                adc_bits: if adc_bits_raw == 0 {
                    None
                } else {
                    Some(adc_bits_raw)
                },
                signed,
            },
            codec_id,
        ));
    }
    let n_chunks = read_u32(buf, &mut p)?;

    // Prepare per-channel output buffers
    let mut decoded: Vec<DecodedChannel> = channels
        .into_iter()
        .map(|(name, mut params, _codec)| {
            params.name = name.clone();
            DecodedChannel {
                name,
                params,
                data: vec![0.0f32; total_events],
            }
        })
        .collect();
    let mut event_offset = 0usize;
    for _chunk_idx in 0..n_chunks {
        let mut chunk_events = 0usize;
        for ch in 0..n_channels {
            if p + CHUNK_HEADER_BYTES > buf.len() {
                return Err(Error::Truncated {
                    needed: p + CHUNK_HEADER_BYTES,
                    have: buf.len(),
                });
            }
            let header = ChunkHeader::read_from(&buf[p..p + CHUNK_HEADER_BYTES])?;
            let payload_start = p + CHUNK_HEADER_BYTES;
            let payload_end = payload_start + header.payload_len as usize;
            if payload_end > buf.len() {
                return Err(Error::Truncated {
                    needed: payload_end,
                    have: buf.len(),
                });
            }
            let payload = &buf[payload_start..payload_end];
            let actual_checksum = xxhash_rust::xxh3::xxh3_64(payload);
            if actual_checksum != header.checksum {
                return Err(Error::ChecksumMismatch {
                    expected: header.checksum,
                    actual: actual_checksum,
                });
            }
            let n = header.decoded_len as usize;
            let codec = codec_from_id(header.codec_id)?;
            let entry = &mut decoded[ch];
            // Reborrow params separately so the slice borrow doesn't fight with
            // a `&entry.params` borrow over the same struct.
            let params_snapshot = entry.params.clone();
            codec.decode_chunk(
                payload,
                &params_snapshot,
                &mut entry.data[event_offset..event_offset + n],
            )?;
            if ch == 0 {
                chunk_events = n;
            } else if n != chunk_events {
                return Err(Error::InvalidParams(
                    "inline decode: channels disagree on chunk event count",
                ));
            }
            p = payload_end;
        }
        event_offset += chunk_events;
    }

    if event_offset != total_events {
        return Err(Error::InvalidParams(
            "inline decode: chunked events != total_events",
        ));
    }
    Ok(decoded)
}

fn write_u16(out: &mut Vec<u8>, v: u16) {
    let mut b = [0u8; 2];
    LittleEndian::write_u16(&mut b, v);
    out.extend_from_slice(&b);
}
fn write_u32(out: &mut Vec<u8>, v: u32) {
    let mut b = [0u8; 4];
    LittleEndian::write_u32(&mut b, v);
    out.extend_from_slice(&b);
}
fn write_u64(out: &mut Vec<u8>, v: u64) {
    let mut b = [0u8; 8];
    LittleEndian::write_u64(&mut b, v);
    out.extend_from_slice(&b);
}
fn write_f32(out: &mut Vec<u8>, v: f32) {
    let mut b = [0u8; 4];
    LittleEndian::write_f32(&mut b, v);
    out.extend_from_slice(&b);
}

fn read_u16(buf: &[u8], p: &mut usize) -> Result<u16> {
    if *p + 2 > buf.len() {
        return Err(Error::Truncated {
            needed: *p + 2,
            have: buf.len(),
        });
    }
    let v = LittleEndian::read_u16(&buf[*p..*p + 2]);
    *p += 2;
    Ok(v)
}
fn read_u32(buf: &[u8], p: &mut usize) -> Result<u32> {
    if *p + 4 > buf.len() {
        return Err(Error::Truncated {
            needed: *p + 4,
            have: buf.len(),
        });
    }
    let v = LittleEndian::read_u32(&buf[*p..*p + 4]);
    *p += 4;
    Ok(v)
}
fn read_u64(buf: &[u8], p: &mut usize) -> Result<u64> {
    if *p + 8 > buf.len() {
        return Err(Error::Truncated {
            needed: *p + 8,
            have: buf.len(),
        });
    }
    let v = LittleEndian::read_u64(&buf[*p..*p + 8]);
    *p += 8;
    Ok(v)
}
fn read_f32(buf: &[u8], p: &mut usize) -> Result<f32> {
    if *p + 4 > buf.len() {
        return Err(Error::Truncated {
            needed: *p + 4,
            have: buf.len(),
        });
    }
    let v = LittleEndian::read_f32(&buf[*p..*p + 4]);
    *p += 4;
    Ok(v)
}

fn codec_from_id(id: u16) -> Result<Box<dyn ColumnCodec>> {
    match CodecId::from_wire(id).ok_or(Error::UnknownCodec(id))? {
        CodecId::LosslessF32BssZstd => Ok(Box::new(BssZstd::default())),
        CodecId::AdcBitpack => Ok(Box::new(AdcBitpack)),
        CodecId::LogQuantization => Ok(Box::new(LogQuantization::default())),
        CodecId::RawZstd => Ok(Box::new(RawZstd::default())),
        CodecId::RawNone => Ok(Box::new(RawNone)),
        other => Err(Error::UnknownCodec(other.to_wire())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
            v.push((i as f32) * 0.5 + (u - 0.5) * 100.0);
        }
        v
    }

    fn linear_params(name: &str) -> ChannelParams {
        ChannelParams {
            name: name.into(),
            stored_bits: 32,
            range: 262_144,
            log_decades: (0.0, 0.0),
            adc_bits: None,
            signed: true,
        }
    }

    #[test]
    fn inline_round_trip() {
        let n = 5_000;
        let fsc = synth(n, 1);
        let ssc = synth(n, 2);
        let columns: Vec<(String, ChannelParams, &[f32], CodecId)> = vec![
            (
                "FSC-A".to_string(),
                linear_params("FSC-A"),
                fsc.as_slice(),
                CodecId::LosslessF32BssZstd,
            ),
            (
                "SSC-A".to_string(),
                linear_params("SSC-A"),
                ssc.as_slice(),
                CodecId::LosslessF32BssZstd,
            ),
        ];

        let buf = encode_inline(&columns, 1024).unwrap();
        let decoded = decode_inline(&buf).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].name, "FSC-A");
        assert_eq!(decoded[1].name, "SSC-A");
        assert_eq!(decoded[0].data, fsc);
        assert_eq!(decoded[1].data, ssc);
    }

    #[test]
    fn inline_rejects_mismatched_lengths() {
        let a = synth(100, 1);
        let b = synth(99, 2);
        let columns: Vec<(String, ChannelParams, &[f32], CodecId)> = vec![
            (
                "A".to_string(),
                linear_params("A"),
                a.as_slice(),
                CodecId::LosslessF32BssZstd,
            ),
            (
                "B".to_string(),
                linear_params("B"),
                b.as_slice(),
                CodecId::LosslessF32BssZstd,
            ),
        ];
        let err = encode_inline(&columns, 64).unwrap_err();
        assert!(matches!(err, Error::InvalidParams(_)));
    }

    #[test]
    fn inline_corruption_caught() {
        let n = 1024;
        let v = synth(n, 1);
        let columns: Vec<(String, ChannelParams, &[f32], CodecId)> = vec![(
            "X".to_string(),
            linear_params("X"),
            v.as_slice(),
            CodecId::LosslessF32BssZstd,
        )];
        let mut buf = encode_inline(&columns, 256).unwrap();
        // Corrupt a byte well past the prelude — should land in a chunk payload.
        let target = buf.len() - 10;
        buf[target] ^= 0xFF;
        let result = decode_inline(&buf);
        assert!(matches!(
            result,
            Err(Error::ChecksumMismatch { .. } | Error::InvalidParams(_) | Error::Truncated { .. })
        ));
    }
}
