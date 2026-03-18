use anyhow::{Context, Result};
use clap::Parser;
use dialoguer::{Confirm, Input};
use flow_fcs::{Fcs, write_fcs_file};
use peacoqc_rs::{
    DoubletConfig, FcsFilter, MarginConfig, PeacoQCConfig, PeacoQCData, QCMode, QCPlotConfig,
    create_qc_plots, peacoqc, remove_doublets, remove_margins,
};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{debug, info, warn};

/// PeacoQC - Quality Control for Flow Cytometry Data
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "peacoqc")]
#[command(about = "Peak-based quality control for flow cytometry FCS files", long_about = None)]
struct Cli {
    /// Path(s) to input FCS file(s) or directory containing FCS files
    /// Can specify multiple files or a directory
    #[arg(value_name = "INPUT_FILES")]
    input: Vec<PathBuf>,

    /// Output directory for cleaned FCS files (optional)
    /// If not specified, output files will be saved alongside input files with "_cleaned" suffix
    #[arg(short, long, value_name = "OUTPUT_DIR")]
    output: Option<PathBuf>,

    /// Channels to analyze (comma-separated, e.g., "FSC-A,SSC-A,FL1-A")
    /// If not specified, all fluorescence channels will be analyzed
    #[arg(short, long, value_delimiter = ',')]
    channels: Option<Vec<String>>,

    /// Quality control mode
    #[arg(short = 'm', long, value_enum, default_value = "all")]
    qc_mode: QCModeArg,

    /// MAD threshold (default: 6.0) - Higher = less strict
    #[arg(long, default_value = "6.0")]
    mad: f64,

    /// Isolation Tree limit (default: 0.6) - Higher = less strict
    #[arg(long, default_value = "0.6")]
    it_limit: f64,

    /// Consecutive bins threshold (default: 5)
    #[arg(long, default_value = "5")]
    consecutive_bins: usize,

    /// Remove zeros before peak detection
    #[arg(long)]
    remove_zeros: bool,

    /// Keep margin events (default: margins are removed)
    #[arg(long)]
    keep_margins: bool,

    /// Keep doublet events (default: doublets are removed)
    #[arg(long)]
    keep_doublets: bool,

    /// Doublet nmad threshold (default: 4.0)
    #[arg(long, default_value = "4.0")]
    doublet_nmad: f64,

    /// Save QC report as JSON (for single file) or directory (for multiple files)
    #[arg(long, value_name = "REPORT_PATH")]
    report: Option<PathBuf>,

    /// Export QC results as boolean CSV (0/1 values)
    /// Recommended format for general use (pandas, R, SQL)
    #[arg(long, value_name = "CSV_PATH")]
    export_csv: Option<PathBuf>,

    /// Export QC results as numeric CSV (2000/6000 values, R-compatible)
    #[arg(long, value_name = "CSV_PATH")]
    export_csv_numeric: Option<PathBuf>,

    /// Export QC metadata as JSON
    #[arg(long, value_name = "JSON_PATH")]
    export_json: Option<PathBuf>,

    /// Column name for CSV exports (default: "PeacoQC")
    #[arg(long, default_value = "PeacoQC")]
    csv_column_name: String,

    /// Generate QC plots after processing (if not specified, will prompt interactively)
    #[arg(long)]
    plots: Option<bool>,

    /// Directory to save QC plots (defaults to same directory as input file if not specified)
    #[arg(long, value_name = "PLOT_DIR")]
    plot_dir: Option<PathBuf>,

    /// Hide spline and MAD threshold lines in plots (shown by default)
    #[arg(long)]
    hide_spline_mad: bool,

    /// Show bin boundaries (gray vertical lines) in plots (hidden by default)
    #[arg(long)]
    show_bin_boundaries: bool,

    /// Plot image width in pixels (default: 2400)
    #[arg(long, value_name = "PIXELS")]
    plot_width: Option<u32>,

    /// Plot image height in pixels (default: 1800)
    #[arg(long, value_name = "PIXELS")]
    plot_height: Option<u32>,

    /// Plot title (caption) font size in points (default: 22)
    #[arg(long, value_name = "SIZE")]
    plot_title_size: Option<u32>,

    /// Plot axis label font size in points (default: 20)
    #[arg(long, value_name = "SIZE")]
    plot_axis_size: Option<u32>,

    /// Plot tick label font size in points (default: 17)
    #[arg(long, value_name = "SIZE")]
    plot_tick_size: Option<u32>,

    /// Plot legend font size in points (default: 17)
    #[arg(long, value_name = "SIZE")]
    plot_legend_size: Option<u32>,

    /// Plot font family for all text (e.g. "sans-serif", "serif"; default: "sans-serif")
    #[arg(long, value_name = "FONT")]
    plot_font: Option<String>,

    /// Cofactor for arcsinh transformation (default: 2000)
    /// Lower values = more compression, higher values = less compression
    #[arg(long, default_value = "2000")]
    cofactor: f32,

