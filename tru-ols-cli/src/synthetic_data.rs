//! Synthetic FCS data generation for debugging spectral unmixing
//!
//! Creates synthetic single-stain controls and mixed samples with known ground truth
//! to test unmixing algorithms and diagnostic plots.
//! Used by examples (e.g. generate_synthetic_test_data, process_compensation_controls).

#![allow(dead_code)]
#![allow(unused_imports)] // imports used by example-only code paths

use anyhow::{Context, Result};
use flow_fcs::file::AccessWrapper;
use flow_fcs::parameter::ParameterMap;
use flow_fcs::{Fcs, Header, Metadata, Parameter, TransformType, write_fcs_file};
use flow_plots::colormap::ColorMaps;
use flow_plots::options::{
    AxisOptions, BasePlotOptions, DensityPlotOptions, SpectralSignaturePlotOptions,
};
use flow_plots::render::RenderConfig;
use flow_plots::{DensityPlot, Plot, SpectralSignaturePlot};
use flow_plots::{generate_normalized_spectral_signature_plot, generate_signal_heatmap};
use ndarray::Array2;
use polars::prelude::*;
use rand::RngExt;
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

/// Spectral signature definition for a fluorophore
#[derive(Debug, Clone)]
pub struct SpectralSignature {
    /// Name of the fluorophore/endmember
    pub name: String,
    /// Primary detector (where signal is strongest, normalized to 1.0)
    pub primary_detector: String,
    /// Signal intensity at each detector (normalized, primary = 1.0)
    pub detector_signals: HashMap<String, f64>,
}

/// Generate synthetic single-stain control FCS file
///
/// Creates events with signal distributed across detectors according to the
/// spectral signature, plus autofluorescence and noise.
///
/// # Arguments
/// * `signature` - Spectral signature definition
/// * `detector_names` - All detector channel names
/// * `n_events` - Number of events to generate
/// * `autofluorescence` - Autofluorescence baseline per detector
/// * `noise_level` - Standard deviation of noise (as fraction of signal)
/// * `output_path` - Where to write the FCS file
pub fn generate_single_stain_control(
    signature: &SpectralSignature,
    detector_names: &[String],
    n_events: usize,
    autofluorescence: &HashMap<String, f32>,
    noise_level: f32,
    output_path: &PathBuf,
) -> Result<()> {
    use rand::RngExt;
    let mut rng = rand::rng();

    // Generate base signal intensity (varies per event)
    let signal_mean = 50000.0;
    let signal_std = 10000.0;
    let signal_dist = Normal::new(signal_mean as f64, signal_std as f64)
        .context("Failed to create signal distribution")?;

    // Create columns for all detectors
    let mut columns: Vec<Column> = Vec::new();
    let mut params = ParameterMap::default();

    // Add scatter channels (required for FCS files)
    // Use normal distributions to mimic real cell populations
    let fsc_a_dist =
        Normal::new(100000.0, 25000.0).context("Failed to create FSC-A distribution")?;
    let fsc_h_dist =
        Normal::new(95000.0, 20000.0).context("Failed to create FSC-H distribution")?;
    let ssc_a_dist =
        Normal::new(50000.0, 15000.0).context("Failed to create SSC-A distribution")?;

    let fsc_a: Vec<f32> = (0..n_events)
        .map(|_| (fsc_a_dist.sample(&mut rng) as f32).max(1000.0))
        .collect();
    let fsc_h: Vec<f32> = (0..n_events)
        .map(|i| {
            // FSC-H is correlated with FSC-A (singlets)
            let correlated = fsc_a[i] * 0.92;
            let noise = fsc_h_dist.sample(&mut rng) as f32 - 95000.0;
            (correlated + noise * 0.1).max(1000.0)
        })
        .collect();
    let ssc_a: Vec<f32> = (0..n_events)
        .map(|_| (ssc_a_dist.sample(&mut rng) as f32).max(500.0))
        .collect();

    columns.push(Column::new("FSC-A".into(), fsc_a.clone()));
    columns.push(Column::new("FSC-H".into(), fsc_h.clone()));
    columns.push(Column::new("SSC-A".into(), ssc_a.clone()));

    params.insert(
        "FSC-A".into(),
        Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
    );
    params.insert(
        "FSC-H".into(),
        Parameter::new(&2, "FSC-H", "FSC-H", &TransformType::Linear),
    );
    params.insert(
        "SSC-A".into(),
        Parameter::new(&3, "SSC-A", "SSC-A", &TransformType::Linear),
    );

    // Generate detector signals
    let mut param_idx = 4;
    for detector_name in detector_names {
        let base_signal = signature
            .detector_signals
            .get(detector_name)
            .copied()
            .unwrap_or(0.0);
        let af = autofluorescence
            .get(detector_name)
            .copied()
            .unwrap_or(100.0);

        let values: Vec<f32> = (0..n_events)
            .map(|_| {
                // Base signal intensity for this event
                let event_signal = signal_dist.sample(&mut rng) as f32;

                // Apply spectral signature (normalized)
                let spectral_component = event_signal * base_signal as f32;

                // Add autofluorescence
                let af_component = af;

                // Add noise
                let noise = rng.random_range(-noise_level..noise_level) * event_signal;

                // Total signal (ensure non-negative)
                (spectral_component + af_component + noise).max(0.0)
            })
            .collect();

        columns.push(Column::new(detector_name.clone().into(), values));
        params.insert(
            detector_name.clone().into(),
            Parameter::new(
                &param_idx,
                detector_name,
                detector_name,
                &TransformType::Linear,
            ),
        );
        param_idx += 1;
    }

    // Create DataFrame
    let df = DataFrame::new(n_events, columns)
        .context("Failed to create DataFrame for synthetic control")?;

    // Ensure parent directory exists before writing
    // #region agent log
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/Users/kfls271/Rust/.cursor/debug.log")
        {
            let log_entry = serde_json::json!({
                "sessionId": "debug-session",
                "runId": "synthetic-generation",
                "hypothesisId": "file-write-error",
                "location": "synthetic_data.rs:130",
                "message": "Before creating FCS struct and writing file",
                "data": {
                    "output_path": output_path.display().to_string(),
                    "parent_dir": output_path.parent().map(|p| p.display().to_string()),
                    "n_events": n_events,
                    "n_detectors": detector_names.len()
                },
                "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
            });
            let _ = writeln!(file, "{}", log_entry);
        }
    }
    // #endregion

    // Ensure parent directory exists before writing
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create parent directory for {}",
                output_path.display()
            )
        })?;
    }

    // #region agent log
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/Users/kfls271/Rust/.cursor/debug.log")
        {
            let log_entry = serde_json::json!({
                "sessionId": "debug-session",
                "runId": "synthetic-generation",
                "hypothesisId": "file-write-error",
                "location": "synthetic_data.rs:145",
                "message": "After creating parent directory, before creating FCS struct",
                "data": {
                    "output_path": output_path.display().to_string(),
                    "parent_exists": output_path.parent().map(|p| p.exists())
                },
                "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
            });
            let _ = writeln!(file, "{}", log_entry);
        }
    }
    // #endregion

    // Create a temporary empty file for AccessWrapper (it requires a real file to memory-map)
    // We'll write the actual FCS file using write_fcs_file, but AccessWrapper needs something to open
    let temp_file = std::env::temp_dir().join(format!("synthetic_temp_{}.fcs", std::process::id()));
    {
        // Create an empty temporary file
        std::fs::File::create(&temp_file)
            .with_context(|| format!("Failed to create temporary file: {}", temp_file.display()))?;
    }

    // #region agent log
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/Users/kfls271/Rust/.cursor/debug.log")
        {
            let log_entry = serde_json::json!({
                "sessionId": "debug-session",
                "runId": "synthetic-generation",
                "hypothesisId": "file-write-error",
                "location": "synthetic_data.rs:182",
                "message": "Created temp file for AccessWrapper",
                "data": {
                    "temp_file": temp_file.display().to_string(),
                    "temp_exists": temp_file.exists(),
                    "output_path": output_path.display().to_string()
                },
                "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
            });
            let _ = writeln!(file, "{}", log_entry);
        }
    }
    // #endregion

    // Create metadata using helper function
    let metadata = Metadata::from_dataframe_and_parameters(&df, &params)
        .with_context(|| "Failed to create metadata from DataFrame and ParameterMap")?;

    // Create FCS struct with temporary file for AccessWrapper
    let fcs = Fcs {
        header: Header::new(),
        metadata,
        parameters: params,
        data_frame: Arc::new(df),
        file_access: AccessWrapper::new(temp_file.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Temp file path contains invalid UTF-8: {}",
                temp_file.display()
            )
        })?)
        .with_context(|| {
            format!(
                "Failed to create AccessWrapper from temp file: {}",
                temp_file.display()
            )
        })?,
    };

    // #region agent log
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/Users/kfls271/Rust/.cursor/debug.log")
        {
            let log_entry = serde_json::json!({
                "sessionId": "debug-session",
                "runId": "synthetic-generation",
                "hypothesisId": "file-write-error",
                "location": "synthetic_data.rs:165",
                "message": "Before write_fcs_file call",
                "data": {
                    "output_path": output_path.display().to_string(),
                    "path_exists": output_path.exists(),
                    "parent_exists": output_path.parent().map(|p| p.exists())
                },
                "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
            });
            let _ = writeln!(file, "{}", log_entry);
        }
    }
    // #endregion

    // Write to file
    write_fcs_file(fcs, output_path).with_context(|| {
        format!(
            "Failed to write synthetic control to {}",
            output_path.display()
        )
    })?;

    // #region agent log
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/Users/kfls271/Rust/.cursor/debug.log")
        {
            let log_entry = serde_json::json!({
                "sessionId": "debug-session",
                "runId": "synthetic-generation",
                "hypothesisId": "file-write-error",
                "location": "synthetic_data.rs:185",
                "message": "After write_fcs_file call - success",
                "data": {
                    "output_path": output_path.display().to_string(),
                    "file_exists": output_path.exists(),
                    "file_size": output_path.exists().then(|| std::fs::metadata(output_path).ok().map(|m| m.len())).flatten()
                },
                "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
            });
            let _ = writeln!(file, "{}", log_entry);
        }
    }
    // #endregion

    Ok(())
}

