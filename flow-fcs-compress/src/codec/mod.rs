//! Codec abstraction. Each [`ColumnCodec`] encodes one chunk of a single FCS
//! parameter (column) into bytes, and decodes those bytes into a caller-provided
//! `&mut [f32]` buffer — no intermediate `Vec<f32>` allocations.

use crate::error::Result;

pub mod adc_bitpack;
pub mod auto;
pub mod log_quant;
pub mod lossless_f32;
#[cfg(feature = "pco-backend")]
pub mod lossless_f32_pco;
#[cfg(feature = "lz4-baseline")]
pub mod lz4_baseline;

/// Stable on-disk codec identifier. Wire numbers MUST NOT be reused or renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
#[non_exhaustive]
pub enum CodecId {
    /// Mode A: byte-stream-split planes + zstd. Lossless f32.
    LosslessF32BssZstd = 0x0001,
    /// Mode A alt backend: pco. Lossless f32. (M3+)
    LosslessF32Pco = 0x0002,
    /// Mode B: quantize via $PnB/$PnR + fastlanes bitpack. Lossless within ADC. (M3)
    AdcBitpack = 0x0010,
    /// Mode C: biexp transform + fixed-point quantize + bitpack. Lossy. (M4)
    LogQuantization = 0x0020,
    /// Baseline: raw f32 LE bytes through zstd. Useful as a comparison codec.
    RawZstd = 0x00F0,
    /// Baseline: raw f32 LE bytes uncompressed (round-trip sanity).
    RawNone = 0x00FF,
}

impl CodecId {
    pub fn from_wire(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::LosslessF32BssZstd),
            0x0002 => Some(Self::LosslessF32Pco),
            0x0010 => Some(Self::AdcBitpack),
            0x0020 => Some(Self::LogQuantization),
            0x00F0 => Some(Self::RawZstd),
            0x00FF => Some(Self::RawNone),
            _ => None,
        }
    }

    pub fn to_wire(self) -> u16 {
        self as u16
    }
}

/// Per-channel parameters derived from FCS keywords. Carried alongside every
/// encode/decode call rather than baked into codec instances so that the same
/// codec object can be reused across channels.
#[derive(Debug, Clone)]
pub struct ChannelParams {
    /// `$PnN` — short name (used for logs/diagnostics, not codec choice).
    pub name: String,
    /// `$PnB` — bits per parameter as stored on disk (not necessarily ADC bits).
    pub stored_bits: u8,
    /// `$PnR` — parameter range (max value + 1, per FCS spec).
    pub range: u32,
    /// `$PnE` — `(decades, offset)`. `(0.0, 0.0)` denotes linear scaling.
    pub log_decades: (f32, f32),
    /// True ADC bit depth, if known. Used by Mode B; falls back to `ceil(log2(range))`.
    pub adc_bits: Option<u8>,
    /// Whether the channel may carry negative values (post-compensation/unmixing).
    pub signed: bool,
}

impl ChannelParams {
    pub fn linear_unsigned(name: impl Into<String>, range: u32) -> Self {
        Self {
            name: name.into(),
            stored_bits: 32,
            range,
            log_decades: (0.0, 0.0),
            adc_bits: None,
            signed: false,
        }
    }

    pub fn is_log(&self) -> bool {
        self.log_decades.0 > 0.0
    }
}

/// Statistics returned from an encode call. Useful for benchmarking and for
/// populating chunk-index metadata in the `.fcz` container.
#[derive(Debug, Clone, Copy, Default)]
pub struct EncodeStats {
    pub input_events: u32,
    pub input_bytes: u64,
    pub output_bytes: u64,
}

impl EncodeStats {
    pub fn ratio(&self) -> f64 {
        if self.output_bytes == 0 {
            0.0
        } else {
            self.input_bytes as f64 / self.output_bytes as f64
        }
    }
}

/// Per-chunk column codec. Implementors MUST be stateless w.r.t. encode/decode
/// (any per-call state belongs in the payload or in [`ChannelParams`]).
pub trait ColumnCodec: Send + Sync {
    fn id(&self) -> CodecId;

    /// Encode `input` events for one column into `out`. Returns stats describing
    /// the produced payload. `out` is appended to, not replaced.
    fn encode_chunk(
        &self,
        input: &[f32],
        params: &ChannelParams,
        out: &mut Vec<u8>,
    ) -> Result<EncodeStats>;

    /// Decode `payload` into `out`. `out.len()` MUST equal the original event
    /// count. Implementations should treat `out` as uninitialized and fully
    /// overwrite every element.
    fn decode_chunk(
        &self,
        payload: &[u8],
        params: &ChannelParams,
        out: &mut [f32],
    ) -> Result<()>;
}