    /// Iterate over multiple cofactor values (comma-separated, e.g., "1000,2000,5000")
    /// When specified, QC will be run for each cofactor value
    #[arg(long, value_delimiter = ',')]
    cofactors: Option<Vec<f32>>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Disable all logging (tracing) output
    #[arg(short, long)]
    quiet: bool,

    /// Run benchmark: process one file with four scenarios (minimal, +FCS write, +CSV export, +plots) and print mean timings. Requires exactly one input file. Logging is disabled during benchmark.
    #[arg(long)]
    benchmark: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum QCModeArg {
    /// Use both Isolation Tree and MAD methods
    All,
    /// Use only Isolation Tree
    It,
    /// Use only MAD method
    Mad,
    /// No quality control, only peak detection
    None,
}

impl From<QCModeArg> for QCMode {
    fn from(mode: QCModeArg) -> Self {
        match mode {
            QCModeArg::All => QCMode::All,
            QCModeArg::It => QCMode::IsolationTree,
            QCModeArg::Mad => QCMode::MAD,
            QCModeArg::None => QCMode::None,
        }
    }
}

/// Build QC plot configuration from CLI arguments.
/// Any option not set uses the library default.
fn build_plot_config(args: &Cli) -> QCPlotConfig {
    let mut config = QCPlotConfig {
        show_spline_and_mad: !args.hide_spline_mad,
        show_bin_boundaries: args.show_bin_boundaries,
        ..Default::default()
    };
    if let Some(w) = args.plot_width {
        config.width = w;
    }
    if let Some(h) = args.plot_height {
        config.height = h;
    }
    if let Some(s) = args.plot_title_size {
        config.caption_font_size = s;
    }
    if let Some(s) = args.plot_axis_size {
        config.axis_label_size = s;
    }
    if let Some(s) = args.plot_tick_size {
        config.tick_label_size = s;
    }
    if let Some(s) = args.plot_legend_size {
        config.legend_font_size = s;
    }
    if let Some(ref f) = args.plot_font {
        config.font_family = Some(f.clone());
    }
    config
}

/// Result of processing a single file
#[derive(Debug)]
struct FileResult {
    filename: String,
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    n_events_before: usize,
    n_events_after: usize,
    n_parameters: usize,
    percentage_removed: f64,
    it_percentage: Option<f64>,
    mad_percentage: Option<f64>,
    consecutive_percentage: f64,
    processing_time_ms: u128,
    error: Option<String>,
    cofactor_used: f32,
    // Store data needed for plot generation
    fcs_data: Option<Fcs>,
    qc_result: Option<peacoqc_rs::PeacoQCResult>,
}

/// Ensure an output directory exists and we can create it. Call this early before
/// running computations so the user gets a clear permission/path error upfront.
fn ensure_output_directory(path: &Path, purpose: &str) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("Cannot create {} directory: {}", purpose, path.display()))
}

/// Collect all FCS files from input paths (handles files and directories)
fn collect_input_files(inputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for input in inputs {
        if input.is_file() {
            if input.extension().and_then(|s| s.to_str()) == Some("fcs")
                || input.extension().and_then(|s| s.to_str()) == Some("FCS")
            {
                files.push(input.clone());
            }
        } else if input.is_dir() {
            // Recursively find FCS files in directory
            for entry in walkdir::WalkDir::new(input).into_iter() {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if ext.eq_ignore_ascii_case(&"fcs") {
                            files.push(path.to_path_buf());
                        }
                    }
                }
            }
        } else {
            return Err(anyhow::anyhow!("Path does not exist: {}", input.display()));
        }
    }

    Ok(files)
}

/// Process a single FCS file
fn process_single_file(
    input_path: &Path,
    output_dir: Option<&Path>,
    config: &ProcessingConfig,
) -> FileResult {
    let start_time = Instant::now();
    let filename = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Determine output path
    let output_path = output_dir.map(|dir| {
        let output_filename = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{}_cleaned.fcs", s))
            .unwrap_or_else(|| "output.fcs".to_string());
        dir.join(output_filename)
    });

    match process_file_internal(input_path, output_path.as_deref(), config) {
        Ok(result) => FileResult {
            filename,
            input_path: input_path.to_path_buf(),
            output_path,
            n_events_before: result.n_events_before,
            n_events_after: result.n_events_after,
            n_parameters: result.n_parameters,
            percentage_removed: result.percentage_removed,
            it_percentage: result.it_percentage,
            mad_percentage: result.mad_percentage,
            consecutive_percentage: result.consecutive_percentage,
            processing_time_ms: start_time.elapsed().as_millis(),
            error: None,
            cofactor_used: result.cofactor_used,
            fcs_data: Some(result.fcs_data),
            qc_result: Some(result.qc_result),
        },
        Err(e) => FileResult {
            filename,
            input_path: input_path.to_path_buf(),
            output_path,
            n_events_before: 0,
            n_events_after: 0,
            n_parameters: 0,
            percentage_removed: 0.0,
            it_percentage: None,
            mad_percentage: None,
            consecutive_percentage: 0.0,
            processing_time_ms: start_time.elapsed().as_millis(),
            error: Some(e.to_string()),
            cofactor_used: config.cofactor,
            fcs_data: None,
            qc_result: None,
        },
    }
}

