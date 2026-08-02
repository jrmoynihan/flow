//! `flow-fcs-compress` — compression codecs tuned for flow cytometry data.
//!
//! Three loss modes (see [`codec::CodecId`]):
//! - **Mode A — Lossless f32**: bit-exact round-trip via byte-stream-split + zstd.
//!   Recommended for unmixed / compensated data that carries full f32 entropy.
//! - **Mode B — ADC-bit lossless**: quantize to instrument bit depth (`$PnB`/`$PnR`),
//!   then bitpack. Recommended for raw spectral data from finite-resolution ADCs.
//! - **Mode C — Log-domain lossy**: biexp transform + fixed-point quantize, with a
//!   user-bounded relative-error tolerance.
//!
//! 0.1 scope: f32 only, offline encode/decode. f64 (`$DATATYPE D`) inputs are expected
//! to be downcast at ingest by the caller.

pub mod chunk;
pub mod codec;
pub mod container;
pub mod error;
pub mod transform;

pub use chunk::{ChunkHeader, ChunkStats, CHUNK_HEADER_BYTES, DEFAULT_CHUNK_EVENTS};
pub use codec::{ChannelParams, CodecId, ColumnCodec};
#[cfg(feature = "pco-backend")]
pub use codec::lossless_f32_pco::{
    ChunkConfig as PcoChunkConfig, DeltaSpec as PcoDeltaSpec, LosslessF32Pco,
    ModeSpec as PcoModeSpec, PagingSpec as PcoPagingSpec,
};
pub use error::{Error, Result};
