//! Export functionality for PeacoQC results
//!
//! This module provides functions to export QC results in various formats:
//! - CSV with boolean values (0/1) - recommended for general use
//! - CSV with numeric codes (2000/6000) - R-compatible format
//! - JSON metadata - structured format with QC metrics
//!
//! # Example
//!
//! ```no_run
//! use peacoqc_rs::{PeacoQCResult, PeacoQCConfig};
//! use std::path::Path;
//!
//! # let result: PeacoQCResult = todo!();
//! # let config: PeacoQCConfig = todo!();
//! // Export boolean CSV (recommended)
//! result.export_csv_boolean("qc_results.csv")?;
//!
//! // Export numeric CSV (R-compatible)
//! result.export_csv_numeric("qc_results_r.csv", 2000, 6000)?;
//!
//! // Export JSON metadata
//! result.export_json_metadata(&config, "qc_metadata.json")?;
//! # Ok::<(), peacoqc_rs::PeacoQCError>(())
//! ```

use crate::error::{PeacoQCError, Result};
use crate::qc::{PeacoQCConfig, PeacoQCResult};
use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QCExportFormat {
    /// Boolean CSV format (0/1 values)
    BooleanCsv,
    /// Numeric CSV format (2000/6000 or custom values)
    NumericCsv,
    /// JSON metadata format
    JsonMetadata,
}

/// Export options for CSV formats
#[derive(Debug, Clone)]
pub struct QCExportOptions {
    /// Column name for CSV exports (default: "PeacoQC")
    pub column_name: String,
    /// Good event value for numeric CSV (default: 2000)
    pub good_value: u16,
    /// Bad event value for numeric CSV (default: 6000)
    pub bad_value: u16,
}

impl Default for QCExportOptions {
    fn default() -> Self {
        Self {
            column_name: "PeacoQC".to_string(),
            good_value: 2000,
            bad_value: 6000,
        }
    }
}

/// JSON structure for QC metadata export
#[derive(Debug, Serialize, serde::Deserialize)]
struct QCJsonMetadata {
    /// Total number of events analyzed
    n_events_before: usize,
    /// Number of good events
    n_events_after: usize,
    /// Number of bad events removed
    n_events_removed: usize,
    /// Percentage of events removed
    percentage_removed: f64,
    /// IT removal percentage (if used)
    it_percentage: Option<f64>,
    /// MAD removal percentage (if used)
    mad_percentage: Option<f64>,
    /// Consecutive filtering percentage
    consecutive_percentage: f64,
    /// Number of bins used
    n_bins: usize,
    /// Events per bin
    events_per_bin: usize,
    /// Channels analyzed
    channels_analyzed: Vec<String>,
    /// QC configuration used
    config: QCConfigJson,
}

/// Simplified config for JSON export
#[derive(Debug, Serialize, serde::Deserialize)]
struct QCConfigJson {
    /// QC mode used
    qc_mode: String,
    /// MAD threshold
    mad: f64,
    /// IT limit
    it_limit: f64,
    /// Consecutive bins threshold
    consecutive_bins: usize,
    /// Remove zeros flag
    remove_zeros: bool,
}

impl From<&PeacoQCConfig> for QCConfigJson {
    fn from(config: &PeacoQCConfig) -> Self {
        Self {
            qc_mode: format!("{:?}", config.determine_good_cells),
            mad: config.mad,
            it_limit: config.it_limit,
            consecutive_bins: config.consecutive_bins,
            remove_zeros: config.remove_zeros,
        }
    }
}