/// Generate synthetic mixed sample FCS file
///
/// Creates events with signals from multiple fluorophores mixed together
/// according to specified abundances.
///
/// # Arguments
/// * `signatures` - All spectral signatures
/// * `detector_names` - All detector channel names
/// * `n_events` - Number of events to generate
/// * `abundances` - Abundance of each endmember per event (n_events × n_endmembers)
/// * `autofluorescence` - Autofluorescence baseline per detector
/// * `noise_level` - Standard deviation of noise
/// * `output_path` - Where to write the FCS file
pub fn generate_mixed_sample(
    signatures: &[SpectralSignature],
    detector_names: &[String],
    n_events: usize,
    abundances: &Array2<f64>,
    autofluorescence: &HashMap<String, f32>,
    noise_level: f32,
    output_path: &PathBuf,
) -> Result<()> {
    use rand::RngExt;
    let mut rng = rand::rng();

    // Verify dimensions
    if abundances.nrows() != n_events {
        return Err(anyhow::anyhow!(
            "Abundances matrix has {} rows but need {} events",
            abundances.nrows(),
            n_events
        ));
    }
    if abundances.ncols() != signatures.len() {
        return Err(anyhow::anyhow!(
            "Abundances matrix has {} columns but have {} signatures",
            abundances.ncols(),
            signatures.len()
        ));
    }

    // Create columns for all detectors
    let mut columns: Vec<Column> = Vec::new();
    let mut params = ParameterMap::default();

    // Add scatter channels
    // Use normal distributions to mimic real cell populations
    let fsc_a_dist =
        Normal::new(100000.0, 25000.0).context("Failed to create FSC-A distribution")?;
    let fsc_h_dist =
        Normal::new(95000.0, 20000.0).context("Failed to create FSC-H distribution")?;
    let ssc_a_dist =
        Normal::new(50000.0, 15000.0).context("Failed to create SSC-A distribution")?;

    let fsc_a: Vec<f32> = (0..n_events)
        .map(|_| (fsc_a_dist.sample(&mut rng) as f32).max(1000.0))
        .collect();
    let fsc_h: Vec<f32> = (0..n_events)
        .map(|i| {
            // FSC-H is correlated with FSC-A (singlets)
            let correlated = fsc_a[i] * 0.92;
            let noise = fsc_h_dist.sample(&mut rng) as f32 - 95000.0;
            (correlated + noise * 0.1).max(1000.0)
        })
        .collect();
    let ssc_a: Vec<f32> = (0..n_events)
        .map(|_| (ssc_a_dist.sample(&mut rng) as f32).max(500.0))
        .collect();

    columns.push(Column::new("FSC-A".into(), fsc_a.clone()));
    columns.push(Column::new("FSC-H".into(), fsc_h.clone()));
    columns.push(Column::new("SSC-A".into(), ssc_a.clone()));

    params.insert(
        "FSC-A".into(),
        Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
    );
    params.insert(
        "FSC-H".into(),
        Parameter::new(&2, "FSC-H", "FSC-H", &TransformType::Linear),
    );
    params.insert(
        "SSC-A".into(),
        Parameter::new(&3, "SSC-A", "SSC-A", &TransformType::Linear),
    );

    // Generate detector signals by mixing all signatures
    // Autofluorescence is added as background signal in all channels (same pattern as unstained control)
    // During unmixing, this autofluorescence vector will be subtracted from each event
    // and placed in its own output channel
    let mut param_idx = 4;
    for detector_name in detector_names {
        let af = autofluorescence
            .get(detector_name)
            .copied()
            .unwrap_or(100.0);

        let values: Vec<f32> = (0..n_events)
            .map(|event_idx| {
                let mut total_signal = 0.0;

                // Sum contributions from all signatures
                for (sig_idx, signature) in signatures.iter().enumerate() {
                    let abundance = abundances[(event_idx, sig_idx)];
                    let spectral_component = signature
                        .detector_signals
                        .get(detector_name)
                        .copied()
                        .unwrap_or(0.0);

                    // Base signal intensity (scaled by abundance)
                    let base_signal = 50000.0 * abundance as f32;
                    total_signal += base_signal * spectral_component as f32;
                }

                // Add autofluorescence as background signal (same pattern as unstained control)
                // This will be extracted during unmixing via vector subtraction
                total_signal += af;

                // Add noise
                let noise = rng.random_range(-noise_level..noise_level) * 50000.0;

                // Total signal (ensure non-negative)
                (total_signal + noise).max(0.0)
            })
            .collect();

        columns.push(Column::new(detector_name.clone().into(), values));
        params.insert(
            detector_name.clone().into(),
            Parameter::new(
                &param_idx,
                detector_name,
                detector_name,
                &TransformType::Linear,
            ),
        );
        param_idx += 1;
    }

    // Create DataFrame
    let df = DataFrame::new(n_events, columns)
        .context("Failed to create DataFrame for synthetic mixed sample")?;

    // Ensure parent directory exists before writing
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create parent directory for {}",
                output_path.display()
            )
        })?;
    }

    // Create a temporary empty file for AccessWrapper (it requires a real file to memory-map)
    let temp_file = std::env::temp_dir().join(format!("synthetic_temp_{}.fcs", std::process::id()));
    {
        std::fs::File::create(&temp_file)
            .with_context(|| format!("Failed to create temporary file: {}", temp_file.display()))?;
    }

    // Create metadata using helper function
    let mut metadata = Metadata::from_dataframe_and_parameters(&df, &params)
        .with_context(|| "Failed to create metadata from DataFrame and ParameterMap")?;

    // Add $FIL keyword (filename)
    let filename = output_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("synthetic.fcs");
    metadata.insert_string_keyword("$FIL".to_string(), filename.to_string());

    // Create FCS struct with temporary file for AccessWrapper
    let fcs = Fcs {
        header: Header::new(),
        metadata,
        parameters: params,
        data_frame: Arc::new(df),
        file_access: AccessWrapper::new(temp_file.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Temp file path contains invalid UTF-8: {}",
                temp_file.display()
            )
        })?)
        .with_context(|| {
            format!(
                "Failed to create AccessWrapper from temp file: {}",
                temp_file.display()
            )
        })?,
    };

    // Write to file
    write_fcs_file(fcs, output_path).with_context(|| {
        format!(
            "Failed to write synthetic mixed sample to {}",
            output_path.display()
        )
    })?;

    // Clean up temporary file (ignore errors)
    let _ = fs::remove_file(&temp_file);

    Ok(())
}

