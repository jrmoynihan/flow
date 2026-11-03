//! Native `.fcz` container.
//!
//! ## On-disk layout (little-endian throughout)
//!
//! ```text
//! [magic         "FCZ1"               4B]
//! [version u16, flags u16             4B]
//! [fcs_text_len u32                   4B]   bytes of mirrored FCS TEXT segment that follow
//! [fcs_text     ...                    ]   may be empty (length 0)
//! [n_channels   u16                   2B]
//! [channel descriptors  (variable)     ]   see ChannelDescriptor
//! [n_chunks     u32                   4B]
//! [events_per_chunk u32               4B]   max events per chunk; last chunk may be smaller
//! [total_events u64                   8B]
//! [chunk_payloads (variable)           ]   each: 18B ChunkHeader + payload bytes,
//!                                          ordered chunk-major then channel-major
//! [chunk_index   (variable)            ]   n_chunks * n_channels entries, each 18B:
//!                                          channel_idx u16, chunk_idx u32, offset u64, len u32
//! [index_offset  u64                  8B]
//! [magic_tail   "FCZ1END!"            8B]
//! ```
//!
//! The trailer-pointer pattern lets readers locate the chunk index in two
//! reads: load the last 16 bytes, verify magic, follow `index_offset`.
//!
//! ## Channel descriptor (variable, but each is contiguous)
//!
//! ```text
//! [name_len u16][name utf8 bytes][codec_id u16]
//! [stored_bits u8][adc_bits u8][range u32]
//! [log_a f32][log_b f32][signed u8][reserved u8]
//! ```
//!
//! `adc_bits == 0` means "unknown" (codec falls back to `ceil(log2(range))`).

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use byteorder::{ByteOrder, LittleEndian};
use flow_linalg::{ContractError, MatrixShape, OrderedRowSink, RowBlock, RowSource};
use memmap3::Mmap;

use crate::chunk::{CHUNK_HEADER_BYTES, ChunkHeader};
use crate::codec::adc_bitpack::AdcBitpack;
use crate::codec::log_quant::LogQuantization;
use crate::codec::lossless_f32::{BssZstd, RawNone, RawZstd};
use crate::codec::{ChannelParams, CodecId, ColumnCodec};
use crate::error::{Error, Result};

const MAGIC: [u8; 4] = *b"FCZ1";
const MAGIC_TAIL: [u8; 8] = *b"FCZ1END!";
const FORMAT_VERSION: u16 = 1;
const TRAILER_BYTES: u64 = 16; // index_offset (8) + magic_tail (8)
const INDEX_ENTRY_BYTES: usize = 2 + 4 + 8 + 4; // 18
static PARTIAL_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Writer for the native `.fcz` container.
/// Pre-encoded chunk produced by [`FczWriter::encode_chunk_payload`].
/// Hand it to [`FczWriter::append_encoded_chunk`] to splice into the file.
pub struct EncodedChunk {
    pub channel_idx: u16,
    pub codec_id: u16,
    pub decoded_len: u32,
    pub payload: Vec<u8>,
}

pub struct FczWriter {
    file: File,
    fcs_text: Vec<u8>,
    channels: Vec<ChannelEntry>,
    index: Vec<IndexEntry>,
    events_per_chunk: u32,
    /// Events seen per channel. All channels MUST end up with the same count;
    /// `finish` enforces this and writes the shared value as `total_events`.
    events_per_channel: Vec<u64>,
    n_chunks: u32,
    n_chunks_offset: Option<u64>,
    total_events_offset: Option<u64>,
    state: WriterState,
}

#[derive(Debug, Clone)]
struct ChannelEntry {
    params: ChannelParams,
    codec_id: u16,
}

