//! FCS file writing utilities
//!
//! This module provides functionality to write FCS files to disk, including:
//! - Duplicating existing files
//! - Editing metadata and persisting changes
//! - Creating new FCS files with data modifications (filtering, concatenation, column addition)
//!
//! ## Memory-Mapping Implications
//!
//! **Important**: When writing FCS files, the original memory-mapped file is not modified.
//! All write operations create new files. The original `Fcs` struct remains valid and
//! can continue to access the original file via memory-mapping until it's dropped.
//!
//! When you call `write_fcs_file()` or any of the modification functions:
//! 1. The data is read from the DataFrame (which is already in memory)
//! 2. A new file is created on disk
//! 3. The original memory-mapped file remains unchanged
//!
//! This means:
//! - You can safely write modified versions without affecting the original
//! - The original `Fcs` struct can still be used after writing
//! - No special handling is needed to "close" or "unmap" before writing
//! - Multiple writes can happen concurrently from the same source file

use crate::{
    Fcs,
    byteorder::ByteOrder,
    keyword::{ByteKeyword, IntegerKeyword, Keyword, StringableKeyword},
    metadata::Metadata,
    version::Version,
};
use anyhow::{Result, anyhow};
use byteorder::{LittleEndian, WriteBytesExt};
use polars::prelude::*;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// What to do when the metadata does not conform to its declared FCS version.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConformancePolicy {
    /// Log each violation and write the file anyway.
    ///
    /// The default, because tightening an existing writer into a hard failure
    /// would break pipelines that have been producing slightly-off files for
    /// years - and a file that is 95% conformant is still more useful on disk
    /// than an error.
    #[default]
    Warn,
    /// Refuse to write if any [`Severity::Error`](crate::conformance::Severity)
    /// violation is present. Deprecation warnings still only warn.
    Strict,
}

/// Whether to compute the FCS 3.2 §3.7 CRC word when writing.
///
/// Note that there is no variant for "write nothing". The eight CRC bytes are
/// structural: a data set that omits them is non-conformant even if it never
/// wanted a checksum. [`Omit`](Self::Omit) writes the spec's opt-out encoding,
/// eight ASCII `0`s.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrcPolicy {
    /// Compute the CRC over HEADER + TEXT + DATA and store it.
    ///
    /// The default. Costs one pass over the assembled bytes, which is cheap
    /// next to serializing them.
    #[default]
    Compute,
    /// Write the "not computed" field instead.
    ///
    /// For reproducing a byte-exact fixture, or for a writer that knows the
    /// caller will patch the segment afterwards and would leave a stale CRC.
    Omit,
}

/// Options controlling how a file is written.
///
/// Constructed with `..Default::default()` so later additions do not break
/// callers.
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteOptions {
    pub conformance: ConformancePolicy,
    pub crc: CrcPolicy,
}

/// Write an FCS file to disk
///
/// **Important**: This function closes the memory-mapped file before writing.
/// The original Fcs struct will no longer be able to access the original file
/// after this operation, but the data is preserved in the DataFrame.
///
/// Conformance violations against the declared version are logged but do not
/// block the write. Use [`write_fcs_file_with`] with
/// [`ConformancePolicy::Strict`] to reject non-conformant metadata instead.
///
/// # Arguments
/// * `fcs` - The FCS struct to write (will consume the struct)
/// * `path` - Output file path
///
/// # Errors
/// Returns an error if:
/// - The path is invalid
/// - The file cannot be written
/// - Metadata cannot be serialized
pub fn write_fcs_file(fcs: Fcs, path: impl AsRef<Path>) -> Result<()> {
    write_fcs_file_with(fcs, path, WriteOptions::default())
}

/// Write an FCS file to disk under explicit [`WriteOptions`].
///
/// # Errors
/// As [`write_fcs_file`], plus: returns an error under
/// [`ConformancePolicy::Strict`] if the metadata violates its declared version.
pub fn write_fcs_file_with(
    fcs: Fcs,
    path: impl AsRef<Path>,
    options: WriteOptions,
) -> Result<()> {
    let path = path.as_ref();

    // Validate file extension
    if path.extension().and_then(|s| s.to_str()) != Some("fcs") {
        return Err(anyhow!("Output file must have .fcs extension"));
    }

    // Get data from DataFrame
    let df = &*fcs.data_frame;
    let n_events = df.height();
    let n_params = df.width();

    if n_events == 0 {
        return Err(anyhow!("Cannot write FCS file with 0 events"));
    }
    if n_params == 0 {
        return Err(anyhow!("Cannot write FCS file with 0 parameters"));
    }

    enforce_conformance(&fcs.metadata, fcs.header.version, options.conformance, path)?;

    // Serialize data segment first (we need its size for metadata)
    let data_segment = serialize_data(df, &fcs.metadata)?;

    let layout = resolve_layout(
        &fcs.metadata,
        HEADER_SIZE,
        n_events,
        n_params,
        data_segment.len(),
    )?;

    // Build header
    let header = build_header(
        &fcs.header.version,
        layout.text_start,
        layout.text_end,
        layout.data_start,
        layout.data_end,
    )?;

    write_segments(
        path,
        &header,
        &layout.text_segment,
        &data_segment,
        options.crc,
    )
}

/// Writes HEADER + TEXT + DATA followed by the §3.7 CRC field.
///
/// Shared by both writers ([`write_fcs_file_with`] and
/// `Fcs::write_inline_fcs`), which had drifted into two identical copies of the
/// same three `write_all` calls. Keeping the CRC append in one place is the
/// point: a writer that forgets it produces a file that looks fine until
/// something conformant reads it.
///
/// The three segments are hashed in place rather than concatenated. They are
/// written back-to-back with no padding, so the file *is* the CRC's input
/// range: §3.7 covers "the first byte of the HEADER segment" through "the last
/// byte of the final segment".
pub(crate) fn write_segments(
    path: &Path,
    header: &[u8],
    text: &[u8],
    data: &[u8],
    policy: CrcPolicy,
) -> Result<()> {
    let checksum = (policy == CrcPolicy::Compute).then(|| {
        let mut crc = crate::crc::Crc16::new();
        crc.update(header);
        crc.update(text);
        crc.update(data);
        crc.finish()
    });

    let mut file = File::create(path)?;
    file.write_all(header)?;
    file.write_all(text)?;
    file.write_all(data)?;
    file.write_all(&crate::crc::format_field(checksum))?;
    file.sync_all()?;

    Ok(())
}

/// Run the conformance rules for `version` and act on them per `policy`.
///
/// Split out so both write paths share one behaviour, and so the eventual
/// per-version dispatch has a single site to redirect.
fn enforce_conformance(
    metadata: &Metadata,
    version: Version,
    policy: ConformancePolicy,
    path: &Path,
) -> Result<()> {
    use crate::conformance::{self, Severity};

    let violations = conformance::check_for_write(metadata, version);
    if violations.is_empty() {
        return Ok(());
    }

    let context = format!("writing {}", path.display());
    let has_errors = conformance::log_violations(&violations, version, &context);

    if policy == ConformancePolicy::Strict && has_errors {
        let listed = violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow!(
            "refusing to write non-conformant {version} file: {listed}"
        ));
    }
    Ok(())
}

/// Duplicate an existing FCS file to a new path
///
/// This creates an exact copy of the file on disk. The original Fcs struct
/// remains valid and can continue to be used.
///
/// # Arguments
/// * `fcs` - Reference to the FCS struct to duplicate
/// * `path` - Output file path
///
/// # Errors
/// Returns an error if the file cannot be written
pub fn duplicate_fcs_file(fcs: &Fcs, path: impl AsRef<Path>) -> Result<()> {
    use std::fs;

    let path = path.as_ref();

    // Simply copy the file on disk
    fs::copy(&fcs.file_access.path, path)?;

    Ok(())
}

/// Edit metadata and persist changes to disk
///
/// This function:
/// 1. Updates the metadata in the Fcs struct
/// 2. Writes the modified file to disk
/// 3. Returns a new Fcs struct pointing to the new file
///
/// **Note**: The original file is not modified. A new file is created.
///
/// # Arguments
/// * `fcs` - The FCS struct to modify
/// * `path` - Output file path for the modified file
/// * `updates` - Function that modifies the metadata
///
/// # Errors
/// Returns an error if the file cannot be written
pub fn edit_metadata_and_save<F>(mut fcs: Fcs, path: impl AsRef<Path>, updates: F) -> Result<Fcs>
where
    F: FnOnce(&mut Metadata),
{
    // Apply updates to metadata
    updates(&mut fcs.metadata);

    // Update $TOT if event count changed
    let n_events = fcs.get_event_count_from_dataframe();
    use crate::keyword::match_and_parse_keyword;
    let tot_keyword = match_and_parse_keyword("$TOT", &n_events.to_string());
    if let crate::keyword::KeywordCreationResult::Int(int_kw) = tot_keyword {
        fcs.metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(int_kw));
    }

    // Write to new file
    write_fcs_file(fcs.clone(), &path)?;

    // Open the new file
    Fcs::open(
        path.as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path"))?,
    )
}