/// Create test spectral signatures for BUV395 and BUV496
///
/// These are designed to be distinct with different primary detectors to test similarity detection.
pub fn create_test_signatures() -> Vec<SpectralSignature> {
    let mut signatures = Vec::new();

    // BUV395 - primary on UV1-A, spillover to UV2-A and UV3-A
    let mut buv395 = HashMap::new();
    buv395.insert("UV1-A".to_string(), 1.0); // Primary
    buv395.insert("UV2-A".to_string(), 0.15); // Spillover
    buv395.insert("UV3-A".to_string(), 0.05); // Minor spillover
    buv395.insert("V1-A".to_string(), 0.02); // Very minor spillover
    signatures.push(SpectralSignature {
        name: "BUV395".to_string(),
        primary_detector: "UV1-A".to_string(),
        detector_signals: buv395,
    });

    // BUV496 - primary on UV2-A (different!), with spillover to UV1-A and UV3-A
    let mut buv496 = HashMap::new();
    buv496.insert("UV2-A".to_string(), 1.0); // Primary (different from BUV395!)
    buv496.insert("UV1-A".to_string(), 0.20); // Spillover back to UV1-A
    buv496.insert("UV3-A".to_string(), 0.30); // Significant spillover to UV3-A
    buv496.insert("V1-A".to_string(), 0.10); // More spillover to V1-A
    signatures.push(SpectralSignature {
        name: "BUV496".to_string(),
        primary_detector: "UV2-A".to_string(),
        detector_signals: buv496,
    });

    signatures
}

/// Generate visualization plots for a synthetic spectral signature
///
/// Creates two plots:
/// 1. Heatmap/bar chart showing raw signal intensity across channels (before normalization)
/// 2. Line plot showing normalized spectral signature (0-1 range)
///
/// # Arguments
/// * `signature` - The spectral signature to visualize
/// * `detector_names` - All detector channel names
/// * `raw_signals` - Raw signal values per detector (before normalization)
/// * `output_dir` - Directory to save plots
/// * `plot_format` - Image format (e.g., "jpg", "png")
/// * `fcs_file_path` - Optional path to FCS file to read actual event data from
/// * `colormap` - Optional colormap to use (defaults to Rainbow)
pub fn generate_spectral_visualization_plots(
    signature: &SpectralSignature,
    detector_names: &[String],
    raw_signals: &HashMap<String, f32>,
    output_dir: &PathBuf,
    plot_format: &str,
    fcs_file_path: Option<&PathBuf>,
    colormap: Option<ColorMaps>,
) -> Result<()> {
    generate_spectral_visualization_plots_with_overlay(
        signature,
        detector_names,
        raw_signals,
        output_dir,
        plot_format,
        fcs_file_path,
        colormap,
        None, // No unstained overlay by default
        None, // No positive medians overlay by default
        None, // No positive geometric means overlay by default
    )
}

/// Generate spectral visualization plots with optional unstained medians overlay
pub fn generate_spectral_visualization_plots_with_overlay(
    signature: &SpectralSignature,
    detector_names: &[String],
    raw_signals: &HashMap<String, f32>,
    output_dir: &PathBuf,
    plot_format: &str,
    fcs_file_path: Option<&PathBuf>,
    colormap: Option<ColorMaps>,
    unstained_medians: Option<&HashMap<String, f32>>,
    positive_medians: Option<&HashMap<String, f32>>,
    positive_geometric_means: Option<&HashMap<String, f32>>,
) -> Result<()> {
    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // 1. Generate heatmap/bar chart of raw signals across channels (with overlays if provided)
    write_signal_heatmap_to_file(
        signature,
        detector_names,
        raw_signals,
        output_dir,
        plot_format,
        fcs_file_path,
        colormap,
        unstained_medians,
        positive_medians,
        positive_geometric_means,
    )?;

    // 2. Generate normalized spectral signature line plot
    generate_normalized_signature_plot(
        signature,
        detector_names,
        output_dir,
        plot_format,
        fcs_file_path,
    )?;

    Ok(())
}

