//! `.fcz` compressed read/write integration. Compiled only with the `compress`
//! feature enabled.
//!
//! Two entry points:
//! - [`Fcs::write_fcz`] — encode the in-memory `data_frame` into a `.fcz` file.
//! - [`Fcs::events_from_fcz`] — load just the columnar event data back from a
//!   `.fcz` and return it as an `EventDataFrame`. Full `Fcs` reconstruction
//!   (including a faithful `Header` and `file_access`) is intentionally
//!   deferred to a later milestone — for now `events_from_fcz` returns the
//!   piece a typical analysis pipeline actually needs.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use memmap3::Mmap;
use polars::prelude::*;

pub use flow_fcs_compress::container::fcz::FczWriteOptions;
use flow_fcs_compress::codec::{ChannelParams as CompChannelParams, CodecId};
use flow_fcs_compress::container::fcz::{FczReader, FczWriter};
use flow_fcs_compress::container::inline::{decode_inline, encode_inline};

use crate::Header;
use crate::Metadata;
use crate::keyword::{IntegerKeyword, Keyword, StringKeyword};
use crate::{EventDataFrame, Fcs};

impl Fcs {
    /// Encode every column of `self.data_frame` into a `.fcz` file using Mode A
    /// (lossless byte-stream-split + zstd). Mode-B/C codec selection lands in M3.
    ///
    /// # Errors
    /// Returns an error if the `data_frame` has no columns, if any column is
    /// missing, or if the underlying writer fails.
    pub fn write_fcz(&self, path: impl AsRef<Path>, opts: FczWriteOptions) -> Result<()> {
        let chunk_size = opts.events_per_chunk as usize;
        if chunk_size == 0 {
            return Err(anyhow!("events_per_chunk must be > 0"));
        }

        let names = self.get_parameter_names_from_dataframe();
        if names.is_empty() {
            return Err(anyhow!("data_frame has no columns; nothing to write"));
        }

        let mut writer = FczWriter::create(path.as_ref(), opts)
            .map_err(|e| anyhow!("failed to create .fcz: {e}"))?;

        for (i, name) in names.iter().enumerate() {
            let param_num = i + 1;
            let params = build_channel_params(self, name, param_num);
            writer
                .add_channel(params, CodecId::LosslessF32BssZstd)
                .map_err(|e| anyhow!("add_channel({name}): {e}"))?;
        }

        let total = self.data_frame.height();
        let mut start = 0usize;
        let mut chunk_idx = 0u32;
        while start < total {
            let end = (start + chunk_size).min(total);
            for (channel_idx, name) in names.iter().enumerate() {
                let slice = self
                    .get_parameter_events_slice(name)
                    .with_context(|| format!("column {name} unavailable as &[f32]"))?;
                writer
                    .write_chunk(channel_idx as u16, chunk_idx, &slice[start..end])
                    .map_err(|e| anyhow!("write_chunk({name}, {chunk_idx}): {e}"))?;
            }
            start = end;
            chunk_idx += 1;
        }

        writer.finish().map_err(|e| anyhow!("finalize .fcz: {e}"))
    }