/// Internal processing result
struct InternalResult {
    n_events_before: usize,
    n_events_after: usize,
    n_parameters: usize,
    percentage_removed: f64,
    it_percentage: Option<f64>,
    mad_percentage: Option<f64>,
    consecutive_percentage: f64,
    cofactor_used: f32,
    // Store data needed for plot generation
    fcs_data: Fcs,
    qc_result: peacoqc_rs::PeacoQCResult,
}

/// Processing configuration
#[derive(Clone)]
struct ProcessingConfig {
    channels: Option<Vec<String>>,
    qc_mode: QCMode,
    mad: f64,
    it_limit: f64,
    consecutive_bins: usize,
    remove_zeros: bool,
    remove_margins: bool,
    remove_doublets: bool,
    doublet_nmad: f64,
    export_csv: Option<PathBuf>,
    export_csv_numeric: Option<PathBuf>,
    export_json: Option<PathBuf>,
    csv_column_name: String,
    cofactor: f32,
    generate_plots: bool,
    plot_dir: Option<PathBuf>,
}

/// Internal function to process a single file (called from process_single_file)
fn process_file_internal(
    input_path: &Path,
    output_path: Option<&Path>,
    config: &ProcessingConfig,
) -> Result<InternalResult> {
    use peacoqc_rs::{
        export_csv_boolean, export_csv_boolean_from_mask, export_csv_numeric,
        export_csv_numeric_from_mask, export_json_metadata,
    };
    // Load FCS file
    let fcs = Fcs::open(
        input_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
    )?;

    // Log event count discrepancy check
    let n_events_from_tot = fcs.get_number_of_events().ok().copied().unwrap_or(0);
    let n_events_initial = fcs.get_event_count_from_dataframe();

    info!(
        "FCS file loaded: {} events from DataFrame, {} events from $TOT keyword",
        n_events_initial, n_events_from_tot
    );

    if n_events_initial != n_events_from_tot {
        warn!(
            "Event count mismatch: DataFrame has {} events but $TOT keyword says {} (difference: {})",
            n_events_initial,
            n_events_from_tot,
            n_events_from_tot as i64 - n_events_initial as i64
        );
    }

    // Log compensation status
    let has_compensation = fcs.has_compensation();

    // Log detailed compensation status
    match fcs.get_spillover_matrix() {
        Ok(Some((matrix, names))) => {
            info!(
                "Compensation status: available ({}x{} matrix, {} parameters)",
                matrix.nrows(),
                matrix.ncols(),
                names.len()
            );
        }
        Ok(None) => {
            info!("Compensation status: not available (SPILLOVER/SPILL/COMP keyword missing)");
        }
        Err(e) => {
            warn!(
                "Compensation status: error reading compensation matrix: {}",
                e
            );
        }
    }

    // Log all available channels
    let all_channels = fcs.channel_names();
    debug!(
        "All available channels ({}): {:?}",
        all_channels.len(),
        all_channels
    );

    // Determine channels
    let channels = config
        .channels
        .clone()
        .unwrap_or_else(|| fcs.get_fluorescence_channels());

    if channels.is_empty() {
        return Err(anyhow::anyhow!("No channels specified or detected"));
    }

    info!(
        "Selected {} channels for analysis: {:?}",
        channels.len(),
        channels
    );

    // Check if Time and AF channels are included/excluded
    let has_time = channels.iter().any(|c| c.to_uppercase().contains("TIME"));
    let has_af = channels.iter().any(|c| c.to_uppercase().contains("AF"));
    debug!(
        "Channel selection: Time={}, AF (autofluorescence)={}",
        has_time, has_af
    );

    let mut current_fcs = fcs;

    // R's preprocessing order: RemoveMargins → RemoveDoublets → Compensate → Transform
    // This order matters because margin/doublet removal should happen on raw data
    // before transformation affects the values.
    // We keep margin/doublet masks so CSV (and other) exports can emit one row per original
    // event, with margin/doublet-removed events assigned to the "bad" bin.
    let mut margin_mask: Option<Vec<bool>> = None;
    let mut doublet_mask: Option<Vec<bool>> = None;

    // Step 1: Remove margins (optional) - BEFORE transformation
    if config.remove_margins {
        let n_events_before_margins = current_fcs.get_event_count_from_dataframe();
        info!("Removing margin events (preprocessing step)");

        let margin_config = MarginConfig {
            channels: channels.clone(),
            channel_specifications: None,
            remove_min: None,
            remove_max: None,
        };

        let margin_result = remove_margins(&current_fcs, &margin_config)?;
        margin_mask = Some(margin_result.mask.clone());

        if margin_result.percentage_removed > 0.0 {
            current_fcs = current_fcs.filter(&margin_result.mask)?;
            let n_events_after_margins = current_fcs.get_event_count_from_dataframe();
            info!(
                "Margin removal: {} events removed ({:.2}%), {} events remaining",
                n_events_before_margins - n_events_after_margins,
                margin_result.percentage_removed,
                n_events_after_margins
            );
        } else {
            debug!("No margin events detected");
        }
    }

    // Step 2: Remove doublets (optional) - BEFORE transformation
    if config.remove_doublets {
        let n_events_before_doublets = current_fcs.get_event_count_from_dataframe();
        info!("Removing doublet events (preprocessing step)");

        let doublet_config = DoubletConfig {
            channel1: "FSC-A".to_string(),
            channel2: "FSC-H".to_string(),
            nmad: config.doublet_nmad,
            b: 0.0,
        };

        match remove_doublets(&current_fcs, &doublet_config) {
            Ok(doublet_result) => {
                doublet_mask = Some(doublet_result.mask.clone());
                if doublet_result.percentage_removed > 0.0 {
                    current_fcs = current_fcs.filter(&doublet_result.mask)?;
                    let n_events_after_doublets = current_fcs.get_event_count_from_dataframe();
                    info!(
                        "Doublet removal: {} events removed ({:.2}%), {} events remaining",
                        n_events_before_doublets - n_events_after_doublets,
                        doublet_result.percentage_removed,
                        n_events_after_doublets
                    );
                } else {
                    debug!("No doublet events detected");
                }
            }
            Err(e) => {
                warn!(
                    "Doublet removal failed (FSC-A/FSC-H channels may be missing): {}, continuing without doublet removal",
                    e
                );
            }
        }
    }

    // Step 3: Apply compensation and transformation (matching R implementation behavior)
    // R logic: if compensation available → biexponential/logicle, else → arcsinh
    // Transformation is CRITICAL for MAD detection - without it, raw fluorescence ranges
    let cofactor = config.cofactor;
    if has_compensation {
        info!(
            "Applying compensation and biexponential transformation (matching R PeacoQC: compensate + estimateLogicle)"
        );
        let fcs_before_preprocess = current_fcs.clone();
        match peacoqc_rs::preprocess_fcs(current_fcs, true, true, cofactor) {
            Ok(preprocessed_fcs) => {
                current_fcs = preprocessed_fcs;
                let n_events_after = current_fcs.get_event_count_from_dataframe();
                info!(
                    "Preprocessing complete: {} events (compensation + biexponential/logicle transform applied)",
                    n_events_after
                );
            }
            Err(e) => {
                warn!(
                    "Failed to apply preprocessing: {}, continuing with raw data (MAD results may differ from R)",
                    e
                );
                current_fcs = fcs_before_preprocess;
            }
        }
    } else {
        // No compensation available - still apply transformation for better MAD results
        info!(
            "No compensation available, applying arcsinh transformation only (cofactor={})",
            cofactor
        );
        let fcs_before_preprocess = current_fcs.clone();
        match peacoqc_rs::preprocess_fcs(current_fcs, false, true, cofactor) {
            Ok(preprocessed_fcs) => {
                current_fcs = preprocessed_fcs;
                info!(
                    "Transformation applied (arcsinh with cofactor={})",
                    cofactor
                );
            }
            Err(e) => {
                warn!(
                    "Failed to apply transformation: {}, continuing with raw data (MAD results may differ from R)",
                    e
                );
                current_fcs = fcs_before_preprocess;
            }
        }
    }

    // Run PeacoQC
    let peacoqc_config = PeacoQCConfig {
        channels: channels.clone(),
        determine_good_cells: config.qc_mode,
        mad: config.mad,
        it_limit: config.it_limit,
        consecutive_bins: config.consecutive_bins,
        remove_zeros: config.remove_zeros,
        ..Default::default()
    };

    let peacoqc_result = peacoqc(&current_fcs, &peacoqc_config)?;

    // Apply filter
    let clean_fcs = current_fcs.filter(&peacoqc_result.good_cells)?;
    let n_events_final = clean_fcs.n_events();

    // Save output (if path provided)
    if let Some(output_path) = output_path {
        info!("Writing cleaned FCS file to: {}", output_path.display());
        write_fcs_file(clean_fcs, output_path)?;
        info!("Successfully wrote cleaned FCS file");
    }

    // Build full-length good/bad mask (one entry per original event) when we removed
    // margins or doublets, so CSV exports have one row per input event and
    // margin/doublet-removed events are assigned to the bad bin.
    let use_full_mask = margin_mask.is_some() || doublet_mask.is_some();
    let full_export_mask: Option<Vec<bool>> = if use_full_mask {
        let mut full_mask = vec![false; n_events_initial];
        for i in 0..n_events_initial {
            let kept_after_margin = margin_mask
                .as_ref()
                .map(|m| m[i])
                .unwrap_or(true);
            if !kept_after_margin {
                continue;
            }
            let margin_idx = margin_mask
                .as_ref()
                .map(|m| m[0..i].iter().filter(|&&x| x).count())
                .unwrap_or(i);
            let kept_after_doublet = doublet_mask
                .as_ref()
                .map(|d| d[margin_idx])
                .unwrap_or(true);
            if !kept_after_doublet {
                continue;
            }
            let qc_idx = doublet_mask
                .as_ref()
                .map(|d| d[0..margin_idx].iter().filter(|&&x| x).count())
                .unwrap_or(margin_idx);
            full_mask[i] = peacoqc_result.good_cells[qc_idx];
        }
        Some(full_mask)
    } else {
        None
    };

    // Export QC results if requested (one row per original input event)
    let input_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    if let Some(ref csv_path) = config.export_csv {
        let export_path = if csv_path.is_dir() {
            csv_path.join(format!("{}.PeacoQC.csv", input_stem))
        } else {
            csv_path.clone()
        };
        if let Some(ref mask) = full_export_mask {
            export_csv_boolean_from_mask(mask, &export_path, Some(&config.csv_column_name))
                .map_err(|e| anyhow::anyhow!("Failed to export CSV: {}", e))?;
        } else {
            export_csv_boolean(&peacoqc_result, &export_path, Some(&config.csv_column_name))
                .map_err(|e| anyhow::anyhow!("Failed to export CSV: {}", e))?;
        }
        info!("Exported boolean CSV to: {}", export_path.display());
    }

    if let Some(ref csv_numeric_path) = config.export_csv_numeric {
        let export_path = if csv_numeric_path.is_dir() {
            csv_numeric_path.join(format!("{}.PeacoQC.csv", input_stem))
        } else {
            csv_numeric_path.clone()
        };
        if let Some(ref mask) = full_export_mask {
            export_csv_numeric_from_mask(
                mask,
                &export_path,
                2000,
                6000,
                Some(&config.csv_column_name),
            )
            .map_err(|e| anyhow::anyhow!("Failed to export numeric CSV: {}", e))?;
        } else {
            export_csv_numeric(
                &peacoqc_result,
                &export_path,
                2000,
                6000,
                Some(&config.csv_column_name),
            )
            .map_err(|e| anyhow::anyhow!("Failed to export numeric CSV: {}", e))?;
        }
        info!("Exported numeric CSV to: {}", export_path.display());
    }

    if let Some(ref json_path) = config.export_json {
        let export_path = if json_path.is_dir() {
            json_path.join(format!("{}.PeacoQC.json", input_stem))
        } else {
            json_path.clone()
        };
        if let Some(ref mask) = full_export_mask {
            let json_result = peacoqc_rs::PeacoQCResult {
                good_cells: mask.clone(),
                ..peacoqc_result.clone()
            };
            export_json_metadata(&json_result, &peacoqc_config, &export_path)
                .map_err(|e| anyhow::anyhow!("Failed to export JSON: {}", e))?;
        } else {
            export_json_metadata(&peacoqc_result, &peacoqc_config, &export_path)
                .map_err(|e| anyhow::anyhow!("Failed to export JSON: {}", e))?;
        }
        info!("Exported JSON metadata to: {}", export_path.display());
    }

    let n_parameters = current_fcs.get_parameter_count_from_dataframe();
    Ok(InternalResult {
        n_events_before: n_events_initial,
        n_events_after: n_events_final,
        n_parameters,
        percentage_removed: peacoqc_result.percentage_removed,
        it_percentage: peacoqc_result.it_percentage,
        mad_percentage: peacoqc_result.mad_percentage,
        consecutive_percentage: peacoqc_result.consecutive_percentage,
        cofactor_used: cofactor,
        fcs_data: current_fcs,
        qc_result: peacoqc_result,
    })
}