/// Generate only the signal heatmap (for unstained controls)
pub fn generate_signal_heatmap_only(
    signature: &SpectralSignature,
    detector_names: &[String],
    raw_signals: &HashMap<String, f32>,
    output_dir: &PathBuf,
    plot_format: &str,
    fcs_file_path: Option<&PathBuf>,
    colormap: Option<ColorMaps>,
    unstained_medians: Option<&HashMap<String, f32>>,
    positive_medians: Option<&HashMap<String, f32>>,
) -> Result<()> {
    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    write_signal_heatmap_to_file(
        signature,
        detector_names,
        raw_signals,
        output_dir,
        plot_format,
        fcs_file_path,
        colormap,
        unstained_medians,
        positive_medians,
        None, // positive_geometric_means - will be calculated in process_compensation_controls.rs
    )
}

/// Generate a heatmap visualization of signal intensity across channels and write to file
///
/// Shows a density distribution of events across intensity levels for each channel.
/// Each channel is a vertical column where color represents the density of events
/// at each intensity level (y-axis). This creates a 1D vertical distribution showing
/// where events cluster in intensity space.
///
/// This is a wrapper function that handles file I/O. The actual plotting logic
/// is in the flow-plots crate.
fn write_signal_heatmap_to_file(
    signature: &SpectralSignature,
    detector_names: &[String],
    raw_signals: &HashMap<String, f32>,
    output_dir: &PathBuf,
    plot_format: &str,
    fcs_file_path: Option<&PathBuf>,
    colormap: Option<ColorMaps>,
    unstained_medians: Option<&HashMap<String, f32>>,
    positive_medians: Option<&HashMap<String, f32>>,
    positive_geometric_means: Option<&HashMap<String, f32>>,
) -> Result<()> {
    // Call the function from flow-plots crate
    let bytes = generate_signal_heatmap(
        &signature.name,
        detector_names,
        raw_signals,
        fcs_file_path.as_ref().map(|p| p.as_path()),
        colormap,
        unstained_medians,
        positive_medians,
        positive_geometric_means,
    )?;

    // Write to file
    let output_path = output_dir.join(format!("{}_signal_heatmap.{}", signature.name, plot_format));
    fs::write(&output_path, bytes)
        .with_context(|| format!("Failed to write heatmap to {}", output_path.display()))?;

    Ok(())
}

// Old implementation removed - now using flow-plots crate

/// Generate normalized spectral signature line plot
///
/// Shows the normalized signature (0-1 range) as a line plot connecting peaks across channels.
/// If signature.detector_signals is empty, calculates normalized signature from FCS file.
///
/// This is a wrapper function that handles file I/O. The actual plotting logic
/// is in the flow-plots crate.
fn generate_normalized_signature_plot(
    signature: &SpectralSignature,
    detector_names: &[String],
    output_dir: &PathBuf,
    plot_format: &str,
    fcs_file_path: Option<&PathBuf>,
) -> Result<()> {
    // Convert detector_signals HashMap<String, f64> to HashMap<String, f64> for the function
    let detector_signals = &signature.detector_signals;

    // Call the function from flow-plots crate
    let bytes = generate_normalized_spectral_signature_plot(
        &signature.name,
        detector_names,
        detector_signals,
        fcs_file_path.as_ref().map(|p| p.as_path()),
    )?;

    // Write to file
    let output_path = output_dir.join(format!(
        "{}_normalized_signature.{}",
        signature.name, plot_format
    ));
    fs::write(&output_path, bytes).with_context(|| {
        format!(
            "Failed to write normalized signature plot to {}",
            output_path.display()
        )
    })?;

    Ok(())
}

// Old implementation code removed - now using flow-plots crate

/// Generate synthetic control with visualization plots
///
/// Convenience function that generates both the FCS file and visualization plots
pub fn generate_single_stain_control_with_plots(
    signature: &SpectralSignature,
    detector_names: &[String],
    n_events: usize,
    autofluorescence: &HashMap<String, f32>,
    noise_level: f32,
    output_path: &PathBuf,
    plot_output_dir: Option<&PathBuf>,
    plot_format: &str,
) -> Result<HashMap<String, f32>> {
    // Generate the FCS file
    generate_single_stain_control(
        signature,
        detector_names,
        n_events,
        autofluorescence,
        noise_level,
        output_path,
    )?;

    // Calculate raw signal medians (simulating what we'd extract from the control)
    // For synthetic data, we can calculate expected medians from the signature
    let signal_mean = 50000.0;
    let mut raw_signals = HashMap::new();

    for detector_name in detector_names {
        let base_signal = signature
            .detector_signals
            .get(detector_name)
            .copied()
            .unwrap_or(0.0);
        let af = autofluorescence
            .get(detector_name)
            .copied()
            .unwrap_or(100.0);

        // Expected median = base_signal * signal_mean + autofluorescence
        let expected_median = (signal_mean * base_signal as f32) + af;
        raw_signals.insert(detector_name.clone(), expected_median);
    }

    // Generate visualization plots if requested
    if let Some(plot_dir) = plot_output_dir {
        generate_spectral_visualization_plots(
            signature,
            detector_names,
            &raw_signals,
            plot_dir,
            plot_format,
            Some(output_path), // Pass FCS file path to read actual event data
            None,              // Use default Rainbow colormap
        )?;
    }

    Ok(raw_signals)
}