#[derive(Debug, Clone, Copy)]
struct IndexEntry {
    channel_idx: u16,
    chunk_idx: u32,
    file_offset: u64,
    byte_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriterState {
    /// Channels declared but no chunks written yet.
    Open,
    /// At least one chunk has been written; channel set is now frozen.
    Streaming,
    /// finish() has been called.
    Closed,
}

/// Options controlling chunk size at write time.
#[derive(Debug, Clone, Copy)]
pub struct FczWriteOptions {
    pub events_per_chunk: u32,
}

impl Default for FczWriteOptions {
    fn default() -> Self {
        Self {
            events_per_chunk: crate::chunk::DEFAULT_CHUNK_EVENTS,
        }
    }
}

/// Failure while incrementally writing an FCZ row stream.
#[derive(Debug, thiserror::Error)]
pub enum FczRowSinkError {
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error("FCZ container operation failed: {0}")]
    Container(#[from] Error),
    #[error("FCZ filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}

/// Incremental row-major FCZ sink with atomic publication.
pub struct FczRowSink {
    writer: Option<FczWriter>,
    final_path: PathBuf,
    partial_path: PathBuf,
    shape: MatrixShape,
    next_row: usize,
    next_chunk: u32,
    events_per_chunk: usize,
    row_buffer: Vec<f32>,
    published: bool,
}

impl FczRowSink {
    /// Create a sink for a fixed schema and expected row count.
    pub fn create(
        path: impl AsRef<Path>,
        shape: MatrixShape,
        channels: &[(ChannelParams, CodecId)],
        options: FczWriteOptions,
        fcs_text: &[u8],
    ) -> std::result::Result<Self, FczRowSinkError> {
        let final_path = path.as_ref().to_path_buf();
        if final_path.extension().and_then(|value| value.to_str()) != Some("fcz") {
            return Err(FczRowSinkError::Invalid(
                "FCZ output path must have .fcz extension".to_string(),
            ));
        }
        if channels.is_empty() || channels.len() != shape.columns() {
            return Err(FczRowSinkError::Invalid(format!(
                "FCZ schema has {} channels but row shape has {} columns",
                channels.len(),
                shape.columns()
            )));
        }
        let events_per_chunk = usize::try_from(options.events_per_chunk)
            .map_err(|_| FczRowSinkError::Invalid("chunk size does not fit usize".to_string()))?;
        if events_per_chunk == 0 {
            return Err(FczRowSinkError::Invalid(
                "events_per_chunk must be non-zero".to_string(),
            ));
        }
        events_per_chunk
            .checked_mul(shape.columns())
            .ok_or_else(|| {
                FczRowSinkError::Invalid("FCZ row-group dimensions overflow usize".to_string())
            })?;
        let partial_path = sibling_partial_path(&final_path)?;
        let initialized = (|| {
            let mut writer = FczWriter::create(&partial_path, options)?;
            writer.set_fcs_text(fcs_text)?;
            for (params, codec) in channels {
                writer.add_channel(params.clone(), *codec)?;
            }
            Ok::<_, FczRowSinkError>(writer)
        })();
        let writer = match initialized {
            Ok(writer) => writer,
            Err(error) => {
                let _ = std::fs::remove_file(&partial_path);
                return Err(error);
            }
        };
        Ok(Self {
            writer: Some(writer),
            final_path,
            partial_path,
            shape,
            next_row: 0,
            next_chunk: 0,
            events_per_chunk,
            row_buffer: Vec::with_capacity(events_per_chunk * shape.columns()),
            published: false,
        })
    }

    pub fn write(&mut self, block: RowBlock<'_, f32>) -> std::result::Result<(), FczRowSinkError> {
        <Self as OrderedRowSink<f32>>::write(self, block)
    }

    pub fn finish(mut self) -> std::result::Result<PathBuf, FczRowSinkError> {
        if self.next_row != self.shape.rows() {
            return Err(ContractError::IncompleteSink {
                expected_rows: self.shape.rows(),
                written_rows: self.next_row,
            }
            .into());
        }
        self.flush_buffer()?;
        self.writer
            .take()
            .ok_or_else(|| FczRowSinkError::Invalid("FCZ sink is already closed".to_string()))?
            .finish()?;
        let reader = FczReader::open(&self.partial_path)?;
        if reader.total_events()
            != u64::try_from(self.shape.rows()).map_err(|_| {
                FczRowSinkError::Invalid("expected row count does not fit u64".to_string())
            })?
            || reader.n_channels() != self.shape.columns()
        {
            return Err(FczRowSinkError::Invalid(
                "final FCZ shape does not match declared sink shape".to_string(),
            ));
        }
        drop(reader);
        std::fs::rename(&self.partial_path, &self.final_path)?;
        sync_parent_directory(&self.final_path);
        self.published = true;
        Ok(self.final_path.clone())
    }

    fn flush_buffer(&mut self) -> std::result::Result<(), FczRowSinkError> {
        if self.row_buffer.is_empty() {
            return Ok(());
        }
        let columns = self.shape.columns();
        let rows = self.row_buffer.len() / columns;
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| FczRowSinkError::Invalid("FCZ sink is already closed".to_string()))?;
        let mut channel = Vec::with_capacity(rows);
        for channel_index in 0..columns {
            channel.clear();
            channel.extend(
                self.row_buffer
                    .chunks_exact(columns)
                    .map(|row| row[channel_index]),
            );
            writer.write_chunk(channel_index as u16, self.next_chunk, &channel)?;
        }
        self.row_buffer.clear();
        self.next_chunk = self
            .next_chunk
            .checked_add(1)
            .ok_or_else(|| FczRowSinkError::Invalid("FCZ chunk index overflow".to_string()))?;
        Ok(())
    }
}

impl OrderedRowSink<f32> for FczRowSink {
    type Error = FczRowSinkError;
    type Output = PathBuf;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn write(&mut self, block: RowBlock<'_, f32>) -> std::result::Result<(), Self::Error> {
        if block.row_start() != self.next_row {
            return Err(ContractError::OutOfOrderBlock {
                expected_start: self.next_row,
                actual_start: block.row_start(),
            }
            .into());
        }
        if block.columns() != self.shape.columns() {
            return Err(ContractError::ColumnMismatch {
                expected: self.shape.columns(),
                actual: block.columns(),
            }
            .into());
        }
        if block.row_end() > self.shape.rows() {
            return Err(ContractError::BlockExceedsShape {
                block_end: block.row_end(),
                expected_rows: self.shape.rows(),
            }
            .into());
        }
        let columns = self.shape.columns();
        for row in block.as_slice().chunks_exact(columns) {
            if self.row_buffer.len() == self.events_per_chunk * columns {
                self.flush_buffer()?;
            }
            self.row_buffer.extend_from_slice(row);
            self.next_row += 1;
        }
        Ok(())
    }

    fn finish(self) -> std::result::Result<Self::Output, Self::Error> {
        FczRowSink::finish(self)
    }
}

impl Drop for FczRowSink {
    fn drop(&mut self) {
        if !self.published {
            drop(self.writer.take());
            let _ = std::fs::remove_file(&self.partial_path);
        }
    }
}

fn sibling_partial_path(final_path: &Path) -> std::result::Result<PathBuf, FczRowSinkError> {
    let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
    let name = final_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| FczRowSinkError::Invalid("FCZ output filename is not UTF-8".to_string()))?;
    let sequence = PARTIAL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{name}.{}.{}.partial",
        std::process::id(),
        sequence
    )))
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) {}