const BENCHMARK_ITERATIONS: usize = 3;

/// Run benchmark: one file, four scenarios (minimal, +FCS write, +CSV export, +plots), report mean ± std.
fn run_benchmark(args: &Cli) -> Result<()> {
    let input_files = collect_input_files(&args.input)?;
    if input_files.len() != 1 {
        return Err(anyhow::anyhow!(
            "--benchmark requires exactly one input file (got {})",
            input_files.len()
        ));
    }
    let input_path = &input_files[0];
    println!("Benchmarking: {}", input_path.display());

    let temp_dir = tempfile::TempDir::new()?;
    let qc_mode: QCMode = args.qc_mode.clone().into();
    let remove_margins = !args.keep_margins;
    let remove_doublets = !args.keep_doublets;

    let base_config = ProcessingConfig {
        channels: args.channels.clone(),
        qc_mode,
        mad: args.mad,
        it_limit: args.it_limit,
        consecutive_bins: args.consecutive_bins,
        remove_zeros: args.remove_zeros,
        remove_margins,
        remove_doublets,
        doublet_nmad: args.doublet_nmad,
        export_csv: None,
        export_csv_numeric: None,
        export_json: None,
        csv_column_name: args.csv_column_name.clone(),
        cofactor: args.cofactor,
        generate_plots: false,
        plot_dir: None,
    };

    fn mean_std(times_ms: &[u128]) -> (f64, f64) {
        if times_ms.is_empty() {
            return (0.0, 0.0);
        }
        let n = times_ms.len() as f64;
        let mean = times_ms.iter().map(|&t| t as f64).sum::<f64>() / n;
        let variance = times_ms
            .iter()
            .map(|&t| (t as f64 - mean).powi(2))
            .sum::<f64>()
            / n;
        let std = variance.sqrt();
        (mean, std)
    }

    // Scenario 1: minimal (no outputs)
    let mut times_minimal = Vec::with_capacity(BENCHMARK_ITERATIONS);
    for _ in 0..BENCHMARK_ITERATIONS {
        let result = process_single_file(input_path, None, &base_config);
        times_minimal.push(result.processing_time_ms);
    }
    let (mean_min, std_min) = mean_std(&times_minimal);

    // Scenario 2: + FCS write
    let mut times_fcs = Vec::with_capacity(BENCHMARK_ITERATIONS);
    for _ in 0..BENCHMARK_ITERATIONS {
        let result = process_single_file(input_path, Some(temp_dir.path()), &base_config);
        times_fcs.push(result.processing_time_ms);
    }
    let (mean_fcs, std_fcs) = mean_std(&times_fcs);

    // Scenario 3: + CSV export (no FCS write)
    let csv_dir = temp_dir.path().to_path_buf();
    let config_csv = ProcessingConfig {
        export_csv: Some(csv_dir.clone()),
        export_csv_numeric: Some(csv_dir),
        ..base_config.clone()
    };
    let mut times_csv = Vec::with_capacity(BENCHMARK_ITERATIONS);
    for _ in 0..BENCHMARK_ITERATIONS {
        let result = process_single_file(input_path, None, &config_csv);
        times_csv.push(result.processing_time_ms);
    }
    let (mean_csv, std_csv) = mean_std(&times_csv);

    // Scenario 4: + plots (create_qc_plots is not inside process_single_file; time it separately)
    let result_minimal = process_single_file(input_path, None, &base_config);
    let (fcs_data, qc_result) = match (result_minimal.fcs_data, result_minimal.qc_result) {
        (Some(fcs), Some(qc)) => (fcs, qc),
        _ => {
            return Err(anyhow::anyhow!(
                "Benchmark minimal run failed or did not return data for plot scenario"
            ));
        }
    };
    let plot_config = build_plot_config(args);
    let plot_path = temp_dir.path().join("bench_qc_plot.png");
    let mut times_plots = Vec::with_capacity(BENCHMARK_ITERATIONS);
    for _ in 0..BENCHMARK_ITERATIONS {
        let t0 = std::time::Instant::now();
        create_qc_plots(&fcs_data, &qc_result, &plot_path, plot_config.clone(), None)
            .map_err(|e| anyhow::anyhow!("Plot generation failed: {}", e))?;
        times_plots.push(t0.elapsed().as_millis());
    }
    let (mean_plots, std_plots) = mean_std(&times_plots);

    println!("\nScenario          Mean (ms)   Std (ms)");
    println!("{:18} {:>10.1}   {:>8.1}", "minimal", mean_min, std_min);
    println!("{:18} {:>10.1}   {:>8.1}", "+ FCS write", mean_fcs, std_fcs);
    println!("{:18} {:>10.1}   {:>8.1}", "+ CSV export", mean_csv, std_csv);
    println!("{:18} {:>10.1}   {:>8.1}", "+ plots", mean_plots, std_plots);
    println!("\nTo measure logging overhead, compare wall time of:");
    println!("  peacoqc <file> -o out  vs  peacoqc --quiet <file> -o out");
    Ok(())
}