/// Create a new FCS file by filtering events
///
/// Removes events where `mask[i] == false`. The mask must have the same length
/// as the number of events in the original file.
///
/// # Arguments
/// * `fcs` - The FCS struct to filter
/// * `path` - Output file path
/// * `mask` - Boolean mask (true = keep, false = remove)
///
/// # Errors
/// Returns an error if:
/// - The mask length doesn't match the number of events
/// - The file cannot be written
pub fn filter_events(fcs: Fcs, path: impl AsRef<Path>, mask: &[bool]) -> Result<Fcs> {
    let df = &*fcs.data_frame;
    let n_events = df.height();

    if mask.len() != n_events {
        return Err(anyhow!(
            "Mask length {} doesn't match number of events {}",
            mask.len(),
            n_events
        ));
    }

    // Filter DataFrame using Polars
    let mask_vec: Vec<bool> = mask.to_vec();
    let mask_series = Series::new("mask".into(), mask_vec);
    let mask_ca = mask_series.bool()?;
    let filtered_df = df.filter(&mask_ca)?;

    // Create new Fcs with filtered data
    let mut new_fcs = fcs.clone();
    new_fcs.data_frame = Arc::new(filtered_df);

    // Update metadata
    let n_events_after = new_fcs.get_event_count_from_dataframe();
    use crate::keyword::match_and_parse_keyword;
    let tot_keyword = match_and_parse_keyword("$TOT", &n_events_after.to_string());
    if let crate::keyword::KeywordCreationResult::Int(int_kw) = tot_keyword {
        new_fcs
            .metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(int_kw));
    }

    // Write to file
    write_fcs_file(new_fcs.clone(), &path)?;

    // Open the new file
    Fcs::open(
        path.as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path"))?,
    )
}

/// Create a new FCS file by concatenating events from multiple files
///
/// All files must have the same parameters (same names and order).
///
/// # Arguments
/// * `files` - Vector of FCS structs to concatenate
/// * `path` - Output file path
///
/// # Errors
/// Returns an error if:
/// - Files have different parameters
/// - The file cannot be written
pub fn concatenate_events(files: Vec<Fcs>, path: impl AsRef<Path>) -> Result<Fcs> {
    if files.is_empty() {
        return Err(anyhow!("Cannot concatenate empty list of files"));
    }

    if files.len() == 1 {
        // Just duplicate the single file
        return duplicate_fcs_file(&files[0], &path).and_then(|_| {
            Fcs::open(
                path.as_ref()
                    .to_str()
                    .ok_or_else(|| anyhow!("Invalid path"))?,
            )
        });
    }

    // Verify all files have the same parameters
    let first_params: Vec<String> = files[0].get_parameter_names_from_dataframe();

    for (idx, fcs) in files.iter().enumerate().skip(1) {
        let params: Vec<String> = fcs.get_parameter_names_from_dataframe();
        if params != first_params {
            return Err(anyhow!("File {} has different parameters than file 0", idx));
        }
    }

    // Concatenate DataFrames using vstack
    let dfs: Vec<DataFrame> = files.iter().map(|f| (*f.data_frame).clone()).collect();
    let concatenated_df = dfs
        .into_iter()
        .reduce(|acc, df| acc.vstack(&df).unwrap_or(acc))
        .ok_or_else(|| anyhow!("No files to concatenate"))?;

    // Create new Fcs using first file as template
    let mut new_fcs = files[0].clone();
    new_fcs.data_frame = Arc::new(concatenated_df);

    // Update metadata
    let n_events_after = new_fcs.get_event_count_from_dataframe();
    use crate::keyword::match_and_parse_keyword;
    let tot_keyword = match_and_parse_keyword("$TOT", &n_events_after.to_string());
    if let crate::keyword::KeywordCreationResult::Int(int_kw) = tot_keyword {
        new_fcs
            .metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(int_kw));
    }

    // Generate new GUID
    new_fcs.metadata.validate_guid();

    // Write to file
    write_fcs_file(new_fcs.clone(), &path)?;

    // Open the new file
    Fcs::open(
        path.as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path"))?,
    )
}

/// Create a new FCS file by adding a column (parameter) to existing data
///
/// This is useful for adding QC results (e.g., a boolean column indicating
/// good/bad events) or other event-level annotations.
///
/// # Arguments
/// * `fcs` - The FCS struct to modify
/// * `path` - Output file path
/// * `column_name` - Name of the new parameter
/// * `values` - Values for the new parameter (must match number of events)
///
/// # Errors
/// Returns an error if:
/// - The values length doesn't match the number of events
/// - The column name already exists
/// - The file cannot be written
pub fn add_column(
    mut fcs: Fcs,
    path: impl AsRef<Path>,
    column_name: &str,
    values: Vec<f32>,
) -> Result<Fcs> {
    let df = &*fcs.data_frame;
    let n_events = df.height();

    if values.len() != n_events {
        return Err(anyhow!(
            "Values length {} doesn't match number of events {}",
            values.len(),
            n_events
        ));
    }

    // Check if column already exists
    if df
        .get_column_names()
        .iter()
        .any(|&name| name == column_name)
    {
        return Err(anyhow!("Column {} already exists", column_name));
    }

    // Add column to DataFrame
    let mut new_df = df.clone();
    let new_series = Series::new(column_name.into(), values);
    new_df
        .with_column(new_series.into())
        .map_err(|e| anyhow!("Failed to add column: {}", e))?;

    // Update Fcs struct
    fcs.data_frame = Arc::new(new_df);

    // Add parameter metadata
    let n_params = fcs.get_parameter_count_from_dataframe();
    let param_num = n_params; // 1-based indexing in FCS

    // Update $PAR keyword
    use crate::keyword::match_and_parse_keyword;
    let par_keyword = match_and_parse_keyword("$PAR", &n_params.to_string());
    if let crate::keyword::KeywordCreationResult::Int(int_kw) = par_keyword {
        fcs.metadata
            .keywords
            .insert("$PAR".to_string(), Keyword::Int(int_kw));
    }

    // Add parameter keywords ($PnN, $PnB, etc.)
    fcs.metadata
        .insert_string_keyword(format!("$P{}N", param_num), column_name.to_string());

    // Default: 32 bits (4 bytes) for float32
    let pnb_keyword = match_and_parse_keyword(&format!("$P{}B", param_num), "32");
    if let crate::keyword::KeywordCreationResult::Int(int_kw) = pnb_keyword {
        fcs.metadata
            .keywords
            .insert(format!("$P{}B", param_num), Keyword::Int(int_kw));
    }

    // Default range
    let pnr_keyword = match_and_parse_keyword(&format!("$P{}R", param_num), "262144");
    if let crate::keyword::KeywordCreationResult::Int(int_kw) = pnr_keyword {
        fcs.metadata
            .keywords
            .insert(format!("$P{}R", param_num), Keyword::Int(int_kw));
    }

    // Default amplification
    fcs.metadata
        .insert_string_keyword(format!("$P{}E", param_num), "0,0".to_string());

    // Add to parameter map
    use crate::TransformType;
    use crate::parameter::Parameter;
    fcs.parameters.insert(
        column_name.to_string().into(),
        Parameter::new(&param_num, column_name, column_name, &TransformType::Linear),
    );

    // Write to file
    write_fcs_file(fcs.clone(), &path)?;

    // Open the new file
    Fcs::open(
        path.as_ref()
            .to_str()
            .ok_or_else(|| anyhow!("Invalid path"))?,
    )
}

// ==================== Internal Helper Functions ====================

/// The primary HEADER is always exactly 58 bytes; TEXT starts immediately after.
pub(crate) const HEADER_SIZE: usize = 58;

/// A serialized TEXT segment together with the segment offsets it agrees with.
pub(crate) struct FcsLayout {
    pub text_segment: Vec<u8>,
    pub text_start: usize,
    pub text_end: usize,
    pub data_start: usize,
    pub data_end: usize,
}