impl FczWriter {
    /// Create a new `.fcz` file at `path`. Truncates existing files.
    pub fn create(path: impl AsRef<Path>, opts: FczWriteOptions) -> Result<Self> {
        if opts.events_per_chunk == 0 {
            return Err(Error::InvalidParams("events_per_chunk must be non-zero"));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Ok(Self {
            file,
            fcs_text: Vec::new(),
            channels: Vec::new(),
            index: Vec::new(),
            events_per_chunk: opts.events_per_chunk,
            events_per_channel: Vec::new(),
            n_chunks: 0,
            n_chunks_offset: None,
            total_events_offset: None,
            state: WriterState::Open,
        })
    }

    /// Mirror the FCS TEXT segment into the container so a `.fcz` is
    /// self-describing without the original `.fcs`.
    pub fn set_fcs_text(&mut self, text: &[u8]) -> Result<()> {
        if self.state != WriterState::Open {
            return Err(Error::InvalidParams(
                "fcs_text must be set before any chunks are written",
            ));
        }
        self.fcs_text = text.to_vec();
        Ok(())
    }

    /// Declare a channel. Returns its zero-based index. All channels MUST be
    /// declared before any chunks are written.
    pub fn add_channel(&mut self, params: ChannelParams, codec_id: CodecId) -> Result<u16> {
        if self.state != WriterState::Open {
            return Err(Error::InvalidParams(
                "channels must be declared before any chunks are written",
            ));
        }
        let idx = u16::try_from(self.channels.len())
            .map_err(|_| Error::InvalidParams("too many channels (max 65_535)"))?;
        self.channels.push(ChannelEntry {
            params,
            codec_id: codec_id.to_wire(),
        });
        self.events_per_channel.push(0);
        Ok(idx)
    }

    fn start_streaming(&mut self) -> Result<()> {
        if self.state != WriterState::Open {
            return Ok(());
        }
        if self.channels.is_empty() {
            return Err(Error::InvalidParams(
                "at least one channel must be declared before writing chunks",
            ));
        }
        let mut prelude = Vec::new();
        prelude.extend_from_slice(&MAGIC);
        write_u16(&mut prelude, FORMAT_VERSION);
        write_u16(&mut prelude, 0);
        let text_len = u32::try_from(self.fcs_text.len())
            .map_err(|_| Error::InvalidParams("FCS TEXT exceeds u32 length"))?;
        write_u32(&mut prelude, text_len);
        prelude.extend_from_slice(&self.fcs_text);
        write_u16(&mut prelude, self.channels.len() as u16);
        for entry in &self.channels {
            write_channel_descriptor(&mut prelude, entry);
        }
        self.n_chunks_offset = Some(prelude.len() as u64);
        write_u32(&mut prelude, 0);
        write_u32(&mut prelude, self.events_per_chunk);
        self.total_events_offset = Some(prelude.len() as u64);
        write_u64(&mut prelude, 0);
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&prelude)?;
        self.state = WriterState::Streaming;
        Ok(())
    }

    /// Encode and append a chunk for one channel. The caller decides the order;
    /// the chunk index records actual offsets, so any order is valid as long as
    /// every (channel, chunk) pair is written exactly once.
    pub fn write_chunk(&mut self, channel_idx: u16, chunk_idx: u32, events: &[f32]) -> Result<()> {
        if self.state == WriterState::Closed {
            return Err(Error::InvalidParams("writer is already closed"));
        }
        if (channel_idx as usize) >= self.channels.len() {
            return Err(Error::InvalidParams("channel_idx out of range"));
        }
        if events.len() > self.events_per_chunk as usize {
            return Err(Error::InvalidParams(
                "chunk has more events than events_per_chunk",
            ));
        }
        self.start_streaming()?;

        let entry = &self.channels[channel_idx as usize];
        let codec = codec_from_id(entry.codec_id)?;

        let mut payload = Vec::new();
        codec.encode_chunk(events, &entry.params, &mut payload)?;
        let checksum = xxhash_rust::xxh3::xxh3_64(&payload);
        let payload_len = u32::try_from(payload.len())
            .map_err(|_| Error::InvalidParams("encoded chunk payload exceeds u32 length"))?;
        let codec_id = CodecId::from_wire(entry.codec_id)
            .ok_or(Error::InvalidParams("channel codec id is invalid"))?;
        let header = ChunkHeader::new(codec_id, payload_len, events.len() as u32, checksum);
        let file_offset = self.file.stream_position()?;
        let mut header_bytes = Vec::with_capacity(CHUNK_HEADER_BYTES);
        header.write_to(&mut header_bytes);
        self.file.write_all(&header_bytes)?;
        self.file.write_all(&payload)?;
        let byte_len = u32::try_from(
            CHUNK_HEADER_BYTES
                .checked_add(payload.len())
                .ok_or(Error::InvalidParams("encoded chunk byte length overflow"))?,
        )
        .map_err(|_| Error::InvalidParams("encoded chunk byte length exceeds u32"))?;

        self.index.push(IndexEntry {
            channel_idx,
            chunk_idx,
            file_offset,
            byte_len,
        });

        if chunk_idx >= self.n_chunks {
            self.n_chunks = chunk_idx
                .checked_add(1)
                .ok_or(Error::InvalidParams("chunk index exceeds FCZ limit"))?;
        }
        self.events_per_channel[channel_idx as usize] = self.events_per_channel
            [channel_idx as usize]
            .checked_add(events.len() as u64)
            .ok_or(Error::InvalidParams("channel event count exceeds u64"))?;
        Ok(())
    }

    /// Encode a chunk's payload outside the writer, suitable for use from
    /// within a `rayon` worker pool. Returns the codec-encoded bytes plus the
    /// codec id and decoded event count, ready to be handed back to
    /// [`Self::append_encoded_chunk`].
    ///
    /// The writer is *not* mutated by this call. Call it from any thread.
    /// All channel descriptors must already be added via [`Self::add_channel`].
    pub fn encode_chunk_payload(&self, channel_idx: u16, events: &[f32]) -> Result<EncodedChunk> {
        if (channel_idx as usize) >= self.channels.len() {
            return Err(Error::InvalidParams("channel_idx out of range"));
        }
        if events.len() > self.events_per_chunk as usize {
            return Err(Error::InvalidParams(
                "chunk has more events than events_per_chunk",
            ));
        }
        let entry = &self.channels[channel_idx as usize];
        let codec = codec_from_id(entry.codec_id)?;
        let mut payload = Vec::new();
        codec.encode_chunk(events, &entry.params, &mut payload)?;
        Ok(EncodedChunk {
            channel_idx,
            codec_id: entry.codec_id,
            decoded_len: events.len() as u32,
            payload,
        })
    }