/// Example: Generate synthetic test data with visualization plots
///
/// Creates synthetic single-stain controls for BUV395 and BUV496 with known signatures,
/// generates FCS files, and creates visualization plots showing:
/// 1. Signal intensity heatmap across channels (before normalization)
/// 2. Normalized spectral signature line plot (0-1 range)
///
/// # Arguments
/// * `output_dir` - Directory to save FCS files and plots
/// * `plot_format` - Image format for plots (e.g., "jpg", "png")
pub fn generate_test_synthetic_data_with_plots(
    output_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    use std::fs;

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // Create subdirectories
    let controls_dir = output_dir.join("controls");
    let plots_dir = output_dir.join("plots");
    fs::create_dir_all(&controls_dir)?;
    fs::create_dir_all(&plots_dir)?;

    // Create test signatures
    let signatures = create_test_signatures();

    // Define detector channels (simplified set for testing)
    let detector_names = vec![
        "UV1-A".to_string(),
        "UV2-A".to_string(),
        "UV3-A".to_string(),
        "V1-A".to_string(),
        "V2-A".to_string(),
    ];

    // Create autofluorescence baseline
    let mut autofluorescence = HashMap::new();
    for det_name in &detector_names {
        autofluorescence.insert(det_name.clone(), 100.0);
    }

    // Generate synthetic controls with plots
    for signature in &signatures {
        let control_path = controls_dir.join(format!("{}.fcs", signature.name));
        let plot_dir = plots_dir.join(&signature.name);

        println!("Generating synthetic control: {}", signature.name);

        let _raw_signals = generate_single_stain_control_with_plots(
            signature,
            &detector_names,
            50000, // n_events
            &autofluorescence,
            0.05, // noise_level (5% of signal)
            &control_path,
            Some(&plot_dir),
            plot_format,
        )?;

        println!("  ✓ Generated FCS file: {}", control_path.display());
        println!("  ✓ Generated plots in: {}", plot_dir.display());
        println!("    - {}_signal_heatmap.{}", signature.name, plot_format);
        println!(
            "    - {}_normalized_signature.{}",
            signature.name, plot_format
        );
    }

    // Also create an unstained control
    let unstained_path = controls_dir.join("Unstained.fcs");
    let unstained_signature = SpectralSignature {
        name: "Unstained".to_string(),
        primary_detector: "UV1-A".to_string(),
        detector_signals: HashMap::new(), // No signal, only autofluorescence
    };

    generate_single_stain_control(
        &unstained_signature,
        &detector_names,
        50000,
        &autofluorescence,
        0.05,
        &unstained_path,
    )?;

    println!(
        "✓ Generated unstained control: {}",
        unstained_path.display()
    );
    println!(
        "\nAll synthetic test data generated in: {}",
        output_dir.display()
    );

    Ok(())
}

/// Generate comprehensive synthetic test data with 25 channels and 10 fluorophores
///
/// Creates:
/// - 25 channels (5 per laser: UV, Violet, Blue, Yellow-Green, Red)
/// - 10 fluorophores with realistic spectral signatures
/// - 3 fully-stained samples with varying cell expression patterns
/// - 1 unstained control with autofluorescence in middle UV and Violet channels
/// - 10 single-stain controls (one per fluorophore)
///
/// # Arguments
/// * `output_dir` - Directory to save FCS files and plots
/// * `plot_format` - Image format for plots (e.g., "jpg", "png")
pub fn generate_comprehensive_synthetic_data(
    output_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    use std::fs;

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // Create subdirectories
    let controls_dir = output_dir.join("controls");
    let plots_dir = output_dir.join("plots");
    let samples_dir = output_dir.join("samples");
    fs::create_dir_all(&controls_dir)?;
    fs::create_dir_all(&plots_dir)?;
    fs::create_dir_all(&samples_dir)?;

    // Define 25 channels (5 per laser)
    let detector_names = vec![
        // UV channels (5)
        "UV379-A".to_string(),
        "UV446-A".to_string(),
        "UV582-A".to_string(),
        "UV736-A".to_string(),
        "UV812-A".to_string(),
        // Violet channels (5)
        "V508-A".to_string(),
        "V525-A".to_string(),
        "V660-A".to_string(),
        "V720-A".to_string(),
        "V780-A".to_string(),
        // Blue channels (5)
        "B510-A".to_string(),
        "B560-A".to_string(),
        "B610-A".to_string(),
        "B660-A".to_string(),
        "B710-A".to_string(),
        // Yellow-Green channels (5)
        "YG585-A".to_string(),
        "YG615-A".to_string(),
        "YG660-A".to_string(),
        "YG750-A".to_string(),
        "YG812-A".to_string(),
        // Red channels (5)
        "R660-A".to_string(),
        "R710-A".to_string(),
        "R750-A".to_string(),
        "R780-A".to_string(),
        "R810-A".to_string(),
    ];

    // Create autofluorescence baseline (higher in middle UV and Violet channels)
    let mut autofluorescence = HashMap::new();
    for det_name in &detector_names {
        let af_value = if det_name.starts_with("UV")
            && (det_name.contains("446") || det_name.contains("582"))
        {
            500.0 // Higher autofluorescence in middle UV channels
        } else if det_name.starts_with("V")
            && (det_name.contains("525") || det_name.contains("660"))
        {
            400.0 // Higher autofluorescence in middle Violet channels
        } else {
            100.0 // Baseline autofluorescence
        };
        autofluorescence.insert(det_name.clone(), af_value);
    }

    // Create 10 fluorophore signatures with realistic spillover patterns
    let signatures = create_10_fluorophore_signatures();

    println!("Generating single-stain controls...");
    // Generate single-stain controls (one per fluorophore)
    for signature in &signatures {
        let control_path = controls_dir.join(format!("{}.fcs", signature.name));
        let plot_dir = plots_dir.join(&signature.name);

        println!("  Generating: {}", signature.name);

        let _raw_signals = generate_single_stain_control_with_plots(
            signature,
            &detector_names,
            50000, // n_events
            &autofluorescence,
            0.05, // noise_level (5% of signal)
            &control_path,
            Some(&plot_dir),
            plot_format,
        )?;
    }

    println!("\nGenerating unstained control...");
    // Generate unstained control
    let unstained_path = controls_dir.join("Unstained_Control.fcs");
    let unstained_signature = SpectralSignature {
        name: "Unstained_Control".to_string(),
        primary_detector: "UV446-A".to_string(),
        detector_signals: HashMap::new(), // No signal, only autofluorescence
    };

    generate_single_stain_control(
        &unstained_signature,
        &detector_names,
        50000,
        &autofluorescence,
        0.05,
        &unstained_path,
    )?;

    println!("\nGenerating fully-stained samples...");
    // Generate 3 fully-stained samples with varying expression patterns
    for sample_idx in 1..=3 {
        let sample_path = samples_dir.join(format!("FullyStained_Sample_{}.fcs", sample_idx));
        println!("  Generating: FullyStained_Sample_{}", sample_idx);

        // Create abundance matrix where cells express subsets of fluorophores
        let n_events = 50000;
        let abundances = create_varying_expression_abundances(n_events, &signatures, sample_idx);

        generate_mixed_sample(
            &signatures,
            &detector_names,
            n_events,
            &abundances,
            &autofluorescence,
            0.05,
            &sample_path,
        )?;
    }

    println!(
        "\n✓ All synthetic test data generated in: {}",
        output_dir.display()
    );
    println!("  - Controls: {}", controls_dir.display());
    println!("  - Samples: {}", samples_dir.display());
    println!("  - Plots: {}", plots_dir.display());

    Ok(())
}