/// Export QC results as boolean CSV (0/1 values)
///
/// This format is recommended for general use and works well with:
/// - pandas: `df[df['PeacoQC'] == 1]`
/// - R: `df[df$PeacoQC == 1, ]`
/// - SQL: `WHERE PeacoQC = 1`
///
/// # Format
///
/// ```csv
/// PeacoQC
/// 1
/// 1
/// 0
/// 1
/// ```
///
/// Where:
/// - `1` = good event (keep)
/// - `0` = bad event (remove)
///
/// # Arguments
///
/// * `result` - PeacoQC result to export
/// * `path` - Output file path
/// * `column_name` - Optional column name (default: "PeacoQC")
///
/// # Errors
///
/// Returns an error if:
/// - The path is invalid
/// - The file cannot be written
/// - The good_cells vector is empty
pub fn export_csv_boolean(
    result: &PeacoQCResult,
    path: impl AsRef<Path>,
    column_name: Option<&str>,
) -> Result<()> {
    let path = path.as_ref();
    let column_name = column_name.unwrap_or("PeacoQC");

    if result.good_cells.is_empty() {
        return Err(PeacoQCError::ExportError(
            "Cannot export empty QC results".to_string(),
        ));
    }

    let file = File::create(path).map_err(|e| {
        PeacoQCError::WriteError(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    let mut writer = BufWriter::new(file);

    // Write header
    writeln!(writer, "{}", column_name)
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to write header: {}", e)))?;

    // Write data
    for &is_good in &result.good_cells {
        let value = if is_good { 1 } else { 0 };
        writeln!(writer, "{}", value)
            .map_err(|e| PeacoQCError::WriteError(format!("Failed to write data: {}", e)))?;
    }

    writer
        .flush()
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to flush file: {}", e)))?;

    Ok(())
}

/// Export QC results as numeric CSV (2000/6000 or custom values)
///
/// This format is compatible with the R PeacoQC package output.
/// Default values match R implementation: 2000 = good, 6000 = bad.
///
/// # Format
///
/// ```csv
/// PeacoQC
/// 2000
/// 2000
/// 6000
/// 2000
/// ```
///
/// Where:
/// - `2000` (or custom good_value) = good event (keep)
/// - `6000` (or custom bad_value) = bad event (remove)
///
/// # Arguments
///
/// * `result` - PeacoQC result to export
/// * `path` - Output file path
/// * `good_value` - Value for good events (default: 2000)
/// * `bad_value` - Value for bad events (default: 6000)
/// * `column_name` - Optional column name (default: "PeacoQC")
///
/// # Errors
///
/// Returns an error if:
/// - The path is invalid
/// - The file cannot be written
/// - The good_cells vector is empty
pub fn export_csv_numeric(
    result: &PeacoQCResult,
    path: impl AsRef<Path>,
    good_value: u16,
    bad_value: u16,
    column_name: Option<&str>,
) -> Result<()> {
    let path = path.as_ref();
    let column_name = column_name.unwrap_or("PeacoQC");

    if result.good_cells.is_empty() {
        return Err(PeacoQCError::ExportError(
            "Cannot export empty QC results".to_string(),
        ));
    }

    let file = File::create(path).map_err(|e| {
        PeacoQCError::WriteError(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    let mut writer = BufWriter::new(file);

    // Write header
    writeln!(writer, "{}", column_name)
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to write header: {}", e)))?;

    // Write data
    for &is_good in &result.good_cells {
        let value = if is_good { good_value } else { bad_value };
        writeln!(writer, "{}", value)
            .map_err(|e| PeacoQCError::WriteError(format!("Failed to write data: {}", e)))?;
    }

    writer
        .flush()
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to flush file: {}", e)))?;

    Ok(())
}

/// Export a full-length good/bad mask as boolean CSV (0/1 values)
///
/// Use this when the mask length matches the original event count (e.g. after
/// expanding margin/doublet-removed events into the "bad" bin so output has one row per input event).
///
/// # Arguments
///
/// * `mask` - Boolean mask (true = good/keep, false = bad/remove), length = original event count
/// * `path` - Output file path
/// * `column_name` - Optional column name (default: "PeacoQC")
pub fn export_csv_boolean_from_mask(
    mask: &[bool],
    path: impl AsRef<Path>,
    column_name: Option<&str>,
) -> Result<()> {
    let path = path.as_ref();
    let column_name = column_name.unwrap_or("PeacoQC");

    if mask.is_empty() {
        return Err(PeacoQCError::ExportError(
            "Cannot export empty mask".to_string(),
        ));
    }

    let file = File::create(path).map_err(|e| {
        PeacoQCError::WriteError(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    let mut writer = BufWriter::new(file);

    writeln!(writer, "{}", column_name)
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to write header: {}", e)))?;

    for &is_good in mask {
        let value = if is_good { 1 } else { 0 };
        writeln!(writer, "{}", value)
            .map_err(|e| PeacoQCError::WriteError(format!("Failed to write data: {}", e)))?;
    }

    writer
        .flush()
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to flush file: {}", e)))?;

    Ok(())
}

/// Export a full-length good/bad mask as numeric CSV (e.g. 2000/6000)
///
/// Use this when the mask length matches the original event count (e.g. after
/// expanding margin/doublet-removed events into the "bad" bin so output has one row per input event).
///
/// # Arguments
///
/// * `mask` - Boolean mask (true = good/keep, false = bad/remove), length = original event count
/// * `path` - Output file path
/// * `good_value` - Value for good events (e.g. 2000)
/// * `bad_value` - Value for bad events (e.g. 6000)
/// * `column_name` - Optional column name (default: "PeacoQC")
pub fn export_csv_numeric_from_mask(
    mask: &[bool],
    path: impl AsRef<Path>,
    good_value: u16,
    bad_value: u16,
    column_name: Option<&str>,
) -> Result<()> {
    let path = path.as_ref();
    let column_name = column_name.unwrap_or("PeacoQC");

    if mask.is_empty() {
        return Err(PeacoQCError::ExportError(
            "Cannot export empty mask".to_string(),
        ));
    }

    let file = File::create(path).map_err(|e| {
        PeacoQCError::WriteError(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    let mut writer = BufWriter::new(file);

    writeln!(writer, "{}", column_name)
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to write header: {}", e)))?;

    for &is_good in mask {
        let value = if is_good { good_value } else { bad_value };
        writeln!(writer, "{}", value)
            .map_err(|e| PeacoQCError::WriteError(format!("Failed to write data: {}", e)))?;
    }

    writer
        .flush()
        .map_err(|e| PeacoQCError::WriteError(format!("Failed to flush file: {}", e)))?;

    Ok(())
}

/// Export QC results as JSON metadata
///
/// This format includes comprehensive QC metrics and configuration,
/// making it ideal for:
/// - Programmatic access to QC results
/// - Reporting and documentation
/// - Provenance tracking
///
/// # Format
///
/// ```json
/// {
///   "n_events_before": 713904,
///   "n_events_after": 631400,
///   "n_events_removed": 82504,
///   "percentage_removed": 11.56,
///   "it_percentage": 0.0,
///   "mad_percentage": 11.56,
///   "consecutive_percentage": 0.0,
///   "n_bins": 1427,
///   "events_per_bin": 500,
///   "channels_analyzed": ["FL1-A", "FL2-A"],
///   "config": {
///     "qc_mode": "All",
///     "mad": 6.0,
///     "it_limit": 0.6,
///     "consecutive_bins": 5,
///     "remove_zeros": false
///   }
/// }
/// ```
///
/// # Arguments
///
/// * `result` - PeacoQC result to export
/// * `config` - QC configuration used
/// * `path` - Output file path
///
/// # Errors
///
/// Returns an error if:
/// - The path is invalid
/// - The file cannot be written
/// - JSON serialization fails
pub fn export_json_metadata(
    result: &PeacoQCResult,
    config: &PeacoQCConfig,
    path: impl AsRef<Path>,
) -> Result<()> {
    let path = path.as_ref();

    let n_events_before = result.good_cells.len();
    let n_events_after = result.good_cells.iter().filter(|&&x| x).count();
    let n_events_removed = n_events_before - n_events_after;

    let metadata = QCJsonMetadata {
        n_events_before,
        n_events_after,
        n_events_removed,
        percentage_removed: result.percentage_removed,
        it_percentage: result.it_percentage,
        mad_percentage: result.mad_percentage,
        consecutive_percentage: result.consecutive_percentage,
        n_bins: result.n_bins,
        events_per_bin: result.events_per_bin,
        channels_analyzed: config.channels.clone(),
        config: QCConfigJson::from(config),
    };

    let file = File::create(path).map_err(|e| {
        PeacoQCError::WriteError(format!("Failed to create file {}: {}", path.display(), e))
    })?;

    serde_json::to_writer_pretty(file, &metadata)
        .map_err(|e| PeacoQCError::ExportError(format!("Failed to serialize JSON: {}", e)))?;

    Ok(())
}

/// Export QC results as a new FCS file with QC parameter column
///
/// This creates a NEW FCS file (doesn't modify the original) with:
/// - All original parameters
/// - New "PeacoQC" parameter with 0/1 values
/// - Only good events (if filter=true) or all events with QC column (if filter=false)
///
/// **Note**: Requires FCS writing support in flow-fcs crate
///
/// # Arguments
///
/// * `fcs` - Original FCS file
/// * `result` - PeacoQC result
/// * `output_path` - Path for new FCS file
/// * `filter` - If true, only export good events; if false, export all with QC column
///
/// # Errors
///
/// Returns an error if FCS writing is not yet implemented
#[cfg(feature = "flow-fcs")]
pub fn export_fcs_with_column(
    _fcs: &flow_fcs::Fcs,
    _result: &PeacoQCResult,
    _output_path: impl AsRef<Path>,
    _filter: bool,
) -> Result<()> {
    Err(PeacoQCError::ExportError(
        "FCS writing not yet implemented in flow-fcs crate".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qc::{PeacoQCConfig, PeacoQCResult, QCMode};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_result() -> PeacoQCResult {
        PeacoQCResult {
            good_cells: vec![true, true, false, true, false],
            removal_reason_per_bin: None,
            percentage_removed: 40.0,
            it_percentage: Some(20.0),
            mad_percentage: Some(20.0),
            consecutive_percentage: 0.0,
            peaks: HashMap::new(),
            n_bins: 10,
            events_per_bin: 50,
        }
    }

    fn create_test_config() -> PeacoQCConfig {
        PeacoQCConfig {
            channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
            determine_good_cells: QCMode::All,
            mad: 6.0,
            it_limit: 0.6,
            consecutive_bins: 5,
            remove_zeros: false,
            ..Default::default()
        }
    }

    #[test]
    fn test_export_csv_boolean() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_boolean.csv");
        let result = create_test_result();

        export_csv_boolean(&result, &path, None).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "PeacoQC");
        assert_eq!(lines[1], "1");
        assert_eq!(lines[2], "1");
        assert_eq!(lines[3], "0");
        assert_eq!(lines[4], "1");
        assert_eq!(lines[5], "0");
    }

    #[test]
    fn test_export_csv_numeric() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_numeric.csv");
        let result = create_test_result();

        export_csv_numeric(&result, &path, 2000, 6000, None).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "PeacoQC");
        assert_eq!(lines[1], "2000");
        assert_eq!(lines[2], "2000");
        assert_eq!(lines[3], "6000");
        assert_eq!(lines[4], "2000");
        assert_eq!(lines[5], "6000");
    }

    #[test]
    fn test_export_json_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_metadata.json");
        let result = create_test_result();
        let config = create_test_config();

        export_json_metadata(&result, &config, &path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let metadata: QCJsonMetadata = serde_json::from_str(&content).unwrap();

        assert_eq!(metadata.n_events_before, 5);
        assert_eq!(metadata.n_events_after, 3);
        assert_eq!(metadata.n_events_removed, 2);
        assert_eq!(metadata.percentage_removed, 40.0);
        assert_eq!(metadata.channels_analyzed, vec!["FL1-A", "FL2-A"]);
    }

    #[test]
    fn test_export_empty_result() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_empty.csv");
        let result = PeacoQCResult {
            good_cells: vec![],
            removal_reason_per_bin: None,
            percentage_removed: 0.0,
            it_percentage: None,
            mad_percentage: None,
            consecutive_percentage: 0.0,
            peaks: HashMap::new(),
            n_bins: 0,
            events_per_bin: 0,
        };

        assert!(export_csv_boolean(&result, &path, None).is_err());
    }

    #[test]
    fn test_custom_column_name() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test_custom.csv");
        let result = create_test_result();

        export_csv_boolean(&result, &path, Some("QC_Result")).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[0], "QC_Result");
    }
}