    /// Append a pre-encoded chunk (typically produced by parallel
    /// [`Self::encode_chunk_payload`] calls). Sequential — the writer's
    /// internal buffer mutates here, so call this from a single thread.
    pub fn append_encoded_chunk(&mut self, chunk_idx: u32, encoded: EncodedChunk) -> Result<()> {
        if self.state == WriterState::Closed {
            return Err(Error::InvalidParams("writer is already closed"));
        }
        let channel_idx = encoded.channel_idx;
        let entry = self
            .channels
            .get(channel_idx as usize)
            .ok_or(Error::InvalidParams("channel_idx out of range"))?;
        if entry.codec_id != encoded.codec_id {
            return Err(Error::InvalidParams(
                "encoded chunk codec does not match channel codec",
            ));
        }
        if encoded.decoded_len > self.events_per_chunk {
            return Err(Error::InvalidParams(
                "chunk has more events than events_per_chunk",
            ));
        }
        self.start_streaming()?;
        let payload = encoded.payload;
        let checksum = xxhash_rust::xxh3::xxh3_64(&payload);
        let payload_len = u32::try_from(payload.len())
            .map_err(|_| Error::InvalidParams("encoded chunk payload exceeds u32 length"))?;
        let codec_id = CodecId::from_wire(encoded.codec_id)
            .ok_or(Error::InvalidParams("encoded chunk codec id is invalid"))?;
        let header = ChunkHeader::new(codec_id, payload_len, encoded.decoded_len, checksum);
        let file_offset = self.file.stream_position()?;
        let mut header_bytes = Vec::with_capacity(CHUNK_HEADER_BYTES);
        header.write_to(&mut header_bytes);
        self.file.write_all(&header_bytes)?;
        self.file.write_all(&payload)?;
        let byte_len = u32::try_from(
            CHUNK_HEADER_BYTES
                .checked_add(payload.len())
                .ok_or(Error::InvalidParams("encoded chunk byte length overflow"))?,
        )
        .map_err(|_| Error::InvalidParams("encoded chunk byte length exceeds u32"))?;
        self.index.push(IndexEntry {
            channel_idx,
            chunk_idx,
            file_offset,
            byte_len,
        });
        if chunk_idx >= self.n_chunks {
            self.n_chunks = chunk_idx
                .checked_add(1)
                .ok_or(Error::InvalidParams("chunk index exceeds FCZ limit"))?;
        }
        self.events_per_channel[channel_idx as usize] = self.events_per_channel
            [channel_idx as usize]
            .checked_add(u64::from(encoded.decoded_len))
            .ok_or(Error::InvalidParams("channel event count exceeds u64"))?;
        Ok(())
    }

    /// Finalize the file: write header, channel descriptors, payloads, index, trailer.
    pub fn finish(mut self) -> Result<()> {
        if self.state == WriterState::Closed {
            return Err(Error::InvalidParams("writer is already closed"));
        }
        self.start_streaming()?;

        // All channels must report identical event counts.
        let total_events = match self.events_per_channel.first().copied() {
            None => 0,
            Some(first) => {
                if !self.events_per_channel.iter().all(|&n| n == first) {
                    return Err(Error::InvalidParams(
                        "channels have unequal event counts; finalize aborted",
                    ));
                }
                first
            }
        };
        let expected_entries = self
            .channels
            .len()
            .checked_mul(self.n_chunks as usize)
            .ok_or(Error::InvalidParams("FCZ index dimensions overflow usize"))?;
        if self.index.len() != expected_entries {
            return Err(Error::InvalidParams(
                "not every channel/chunk pair was written exactly once",
            ));
        }
        let mut pairs = HashSet::with_capacity(self.index.len());
        if !self
            .index
            .iter()
            .all(|entry| pairs.insert((entry.channel_idx, entry.chunk_idx)))
        {
            return Err(Error::InvalidParams("duplicate channel/chunk pair"));
        }

        let n_chunks_offset = self
            .n_chunks_offset
            .ok_or(Error::InvalidParams("FCZ header was not initialized"))?;
        let total_events_offset = self
            .total_events_offset
            .ok_or(Error::InvalidParams("FCZ header was not initialized"))?;
        self.file.seek(SeekFrom::Start(n_chunks_offset))?;
        self.file.write_all(&self.n_chunks.to_le_bytes())?;
        self.file.seek(SeekFrom::Start(total_events_offset))?;
        self.file.write_all(&total_events.to_le_bytes())?;

        let index_offset = self.file.seek(SeekFrom::End(0))?;
        for entry in &self.index {
            self.file.write_all(&entry.channel_idx.to_le_bytes())?;
            self.file.write_all(&entry.chunk_idx.to_le_bytes())?;
            self.file.write_all(&entry.file_offset.to_le_bytes())?;
            self.file.write_all(&entry.byte_len.to_le_bytes())?;
        }
        self.file.write_all(&index_offset.to_le_bytes())?;
        self.file.write_all(&MAGIC_TAIL)?;
        self.file.sync_all()?;
        self.state = WriterState::Closed;
        Ok(())
    }
}