/// Create 10 fluorophore signatures with realistic spectral spillover patterns
///
/// Includes:
/// - Cross-laser spillover: emission spills into detectors on different lasers with similar wavelengths
/// - Cross-excitation: fluorophores excited by multiple lasers with different emission intensities
fn create_10_fluorophore_signatures() -> Vec<SpectralSignature> {
    let mut signatures = Vec::new();

    // Fluor 1: UV379 (UV laser primary, also excited by Violet laser)
    // Emission ~379nm, spills into UV and Violet detectors
    let mut flu1 = HashMap::new();
    flu1.insert("UV379-A".to_string(), 1.0); // Primary (UV laser)
    flu1.insert("UV446-A".to_string(), 0.18); // Same laser spillover
    flu1.insert("V508-A".to_string(), 0.12); // Cross-laser: Violet detector (similar wavelength)
    flu1.insert("V525-A".to_string(), 0.08); // Cross-laser: Violet detector
    flu1.insert("B510-A".to_string(), 0.04); // Cross-laser: Blue detector (weak)
    signatures.push(SpectralSignature {
        name: "Fluor_UV379".to_string(),
        primary_detector: "UV379-A".to_string(),
        detector_signals: flu1,
    });

    // Fluor 2: UV446 (UV laser primary, also excited by Violet laser)
    // Emission ~446nm, strong spillover to Violet detectors
    let mut flu2 = HashMap::new();
    flu2.insert("UV446-A".to_string(), 1.0); // Primary (UV laser)
    flu2.insert("UV379-A".to_string(), 0.12); // Same laser spillover
    flu2.insert("UV582-A".to_string(), 0.25); // Same laser spillover
    flu2.insert("V508-A".to_string(), 0.22); // Cross-laser: Violet detector (strong - similar wavelength)
    flu2.insert("V525-A".to_string(), 0.15); // Cross-laser: Violet detector
    flu2.insert("B510-A".to_string(), 0.08); // Cross-laser: Blue detector
    signatures.push(SpectralSignature {
        name: "Fluor_UV446".to_string(),
        primary_detector: "UV446-A".to_string(),
        detector_signals: flu2,
    });

    // Fluor 3: UV736 (UV laser primary, also excited by Violet and Yellow-Green lasers)
    // Emission ~736nm, spills into UV, Violet, and YG detectors
    let mut flu3 = HashMap::new();
    flu3.insert("UV736-A".to_string(), 1.0); // Primary (UV laser)
    flu3.insert("UV582-A".to_string(), 0.15); // Same laser spillover
    flu3.insert("UV812-A".to_string(), 0.20); // Same laser spillover
    flu3.insert("V720-A".to_string(), 0.18); // Cross-laser: Violet detector (similar wavelength)
    flu3.insert("V780-A".to_string(), 0.12); // Cross-laser: Violet detector
    flu3.insert("YG750-A".to_string(), 0.10); // Cross-laser: YG detector (similar wavelength)
    flu3.insert("B710-A".to_string(), 0.06); // Cross-laser: Blue detector (weak)
    signatures.push(SpectralSignature {
        name: "Fluor_UV736".to_string(),
        primary_detector: "UV736-A".to_string(),
        detector_signals: flu3,
    });

    // Fluor 4: V660 (Violet laser primary, also excited by Blue and Red lasers)
    // Emission ~660nm, spills into Violet, Blue, and Red detectors (all have 660nm detectors!)
    let mut flu4 = HashMap::new();
    flu4.insert("V660-A".to_string(), 1.0); // Primary (Violet laser)
    flu4.insert("V525-A".to_string(), 0.10); // Same laser spillover
    flu4.insert("V720-A".to_string(), 0.15); // Same laser spillover
    flu4.insert("B660-A".to_string(), 0.25); // Cross-laser: Blue detector (same wavelength!)
    flu4.insert("B610-A".to_string(), 0.12); // Cross-laser: Blue detector
    flu4.insert("R660-A".to_string(), 0.20); // Cross-laser: Red detector (same wavelength!)
    flu4.insert("R710-A".to_string(), 0.08); // Cross-laser: Red detector
    signatures.push(SpectralSignature {
        name: "Fluor_V660".to_string(),
        primary_detector: "V660-A".to_string(),
        detector_signals: flu4,
    });

    // Fluor 5: V720 (Violet laser primary, also excited by UV laser)
    // Emission ~720nm, spills into Violet and UV detectors
    let mut flu5 = HashMap::new();
    flu5.insert("V720-A".to_string(), 1.0); // Primary (Violet laser)
    flu5.insert("V660-A".to_string(), 0.20); // Same laser spillover
    flu5.insert("V780-A".to_string(), 0.18); // Same laser spillover
    flu5.insert("UV736-A".to_string(), 0.15); // Cross-laser: UV detector (similar wavelength)
    flu5.insert("UV812-A".to_string(), 0.08); // Cross-laser: UV detector
    flu5.insert("B710-A".to_string(), 0.10); // Cross-laser: Blue detector
    signatures.push(SpectralSignature {
        name: "Fluor_V720".to_string(),
        primary_detector: "V720-A".to_string(),
        detector_signals: flu5,
    });

    // Fluor 6: B510 (Blue laser primary, also excited by Violet laser)
    // Emission ~510nm, spills into Blue and Violet detectors
    let mut flu6 = HashMap::new();
    flu6.insert("B510-A".to_string(), 1.0); // Primary (Blue laser)
    flu6.insert("B560-A".to_string(), 0.22); // Same laser spillover
    flu6.insert("V508-A".to_string(), 0.18); // Cross-laser: Violet detector (similar wavelength)
    flu6.insert("V525-A".to_string(), 0.12); // Cross-laser: Violet detector
    flu6.insert("YG585-A".to_string(), 0.08); // Cross-laser: YG detector (weak)
    flu6.insert("UV446-A".to_string(), 0.05); // Cross-laser: UV detector (very weak)
    signatures.push(SpectralSignature {
        name: "Fluor_B510".to_string(),
        primary_detector: "B510-A".to_string(),
        detector_signals: flu6,
    });

    // Fluor 7: B660 (Blue laser primary, also excited by Violet and Red lasers)
    // Emission ~660nm, spills into Blue, Violet, and Red detectors (all have 660nm!)
    let mut flu7 = HashMap::new();
    flu7.insert("B660-A".to_string(), 1.0); // Primary (Blue laser)
    flu7.insert("B610-A".to_string(), 0.18); // Same laser spillover
    flu7.insert("B710-A".to_string(), 0.20); // Same laser spillover
    flu7.insert("V660-A".to_string(), 0.25); // Cross-laser: Violet detector (same wavelength!)
    flu7.insert("V720-A".to_string(), 0.10); // Cross-laser: Violet detector
    flu7.insert("R660-A".to_string(), 0.22); // Cross-laser: Red detector (same wavelength!)
    flu7.insert("R710-A".to_string(), 0.12); // Cross-laser: Red detector
    signatures.push(SpectralSignature {
        name: "Fluor_B660".to_string(),
        primary_detector: "B660-A".to_string(),
        detector_signals: flu7,
    });

    // Fluor 8: YG585 (Yellow-Green laser primary, also excited by Blue laser)
    // Emission ~585nm, spills into YG and Blue detectors
    let mut flu8 = HashMap::new();
    flu8.insert("YG585-A".to_string(), 1.0); // Primary (YG laser)
    flu8.insert("YG615-A".to_string(), 0.25); // Same laser spillover
    flu8.insert("YG660-A".to_string(), 0.10); // Same laser spillover
    flu8.insert("B560-A".to_string(), 0.18); // Cross-laser: Blue detector (similar wavelength)
    flu8.insert("B610-A".to_string(), 0.12); // Cross-laser: Blue detector
    flu8.insert("V525-A".to_string(), 0.08); // Cross-laser: Violet detector (weak)
    signatures.push(SpectralSignature {
        name: "Fluor_YG585".to_string(),
        primary_detector: "YG585-A".to_string(),
        detector_signals: flu8,
    });

    // Fluor 9: YG750 (Yellow-Green laser primary, also excited by UV and Violet lasers)
    // Emission ~750nm, spills into YG, UV, and Violet detectors
    let mut flu9 = HashMap::new();
    flu9.insert("YG750-A".to_string(), 1.0); // Primary (YG laser)
    flu9.insert("YG660-A".to_string(), 0.20); // Same laser spillover
    flu9.insert("YG812-A".to_string(), 0.18); // Same laser spillover
    flu9.insert("UV736-A".to_string(), 0.15); // Cross-laser: UV detector (similar wavelength)
    flu9.insert("UV812-A".to_string(), 0.10); // Cross-laser: UV detector
    flu9.insert("V720-A".to_string(), 0.12); // Cross-laser: Violet detector
    flu9.insert("V780-A".to_string(), 0.10); // Cross-laser: Violet detector
    flu9.insert("R750-A".to_string(), 0.08); // Cross-laser: Red detector (same wavelength!)
    signatures.push(SpectralSignature {
        name: "Fluor_YG750".to_string(),
        primary_detector: "YG750-A".to_string(),
        detector_signals: flu9,
    });

    // Fluor 10: R710 (Red laser primary, also excited by Blue and Yellow-Green lasers)
    // Emission ~710nm, spills into Red, Blue, and YG detectors
    let mut flu10 = HashMap::new();
    flu10.insert("R710-A".to_string(), 1.0); // Primary (Red laser)
    flu10.insert("R660-A".to_string(), 0.15); // Same laser spillover
    flu10.insert("R750-A".to_string(), 0.22); // Same laser spillover
    flu10.insert("R780-A".to_string(), 0.12); // Same laser spillover
    flu10.insert("B710-A".to_string(), 0.18); // Cross-laser: Blue detector (same wavelength!)
    flu10.insert("B660-A".to_string(), 0.10); // Cross-laser: Blue detector
    flu10.insert("V720-A".to_string(), 0.08); // Cross-laser: Violet detector (weak)
    flu10.insert("YG750-A".to_string(), 0.10); // Cross-laser: YG detector (similar wavelength)
    signatures.push(SpectralSignature {
        name: "Fluor_R710".to_string(),
        primary_detector: "R710-A".to_string(),
        detector_signals: flu10,
    });

    signatures
}