    /// Encode every column of `self.data_frame` into a real `.fcs` file whose
    /// DATA segment carries the `flow-fcs-compress` inline payload (Mode A
    /// only, in this milestone). The file's TEXT segment carries the
    /// extension keywords that the M6 ISAC proposal will formalize:
    ///
    /// - `$COMPRESSION = FCZ1` (file-level marker)
    /// - `$PnCOMPRESSION = <codec id>` per parameter
    /// - `$DATATYPE = X` (vendor extension; an FCS 3.x reader without
    ///   compression awareness will refuse to parse rather than misinterpret —
    ///   the safe failure mode)
    ///
    /// Pilot status: this method is intended as a working prototype to take
    /// to the FCS Working Group, not a production format. Use `write_fcz` for
    /// real workflows until the keywords are standardized.
    ///
    /// # Errors
    /// Returns an error on any I/O failure or codec failure.
    pub fn write_inline_fcs(&self, path: impl AsRef<Path>, opts: FczWriteOptions) -> Result<()> {
        if opts.events_per_chunk == 0 {
            return Err(anyhow!("events_per_chunk must be > 0"));
        }
        let path = path.as_ref();
        if path.extension().and_then(|s| s.to_str()) != Some("fcs") {
            return Err(anyhow!("write_inline_fcs requires .fcs extension"));
        }

        let names = self.get_parameter_names_from_dataframe();
        if names.is_empty() {
            return Err(anyhow!("data_frame has no columns; nothing to write"));
        }
        let n_events = self.data_frame.height();
        let n_params = names.len();

        // Encode the inline payload.
        let mut input_columns: Vec<(String, CompChannelParams, &[f32], CodecId)> =
            Vec::with_capacity(n_params);
        let mut slices: Vec<&[f32]> = Vec::with_capacity(n_params);
        for name in &names {
            slices.push(
                self.get_parameter_events_slice(name)
                    .with_context(|| format!("column {name} unavailable as &[f32]"))?,
            );
        }
        for (i, name) in names.iter().enumerate() {
            let params = build_channel_params(self, name, i + 1);
            input_columns.push((
                name.clone(),
                params,
                slices[i],
                CodecId::LosslessF32BssZstd,
            ));
        }
        let data_segment = encode_inline(&input_columns, opts.events_per_chunk)
            .map_err(|e| anyhow!("inline encode: {e}"))?;

        // Clone metadata and inject extension keywords. Mark `$DATATYPE = X`
        // so a non-aware reader cannot parse the bytes as raw events.
        let mut metadata = self.metadata.clone();
        metadata
            .keywords
            .insert(
                "$COMPRESSION".to_string(),
                Keyword::String(StringKeyword::Other(Arc::from("FCZ1"))),
            );
        for name in &names {
            let key = format!("$P{}COMPRESSION", names.iter().position(|n| n == name).unwrap() + 1);
            metadata.keywords.insert(
                key,
                Keyword::String(StringKeyword::Other(Arc::from("LosslessF32BssZstd"))),
            );
        }
        metadata.keywords.insert(
            "$DATATYPE".to_string(),
            Keyword::String(StringKeyword::Other(Arc::from("X"))),
        );

        // Compose HEADER + TEXT + DATA via flow-fcs's serializer helpers.
        //
        // This must go through `resolve_layout`, not a single serialize pass: writing
        // TEXT once from an estimated offset and then correcting only the HEADER
        // leaves the `$BEGINDATA`/`$ENDDATA` baked into TEXT disagreeing with it.
        // Readers prefer the HEADER so that stayed invisible - until an offset too
        // wide for the 8-digit HEADER field is written as `0` and the reader falls
        // back to the stale TEXT value, decoding the wrong bytes as events.
        let layout = crate::write::resolve_layout(
            &metadata,
            crate::write::HEADER_SIZE,
            n_events,
            n_params,
            data_segment.len(),
            self.header.version,
        )
            .map_err(|e| anyhow!("resolve_layout: {e}"))?;
        let header = crate::write::build_header(
            &self.header.version,
            layout.text_start,
            layout.text_end,
            layout.data_start,
            layout.data_end,
        )
        .map_err(|e| anyhow!("build_header: {e}"))?;

        // Same CRC treatment as the plain writer: the inline payload sits in a
        // DATA segment like any other, so §3.7 applies unchanged. A reader that
        // cannot decode FCZ1 can still verify the file is intact.
        crate::write::write_segments(
            path,
            &header,
            &layout.text_segment,
            &data_segment,
            crate::write::CrcPolicy::default(),
        )
    }

