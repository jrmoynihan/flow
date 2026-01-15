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
    datatype::FcsDataType,
    header::Header,
    keyword::{IntegerKeyword, Keyword, StringKeyword},
    metadata::Metadata,
    parameter::ParameterMap,
    version::Version,
};
use anyhow::{Result, anyhow};
use byteorder::{LittleEndian, WriteBytesExt};
use polars::prelude::*;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// Write an FCS file to disk
///
/// **Important**: This function closes the memory-mapped file before writing.
/// The original Fcs struct will no longer be able to access the original file
/// after this operation, but the data is preserved in the DataFrame.
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

    // Serialize data segment first (we need its size for metadata)
    let data_segment = serialize_data(df, &fcs.metadata)?;

    // Calculate offsets
    let header_size = 58;
    let text_start = header_size;
    // Estimate text segment size (will recalculate after)
    let estimated_text_size = estimate_text_segment_size(&fcs.metadata, n_events, n_params);
    let estimated_text_end = text_start + estimated_text_size - 1;
    let data_start = estimated_text_end + 1;
    let data_end = data_start + data_segment.len() - 1;

    // Serialize metadata to text segment (now we know data offsets)
    let text_segment = serialize_metadata(&fcs.metadata, n_events, n_params, data_start, data_end)?;

    // Recalculate offsets with actual text segment size
    let text_end = text_start + text_segment.len() - 1;
    let data_start = text_end + 1;
    let data_end = data_start + data_segment.len() - 1;

    // Build header
    let header = build_header(
        &fcs.header.version,
        text_start,
        text_end,
        data_start,
        data_end,
    )?;

    // Write file
    let mut file = File::create(path)?;
    file.write_all(&header)?;
    file.write_all(&text_segment)?;
    file.write_all(&data_segment)?;
    file.sync_all()?;

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
        .unwrap();

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
        .with_column(new_series)
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

fn estimate_text_segment_size(metadata: &Metadata, n_events: usize, n_params: usize) -> usize {
    // Rough estimate: base size + keywords
    let base_size = 200; // Base keywords
    let keyword_size = metadata.keywords.len() * 50; // Average keyword size
    let param_keywords = n_params * 100; // Parameter keywords
    base_size + keyword_size + param_keywords
}

fn serialize_metadata(
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
    add_keyword("BEGINANALYSIS", "0");
    add_keyword("ENDANALYSIS", "0");
    add_keyword("BEGINSTEXT", "0");
    add_keyword("ENDSTEXT", "0");
    add_keyword("BEGINDATA", &data_start.to_string());
    add_keyword("ENDDATA", &data_end.to_string());

    // Serialize all keywords from metadata
    let mut sorted_keys: Vec<_> = metadata.keywords.keys().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        // Skip keywords we've already added
        if matches!(
            key.as_str(),
            "$BEGINANALYSIS"
                | "$ENDANALYSIS"
                | "$BEGINSTEXT"
                | "$ENDSTEXT"
                | "$BEGINDATA"
                | "$ENDDATA"
        ) {
            continue;
        }

        let keyword = metadata.keywords.get(key).unwrap();
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
                _ => int_kw.to_string(),
            },
            Keyword::String(str_kw) => str_kw.to_string(),
            Keyword::Float(float_kw) => float_kw.to_string(),
            Keyword::Byte(byte_kw) => byte_kw.to_string(),
            Keyword::Mixed(mixed_kw) => mixed_kw.to_string(),
        };

        // Remove $ prefix for serialization (it will be added back)
        let key_without_prefix = key.strip_prefix('$').unwrap_or(key);
        add_keyword(key_without_prefix, &value_str);
    }

    Ok(text_segment)
}

fn serialize_data(df: &DataFrame, metadata: &Metadata) -> Result<Vec<u8>> {
    let n_events = df.height();
    let n_params = df.width();

    // Get bytes per parameter from metadata
    let bytes_per_param = metadata
        .calculate_bytes_per_event()
        .map(|bytes_per_event| bytes_per_event / n_params)
        .unwrap_or(4); // Default to 4 bytes (float32)

    let mut data = Vec::with_capacity(n_events * n_params * bytes_per_param);

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

    // Write row by row
    for row_idx in 0..n_events {
        for col_data in &column_data {
            let value = col_data[row_idx];

            // Write as float32 (4 bytes)
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

fn build_header(
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

    // Text segment offsets (bytes 10-17 and 18-25) - right-aligned, space-padded
    let text_start_str = format!("{:>8}", text_start);
    header[10..18].copy_from_slice(text_start_str.as_bytes());
    let text_end_str = format!("{:>8}", text_end);
    header[18..26].copy_from_slice(text_end_str.as_bytes());

    // Data segment offsets (bytes 26-33 and 34-41)
    let data_start_str = format!("{:>8}", data_start);
    header[26..34].copy_from_slice(data_start_str.as_bytes());
    let data_end_str = format!("{:>8}", data_end);
    header[34..42].copy_from_slice(data_end_str.as_bytes());

    // Analysis segment offsets (bytes 42-49 and 50-57) - set to 0
    header[42..50].copy_from_slice(b"       0");
    header[50..58].copy_from_slice(b"       0");

    Ok(header)
}