fn main() -> Result<()> {
    let args = Cli::parse();

    // Initialize tracing subscriber after parsing so --quiet / --benchmark can disable logging
    let filter = if args.quiet || args.benchmark {
        tracing_subscriber::EnvFilter::new("off")
    } else {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    if args.benchmark {
        run_benchmark(&args)?;
        return Ok(());
    }

    println!("🧬 PeacoQC - Flow Cytometry Quality Control");
    println!("============================================\n");

    // Collect input files (expand directories if needed)
    let input_files = collect_input_files(&args.input)?;

    if input_files.is_empty() {
        eprintln!("❌ Error: No FCS files found");
        std::process::exit(1);
    }

    println!("📂 Found {} file(s) to process\n", input_files.len());

    // Ensure all output directories exist and we can create them before running computations
    if let Some(ref output_dir) = args.output {
        ensure_output_directory(output_dir, "output")?;
    }
    if let Some(ref dir) = args.plot_dir {
        ensure_output_directory(dir, "plot")?;
    }
    if let Some(ref report_path) = args.report {
        let used_as_dir = report_path.is_dir()
            || report_path.extension().is_none();
        if used_as_dir {
            ensure_output_directory(report_path, "report")?;
        }
    }
    for (path, purpose) in [
        (args.export_csv.as_ref(), "export CSV"),
        (args.export_csv_numeric.as_ref(), "export CSV numeric"),
        (args.export_json.as_ref(), "export JSON"),
    ] {
        if let Some(ref p) = path {
            if p.is_dir() || p.extension().is_none() {
                ensure_output_directory(p, purpose)?;
            } else if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    ensure_output_directory(parent, purpose)?;
                }
            }
        }
    }

    // Determine cofactors to use
    let cofactors_to_use = if let Some(ref cofactors) = args.cofactors {
        cofactors.clone()
    } else {
        vec![args.cofactor]
    };

    // Determine if plots should be generated
    let generate_plots = if let Some(plots_flag) = args.plots {
        plots_flag // Use the flag value directly - this fixes the bug where --plots true didn't work
    } else {
        // Prompt user interactively if not specified
        Confirm::new()
            .with_prompt("Generate QC plots?")
            .default(true)
            .interact()
            .unwrap_or(false)
    };

    // Determine plot directory
    let plot_dir = if generate_plots {
        if let Some(ref dir) = args.plot_dir {
            Some(dir.clone())
        } else {
            // Prompt for directory with default
            let default_dir = if input_files.len() == 1 {
                input_files[0]
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_path_buf()
            } else {
                Path::new(".").to_path_buf()
            };

            let default_str = default_dir.to_string_lossy().to_string();
            let dir_input: String = Input::new()
                .with_prompt(format!("Plot directory (default: {})", default_str))
                .default(default_str)
                .interact()
                .unwrap_or_default();

            Some(PathBuf::from(dir_input))
        }
    } else {
        None
    };

    // Create plot directory if needed
    if let Some(ref dir) = plot_dir {
        std::fs::create_dir_all(dir)?;
    }

    // Start timing AFTER all user interactions and setup
    let start_time = Instant::now();

    // Convert qc_mode once before the loop
    let qc_mode = args.qc_mode.clone().into();

    // Process files with each cofactor
    let mut all_results: Vec<FileResult> = Vec::new();

    for cofactor in &cofactors_to_use {
        if cofactors_to_use.len() > 1 {
            println!("\n🔄 Processing with cofactor: {}\n", cofactor);
        }

        // Prepare processing configuration
        // keep_margins/keep_doublets default to false, so removal happens by default
        let remove_margins = !args.keep_margins;
        let remove_doublets = !args.keep_doublets;

        let processing_config = ProcessingConfig {
            channels: args.channels.clone(),
            qc_mode: qc_mode,
            mad: args.mad,
            it_limit: args.it_limit,
            consecutive_bins: args.consecutive_bins,
            remove_zeros: args.remove_zeros,
            remove_margins,
            remove_doublets,
            doublet_nmad: args.doublet_nmad,
            export_csv: args.export_csv.clone(),
            export_csv_numeric: args.export_csv_numeric.clone(),
            export_json: args.export_json.clone(),
            csv_column_name: args.csv_column_name.clone(),
            cofactor: *cofactor,
            generate_plots: false, // Will handle plots after all processing
            plot_dir: plot_dir.clone(),
        };

        // Process files in parallel
        let total_files = input_files.len();
        let results: Vec<FileResult> = input_files
            .par_iter()
            .enumerate()
            .map(|(idx, input_path)| {
                if total_files > 1 {
                    info!(
                        "Processing file {}/{}: {}",
                        idx + 1,
                        total_files,
                        input_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                    );
                }
                process_single_file(input_path, args.output.as_deref(), &processing_config)
            })
            .collect();

        all_results.extend(results);
    }

    let results = all_results;

    // Print results
    let total_time = start_time.elapsed().as_secs_f64();
    let successful: Vec<&FileResult> = results.iter().filter(|r| r.error.is_none()).collect();
    let failed: Vec<&FileResult> = results.iter().filter(|r| r.error.is_some()).collect();

    println!("\n✅ Processing Complete!");
    println!("   Processed: {} file(s)", results.len());
    println!("   Successful: {}", successful.len());
    if !failed.is_empty() {
        println!("   Failed: {}", failed.len());
    }
    println!("   ⏱️  Total time: {:.2}s", total_time);
    let n_files = successful.len();
    if n_files > 0 {
        let avg_time_per_file = total_time / n_files as f64;
        println!("   ⏱️  Average time per file: {:.2}s", avg_time_per_file);
        let total_events: usize = successful.iter().map(|r| r.n_events_after).sum();
        let total_params: usize = successful.iter().map(|r| r.n_parameters).sum();
        let total_observations: usize = successful
            .iter()
            .map(|r| r.n_events_after * r.n_parameters)
            .sum();
        println!("   📊 Average parameters per file: {:.1}", total_params as f64 / n_files as f64);
        println!("   📊 Average events per file: {:.0}", total_events as f64 / n_files as f64);
        println!("   📊 Total events: {}", total_events);
        println!("   📊 Total observations (events × parameters): {}", total_observations);
        if total_time > 0.0 {
            println!(
                "   📊 Throughput: {:.0} observations/s",
                total_observations as f64 / total_time
            );
        }
    }
    println!();

    // Print summaries
    if args.verbose && !successful.is_empty() {
        println!("📊 Results:");
        for result in &successful {
            if results.len() > 1 {
                println!(
                    "   {}: {} → {} events ({:.2}% removed) [{:.2}s]",
                    result.filename,
                    result.n_events_before,
                    result.n_events_after,
                    result.percentage_removed,
                    result.processing_time_ms as f64 / 1000.0
                );
            } else {
                println!(
                    "   {}: {} → {} events ({:.2}% removed)",
                    result.filename,
                    result.n_events_before,
                    result.n_events_after,
                    result.percentage_removed
                );
            }
        }
        println!();
    }

    // Print errors if any
    if !failed.is_empty() {
        eprintln!("❌ Errors:");
        for result in &failed {
            eprintln!("   {}: {}", result.filename, result.error.as_ref().unwrap());
        }
        eprintln!();
    }

    // Save report(s) if requested
    if let Some(ref report_path) = args.report {
        if results.len() == 1 {
            // Single file: save single report
            let result = &results[0];
            let report = serde_json::json!({
                "filename": result.filename,
                "n_events_before": result.n_events_before,
                "n_events_after": result.n_events_after,
                "percentage_removed": result.percentage_removed,
                "it_percentage": result.it_percentage,
                "mad_percentage": result.mad_percentage,
                "consecutive_percentage": result.consecutive_percentage,
                "processing_time_ms": result.processing_time_ms,
            });
            std::fs::write(report_path, serde_json::to_string_pretty(&report)?)?;
        } else {
            // Multiple files: save combined report or directory of reports
            if report_path.is_dir() || report_path.extension().is_none() {
                // Directory: save individual reports
                std::fs::create_dir_all(report_path)?;
                for result in &results {
                    let report_filename = format!("{}.json", result.filename);
                    let report_path = report_path.join(report_filename);
                    let report = serde_json::json!({
                        "filename": result.filename,
                        "n_events_before": result.n_events_before,
                        "n_events_after": result.n_events_after,
                        "percentage_removed": result.percentage_removed,
                        "it_percentage": result.it_percentage,
                        "mad_percentage": result.mad_percentage,
                        "consecutive_percentage": result.consecutive_percentage,
                        "processing_time_ms": result.processing_time_ms,
                        "error": result.error,
                    });
                    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
                }
            } else {
                // Single file: save combined report
                let combined_report = serde_json::json!({
                    "total_files": results.len(),
                    "successful": successful.len(),
                    "failed": failed.len(),
                    "total_time_seconds": total_time,
                    "results": results.iter().map(|r| serde_json::json!({
                        "filename": r.filename,
                        "n_events_before": r.n_events_before,
                        "n_events_after": r.n_events_after,
                        "percentage_removed": r.percentage_removed,
                        "processing_time_ms": r.processing_time_ms,
                        "error": r.error,
                    })).collect::<Vec<_>>(),
                });
                std::fs::write(report_path, serde_json::to_string_pretty(&combined_report)?)?;
            }
        }
    }

    // Handle plot generation (use generate_plots and plot_dir already decided earlier in main)
    if successful.is_empty() {
        // No successful files to plot
    } else if let Some(ref plot_dir) = plot_dir {
            std::fs::create_dir_all(plot_dir)?;
            println!("\n📊 Generating QC plots...");

            // Build plot config from CLI flags
            let plot_config = build_plot_config(&args);

            // Generate plots for each successful file
            for result in &successful {
                if let (Some(fcs_data), Some(qc_result)) = (&result.fcs_data, &result.qc_result) {
                    let plot_filename = result
                        .input_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| format!("{}_qc_plot.png", s))
                        .unwrap_or_else(|| "qc_plot.png".to_string());
                    let plot_path = plot_dir.join(&plot_filename);

                    match create_qc_plots(fcs_data, qc_result, &plot_path, plot_config.clone(), None)
                    {
                        Ok(()) => {
                            println!("   ✅ Generated plot: {}", plot_path.display());
                        }
                        Err(e) => {
                            warn!(
                                "   ⚠️  Failed to generate plot for {}: {}",
                                result.filename, e
                            );
                        }
                    }
                }
            }
            println!();
    }

    // Exit with error code if any files failed
    if !failed.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