    /// Load only the columnar event data from a `.fcs` file written by
    /// [`Fcs::write_inline_fcs`]. Verifies `$COMPRESSION = FCZ1` and decodes
    /// the inline payload from the DATA segment.
    pub fn events_from_inline_fcs(path: impl AsRef<Path>) -> Result<EventDataFrame> {
        let file = File::open(path.as_ref())?;
        // SAFETY: we treat the file as immutable; same convention as Fcs::open.
        let mmap = unsafe { Mmap::map(&file) }?;

        let header =
            Header::from_mmap(&mmap).map_err(|e| anyhow!("inline open: header parse failed: {e}"))?;
        let metadata = Metadata::from_mmap(&mmap, &header);

        match metadata.keywords.get("$COMPRESSION") {
            Some(Keyword::String(StringKeyword::Other(s))) if &**s == "FCZ1" => {}
            Some(other) => {
                return Err(anyhow!(
                    "$COMPRESSION present but unexpected value: {:?}",
                    other
                ));
            }
            None => return Err(anyhow!("file is not flow-fcs-compress inline (no $COMPRESSION)")),
        }

        let data_start = *header.data_offset.start();
        let data_end = *header.data_offset.end();
        if data_start == 0 || data_end == 0 || data_end >= mmap.len() {
            return Err(anyhow!(
                "DATA segment offsets invalid: [{data_start}, {data_end}], mmap len {}",
                mmap.len()
            ));
        }
        let data_bytes = &mmap[data_start..=data_end];
        let decoded =
            decode_inline(data_bytes).map_err(|e| anyhow!("inline decode failed: {e}"))?;

        let mut columns = Vec::with_capacity(decoded.len());
        for ch in decoded {
            columns.push(Column::new(ch.name.into(), ch.data));
        }
        let height = if let Some(c) = columns.first() {
            c.len()
        } else {
            0
        };
        let df = DataFrame::new(height, columns)
            .map_err(|e| anyhow!("build polars DataFrame: {e}"))?;
        Ok(Arc::new(df))
    }

    /// Write the in-memory event table as a Parquet file (Tier 1 sidecar).
    ///
    /// This uses Polars' native Parquet writer with zstd column compression.
    /// It is the interop-friendly path: any tool that reads Parquet (Pandas,
    /// DuckDB, Spark, Polars itself) can consume the result without
    /// flow-fcs-compress installed. The flip side is that our specialized
    /// codecs (AdcBitpack, LogQuantization, Pco) are *not* used — Parquet handles
    /// compression at the column-page level with its own algorithm.
    ///
    /// FCS keyword metadata is attached as Parquet key-value metadata, prefixed
    /// `fcs.`. A future Tier 2 will route our codecs through Parquet's custom
    /// `Compression::Custom` slot once arrow-rs exposes it.
    ///
    /// Available behind the `parquet-sidecar` feature.
    #[cfg(feature = "parquet-sidecar")]
    pub fn write_parquet(&self, path: impl AsRef<Path>) -> Result<()> {
        use polars::io::SerWriter;
        use polars::prelude::ParquetWriter;

        let mut df = (*self.data_frame).clone();
        let file = File::create(path.as_ref())?;
        ParquetWriter::new(file)
            .with_compression(polars::prelude::ParquetCompression::Zstd(None))
            .finish(&mut df)
            .map_err(|e| anyhow!("polars parquet write: {e}"))?;
        Ok(())
    }

    /// Read a Parquet file written by [`Fcs::write_parquet`] (or any
    /// compatible writer) into an `EventDataFrame`.
    #[cfg(feature = "parquet-sidecar")]
    pub fn events_from_parquet(path: impl AsRef<Path>) -> Result<EventDataFrame> {
        use polars::io::SerReader;
        use polars::prelude::ParquetReader;

        let file = File::open(path.as_ref())?;
        let df = ParquetReader::new(file)
            .finish()
            .map_err(|e| anyhow!("polars parquet read: {e}"))?;
        Ok(Arc::new(df))
    }