/// Create abundance matrix where cells express subsets of fluorophores
///
/// This mimics real cell expression where not all cells express all markers.
/// Different samples have different expression patterns.
fn create_varying_expression_abundances(
    n_events: usize,
    signatures: &[SpectralSignature],
    sample_idx: usize,
) -> Array2<f64> {
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut abundances = Array2::zeros((n_events, signatures.len()));

    // Define expression patterns for each sample
    // Each pattern specifies which fluorophores are commonly expressed together
    let patterns: Vec<Vec<usize>> = match sample_idx {
        1 => vec![
            vec![0, 1, 2],       // Pattern 1: UV fluors
            vec![3, 4],          // Pattern 2: Violet fluors
            vec![5, 6],          // Pattern 3: Blue fluors
            vec![7, 8],          // Pattern 4: Yellow-Green fluors
            vec![9],             // Pattern 5: Red fluor alone
            vec![0, 3, 5, 7, 9], // Pattern 6: One from each laser
        ],
        2 => vec![
            vec![1, 2, 3], // Pattern 1: UV446, UV736, V660
            vec![4, 5, 6], // Pattern 2: V720, B510, B660
            vec![7, 8, 9], // Pattern 3: YG585, YG750, R710
            vec![0, 4, 7], // Pattern 4: UV379, V720, YG585
            vec![2, 6, 9], // Pattern 5: UV736, B660, R710
        ],
        3 => vec![
            vec![0, 1, 2, 3, 4], // Pattern 1: All UV and Violet
            vec![5, 6, 7, 8, 9], // Pattern 2: All Blue, YG, and Red
            vec![0, 3, 5, 7, 9], // Pattern 3: One from each laser
            vec![1, 2, 4, 6, 8], // Pattern 4: Different set from each laser
        ],
        _ => vec![vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]], // All fluors
    };

    for event_idx in 0..n_events {
        // Randomly select a pattern for this event
        let pattern_idx = rng.random_range(0..patterns.len());
        let pattern = &patterns[pattern_idx];

        // For each fluorophore in the pattern, assign abundance
        for &fluor_idx in pattern {
            // Abundance varies: some cells express strongly, others weakly
            let base_abundance = if rng.random_bool(0.7) {
                // 70% chance of strong expression
                rng.random_range(0.6..1.0)
            } else {
                // 30% chance of weak expression
                rng.random_range(0.1..0.5)
            };
            abundances[(event_idx, fluor_idx)] = base_abundance;
        }

        // Some events may have additional weak expression from other fluors (spillover-like)
        for fluor_idx in 0..signatures.len() {
            if !pattern.contains(&fluor_idx) && rng.random_bool(0.15) {
                // 15% chance of weak expression from non-pattern fluors
                abundances[(event_idx, fluor_idx)] = rng.random_range(0.05..0.2);
            }
        }
    }

    abundances
}

// ===========================================================================
// Benchmark-oriented synthetic data engine
// ===========================================================================

/// Describes a synthetic benchmark dataset (pure numeric matrices, no FCS I/O).
#[derive(Debug, Clone)]
pub struct SyntheticBenchmarkData {
    /// Mixing matrix (detectors × endmembers).
    pub mixing_matrix: ndarray::Array2<f64>,
    /// True abundances per event (events × endmembers).
    pub true_abundances: ndarray::Array2<f64>,
    /// Observed detector signals (events × detectors): `M * alpha_t + epsilon`.
    pub observations: ndarray::Array2<f64>,
    /// Unstained control observations (events × detectors).
    pub unstained_observations: ndarray::Array2<f64>,
    /// Noise level used (standard deviation of additive Gaussian noise).
    pub noise_sigma: f64,
    /// Endmember names.
    pub endmember_names: Vec<String>,
    /// Detector names.
    pub detector_names: Vec<String>,
    /// Label for this dataset.
    pub label: String,
}

/// Panel overlap regime used for 4-color synthetic mimics.
#[derive(Debug, Clone, Copy)]
pub enum PanelOverlap {
    /// Dyes with minimal spectral overlap.
    WellSeparated,
    /// Dyes with high collinearity.
    HighlyOverlapping,
}