/// Serialize TEXT and resolve the segment offsets to a fixed point.
///
/// `$BEGINDATA`/`$ENDDATA` are keywords *inside* TEXT, so their digit count feeds
/// back into TEXT's own length and therefore into the offsets themselves. Serialize,
/// recompute, repeat until stable - a single pass leaves the HEADER and the TEXT
/// keywords disagreeing about where DATA starts.
///
/// `text_start` is [`HEADER_SIZE`] for the first data set in a file; datasets reached
/// through a `$NEXTDATA` chain begin after the previous data set's DATA instead, since
/// the 58-byte primary HEADER exists only once, at file start.
///
/// # Errors
/// Returns `Err` if `data_len` is zero, if serialization fails, or if the offsets
/// fail to settle within [`MAX_LAYOUT_PASSES`].
pub(crate) fn resolve_layout(
    metadata: &Metadata,
    text_start: usize,
    n_events: usize,
    n_params: usize,
    data_len: usize,
) -> Result<FcsLayout> {
    if data_len == 0 {
        return Err(anyhow!("Cannot lay out an FCS file with an empty DATA segment"));
    }

    // Seed from a real serialization rather than a per-keyword byte guess: only the
    // $BEGINDATA/$ENDDATA digit counts can still move, so the first pass lands within
    // a few bytes even when one keyword is tens of KB. The previous heuristic assumed
    // a flat 50 bytes per keyword, which a 64x40 $TRUOLS_MIXMAT (~30 KB) missed by the
    // entire 30 KB, costing extra full passes over a large TEXT.
    let mut data_start = text_start + serialize_metadata(metadata, n_events, n_params, 0, 0)?.len();
    let mut data_end = data_start + data_len - 1;

    for _ in 0..MAX_LAYOUT_PASSES {
        let text_segment = serialize_metadata(metadata, n_events, n_params, data_start, data_end)?;
        let text_end = text_start + text_segment.len() - 1;
        let next_data_start = text_end + 1;
        let next_data_end = next_data_start + data_len - 1;
        if next_data_start == data_start && next_data_end == data_end {
            return Ok(FcsLayout {
                text_segment,
                text_start,
                text_end,
                data_start,
                data_end,
            });
        }
        data_start = next_data_start;
        data_end = next_data_end;
    }

    Err(anyhow!(
        "FCS TEXT/data offsets failed to converge after {MAX_LAYOUT_PASSES} passes \
         (last data {data_start}-{data_end})"
    ))
}

/// Upper bound on TEXT re-serializations in [`resolve_layout`].
pub(crate) const MAX_LAYOUT_PASSES: usize = 8;