fn write_channel_descriptor(out: &mut Vec<u8>, entry: &ChannelEntry) {
    let name = entry.params.name.as_bytes();
    write_u16(out, name.len() as u16);
    out.extend_from_slice(name);
    write_u16(out, entry.codec_id);
    out.push(entry.params.stored_bits);
    out.push(entry.params.adc_bits.unwrap_or(0));
    write_u32(out, entry.params.range);
    write_f32(out, entry.params.log_decades.0);
    write_f32(out, entry.params.log_decades.1);
    out.push(entry.params.signed as u8);
    out.push(0); // reserved
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

fn codec_from_id(id: u16) -> Result<Box<dyn ColumnCodec>> {
    match CodecId::from_wire(id).ok_or(Error::UnknownCodec(id))? {
        CodecId::LosslessF32BssZstd => Ok(Box::new(BssZstd::default())),
        CodecId::AdcBitpack => Ok(Box::new(AdcBitpack)),
        CodecId::LogQuantization => Ok(Box::new(LogQuantization::default())),
        CodecId::RawZstd => Ok(Box::new(RawZstd::default())),
        CodecId::RawNone => Ok(Box::new(RawNone)),
        #[cfg(feature = "pco-backend")]
        CodecId::LosslessF32Pco => Ok(Box::new(
            crate::codec::lossless_f32_pco::LosslessF32Pco::default(),
        )),
        #[cfg(not(feature = "pco-backend"))]
        CodecId::LosslessF32Pco => Err(Error::UnknownCodec(0x0002)),
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

/// Memory-mapped reader for `.fcz` files. Random-access by `(channel, chunk)`.
pub struct FczReader {
    mmap: Mmap,
    fcs_text_range: (usize, usize),
    channels: Vec<ChannelEntry>,
    n_chunks: u32,
    events_per_chunk: u32,
    total_events: u64,
    /// `index[channel_idx][chunk_idx]` -> (file_offset, byte_len)
    index: Vec<Vec<(u64, u32)>>,
}

/// A selected-channel [`RowSource`] over an open FCZ reader.
pub struct FczRowSource<'a> {
    reader: &'a FczReader,
    channels: Vec<usize>,
    shape: MatrixShape,
}

impl<'a> FczRowSource<'a> {
    /// Validate a channel selection and create a fixed-shape source.
    pub fn new(reader: &'a FczReader, channels: &[usize]) -> Result<Self> {
        let rows = usize::try_from(reader.total_events)
            .map_err(|_| Error::InvalidParams("total_events does not fit usize"))?;
        let shape = MatrixShape::new(rows, channels.len())
            .map_err(|_| Error::InvalidParams("FCZ row-source shape overflows usize"))?;
        let mut storage = Vec::new();
        reader.read_selected_rows_into(channels, 0..0, &mut storage)?;
        Ok(Self {
            reader,
            channels: channels.to_vec(),
            shape,
        })
    }

    pub fn channels(&self) -> &[usize] {
        &self.channels
    }
}

impl RowSource<f32> for FczRowSource<'_> {
    type Error = Error;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn read_rows(&mut self, rows: Range<usize>, storage: &mut Vec<f32>) -> Result<()> {
        self.reader
            .read_selected_rows_into(&self.channels, rows, storage)
    }
}

impl FczReader {
    /// Open and parse the header + chunk index. Mmaps the whole file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        // SAFETY: mmap is unsafe because external mutation could race with us.
        // The reader treats the file as immutable; concurrent writers are a
        // user-level mistake we explicitly don't guard against. Same model as
        // fcs::Fcs::open.
        let mmap = unsafe { Mmap::map(&file) }?;
        Self::parse(mmap)
    }

    fn parse(mmap: Mmap) -> Result<Self> {
        let bytes = &mmap[..];
        let len = bytes.len();
        if len < 4 + 4 + (TRAILER_BYTES as usize) {
            return Err(Error::Truncated {
                needed: 4 + 4 + TRAILER_BYTES as usize,
                have: len,
            });
        }
        if bytes[..4] != MAGIC {
            return Err(Error::InvalidParams("missing FCZ1 magic"));
        }
        let trailer_start = len - TRAILER_BYTES as usize;
        let index_offset = LittleEndian::read_u64(&bytes[trailer_start..trailer_start + 8]);
        if bytes[trailer_start + 8..] != MAGIC_TAIL {
            return Err(Error::InvalidParams("missing FCZ1END! trailer magic"));
        }

        // Header parsing
        let mut p = 4;
        let _version = read_u16(bytes, &mut p)?;
        let _flags = read_u16(bytes, &mut p)?;
        let fcs_text_len = read_u32(bytes, &mut p)? as usize;
        let fcs_text_start = p;
        if p + fcs_text_len > len {
            return Err(Error::Truncated {
                needed: p + fcs_text_len,
                have: len,
            });
        }
        p += fcs_text_len;
        let fcs_text_range = (fcs_text_start, fcs_text_start + fcs_text_len);

        let n_channels = read_u16(bytes, &mut p)? as usize;
        let mut channels = Vec::with_capacity(n_channels);
        for _ in 0..n_channels {
            channels.push(read_channel_descriptor(bytes, &mut p)?);
        }
        let n_chunks = read_u32(bytes, &mut p)?;
        let events_per_chunk = read_u32(bytes, &mut p)?;
        let total_events = read_u64(bytes, &mut p)?;

        // Chunk index lives at index_offset
        let n_entries = (n_chunks as usize) * n_channels;
        let needed_index_bytes = n_entries * INDEX_ENTRY_BYTES;
        if (index_offset as usize) + needed_index_bytes > trailer_start {
            return Err(Error::Truncated {
                needed: index_offset as usize + needed_index_bytes,
                have: trailer_start,
            });
        }

        let mut index: Vec<Vec<(u64, u32)>> = (0..n_channels)
            .map(|_| vec![(0, 0); n_chunks as usize])
            .collect();
        let mut q = index_offset as usize;
        for _ in 0..n_entries {
            let channel_idx = LittleEndian::read_u16(&bytes[q..q + 2]) as usize;
            let chunk_idx = LittleEndian::read_u32(&bytes[q + 2..q + 6]) as usize;
            let offset = LittleEndian::read_u64(&bytes[q + 6..q + 14]);
            let byte_len = LittleEndian::read_u32(&bytes[q + 14..q + 18]);
            if channel_idx >= n_channels || chunk_idx >= n_chunks as usize {
                return Err(Error::InvalidParams("index entry out of range"));
            }
            index[channel_idx][chunk_idx] = (offset, byte_len);
            q += INDEX_ENTRY_BYTES;
        }

        Ok(Self {
            mmap,
            fcs_text_range,
            channels,
            n_chunks,
            events_per_chunk,
            total_events,
            index,
        })
    }

    pub fn n_channels(&self) -> usize {
        self.channels.len()
    }

    pub fn n_chunks(&self) -> u32 {
        self.n_chunks
    }

    pub fn events_per_chunk(&self) -> u32 {
        self.events_per_chunk
    }

    pub fn total_events(&self) -> u64 {
        self.total_events
    }

    pub fn fcs_text(&self) -> &[u8] {
        &self.mmap[self.fcs_text_range.0..self.fcs_text_range.1]
    }

    /// Touch every OS page of the memory-mapped file so subsequent reads
    /// don't incur page faults. Useful when callers intend to time decode
    /// throughput or need predictable latency on the first read pass.
    ///
    /// Reads one byte per 4 KiB page in order; `black_box` prevents the
    /// compiler from eliding the loads.
    pub fn warm_cache(&self) {
        const PAGE: usize = 4096;
        let bytes = &self.mmap[..];
        let mut i = 0;
        while i < bytes.len() {
            std::hint::black_box(bytes[i]);
            i += PAGE;
        }
    }

    pub fn channel(&self, idx: usize) -> Option<&ChannelParams> {
        self.channels.get(idx).map(|e| &e.params)
    }

    pub fn channel_codec_id(&self, idx: usize) -> Option<CodecId> {
        self.channels
            .get(idx)
            .and_then(|e| CodecId::from_wire(e.codec_id))
    }

    /// Decode one chunk for one channel into `out`. `out.len()` MUST equal the
    /// chunk's decoded event count (read via [`Self::chunk_event_count`]).
    pub fn decode_chunk_into(
        &self,
        channel_idx: usize,
        chunk_idx: u32,
        out: &mut [f32],
    ) -> Result<()> {
        let entry = self
            .channels
            .get(channel_idx)
            .ok_or(Error::InvalidParams("channel_idx out of range"))?;
        let (offset, byte_len) = self
            .index
            .get(channel_idx)
            .and_then(|row| row.get(chunk_idx as usize))
            .copied()
            .ok_or(Error::InvalidParams("chunk_idx out of range"))?;

        let region = &self.mmap[offset as usize..offset as usize + byte_len as usize];
        let header = ChunkHeader::read_from(&region[..CHUNK_HEADER_BYTES])?;
        let payload = &region[CHUNK_HEADER_BYTES..CHUNK_HEADER_BYTES + header.payload_len as usize];

        let actual_checksum = xxhash_rust::xxh3::xxh3_64(payload);
        if actual_checksum != header.checksum {
            return Err(Error::ChecksumMismatch {
                expected: header.checksum,
                actual: actual_checksum,
            });
        }
        if out.len() != header.decoded_len as usize {
            return Err(Error::LengthMismatch {
                expected: header.decoded_len as usize,
                actual: out.len(),
            });
        }
        let codec = codec_from_id(header.codec_id)?;
        codec.decode_chunk(payload, &entry.params, out)
    }

    pub fn chunk_event_count(&self, channel_idx: usize, chunk_idx: u32) -> Result<u32> {
        let (offset, _byte_len) = self
            .index
            .get(channel_idx)
            .and_then(|row| row.get(chunk_idx as usize))
            .copied()
            .ok_or(Error::InvalidParams("chunk_idx out of range"))?;
        let region = &self.mmap[offset as usize..offset as usize + CHUNK_HEADER_BYTES];
        let header = ChunkHeader::read_from(region)?;
        Ok(header.decoded_len)
    }

    /// Decode all chunks of one channel into a fresh `Vec<f32>` of length
    /// `total_events`.
    pub fn read_full_channel(&self, channel_idx: usize) -> Result<Vec<f32>> {
        let mut out = vec![0.0f32; self.total_events as usize];
        let mut written = 0usize;
        for chunk_idx in 0..self.n_chunks {
            let n = self.chunk_event_count(channel_idx, chunk_idx)? as usize;
            self.decode_chunk_into(channel_idx, chunk_idx, &mut out[written..written + n])?;
            written += n;
        }
        out.truncate(written);
        Ok(out)
    }

    /// Decode selected channels for a global row range into reusable row-major
    /// storage, crossing compressed chunk boundaries as needed.
    pub fn read_selected_rows_into(
        &self,
        channels: &[usize],
        rows: Range<usize>,
        storage: &mut Vec<f32>,
    ) -> Result<()> {
        if channels.is_empty() {
            return Err(Error::InvalidParams(
                "at least one channel must be selected",
            ));
        }
        let mut seen = HashSet::with_capacity(channels.len());
        for &channel in channels {
            if channel >= self.n_channels() {
                return Err(Error::InvalidParams("channel_idx out of range"));
            }
            if !seen.insert(channel) {
                return Err(Error::InvalidParams("duplicate channel selection"));
            }
        }
        let total_rows = usize::try_from(self.total_events)
            .map_err(|_| Error::InvalidParams("total_events does not fit usize"))?;
        if rows.start > rows.end || rows.end > total_rows {
            return Err(Error::InvalidParams("row range out of bounds"));
        }
        let row_count = rows.end - rows.start;
        let elements = row_count
            .checked_mul(channels.len())
            .ok_or(Error::InvalidParams(
                "selected row dimensions overflow usize",
            ))?;
        storage.clear();
        storage.resize(elements, 0.0);
        if row_count == 0 {
            return Ok(());
        }

        let mut chunk_start = 0usize;
        let mut temporary = Vec::new();
        for chunk_idx in 0..self.n_chunks {
            let chunk_rows = self.chunk_event_count(channels[0], chunk_idx)? as usize;
            let chunk_end = chunk_start
                .checked_add(chunk_rows)
                .ok_or(Error::InvalidParams("chunk row offset overflow"))?;
            if chunk_end <= rows.start {
                chunk_start = chunk_end;
                continue;
            }
            if chunk_start >= rows.end {
                break;
            }
            let copy_start = rows.start.max(chunk_start);
            let copy_end = rows.end.min(chunk_end);
            for (output_column, &channel) in channels.iter().enumerate() {
                let channel_rows = self.chunk_event_count(channel, chunk_idx)? as usize;
                if channel_rows != chunk_rows {
                    return Err(Error::InvalidParams(
                        "selected channels have unequal chunk row counts",
                    ));
                }
                temporary.resize(chunk_rows, 0.0);
                self.decode_chunk_into(channel, chunk_idx, &mut temporary)?;
                for global_row in copy_start..copy_end {
                    let output_row = global_row - rows.start;
                    storage[output_row * channels.len() + output_column] =
                        temporary[global_row - chunk_start];
                }
            }
            chunk_start = chunk_end;
        }
        if chunk_start < rows.end {
            return Err(Error::InvalidParams(
                "chunk index ended before requested row range",
            ));
        }
        Ok(())
    }

    /// Decode every (channel, chunk) into pre-allocated per-channel buffers in
    /// parallel. `buffers[channel_idx]` must be sized to `total_events`.
    /// Available behind the `multithread` feature.
    #[cfg(feature = "multithread")]
    pub fn decode_all_par(&self, buffers: &mut [Vec<f32>]) -> Result<()> {
        use rayon::prelude::*;

        if buffers.len() != self.n_channels() {
            return Err(Error::InvalidParams("buffers len != n_channels"));
        }
        for buf in buffers.iter_mut() {
            buf.resize(self.total_events as usize, 0.0);
        }

        // Build a flat work list of (channel_idx, chunk_idx, output_offset).
        // We need stable per-chunk offsets so multiple threads can write into
        // disjoint slices of the same channel buffer without coordination.
        let mut tasks: Vec<(usize, u32, usize)> =
            Vec::with_capacity(self.n_channels() * self.n_chunks as usize);
        for ch in 0..self.n_channels() {
            let mut offset = 0usize;
            for chunk in 0..self.n_chunks {
                let n = self.chunk_event_count(ch, chunk)? as usize;
                tasks.push((ch, chunk, offset));
                offset += n;
            }
        }

        // Per-channel base pointer + length, wrapped so the Vec is Sync.
        let bufs: Vec<SyncPtr> = buffers
            .iter_mut()
            .map(|v| SyncPtr {
                ptr: v.as_mut_ptr(),
                len: v.len(),
            })
            .collect();

        // SAFETY: each task writes to a disjoint `[output_offset .. output_offset + n)`
        // slice within `buffers[channel_idx]`. Per-channel offsets are
        // monotonically increasing and disjoint per chunk_idx, so no two tasks
        // alias. We pass raw pointers because rayon's par_iter doesn't
        // statically express disjoint mutable subslices across an outer Vec.
        tasks.par_iter().try_for_each(|&(ch, chunk, offset)| {
            let n = self.chunk_event_count(ch, chunk)? as usize;
            let entry = &bufs[ch];
            assert!(offset + n <= entry.len);
            // SAFETY: see argument above.
            let slice = unsafe { std::slice::from_raw_parts_mut(entry.ptr.add(offset), n) };
            self.decode_chunk_into(ch, chunk, slice)
        })?;

        Ok(())
    }
}