/// Generate a benchmark dataset using the linear mixing model `o = M · α + ε`.
///
/// The mixing matrix is built from the provided `SpectralSignature` list.
/// True abundances are sampled randomly; noise is additive Gaussian.
///
/// # Arguments
/// * `signatures` – spectral signatures (one per endmember).
/// * `detector_names` – all detector channel names.
/// * `n_events` – number of stained events to generate.
/// * `n_unstained` – number of unstained control events.
/// * `noise_sigma` – standard deviation of additive Gaussian noise (0 = noise-free).
/// * `label` – human-readable label for the dataset.
pub fn generate_benchmark_data(
    signatures: &[SpectralSignature],
    detector_names: &[String],
    n_events: usize,
    n_unstained: usize,
    noise_sigma: f64,
    label: &str,
) -> Result<SyntheticBenchmarkData> {
    use ndarray::Array2;
    use rand_distr::{Distribution, Normal};

    let n_det = detector_names.len();
    let n_em = signatures.len();

    // Build mixing matrix M (detectors × endmembers)
    let mut mixing = Array2::<f64>::zeros((n_det, n_em));
    for (em_idx, sig) in signatures.iter().enumerate() {
        for (det_idx, det_name) in detector_names.iter().enumerate() {
            mixing[(det_idx, em_idx)] = sig.detector_signals.get(det_name).copied().unwrap_or(0.0);
        }
    }

    let mut rng = rand::rng();
    let noise_dist =
        Normal::new(0.0, noise_sigma.max(1e-30)).context("Failed to create noise distribution")?;

    // Generate true abundances: mixture of positive (stained) and zero (negative) entries
    let mut true_abundances = Array2::<f64>::zeros((n_events, n_em));
    for ev in 0..n_events {
        let n_active = 1 + (ev % n_em);
        for em in 0..n_active.min(n_em) {
            use rand::RngExt;
            true_abundances[(ev, em)] = rng.random_range(0.2..1.0);
        }
    }

    // Observations = M · alpha^T (then transpose back) + noise
    let mut observations = Array2::<f64>::zeros((n_events, n_det));
    for ev in 0..n_events {
        for det in 0..n_det {
            let mut signal = 0.0;
            for em in 0..n_em {
                signal += mixing[(det, em)] * true_abundances[(ev, em)];
            }
            let noise_val = if noise_sigma > 0.0 {
                noise_dist.sample(&mut rng)
            } else {
                0.0
            };
            observations[(ev, det)] = signal + noise_val;
        }
    }

    // Unstained control: just noise (no true signal)
    let mut unstained = Array2::<f64>::zeros((n_unstained, n_det));
    for ev in 0..n_unstained {
        for det in 0..n_det {
            let noise_val = if noise_sigma > 0.0 {
                noise_dist.sample(&mut rng)
            } else {
                0.0
            };
            unstained[(ev, det)] = noise_val.max(0.0);
        }
    }

    Ok(SyntheticBenchmarkData {
        mixing_matrix: mixing,
        true_abundances,
        observations,
        unstained_observations: unstained,
        noise_sigma,
        endmember_names: signatures.iter().map(|s| s.name.clone()).collect(),
        detector_names: detector_names.to_vec(),
        label: label.to_string(),
    })
}

/// Create a 4-color panel preset (well-separated or overlapping).
pub fn create_4color_panel(overlap: PanelOverlap) -> (Vec<SpectralSignature>, Vec<String>) {
    let detector_names: Vec<String> = vec![
        "UV1-A", "UV2-A", "V1-A", "V2-A", "B1-A", "B2-A", "R1-A", "R2-A",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let signatures = match overlap {
        PanelOverlap::WellSeparated => {
            vec![
                make_sig("AF488", &[("B1-A", 1.0), ("B2-A", 0.05)]),
                make_sig("BV421", &[("V1-A", 1.0), ("V2-A", 0.08)]),
                make_sig("BUV395", &[("UV1-A", 1.0), ("UV2-A", 0.06)]),
                make_sig("AF700", &[("R1-A", 1.0), ("R2-A", 0.10)]),
            ]
        }
        PanelOverlap::HighlyOverlapping => {
            vec![
                make_sig("PacBlue", &[("V1-A", 1.0), ("V2-A", 0.35), ("UV2-A", 0.20)]),
                make_sig("BV421", &[("V1-A", 0.85), ("V2-A", 1.0), ("UV2-A", 0.15)]),
                make_sig(
                    "SB436",
                    &[
                        ("V1-A", 0.70),
                        ("V2-A", 0.90),
                        ("UV2-A", 0.25),
                        ("B1-A", 0.10),
                    ],
                ),
                make_sig("AF700", &[("R1-A", 1.0), ("R2-A", 0.10)]),
            ]
        }
    };

    (signatures, detector_names)
}

fn make_sig(name: &str, signals: &[(&str, f64)]) -> SpectralSignature {
    SpectralSignature {
        name: name.to_string(),
        primary_detector: signals[0].0.to_string(),
        detector_signals: signals.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    }
}

/// Generate a suite of benchmark datasets at multiple noise levels.
pub fn generate_benchmark_suite(
    n_events: usize,
    n_unstained: usize,
    noise_levels: &[f64],
) -> Result<Vec<SyntheticBenchmarkData>> {
    let mut datasets = Vec::new();

    for &overlap in &[PanelOverlap::WellSeparated, PanelOverlap::HighlyOverlapping] {
        let (sigs, dets) = create_4color_panel(overlap);
        let overlap_label = match overlap {
            PanelOverlap::WellSeparated => "well_separated",
            PanelOverlap::HighlyOverlapping => "overlapping",
        };

        for &noise in noise_levels {
            let label = format!("{}_noise_{:.4}", overlap_label, noise);
            let data = generate_benchmark_data(&sigs, &dets, n_events, n_unstained, noise, &label)?;
            datasets.push(data);
        }
    }

    // Also generate high-dimensional (10-fluor, 25-channel) datasets
    let sigs = create_10_fluorophore_signatures();
    let dets: Vec<String> = vec![
        "UV379-A", "UV446-A", "UV582-A", "UV736-A", "UV812-A", "V508-A", "V525-A", "V660-A",
        "V720-A", "V780-A", "B510-A", "B560-A", "B610-A", "B660-A", "B710-A", "YG585-A", "YG615-A",
        "YG660-A", "YG750-A", "YG812-A", "R660-A", "R710-A", "R750-A", "R780-A", "R810-A",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    for &noise in noise_levels {
        let label = format!("high_dim_10fluor_noise_{:.4}", noise);
        let data = generate_benchmark_data(&sigs, &dets, n_events, n_unstained, noise, &label)?;
        datasets.push(data);
    }

    Ok(datasets)
}