pub(crate) fn serialize_metadata(
    metadata: &Metadata,
    n_events: usize,
    n_params: usize,
    data_start: usize,
    data_end: usize,
) -> Result<Vec<u8>> {
    let delimiter = metadata.delimiter as u8;
    let mut text_segment = Vec::new();

    // Helper to add keyword-value pair
    let mut add_keyword = |key: &str, value: &str| {
        text_segment.push(delimiter);
        text_segment.extend_from_slice(format!("${}", key).as_bytes());
        text_segment.push(delimiter);
        text_segment.extend_from_slice(value.as_bytes());
    };

    // Required keywords (order matters for FCS compatibility)
    // Write these first, then metadata keywords will be added (some may overwrite these)
    add_keyword("BEGINANALYSIS", "0");
    add_keyword("ENDANALYSIS", "0");
    add_keyword("BEGINSTEXT", "0");
    add_keyword("ENDSTEXT", "0");
    add_keyword("BEGINDATA", &data_start.to_string());
    add_keyword("ENDDATA", &data_end.to_string());

    // Ensure required keywords are written (use metadata values if present, otherwise defaults)
    let byteord_value = metadata
        .keywords
        .get("$BYTEORD")
        .and_then(|k| match k {
            Keyword::Byte(ByteKeyword::BYTEORD(bo)) => Some(bo.to_keyword_str()),
            _ => None,
        })
        .unwrap_or("1,2,3,4");
    add_keyword("BYTEORD", byteord_value);

    let datatype_value = metadata
        .keywords
        .get("$DATATYPE")
        .and_then(|k| match k {
            Keyword::Byte(ByteKeyword::DATATYPE(dt)) => Some(dt.to_keyword_str()),
            _ => None,
        })
        .unwrap_or("F");
    add_keyword("DATATYPE", datatype_value);

    let mode_value = metadata
        .keywords
        .get("$MODE")
        .and_then(|k| match k {
            Keyword::String(sk) => Some(sk.get_str().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "L".to_string());
    add_keyword("MODE", &mode_value);

    add_keyword("PAR", &n_params.to_string());
    add_keyword("TOT", &n_events.to_string());

    let nextdata_value = metadata
        .keywords
        .get("$NEXTDATA")
        .and_then(|k| match k {
            Keyword::String(sk) => Some(sk.get_str().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "0".to_string());
    add_keyword("NEXTDATA", &nextdata_value);

    // Serialize all other keywords from metadata
    let mut sorted_keys: Vec<_> = metadata.keywords.keys().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        // Skip keywords we've already written
        if matches!(
            key.as_str(),
            "$BEGINANALYSIS"
                | "$ENDANALYSIS"
                | "$BEGINSTEXT"
                | "$ENDSTEXT"
                | "$BEGINDATA"
                | "$ENDDATA"
                | "$BYTEORD"
                | "$DATATYPE"
                | "$MODE"
                | "$PAR"
                | "$TOT"
                | "$NEXTDATA"
        ) {
            continue;
        }

        let keyword = metadata
            .keywords
            .get(key)
            .ok_or_else(|| anyhow!("Keyword '{}' not found in metadata", key))?;
        let value_str = match keyword {
            Keyword::Int(int_kw) => match int_kw {
                IntegerKeyword::TOT(_) => {
                    // Use actual event count
                    n_events.to_string()
                }
                IntegerKeyword::PAR(_) => {
                    // Use actual parameter count
                    n_params.to_string()
                }
                _ => int_kw.get_str().to_string(),
            },
            Keyword::String(str_kw) => str_kw.get_str().to_string(),
            Keyword::Float(float_kw) => float_kw.to_string(),
            Keyword::Byte(byte_kw) => byte_kw.get_str().to_string(),
            Keyword::Mixed(mixed_kw) => {
                // Serialize mixed keywords in FCS format (not Debug format)
                use crate::keyword::MixedKeyword;
                match mixed_kw {
                    MixedKeyword::PnE(f1, f2) => format!("{},{}", f1, f2),
                    MixedKeyword::PnL(wavelengths) => {
                        format!(
                            "({})",
                            wavelengths
                                .iter()
                                .map(|w| w.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                    MixedKeyword::PnD(scale_type, lower, upper) => {
                        format!("({},{},{})", scale_type, lower, upper)
                    }
                    MixedKeyword::PnCalibration(f1, s) => {
                        format!("{}/{}", f1, s)
                    }
                    MixedKeyword::RnW(widths) => {
                        format!(
                            "({})",
                            widths
                                .iter()
                                .map(|w| w.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                    MixedKeyword::SPILLOVER {
                        n_parameters,
                        parameter_names,
                        matrix_values,
                    } => {
                        let mut result = format!("{}", n_parameters);
                        for name in parameter_names {
                            result.push(',');
                            result.push_str(name);
                        }
                        for val in matrix_values {
                            result.push(',');
                            result.push_str(&val.to_string());
                        }
                        result
                    }
                    MixedKeyword::MixingMatrix {
                        n_detectors,
                        n_endmembers,
                        detector_names,
                        endmember_names,
                        matrix_values,
                    } => {
                        // nDet,nEm,<detector names>,<endmember names>,<row-major values>
                        let mut result = format!("{n_detectors},{n_endmembers}");
                        for name in detector_names.iter().chain(endmember_names) {
                            result.push(',');
                            result.push_str(name);
                        }
                        for val in matrix_values {
                            result.push(',');
                            result.push_str(&val.to_string());
                        }
                        result
                    }
                    MixedKeyword::GnE(f1, f2) => format!("{},{}", f1, f2),
                }
            }
        };

        // Remove $ prefix for serialization (it will be added back)
        let key_without_prefix = key.strip_prefix('$').unwrap_or(key);
        add_keyword(key_without_prefix, &value_str);
    }

    // Add trailing delimiter after the last value to properly terminate the text segment
    // The parser expects the text segment to end with a delimiter after the last value
    text_segment.push(delimiter);

    Ok(text_segment)
}

fn serialize_data(df: &DataFrame, metadata: &Metadata) -> Result<Vec<u8>> {
    let n_params = df.width();

    // Get byte order
    let byte_order = metadata
        .get_byte_order()
        .unwrap_or(&ByteOrder::LittleEndian);
    let is_little_endian = matches!(byte_order, ByteOrder::LittleEndian);

    // Serialize row by row (FCS format: event1_param1, event1_param2, ..., event2_param1, ...)
    // Get all columns as f32 slices for efficient access
    let column_names = df.get_column_names();
    let mut column_data: Vec<&[f32]> = Vec::with_capacity(n_params);

    for col_name in &column_names {
        let series = df.column(col_name)?;
        let f32_series = series
            .f32()
            .map_err(|e| anyhow!("Column {} is not f32: {}", col_name, e))?;
        let slice = f32_series
            .cont_slice()
            .map_err(|e| anyhow!("Column {} data is not contiguous: {}", col_name, e))?;
        column_data.push(slice);
    }

    serialize_f32_columns(&column_data, is_little_endian)
}

/// Row-major float32 DATA segment bytes from contiguous column slices.
///
/// Note: LE bytemuck single-buffer variants were A/B'd (see `fcs/docs/PERF_AB.md`)
/// and did not meet the ≥5% keep rule at 1M×20; keep the endian writer path.
#[doc(hidden)]
pub fn serialize_f32_columns(column_data: &[&[f32]], is_little_endian: bool) -> Result<Vec<u8>> {
    let n_params = column_data.len();
    if n_params == 0 {
        return Ok(Vec::new());
    }
    let n_events = column_data[0].len();
    for (i, col) in column_data.iter().enumerate() {
        if col.len() != n_events {
            return Err(anyhow!(
                "Column {} length {} != n_events {}",
                i,
                col.len(),
                n_events
            ));
        }
    }

    let mut data = Vec::with_capacity(n_events * n_params * 4);
    for row_idx in 0..n_events {
        for col_data in column_data {
            let value = col_data[row_idx];
            if is_little_endian {
                data.write_f32::<LittleEndian>(value)?;
            } else {
                use byteorder::BigEndian;
                data.write_f32::<BigEndian>(value)?;
            }
        }
    }

    Ok(data)
}

/// Largest offset representable in an 8-byte ASCII HEADER field.
pub(crate) const MAX_HEADER_OFFSET: usize = 99_999_999;

/// Write an 8-byte ASCII, right-justified offset into `header` at `start`.
///
/// Offsets wider than 8 digits are written as `0`. FCS 3.1/3.2 defines that as the
/// signal to read the real value from the corresponding TEXT keyword pair -
/// `$BEGINDATA`/`$ENDDATA` for DATA, `$BEGINANALYSIS`/`$ENDANALYSIS` for ANALYSIS -
/// which [`serialize_metadata`] always emits. Without this the `format!` produces a
/// 9-byte string and `copy_from_slice` panics, which a ~100 MB DATA segment reaches.
fn write_header_offset(header: &mut [u8], start: usize, value: usize) {
    let field = &mut header[start..start + 8];
    if value > MAX_HEADER_OFFSET {
        field.copy_from_slice(b"       0");
    } else {
        field.copy_from_slice(format!("{value:>8}").as_bytes());
    }
}

pub(crate) fn build_header(
    version: &Version,
    text_start: usize,
    text_end: usize,
    data_start: usize,
    data_end: usize,
) -> Result<Vec<u8>> {
    let mut header = vec![0u8; 58];

    // Version string (bytes 0-5)
    let version_str = format!("{}", version);
    if version_str.len() > 6 {
        return Err(anyhow!("Version string too long: {}", version_str));
    }
    header[0..version_str.len()].copy_from_slice(version_str.as_bytes());

    // 4 spaces (bytes 6-9)
    header[6..10].fill(b' ');

    // Text segment offsets (bytes 10-17 and 18-25) - right-aligned, space-padded.
    // Unlike DATA and ANALYSIS these have no `0` escape: the keywords that would
    // carry an oversized value live inside TEXT, so a reader that cannot locate
    // TEXT from the HEADER cannot recover. ($BEGINSTEXT/$ENDSTEXT describe the
    // *supplemental* TEXT segment, not this one.) Refuse rather than emit a file
    // nothing can open.
    if text_start > MAX_HEADER_OFFSET || text_end > MAX_HEADER_OFFSET {
        return Err(anyhow!(
            "TEXT segment offsets {text_start}-{text_end} exceed the 8-digit HEADER field \
             and TEXT has no keyword fallback"
        ));
    }
    write_header_offset(&mut header, 10, text_start);
    write_header_offset(&mut header, 18, text_end);

    // Data segment offsets (bytes 26-33 and 34-41)
    write_header_offset(&mut header, 26, data_start);
    write_header_offset(&mut header, 34, data_end);

    // Analysis segment offsets (bytes 42-49 and 50-57) - set to 0
    header[42..50].copy_from_slice(b"       0");
    header[50..58].copy_from_slice(b"       0");

    Ok(header)
}

#[cfg(test)]
mod header_offset_tests {
    use super::*;

    /// A ~100 MB DATA segment (3M events x 64 detectors is ~768 MB) used to panic
    /// here: `format!("{:>8}", 100_000_000)` is 9 bytes into an 8-byte slice.
    #[test]
    fn data_offsets_beyond_eight_digits_fall_back_to_zero() {
        let header = build_header(&Version::V3_1, 58, 4_095, 100_000_000, 900_000_000)
            .expect("oversized DATA offsets must not fail");

        assert_eq!(header.len(), 58);
        assert_eq!(&header[26..34], b"       0", "$BEGINDATA escape");
        assert_eq!(&header[34..42], b"       0", "$ENDDATA escape");
        // TEXT is unaffected and still carries real offsets.
        assert_eq!(&header[10..18], b"      58");
    }

    #[test]
    fn data_offsets_at_the_eight_digit_boundary_are_written_verbatim() {
        let header = build_header(&Version::V3_1, 58, 4_095, 10_000_000, MAX_HEADER_OFFSET)
            .expect("boundary offsets fit");

        assert_eq!(&header[26..34], b"10000000");
        assert_eq!(&header[34..42], b"99999999");
    }

    /// TEXT has no keyword escape - see the comment in `build_header`.
    #[test]
    fn oversized_text_offsets_are_rejected() {
        let err = build_header(&Version::V3_1, 58, 100_000_000, 100_000_058, 100_001_000)
            .expect_err("oversized TEXT must be refused, not silently zeroed");

        assert!(
            err.to_string().contains("TEXT segment offsets"),
            "unexpected error: {err}"
        );
    }
}

#[cfg(test)]
mod offset_convergence_tests {
    use super::*;
    use crate::{
        Header, Parameter, TransformType,
        file::AccessWrapper,
        keyword::{IntegerKeyword, Keyword, MixedKeyword},
        parameter::ParameterMap,
    };
    use polars::prelude::Column;
    use std::sync::Arc;

    #[test]
    fn write_fcs_header_and_text_data_offsets_agree() {
        // Many keywords make the TEXT size estimate wrong so the old one-shot
        // write left stale $BEGINDATA/$ENDDATA in TEXT while the primary header
        // had the corrected offsets.
        let tmp = std::env::temp_dir().join("flow_fcs_offset_agree.fcs");
        let stub = std::env::temp_dir().join("flow_fcs_offset_agree_src.tmp");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&stub).expect("stub");
            f.write_all(b"x").expect("write stub");
        }

        let n_events = 1_000usize;
        let mut columns = Vec::new();
        columns.push(Column::new(
            "FSC-A".into(),
            vec![1.0f32; n_events],
        ));
        columns.push(Column::new(
            "FL1-A".into(),
            vec![2.0f32; n_events],
        ));
        let df = DataFrame::new_infer_height(columns).expect("df");

        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );
        params.insert(
            "FL1-A".into(),
            Parameter::new(&2, "FL1-A", "FITC", &TransformType::Linear),
        );

        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        // Inflate TEXT so estimate_text_segment_size undershoots/overshoots.
        for i in 0..80 {
            metadata.insert_string_keyword(
                format!("CUSTOM{i}"),
                format!("value-with-padding-{i:04}-xxxxxxxx"),
            );
        }
        metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
        metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
        metadata.insert_string_keyword("$P1N".into(), "FSC-A".into());
        metadata.insert_string_keyword("$P2N".into(), "FL1-A".into());
        metadata.insert_string_keyword("$P1S".into(), "".into());
        metadata.insert_string_keyword("$P2S".into(), "FITC".into());
        for n in 1..=2 {
            metadata.keywords.insert(
                format!("$P{n}B"),
                Keyword::Int(IntegerKeyword::PnB(32)),
            );
            metadata.keywords.insert(
                format!("$P{n}R"),
                Keyword::Int(IntegerKeyword::PnR(262144)),
            );
            metadata.keywords.insert(
                format!("$P{n}E"),
                Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
            );
        }

        let fcs = Fcs {
            header: Header::new(),
            metadata,
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(stub.to_str().unwrap()).expect("access"),
        };

        write_fcs_file(fcs, &tmp).expect("write");

        let bytes = std::fs::read(&tmp).expect("read back");
        let text_start = std::str::from_utf8(&bytes[10..18])
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .expect("text_start");
        let text_end = std::str::from_utf8(&bytes[18..26])
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .expect("text_end");
        let data_start = std::str::from_utf8(&bytes[26..34])
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .expect("data_start");
        let data_end = std::str::from_utf8(&bytes[34..42])
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .expect("data_end");

        assert_eq!(text_start, 58);
        assert_eq!(text_end + 1, data_start);
        assert_eq!(data_end - data_start + 1, n_events * 2 * 4);
        // The file no longer ends at the DATA segment: §3.7's eight CRC bytes sit
        // immediately after it, with nothing in between. Asserting the exact
        // length pins both facts - that the CRC is written, and that it is not
        // separated from DATA by stray padding.
        assert_eq!(
            data_end + 1 + crate::crc::FIELD_LEN,
            bytes.len(),
            "DATA must be followed by exactly the 8-byte CRC field"
        );
        let crc_field = &bytes[data_end + 1..];
        assert_eq!(
            crate::crc::parse_field(&bytes, data_end + 1),
            crate::crc::StoredCrc::Value(crate::crc::compute(&bytes[..=data_end])),
            "stored CRC must match the bytes it covers; field={:?}",
            std::str::from_utf8(crc_field)
        );

        let text = &bytes[text_start..=text_end];
        let delim = text[0];
        let parts: Vec<&[u8]> = text.split(|&b| b == delim).collect();
        let mut kw = std::collections::HashMap::new();
        let mut i = 1;
        while i + 1 < parts.len() {
            let k = std::str::from_utf8(parts[i]).unwrap_or("");
            let v = std::str::from_utf8(parts[i + 1]).unwrap_or("");
            kw.insert(k.to_string(), v.to_string());
            i += 2;
        }
        let begin = data_start.to_string();
        let end = data_end.to_string();
        assert_eq!(
            kw.get("$BEGINDATA").map(String::as_str),
            Some(begin.as_str()),
            "TEXT $BEGINDATA must match primary header"
        );
        assert_eq!(
            kw.get("$ENDDATA").map(String::as_str),
            Some(end.as_str()),
            "TEXT $ENDDATA must match primary header"
        );

        let reopened = Fcs::open(tmp.to_str().unwrap()).expect("reopen");
        assert_eq!(reopened.get_event_count_from_dataframe(), n_events);
        assert_eq!(reopened.get_parameter_count_from_dataframe(), 2);

        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&stub);
    }

    /// flow-crates-x17.2: a single very large keyword must not defeat offset
    /// resolution. `$TRUOLS_MIXMAT` for a 64-detector x 40-endmember panel is
    /// 2,560 ASCII floats plus 104 names in one keyword value - roughly 17 KB,
    /// which is 340x the flat "50 bytes per keyword" the old estimate assumed.
    /// Asserts the resolved layout is a genuine fixpoint rather than
    /// merely "didn't error": re-serializing TEXT with the offsets the layout
    /// reports must reproduce it byte-for-byte, which is exactly the property
    /// a reader relies on when it trusts TEXT's `$BEGINDATA` over the HEADER.
    #[test]
    fn a_very_large_keyword_still_resolves_to_a_stable_layout() {
        let n_events = 100usize;
        let n_params = 2usize;
        let data_len = n_events * n_params * 4;

        // The real `$TRUOLS_MIXMAT` keyword at the shape a spectral panel
        // actually produces. Alternating sign because unmixing coefficients go
        // negative, and the leading `-` is a byte the serializer must account
        // for.
        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
        metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
        metadata.insert_string_keyword("$P1N".into(), "FSC-A".into());
        metadata.insert_string_keyword("$P2N".into(), "FL1-A".into());
        metadata.keywords.insert(
            "$TRUOLS_MIXMAT".into(),
            Keyword::Mixed(MixedKeyword::MixingMatrix {
                n_detectors: 64,
                n_endmembers: 40,
                detector_names: (1..=64).map(|d| format!("D{d}-A")).collect(),
                endmember_names: (1..=40).map(|e| format!("EM{e}")).collect(),
                matrix_values: (0..64 * 40)
                    .map(|i| {
                        let v = i as f32 / 1000.0;
                        if i % 2 == 0 { v } else { -v }
                    })
                    .collect(),
            }),
        );

        let layout = resolve_layout(&metadata, HEADER_SIZE, n_events, n_params, data_len)
            .expect("30 KB keyword must not defeat layout resolution");

        assert_eq!(layout.text_start, HEADER_SIZE);
        assert_eq!(
            layout.text_end - layout.text_start + 1,
            layout.text_segment.len(),
            "reported TEXT extent must match the bytes actually produced"
        );
        assert_eq!(
            layout.data_start,
            layout.text_end + 1,
            "DATA must begin immediately after TEXT"
        );
        assert_eq!(layout.data_end - layout.data_start + 1, data_len);
        assert!(
            layout.text_segment.len() > 15_000,
            "TEXT should carry the large keyword, got {} bytes",
            layout.text_segment.len()
        );

        // The fixpoint check: feeding the resolved offsets back into the
        // serializer must be a no-op. If TEXT grew or shrank here, the
        // `$BEGINDATA` baked into TEXT would disagree with where DATA lands.
        let reserialized = serialize_metadata(
            &metadata,
            n_events,
            n_params,
            layout.data_start,
            layout.data_end,
        )
        .expect("reserialize");
        assert_eq!(
            reserialized, layout.text_segment,
            "resolved layout is not a fixpoint"
        );
    }

    /// flow-crates-x17.5: the serializer arm and `parse_mixing_matrix` are two
    /// independent statements of the same encoding, so only a trip through a
    /// real file proves they agree. Parser-only tests would pass happily while
    /// the writer emitted a format nothing could read back.
    #[test]
    fn a_mixing_matrix_survives_write_and_reopen() {
        let tmp = std::env::temp_dir().join("flow_fcs_mixmat_roundtrip.fcs");
        let stub = std::env::temp_dir().join("flow_fcs_mixmat_roundtrip_src.tmp");
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&stub).expect("stub");
            f.write_all(b"x").expect("write stub");
        }

        // Rectangular, with a negative coefficient and a name carrying the `-`
        // that detector labels normally do.
        let detector_names = vec!["B1-A".to_string(), "B2-A".to_string()];
        let endmember_names = vec!["FITC".to_string(), "PE".to_string(), "AF".to_string()];
        let matrix_values = vec![0.9375f32, 0.125, -0.03125, 0.0625, 0.8125, 0.25];

        let n_events = 10usize;
        let df = DataFrame::new_infer_height(vec![
            Column::new("B1-A".into(), vec![1.0f32; n_events]),
            Column::new("B2-A".into(), vec![2.0f32; n_events]),
        ])
        .expect("df");

        let mut params = ParameterMap::default();
        params.insert(
            "B1-A".into(),
            Parameter::new(&1, "B1-A", "B1-A", &TransformType::Linear),
        );
        params.insert(
            "B2-A".into(),
            Parameter::new(&2, "B2-A", "B2-A", &TransformType::Linear),
        );

        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
        metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
        metadata.insert_string_keyword("$P1N".into(), "B1-A".into());
        metadata.insert_string_keyword("$P2N".into(), "B2-A".into());
        for n in 1..=2 {
            metadata
                .keywords
                .insert(format!("$P{n}B"), Keyword::Int(IntegerKeyword::PnB(32)));
            metadata
                .keywords
                .insert(format!("$P{n}R"), Keyword::Int(IntegerKeyword::PnR(262144)));
        }
        metadata.keywords.insert(
            "$TRUOLS_MIXMAT".into(),
            Keyword::Mixed(MixedKeyword::MixingMatrix {
                n_detectors: 2,
                n_endmembers: 3,
                detector_names: detector_names.clone(),
                endmember_names: endmember_names.clone(),
                matrix_values: matrix_values.clone(),
            }),
        );

        let fcs = Fcs {
            header: Header::new(),
            metadata,
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(stub.to_str().unwrap()).expect("access"),
        };
        write_fcs_file(fcs, &tmp).expect("write");

        let reopened = Fcs::open(tmp.to_str().unwrap()).expect("reopen");
        let recovered = reopened
            .metadata
            .keywords
            .get("$TRUOLS_MIXMAT")
            .expect("$TRUOLS_MIXMAT missing after round trip");

        let Keyword::Mixed(MixedKeyword::MixingMatrix {
            n_detectors,
            n_endmembers,
            detector_names: got_detectors,
            endmember_names: got_endmembers,
            matrix_values: got_values,
        }) = recovered
        else {
            // A dispatch miss lands the keyword in `StringKeyword::Other`,
            // which round-trips its text perfectly while losing the type - so
            // asserting on the value alone would not catch it.
            panic!("$TRUOLS_MIXMAT came back as {recovered:?}, not a MixingMatrix");
        };

        assert_eq!((*n_detectors, *n_endmembers), (2, 3));
        assert_eq!(*got_detectors, detector_names);
        assert_eq!(*got_endmembers, endmember_names);
        // Values are powers of two, so they are exact in f32 ASCII round trip.
        assert_eq!(*got_values, matrix_values);

        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_file(&stub);
    }

    /// flow-crates-d35: `$PnR` (true resolution) must be used to mask off garbage
    /// in the unused high bits of a wider `$PnB` field. Beckman FC500/Gallios/Navios
    /// and older BD instruments commonly store sub-16-bit ADC resolution (e.g. 10-bit,
    /// $PnR=1024) inside a 16-bit field ($PnB=16), leaving the top bits as instrument
    /// noise rather than zeros. Hand-builds the file (not via `write_fcs_file`, which
    /// always serializes f32 data) so the on-disk bytes are real 16-bit integers with
    /// deliberately corrupted high bits.
    #[test]
    fn pnr_mask_strips_garbage_high_bits_from_int16_data() {
        let tmp = std::env::temp_dir().join("flow_fcs_pnr_mask.fcs");

        let n_events = 4usize;
        // True values are all within the declared $PnR=1024 (10-bit) resolution.
        let true_values: [u16; 4] = [0, 1, 500, 1023];
        // Garbage in the unused high 6 bits of the 16-bit field (bits 10-15).
        let garbage: u16 = 0xFC00;

        let mut data_bytes = Vec::with_capacity(n_events * 2);
        for &v in &true_values {
            let corrupted = v | garbage;
            data_bytes.extend_from_slice(&corrupted.to_le_bytes());
        }

        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        metadata.keywords.insert(
            "$BYTEORD".to_string(),
            Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)),
        );
        metadata.keywords.insert(
            "$DATATYPE".to_string(),
            Keyword::Byte(ByteKeyword::DATATYPE(crate::datatype::FcsDataType::I)),
        );
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
        metadata.insert_string_keyword("$P1N".into(), "FL1-A".into());
        metadata.insert_string_keyword("$P1S".into(), "".into());
        metadata
            .keywords
            .insert("$P1B".to_string(), Keyword::Int(IntegerKeyword::PnB(16)));
        metadata
            .keywords
            .insert("$P1R".to_string(), Keyword::Int(IntegerKeyword::PnR(1024)));

        let layout =
            resolve_layout(&metadata, HEADER_SIZE, n_events, 1, data_bytes.len()).expect("layout");
        let FcsLayout {
            text_segment,
            text_start,
            text_end,
            data_start,
            data_end,
        } = layout;

        let header =
            build_header(&Version::V3_1, text_start, text_end, data_start, data_end).expect("header");

        let mut bytes = header;
        bytes.extend_from_slice(&text_segment);
        bytes.extend_from_slice(&data_bytes);
        std::fs::write(&tmp, &bytes).expect("write fcs bytes");

        let fcs = Fcs::open(tmp.to_str().unwrap()).expect("reopen");
        let values = fcs
            .get_parameter_events_slice("FL1-A")
            .expect("FL1-A column");

        let expected: Vec<f32> = true_values.iter().map(|&v| v as f32).collect();
        assert_eq!(
            values, expected.as_slice(),
            "$PnR mask should strip garbage high bits, leaving only the true 10-bit value"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    /// flow-crates-bk6: non-byte-aligned `$PnB` (bit-packed) records must use
    /// `ceil(sum(bits)/8)` for the record stride, not `sum(ceil(bits/8))` — the
    /// latter overcounts whenever a parameter's width isn't a multiple of 8.
    /// Hand-packs 8 channels of `$PnB=10` (80 bits = 10 bytes/event, not the
    /// wrong 16 bytes/event `sum(ceil(10/8))*8` would predict) using the same
    /// MSB-first bit order the production `BitReader` decodes, then asserts
    /// both the corrected stride and the decoded values.
    #[test]
    fn bit_packed_pnb10_record_uses_correct_stride_and_decodes() {
        let tmp = std::env::temp_dir().join("flow_fcs_bit_packed.fcs");

        let n_params = 8usize;
        let n_events = 2usize;
        let bits_per_param = 10usize;
        // All values fit in 10 bits (0..=1023); chosen to exercise low/high/mid values.
        let event_values: [[u16; 8]; 2] = [
            [0, 1, 2, 3, 511, 512, 1000, 1023],
            [1023, 1000, 512, 511, 3, 2, 1, 0],
        ];

        // Pack MSB-first within the byte stream, matching `BitReader::read_bits`.
        let mut data_bytes: Vec<u8> = Vec::new();
        let mut bit_pos = 0usize;
        for event in &event_values {
            for &value in event {
                for i in (0..bits_per_param).rev() {
                    let bit = ((value >> i) & 1) as u8;
                    let byte_idx = bit_pos / 8;
                    if byte_idx == data_bytes.len() {
                        data_bytes.push(0);
                    }
                    let shift = 7 - (bit_pos % 8);
                    data_bytes[byte_idx] |= bit << shift;
                    bit_pos += 1;
                }
            }
        }
        assert_eq!(
            data_bytes.len(),
            20,
            "8 params x 10 bits = 10 bytes/event x 2 events should pack into 20 bytes total"
        );

        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        metadata.keywords.insert(
            "$BYTEORD".to_string(),
            Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)),
        );
        metadata.keywords.insert(
            "$DATATYPE".to_string(),
            Keyword::Byte(ByteKeyword::DATATYPE(crate::datatype::FcsDataType::I)),
        );
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
        metadata
            .keywords
            .insert("$PAR".to_string(), Keyword::Int(IntegerKeyword::PAR(n_params)));
        for n in 1..=n_params {
            metadata.insert_string_keyword(format!("$P{n}N"), format!("P{n}"));
            metadata.insert_string_keyword(format!("$P{n}S"), "".into());
            metadata.keywords.insert(
                format!("$P{n}B"),
                Keyword::Int(IntegerKeyword::PnB(bits_per_param)),
            );
            metadata.keywords.insert(
                format!("$P{n}R"),
                Keyword::Int(IntegerKeyword::PnR(1024)),
            );
        }

        assert_eq!(
            metadata.calculate_bytes_per_event().expect("stride"),
            10,
            "record stride must be ceil(sum(bits)/8) = ceil(80/8) = 10, not sum(ceil(10/8)) = 16"
        );

        let FcsLayout {
            text_segment,
            text_start,
            text_end,
            data_start,
            data_end,
        } = resolve_layout(&metadata, HEADER_SIZE, n_events, n_params, data_bytes.len())
            .expect("layout");

        let header =
            build_header(&Version::V3_1, text_start, text_end, data_start, data_end).expect("header");

        let mut bytes = header;
        bytes.extend_from_slice(&text_segment);
        bytes.extend_from_slice(&data_bytes);
        std::fs::write(&tmp, &bytes).expect("write fcs bytes");

        let fcs = Fcs::open(tmp.to_str().unwrap()).expect("reopen");
        for (param_idx, expected_values) in (1..=n_params).zip(
            (0..n_params).map(|p| event_values.iter().map(|e| e[p] as f32).collect::<Vec<_>>()),
        ) {
            let channel = format!("P{param_idx}");
            let values = fcs
                .get_parameter_events_slice(&channel)
                .unwrap_or_else(|_| panic!("{channel} column"));
            assert_eq!(
                values, expected_values.as_slice(),
                "channel {channel} should decode the bit-packed values in event order"
            );
        }

        let _ = std::fs::remove_file(&tmp);
    }

    /// flow-crates-1mg: `$NEXTDATA` traversal for multi-dataset FCS files (e.g. Beckman
    /// `.lmd` exports, which chain several datasets in one file). Hand-builds a
    /// two-dataset file: dataset 1 is a normal primary-header + TEXT + DATA layout
    /// whose `$NEXTDATA` points at dataset 2's TEXT start; dataset 2 has NO second
    /// 58-byte primary header (the FCS HEADER only exists once, at file start) — it's
    /// just a second TEXT segment immediately following dataset 1's DATA, followed by
    /// its own DATA. `Fcs::open()` must still return only dataset 1 (unchanged,
    /// zero-cost default); `Fcs::open_all()` must return both, in order.
    #[test]
    fn open_all_traverses_nextdata_chain_across_two_datasets() {
        let tmp = std::env::temp_dir().join("flow_fcs_nextdata_chain.fcs");

        fn build_dataset_metadata(nextdata: usize) -> Metadata {
            let mut metadata = Metadata::new();
            metadata.delimiter = '\u{000c}';
            metadata.keywords.insert(
                "$BYTEORD".to_string(),
                Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)),
            );
            metadata.keywords.insert(
                "$DATATYPE".to_string(),
                Keyword::Byte(ByteKeyword::DATATYPE(crate::datatype::FcsDataType::F)),
            );
            metadata.insert_string_keyword("$MODE".into(), "L".into());
            metadata.insert_string_keyword("$NEXTDATA".into(), nextdata.to_string());
            metadata.insert_string_keyword("$P1N".into(), "FSC-A".into());
            metadata.insert_string_keyword("$P1S".into(), "".into());
            metadata
                .keywords
                .insert("$P1B".to_string(), Keyword::Int(IntegerKeyword::PnB(32)));
            metadata.keywords.insert(
                "$P1R".to_string(),
                Keyword::Int(IntegerKeyword::PnR(262_144)),
            );
            metadata
                .keywords
                .insert("$P1E".to_string(), Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)));
            metadata
        }

        /// Converge TEXT/DATA offsets for a dataset whose TEXT starts at `text_start`.
        /// Chained datasets do not start at [`HEADER_SIZE`] — the 58-byte primary
        /// HEADER exists only once, at file start — which is why `resolve_layout`
        /// takes `text_start` rather than assuming it.
        fn build_dataset_bytes(
            metadata: &Metadata,
            text_start: usize,
            n_events: usize,
            n_params: usize,
            data_bytes: &[u8],
        ) -> (Vec<u8>, usize, usize, usize) {
            let FcsLayout {
                text_segment,
                text_end,
                data_start,
                data_end,
                ..
            } = resolve_layout(metadata, text_start, n_events, n_params, data_bytes.len())
                .expect("layout");
            (text_segment, text_end, data_start, data_end)
        }

        let n_events = 3usize;
        let dataset1_values: [f32; 3] = [1.0, 2.0, 3.0];
        let dataset2_values: [f32; 3] = [10.0, 20.0, 30.0];
        let data_bytes1 = serialize_f32_columns(&[&dataset1_values], true).expect("data1");
        let data_bytes2 = serialize_f32_columns(&[&dataset2_values], true).expect("data2");

        let text_start1 = 58usize;

        // Converge dataset 1's own TEXT/DATA layout AND the file offset where dataset 2's
        // TEXT begins ($NEXTDATA) together: changing $NEXTDATA's digit count changes
        // dataset 1's TEXT length, which shifts where dataset 2 starts.
        let mut next_data_guess = text_start1 + data_bytes1.len() * 2; // arbitrary seed
        let (text_segment1, _text_end1, data_start1, data_end1) = loop {
            let metadata1 = build_dataset_metadata(next_data_guess);
            let (text_segment1, text_end1, data_start1, data_end1) =
                build_dataset_bytes(&metadata1, text_start1, n_events, 1, &data_bytes1);
            let actual_next_data = data_end1 + 1;
            if actual_next_data == next_data_guess {
                break (text_segment1, text_end1, data_start1, data_end1);
            }
            next_data_guess = actual_next_data;
        };
        let text_start2 = data_end1 + 1;

        let metadata2 = build_dataset_metadata(0);
        let (text_segment2, _text_end2, _data_start2, data_end2) =
            build_dataset_bytes(&metadata2, text_start2, n_events, 1, &data_bytes2);

        let header = build_header(&Version::V3_1, text_start1, data_start1 - 1, data_start1, data_end1)
            .expect("header");

        let mut bytes = header;
        bytes.extend_from_slice(&text_segment1);
        bytes.extend_from_slice(&data_bytes1);
        bytes.extend_from_slice(&text_segment2);
        bytes.extend_from_slice(&data_bytes2);
        assert_eq!(bytes.len(), data_end2 + 1);
        std::fs::write(&tmp, &bytes).expect("write fcs bytes");

        // open() must still return only dataset 1 — the zero-cost, unchanged default.
        let first_only = Fcs::open(tmp.to_str().unwrap()).expect("open first dataset");
        assert_eq!(
            first_only
                .get_parameter_events_slice("FSC-A")
                .expect("FSC-A column"),
            dataset1_values.as_slice()
        );

        // open_all() must return both datasets, in order.
        let all = Fcs::open_all(tmp.to_str().unwrap()).expect("open_all");
        assert_eq!(all.len(), 2, "expected 2 chained datasets");
        assert_eq!(
            all[0]
                .get_parameter_events_slice("FSC-A")
                .expect("dataset 1 FSC-A column"),
            dataset1_values.as_slice()
        );
        assert_eq!(
            all[1]
                .get_parameter_events_slice("FSC-A")
                .expect("dataset 2 FSC-A column"),
            dataset2_values.as_slice()
        );

        let _ = std::fs::remove_file(&tmp);
    }
}

