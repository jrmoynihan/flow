//! Container adapters for flow-fcs-compress.
//!
//! - [`fcz`]: native `.fcz` container with chunk index trailer (decode-first).
//! - `inline` (M5): writes compressed bytes into an existing FCS DATA segment.
//! - `parquet` (M5): per-column-precompressed Parquet sidecar.

pub mod fcz;
pub mod inline;
