use anyhow::Result;
use clap::Parser;
use flow_fcs::Fcs;
use peacoqc_rs::{
    DoubletConfig, FcsFilter, MarginConfig, PeacoQCConfig, PeacoQCData, QCMode, peacoqc,
    remove_doublets, remove_margins,
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

    /// Remove margin events before QC
    #[arg(long, default_value = "true")]
    remove_margins: bool,

    /// Remove doublets before QC
    #[arg(long, default_value = "true")]
    remove_doublets: bool,

    /// Doublet nmad threshold (default: 4.0)
    #[arg(long, default_value = "4.0")]
    doublet_nmad: f64,

    /// Save QC report as JSON (for single file) or directory (for multiple files)
    #[arg(long, value_name = "REPORT_PATH")]
    report: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
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

/// Result of processing a single file
#[derive(Debug)]
struct FileResult {
    filename: String,
    input_path: PathBuf,
    output_path: Option<PathBuf>,
    n_events_before: usize,
    n_events_after: usize,
    percentage_removed: f64,
    it_percentage: Option<f64>,
    mad_percentage: Option<f64>,
    consecutive_percentage: f64,
    processing_time_ms: u128,
    error: Option<String>,
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
            percentage_removed: result.percentage_removed,
            it_percentage: result.it_percentage,
            mad_percentage: result.mad_percentage,
            consecutive_percentage: result.consecutive_percentage,
            processing_time_ms: start_time.elapsed().as_millis(),
            error: None,
        },
        Err(e) => FileResult {
            filename,
            input_path: input_path.to_path_buf(),
            output_path,
            n_events_before: 0,
            n_events_after: 0,
            percentage_removed: 0.0,
            it_percentage: None,
            mad_percentage: None,
            consecutive_percentage: 0.0,
            processing_time_ms: start_time.elapsed().as_millis(),
            error: Some(e.to_string()),
        },
    }
}

/// Internal processing result
struct InternalResult {
    n_events_before: usize,
    n_events_after: usize,
    percentage_removed: f64,
    it_percentage: Option<f64>,
    mad_percentage: Option<f64>,
    consecutive_percentage: f64,
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
}

/// Internal function to process a single file (called from process_single_file)
fn process_file_internal(
    input_path: &Path,
    output_path: Option<&Path>,
    config: &ProcessingConfig,
) -> Result<InternalResult> {
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
    info!(
        "Compensation status: {} (SPILLOVER keyword {})",
        if has_compensation {
            "available"
        } else {
            "not available"
        },
        if has_compensation {
            "present"
        } else {
            "missing"
        }
    );

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

    // Apply compensation and transformation (matching R implementation behavior)
    // Transformation is CRITICAL for MAD detection - without it, raw fluorescence ranges
    if has_compensation {
        info!("Applying compensation and arcsinh transformation (cofactor=2000, matching R PeacoQC preprocessing)");
        match peacoqc_rs::preprocess_fcs(current_fcs, true, true, 2000.0) {
            Ok(preprocessed_fcs) => {
                current_fcs = preprocessed_fcs;
                let n_events_after = current_fcs.get_event_count_from_dataframe();
                info!(
                    "Preprocessing complete: {} events (compensation + arcsinh transform applied, cofactor=2000)",
                    n_events_after
                );
            }
            Err(e) => {
                warn!(
                    "Failed to apply preprocessing: {}, continuing with raw data (MAD results may differ from R)",
                    e
                );
                // Re-open the file if preprocessing failed
                current_fcs = Fcs::open(
                    input_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
                )?;
            }
        }
    } else {
        // No compensation available - still apply transformation for better MAD results
        info!("No compensation available, applying arcsinh transformation only (cofactor=2000)");
        match peacoqc_rs::preprocess_fcs(current_fcs, false, true, 2000.0) {
            Ok(preprocessed_fcs) => {
                current_fcs = preprocessed_fcs;
                info!("Transformation applied (arcsinh with cofactor=2000)");
            }
            Err(e) => {
                warn!(
                    "Failed to apply transformation: {}, continuing with raw data (MAD results may differ from R)",
                    e
                );
                // Re-open the file if transformation failed
                current_fcs = Fcs::open(
                    input_path
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
                )?;
            }
        }
    }

    // Remove margins (optional)
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

    // Remove doublets (optional)
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
        // TODO: Implement write_to_file on Fcs in flow-fcs crate
        // This is needed for feature parity with the R PeacoQC package (save_fcs=TRUE)
        eprintln!(
            "Note: FCS file writing not yet implemented, would save to: {}",
            output_path.display()
        );
    }

    Ok(InternalResult {
        n_events_before: n_events_initial,
        n_events_after: n_events_final,
        percentage_removed: peacoqc_result.percentage_removed,
        it_percentage: peacoqc_result.it_percentage,
        mad_percentage: peacoqc_result.mad_percentage,
        consecutive_percentage: peacoqc_result.consecutive_percentage,
    })
}

fn main() -> Result<()> {
    // Initialize tracing subscriber with environment filter
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let args = Cli::parse();

    println!("🧬 PeacoQC - Flow Cytometry Quality Control");
    println!("============================================\n");

    let start_time = Instant::now();

    // Collect input files (expand directories if needed)
    let input_files = collect_input_files(&args.input)?;

    if input_files.is_empty() {
        eprintln!("❌ Error: No FCS files found");
        std::process::exit(1);
    }

    println!("📂 Found {} file(s) to process\n", input_files.len());

    // Create output directory if specified
    if let Some(ref output_dir) = args.output {
        std::fs::create_dir_all(output_dir)?;
    }

    // Prepare processing configuration
    let processing_config = ProcessingConfig {
        channels: args.channels,
        qc_mode: args.qc_mode.into(),
        mad: args.mad,
        it_limit: args.it_limit,
        consecutive_bins: args.consecutive_bins,
        remove_zeros: args.remove_zeros,
        remove_margins: args.remove_margins,
        remove_doublets: args.remove_doublets,
        doublet_nmad: args.doublet_nmad,
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
    println!("   ⏱️  Total time: {:.2}s\n", total_time);

    // Print summaries
    if args.verbose && !successful.is_empty() {
        println!("📊 Results:");
        for result in &successful {
            println!(
                "   {}: {} → {} events ({:.2}% removed)",
                result.filename,
                result.n_events_before,
                result.n_events_after,
                result.percentage_removed
            );
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

    // Exit with error code if any files failed
    if !failed.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