/// flow-crates-x17.4: conformance checking on the write path.
///
/// The behaviour under test is the *policy*, not the rules (those are covered
/// in `conformance::tests`): a non-conformant file must still be written by
/// default, and must be refused under `Strict`.
#[cfg(test)]
mod conformance_on_write_tests {
    use super::*;
    use crate::{
        Header, Parameter, TransformType,
        file::AccessWrapper,
        keyword::{IntegerKeyword, Keyword},
        parameter::ParameterMap,
    };
    use polars::prelude::Column;
    use std::sync::Arc;

    /// An `Fcs` declaring FCS 3.2 with a complete required-keyword set.
    /// Tests remove or add exactly one thing from this baseline.
    fn v3_2_fcs(tag: &str) -> (Fcs, std::path::PathBuf) {
        let stub = std::env::temp_dir().join(format!("flow_fcs_conf_{tag}_src.tmp"));
        std::fs::write(&stub, b"x").expect("stub");

        let n_events = 10usize;
        let df = DataFrame::new_infer_height(vec![
            Column::new("FSC-A".into(), vec![1.0f32; n_events]),
            Column::new("FL1-A".into(), vec![2.0f32; n_events]),
        ])
        .expect("df");

        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );
        params.insert(
            "FL1-A".into(),
            Parameter::new(&2, "FL1-A", "FITC", &TransformType::Linear),
        );

        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        for (key, value) in [
            ("$BEGINDATA", "0"),
            ("$BYTEORD", "1,2,3,4"),
            ("$CYT", "Test Cytometer"),
            ("$DATATYPE", "F"),
            ("$ENDDATA", "0"),
            ("$NEXTDATA", "0"),
            ("$PAR", "2"),
            ("$TOT", "10"),
            ("$P1N", "FSC-A"),
            ("$P2N", "FL1-A"),
        ] {
            metadata.insert_string_keyword(key.to_string(), value.to_string());
        }
        for n in 1..=2 {
            metadata
                .keywords
                .insert(format!("$P{n}B"), Keyword::Int(IntegerKeyword::PnB(32)));
            metadata
                .keywords
                .insert(format!("$P{n}R"), Keyword::Int(IntegerKeyword::PnR(262_144)));
        }