#[cfg(feature = "multithread")]
#[derive(Clone, Copy)]
struct SyncPtr {
    ptr: *mut f32,
    len: usize,
}
// SAFETY: pointers are only dereferenced under the disjoint-slice invariant
// described in `decode_all_par`.
#[cfg(feature = "multithread")]
unsafe impl Send for SyncPtr {}
#[cfg(feature = "multithread")]
unsafe impl Sync for SyncPtr {}

fn read_u16(bytes: &[u8], p: &mut usize) -> Result<u16> {
    if *p + 2 > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + 2,
            have: bytes.len(),
        });
    }
    let v = LittleEndian::read_u16(&bytes[*p..*p + 2]);
    *p += 2;
    Ok(v)
}
fn read_u32(bytes: &[u8], p: &mut usize) -> Result<u32> {
    if *p + 4 > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + 4,
            have: bytes.len(),
        });
    }
    let v = LittleEndian::read_u32(&bytes[*p..*p + 4]);
    *p += 4;
    Ok(v)
}
fn read_u64(bytes: &[u8], p: &mut usize) -> Result<u64> {
    if *p + 8 > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + 8,
            have: bytes.len(),
        });
    }
    let v = LittleEndian::read_u64(&bytes[*p..*p + 8]);
    *p += 8;
    Ok(v)
}
fn read_f32(bytes: &[u8], p: &mut usize) -> Result<f32> {
    if *p + 4 > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + 4,
            have: bytes.len(),
        });
    }
    let v = LittleEndian::read_f32(&bytes[*p..*p + 4]);
    *p += 4;
    Ok(v)
}