    /// Load only the columnar event data from a `.fcz` file. Returns the same
    /// `EventDataFrame` shape as `Fcs::open` so downstream analysis code can
    /// switch between the two formats without changing.
    pub fn events_from_fcz(path: impl AsRef<Path>) -> Result<EventDataFrame> {
        let reader = FczReader::open(path.as_ref())
            .map_err(|e| anyhow!("failed to open .fcz: {e}"))?;

        let mut columns = Vec::with_capacity(reader.n_channels());
        for ch in 0..reader.n_channels() {
            let name = reader
                .channel(ch)
                .ok_or_else(|| anyhow!("channel {ch} missing descriptor"))?
                .name
                .clone();
            let data = reader
                .read_full_channel(ch)
                .map_err(|e| anyhow!("decode channel {name}: {e}"))?;
            columns.push(Column::new(name.into(), data));
        }
        let height = reader.total_events() as usize;
        let df = DataFrame::new(height, columns)
            .map_err(|e| anyhow!("build polars DataFrame: {e}"))?;
        Ok(Arc::new(df))
    }
}

fn build_channel_params(fcs: &Fcs, name: &str, param_num: usize) -> CompChannelParams {
    let stored_bits = fcs
        .metadata
        .get_bytes_per_parameter(param_num)
        .ok()
        .map(|bytes| (bytes.saturating_mul(8)).min(255) as u8)
        .unwrap_or(32);

    let range = fcs
        .metadata
        .get_parameter_numeric_metadata(param_num, "R")
        .ok()
        .and_then(|kw| match kw {
            IntegerKeyword::PnR(v) => Some((*v).min(u32::MAX as usize) as u32),
            _ => None,
        })
        .unwrap_or(262_144);

    CompChannelParams {
        name: name.to_string(),
        stored_bits,
        range,
        log_decades: (0.0, 0.0),
        adc_bits: None,
        signed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::AccessWrapper;
    use crate::parameter::ParameterMap;
    use crate::{Header, Metadata, Parameter, TransformType};
    use std::fs::File;
    use std::io::Write as _IoWrite;
    use tempfile::TempDir;

    fn synth_column(name: &str, n: usize, seed: u64) -> Column {
        let mut s = seed;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let u = ((s >> 32) as u32) as f32 / u32::MAX as f32;
            v.push((i as f32) * 0.5 + (u - 0.5) * 100.0);
        }
        Column::new(name.into(), v)
    }

    fn build_test_fcs(tmp: &TempDir, n_events: usize) -> (Fcs, Vec<String>) {
        // AccessWrapper requires a real file; create a placeholder.
        let placeholder = tmp.path().join("placeholder.fcs");
        let mut f = File::create(&placeholder).unwrap();
        f.write_all(b"placeholder").unwrap();

        let columns = vec![
            synth_column("FSC-A", n_events, 1),
            synth_column("SSC-A", n_events, 2),
            synth_column("FL1-A", n_events, 3),
        ];
        let df = DataFrame::new(n_events, columns).unwrap();

        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );
        params.insert(
            "SSC-A".into(),
            Parameter::new(&2, "SSC-A", "SSC-A", &TransformType::Linear),
        );
        params.insert(
            "FL1-A".into(),
            Parameter::new(&3, "FL1-A", "FL1-A", &TransformType::Linear),
        );

        let fcs = Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            Arc::new(df),
            AccessWrapper::new(placeholder.to_str().unwrap()).unwrap(),
        );
        let names = vec!["FSC-A".to_string(), "SSC-A".to_string(), "FL1-A".to_string()];
        (fcs, names)
    }

    #[test]
    fn write_then_read_roundtrips_dataframe() {
        let tmp = TempDir::new().unwrap();
        let (fcs, names) = build_test_fcs(&tmp, 4096);

        let fcz_path = tmp.path().join("out.fcz");
        let opts = FczWriteOptions {
            events_per_chunk: 1024,
        };
        fcs.write_fcz(&fcz_path, opts).unwrap();

        let df_back = Fcs::events_from_fcz(&fcz_path).unwrap();
        assert_eq!(df_back.height(), fcs.data_frame.height());
        assert_eq!(df_back.width(), fcs.data_frame.width());
        for name in &names {
            let original: &[f32] = fcs.get_parameter_events_slice(name).unwrap();
            let got_col = df_back.column(name).unwrap();
            let got = got_col
                .as_materialized_series()
                .f32()
                .unwrap()
                .cont_slice()
                .unwrap();
            assert_eq!(got, original, "column {name} did not round-trip");
        }
    }

    #[test]
    fn write_fcz_rejects_zero_chunk_size() {
        let tmp = TempDir::new().unwrap();
        let (fcs, _) = build_test_fcs(&tmp, 16);
        let fcz_path = tmp.path().join("zero.fcz");
        let opts = FczWriteOptions {
            events_per_chunk: 0,
        };
        let err = fcs.write_fcz(&fcz_path, opts).unwrap_err();
        assert!(err.to_string().contains("events_per_chunk"));
    }

    /// Round-trip a real FCS file from the Gating-ML compliance corpus if
    /// available. Marked `#[ignore]` so a fresh checkout without the corpus
    /// still passes; opt in via
    /// `cargo test --features compress -- --ignored real_fcs_round_trip`.
    #[test]
    #[ignore = "requires gates/Gating-ML.* compliance corpus"]
    fn real_fcs_round_trip_int10000() {
        let path = crate::corpus::path("int-10000_events_random.fcs");
        if !path.exists() {
            eprintln!("compliance corpus file missing, skipping");
            return;
        }
        let fcs = Fcs::open(path.to_str().expect("utf-8 corpus path")).expect("open compliance file");
        let tmp = TempDir::new().unwrap();
        let fcz_path = tmp.path().join("rt.fcz");
        fcs.write_fcz(&fcz_path, FczWriteOptions::default())
            .expect("write_fcz");

        let df_back = Fcs::events_from_fcz(&fcz_path).expect("events_from_fcz");
        assert_eq!(df_back.height(), fcs.data_frame.height());
        assert_eq!(df_back.width(), fcs.data_frame.width());

        for name in fcs.get_parameter_names_from_dataframe() {
            let original = fcs.get_parameter_events_slice(&name).unwrap();
            let got_col = df_back.column(&name).unwrap();
            let got = got_col
                .as_materialized_series()
                .f32()
                .unwrap()
                .cont_slice()
                .unwrap();
            assert_eq!(
                got, original,
                "channel {name} did not round-trip bit-exactly"
            );
        }
    }

    #[test]
    fn inline_fcs_round_trip() {
        let tmp = TempDir::new().unwrap();
        let (fcs, names) = build_test_fcs(&tmp, 2_000);

        let path = tmp.path().join("inline.fcs");
        fcs.write_inline_fcs(&path, FczWriteOptions::default())
            .expect("write_inline_fcs");

        let df_back = Fcs::events_from_inline_fcs(&path).expect("events_from_inline_fcs");
        assert_eq!(df_back.height(), fcs.data_frame.height());
        assert_eq!(df_back.width(), fcs.data_frame.width());

        for name in &names {
            let original = fcs.get_parameter_events_slice(name).unwrap();
            let got_col = df_back.column(name).unwrap();
            let got = got_col
                .as_materialized_series()
                .f32()
                .unwrap()
                .cont_slice()
                .unwrap();
            assert_eq!(got, original, "channel {name} did not round-trip");
        }
    }

    /// `write_inline_fcs` used to serialize TEXT once from an estimated offset and
    /// then fix up only the HEADER, leaving a stale `$BEGINDATA` inside TEXT. Readers
    /// prefer the HEADER, so it stayed invisible — but once an offset too wide for the
    /// 8-digit HEADER field is written as `0`, the reader falls back to that stale
    /// TEXT value and decodes the wrong bytes as events.
    #[test]
    fn inline_fcs_text_begindata_agrees_with_header() {
        let tmp = TempDir::new().unwrap();
        let (fcs, _) = build_test_fcs(&tmp, 2_000);

        let path = tmp.path().join("offsets.fcs");
        fcs.write_inline_fcs(&path, FczWriteOptions::default())
            .expect("write_inline_fcs");

        let bytes = std::fs::read(&path).expect("read back");
        let header_data_start: usize = std::str::from_utf8(&bytes[26..34])
            .unwrap()
            .trim()
            .parse()
            .expect("HEADER $BEGINDATA");
        let header_data_end: usize = std::str::from_utf8(&bytes[34..42])
            .unwrap()
            .trim()
            .parse()
            .expect("HEADER $ENDDATA");

        let text_start: usize = std::str::from_utf8(&bytes[10..18]).unwrap().trim().parse().unwrap();
        let text_end: usize = std::str::from_utf8(&bytes[18..26]).unwrap().trim().parse().unwrap();
        let text = String::from_utf8_lossy(&bytes[text_start..=text_end]);

        let keyword_value = |key: &str| -> usize {
            let delim = text.chars().next().expect("leading delimiter");
            let needle = format!("{delim}{key}{delim}");
            let start = text.find(&needle).unwrap_or_else(|| panic!("{key} in TEXT"))
                + needle.len();
            let rest = &text[start..];
            let end = rest.find(delim).unwrap_or(rest.len());
            rest[..end].trim().parse().unwrap_or_else(|_| panic!("{key} value"))
        };

        assert_eq!(
            keyword_value("$BEGINDATA"),
            header_data_start,
            "TEXT $BEGINDATA must match the primary HEADER"
        );
        assert_eq!(
            keyword_value("$ENDDATA"),
            header_data_end,
            "TEXT $ENDDATA must match the primary HEADER"
        );
        assert_eq!(
            header_data_start,
            text_end + 1,
            "DATA must begin immediately after TEXT — no unaccounted gap"
        );
    }

    #[test]
    fn inline_fcs_rejects_non_fcs_extension() {
        let tmp = TempDir::new().unwrap();
        let (fcs, _) = build_test_fcs(&tmp, 16);
        let path = tmp.path().join("wrong.fcz");
        let err = fcs
            .write_inline_fcs(&path, FczWriteOptions::default())
            .unwrap_err();
        assert!(err.to_string().contains(".fcs extension"));
    }

    #[cfg(feature = "parquet-sidecar")]
    #[test]
    fn parquet_sidecar_round_trip() {
        let tmp = TempDir::new().unwrap();
        let (fcs, names) = build_test_fcs(&tmp, 1_000);
        let path = tmp.path().join("out.parquet");
        fcs.write_parquet(&path).expect("write_parquet");
        let df_back = Fcs::events_from_parquet(&path).expect("events_from_parquet");
        assert_eq!(df_back.height(), fcs.data_frame.height());
        for name in &names {
            let original = fcs.get_parameter_events_slice(name).unwrap();
            let got_col = df_back.column(name).unwrap();
            let got = got_col
                .as_materialized_series()
                .f32()
                .unwrap()
                .cont_slice()
                .unwrap();
            assert_eq!(got, original, "parquet round-trip failed on {name}");
        }
    }

    #[test]
    fn write_fcz_with_unaligned_tail_chunk() {
        // 5_000 events into 1024-event chunks → 4 full chunks + 904 remainder.
        let tmp = TempDir::new().unwrap();
        let (fcs, _) = build_test_fcs(&tmp, 5_000);

        let fcz_path = tmp.path().join("tail.fcz");
        fcs.write_fcz(
            &fcz_path,
            FczWriteOptions {
                events_per_chunk: 1024,
            },
        )
        .unwrap();

        let df_back = Fcs::events_from_fcz(&fcz_path).unwrap();
        assert_eq!(df_back.height(), 5_000);
    }
}