        let mut header = Header::new();
        header.version = Version::V3_2;

        let fcs = Fcs {
            header,
            metadata,
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(stub.to_str().unwrap()).expect("access"),
        };
        (fcs, stub)
    }

    #[test]
    fn a_non_conformant_file_is_still_written_by_default() {
        let (mut fcs, stub) = v3_2_fcs("warn");
        // $CYT is required in 3.2 and in nothing earlier - a realistic gap for
        // a file upgraded from 3.1 without touching the keyword set.
        fcs.metadata.keywords.remove("$CYT");

        let out = std::env::temp_dir().join("flow_fcs_conf_warn.fcs");
        write_fcs_file(fcs, &out).expect("Warn policy must not block the write");
        assert!(out.exists(), "file should have been written");

        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&stub);
    }

    #[test]
    fn strict_policy_refuses_and_names_every_violation() {
        let (mut fcs, stub) = v3_2_fcs("strict");
        // Two violations from two different rules. $CYT is the only entry in
        // 3.2's required set the serializer does not supply itself, so a
        // second rule has to be tripped to test that reporting accumulates.
        fcs.metadata.keywords.remove("$CYT");
        fcs.metadata
            .keywords
            .insert("$P2B".to_string(), Keyword::Int(IntegerKeyword::PnB(10)));

        let out = std::env::temp_dir().join("flow_fcs_conf_strict.fcs");
        let err = write_fcs_file_with(
            fcs,
            &out,
            WriteOptions {
                conformance: ConformancePolicy::Strict,
                ..Default::default()
            },
        )
        .expect_err("Strict policy must reject a non-conformant file");

        let message = err.to_string();
        assert!(message.contains("$CYT"), "should name $CYT: {message}");
        assert!(message.contains("$P2B"), "should name $P2B: {message}");
        assert!(
            !out.exists(),
            "nothing should be written when the write is refused"
        );

        let _ = std::fs::remove_file(&stub);
    }

    #[test]
    fn strict_policy_tolerates_deprecation_warnings() {
        let (mut fcs, stub) = v3_2_fcs("deprecated");
        // Deprecated in 3.2, but the file is otherwise conformant. Stripping
        // these would lose information a 3.1 reader still wants, so they must
        // not be treated as errors.
        fcs.metadata
            .insert_string_keyword("$PLATEID".to_string(), "PLATE-1".to_string());
        fcs.metadata
            .insert_string_keyword("$DATE".to_string(), "06-AUG-2026".to_string());

        let out = std::env::temp_dir().join("flow_fcs_conf_deprecated.fcs");
        write_fcs_file_with(
            fcs,
            &out,
            WriteOptions {
                conformance: ConformancePolicy::Strict,
                ..Default::default()
            },
        )
        .expect("deprecation warnings must not block a strict write");
        assert!(out.exists());

        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&stub);
    }

    /// A passthrough write declares whatever version came in.
    ///
    /// `tru-ols` stamps its *derived* products as 3.2, and the temptation when
    /// doing that is to make 3.2 the writer's default. It must not be:
    /// re-declaring a vendor file as 3.2 asserts conformance nobody checked,
    /// and `peacoqc-cli` writes vendor files straight through.
    #[test]
    fn the_writer_preserves_the_declared_version_rather_than_imposing_one() {
        for version in [Version::V2_0, Version::V3_0, Version::V3_1, Version::V3_2] {
            let (mut fcs, stub) = v3_2_fcs(&format!("passthrough_{version}"));
            fcs.header.version = version;

            let out = std::env::temp_dir().join(format!("flow_fcs_version_{version}.fcs"));
            write_fcs_file(fcs, &out).expect("write");

            let reopened = Fcs::open(out.to_str().expect("utf8")).expect("reopen");
            assert_eq!(
                reopened.header.version, version,
                "{version} must survive a write/reopen unchanged"
            );

            let _ = std::fs::remove_file(&out);
            let _ = std::fs::remove_file(&stub);
        }
    }

    /// FCS 3.2 §3.7 round-trip: a file we write carries a CRC that verifies.
    #[test]
    fn a_written_file_carries_a_crc_that_verifies_on_reopen() {
        let (fcs, stub) = v3_2_fcs("crc_roundtrip");
        let out = std::env::temp_dir().join("flow_fcs_crc_roundtrip.fcs");
        write_fcs_file(fcs, &out).expect("write");

        let reopened = Fcs::open(out.to_str().expect("utf8")).expect("reopen");
        let computed = reopened.computed_crc().expect("computable");
        assert_eq!(
            reopened.stored_crc(),
            crate::crc::StoredCrc::Value(computed),
            "the file must store the CRC of its own bytes"
        );
        reopened.verify_crc().expect("a freshly written file must verify");
        Fcs::open_verified(out.to_str().expect("utf8")).expect("open_verified must accept it");

        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&stub);
    }

    /// Corrupting a DATA byte must be caught. This is the only test that proves
    /// the CRC is doing its job rather than merely being present and
    /// self-consistent - a stored CRC computed over the wrong range would still
    /// round-trip happily in the test above.
    #[test]
    fn a_flipped_data_byte_is_rejected_by_open_verified_but_tolerated_by_open() {
        let (fcs, stub) = v3_2_fcs("crc_corrupt");
        let out = std::env::temp_dir().join("flow_fcs_crc_corrupt.fcs");
        write_fcs_file(fcs, &out).expect("write");

        // Flip one bit in the middle of DATA, leaving every offset and the CRC
        // field itself untouched, so nothing but the checksum can detect it.
        let mut bytes = std::fs::read(&out).expect("read back");
        let data_start = std::str::from_utf8(&bytes[26..34])
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .expect("data_start");
        bytes[data_start + 4] ^= 0b0000_0001;
        std::fs::write(&out, &bytes).expect("rewrite");

        let path = out.to_str().expect("utf8");
        let reopened = Fcs::open(path).expect("open must tolerate a bad CRC and still parse");
        assert!(
            reopened.stored_crc().conflicts_with(reopened.computed_crc().expect("computable")),
            "the flipped bit must change the computed CRC"
        );
        assert!(reopened.verify_crc().is_err(), "verify_crc must report it");

        let error = Fcs::open_verified(path).expect_err("open_verified must refuse a corrupt file");
        let message = error.to_string();
        assert!(
            message.contains("CRC mismatch"),
            "error should name the problem: {message}"
        );

        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&stub);
    }

    /// The opt-out still writes eight bytes. Omitting the field entirely - which
    /// is what this crate did before `flow-crates-x17.3` - is not one of the two
    /// encodings §3.7 permits.
    #[test]
    fn the_omit_policy_writes_the_not_computed_field_rather_than_nothing() {
        let (fcs, stub) = v3_2_fcs("crc_omit");
        let out = std::env::temp_dir().join("flow_fcs_crc_omit.fcs");
        write_fcs_file_with(
            fcs,
            &out,
            WriteOptions {
                crc: CrcPolicy::Omit,
                ..Default::default()
            },
        )
        .expect("write");

        let bytes = std::fs::read(&out).expect("read back");
        assert_eq!(
            &bytes[bytes.len() - crate::crc::FIELD_LEN..],
            b"00000000",
            "the opt-out is eight ASCII zeroes"
        );

        // An absent CRC asserts nothing, so it must not be treated as corruption.
        let path = out.to_str().expect("utf8");
        let reopened = Fcs::open(path).expect("reopen");
        assert_eq!(reopened.stored_crc(), crate::crc::StoredCrc::Absent);
        reopened.verify_crc().expect("an absent CRC is not a mismatch");
        Fcs::open_verified(path).expect("open_verified must accept a file with no CRC");

        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&stub);
    }

    /// Every FCS file this workspace wrote before CRC support ends at its DATA
    /// segment. Those files must keep opening, and must not be reported as
    /// corrupt - the field is missing, not wrong.
    #[test]
    fn a_pre_crc_file_still_opens_and_is_not_called_corrupt() {
        let (fcs, stub) = v3_2_fcs("crc_legacy");
        let out = std::env::temp_dir().join("flow_fcs_crc_legacy.fcs");
        write_fcs_file(fcs, &out).expect("write");

        // Truncate the CRC field to recreate a file written by the old code.
        let bytes = std::fs::read(&out).expect("read back");
        std::fs::write(&out, &bytes[..bytes.len() - crate::crc::FIELD_LEN]).expect("truncate");

        let path = out.to_str().expect("utf8");
        let reopened = Fcs::open(path).expect("a pre-CRC file must still open");
        assert_eq!(reopened.stored_crc(), crate::crc::StoredCrc::Missing);
        reopened.verify_crc().expect("a missing CRC is not a mismatch");
        Fcs::open_verified(path).expect("open_verified must accept a pre-CRC file");

        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&stub);
    }
}