fn read_channel_descriptor(bytes: &[u8], p: &mut usize) -> Result<ChannelEntry> {
    let name_len = read_u16(bytes, p)? as usize;
    if *p + name_len > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + name_len,
            have: bytes.len(),
        });
    }
    let name = std::str::from_utf8(&bytes[*p..*p + name_len])
        .map_err(|_| Error::InvalidParams("channel name not valid UTF-8"))?
        .to_string();
    *p += name_len;
    let codec_id = read_u16(bytes, p)?;
    if *p + 2 > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + 2,
            have: bytes.len(),
        });
    }
    let stored_bits = bytes[*p];
    let adc_bits_raw = bytes[*p + 1];
    *p += 2;
    let range = read_u32(bytes, p)?;
    let log_a = read_f32(bytes, p)?;
    let log_b = read_f32(bytes, p)?;
    if *p + 2 > bytes.len() {
        return Err(Error::Truncated {
            needed: *p + 2,
            have: bytes.len(),
        });
    }
    let signed = bytes[*p] != 0;
    let _reserved = bytes[*p + 1];
    *p += 2;

    Ok(ChannelEntry {
        params: ChannelParams {
            name,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn synth(n: usize, seed: u64) -> Vec<f32> {
        let mut x = Vec::with_capacity(n);
        let mut s = seed;
        for i in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
            x.push((i as f32) * 0.5 + (u - 0.5) * 100.0);
        }
        x
    }

    fn write_two_channel_fcz(path: &Path, ch_size: u32) -> (Vec<f32>, Vec<f32>) {
        let opts = FczWriteOptions {
            events_per_chunk: ch_size,
        };
        let mut w = FczWriter::create(path, opts).unwrap();
        w.set_fcs_text(b"$DUMMY/value/").unwrap();
        let p_fsc = ChannelParams {
            name: "FSC-A".into(),
            stored_bits: 32,
            range: 262_144,
            log_decades: (0.0, 0.0),
            adc_bits: Some(22),
            signed: false,
        };
        let p_ssc = ChannelParams {
            name: "SSC-A".into(),
            stored_bits: 32,
            range: 262_144,
            log_decades: (0.0, 0.0),
            adc_bits: Some(22),
            signed: false,
        };
        w.add_channel(p_fsc, CodecId::LosslessF32BssZstd).unwrap();
        w.add_channel(p_ssc, CodecId::LosslessF32BssZstd).unwrap();

        let total = (ch_size as usize) * 3 + 17; // 3 full chunks + tail
        let fsc = synth(total, 1);
        let ssc = synth(total, 2);

        let mut written = 0;
        let mut chunk_idx = 0;
        while written < total {
            let n = (total - written).min(ch_size as usize);
            w.write_chunk(0, chunk_idx, &fsc[written..written + n])
                .unwrap();
            w.write_chunk(1, chunk_idx, &ssc[written..written + n])
                .unwrap();
            written += n;
            chunk_idx += 1;
        }
        w.finish().unwrap();
        (fsc, ssc)
    }

    #[test]
    fn round_trip_via_full_channel_read() {
        let tmp = NamedTempFile::new().unwrap();
        let (fsc, ssc) = write_two_channel_fcz(tmp.path(), 1024);

        let r = FczReader::open(tmp.path()).unwrap();
        assert_eq!(r.n_channels(), 2);
        assert_eq!(r.total_events() as usize, fsc.len());
        assert_eq!(r.fcs_text(), b"$DUMMY/value/");

        let got_fsc = r.read_full_channel(0).unwrap();
        let got_ssc = r.read_full_channel(1).unwrap();
        assert_eq!(got_fsc, fsc);
        assert_eq!(got_ssc, ssc);
    }

    #[test]
    fn round_trip_random_chunk_access() {
        let tmp = NamedTempFile::new().unwrap();
        let (fsc, _ssc) = write_two_channel_fcz(tmp.path(), 1024);

        let r = FczReader::open(tmp.path()).unwrap();
        // Decode chunk 2 of channel 0 and verify against the source slice.
        let chunk_idx = 2u32;
        let n = r.chunk_event_count(0, chunk_idx).unwrap() as usize;
        let mut out = vec![0.0f32; n];
        r.decode_chunk_into(0, chunk_idx, &mut out).unwrap();
        let start = (chunk_idx as usize) * 1024;
        assert_eq!(out, fsc[start..start + n]);
    }

    #[cfg(feature = "multithread")]
    #[test]
    fn decode_all_par_matches_serial() {
        let tmp = NamedTempFile::new().unwrap();
        let (fsc, ssc) = write_two_channel_fcz(tmp.path(), 1024);

        let r = FczReader::open(tmp.path()).unwrap();
        let mut buffers = vec![Vec::new(); 2];
        r.decode_all_par(&mut buffers).unwrap();
        assert_eq!(buffers[0], fsc);
        assert_eq!(buffers[1], ssc);
    }

    #[test]
    fn checksum_mismatch_detected() {
        let tmp = NamedTempFile::new().unwrap();
        let _ = write_two_channel_fcz(tmp.path(), 256);

        // Corrupt one byte inside a chunk payload region. We pick a byte
        // somewhere comfortably past the header but before any index region.
        let mut data = std::fs::read(tmp.path()).unwrap();
        // Skip over the prelude conservatively — find a byte ~halfway into the file
        // which is very likely inside a chunk payload, not the trailer or index.
        let target = data.len() / 2;
        data[target] ^= 0xFF;
        std::fs::write(tmp.path(), &data).unwrap();

        let r = FczReader::open(tmp.path()).unwrap();
        let mut hit_checksum_err = false;
        // Walk every chunk; corruption may also surface as a decode error.
        for ch in 0..r.n_channels() {
            for ck in 0..r.n_chunks() {
                let n = match r.chunk_event_count(ch, ck) {
                    Ok(v) => v as usize,
                    Err(_) => continue,
                };
                let mut out = vec![0.0f32; n];
                if let Err(Error::ChecksumMismatch { .. }) = r.decode_chunk_into(ch, ck, &mut out) {
                    hit_checksum_err = true;
                }
            }
        }
        assert!(
            hit_checksum_err,
            "a corrupted byte should surface as ChecksumMismatch"
        );
    }
}
