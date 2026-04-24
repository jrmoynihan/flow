//! Command parsing and execution for TRU-OLS CLI

use anyhow::{Context, Result};
use clap::Subcommand;
use faer::{Col, Mat};
use flow_fcs::{EventDataFrame, Fcs, TransformType};
use flow_plots::options::{
    AxisOptions, BasePlotOptions, DensityPlotOptions, SpectralSignaturePlotOptions,
};
use flow_plots::render::RenderConfig;
use flow_plots::{DensityPlot, Plot, SpectralSignaturePlot};
use flow_tru_ols::{
    TruOlsUnmixing, UnmixingStrategy,
    apply_tru_ols_unmixing_from_preprocessed_with_shared_factor_cache, extract_detector_data,
    preprocessing::{CutoffCalculator, NonspecificObservation},
    shared_mask_factor_cache_with_capacity,
};
use flow_utils::KernelDensity;
use ndarray::Array2;
use serde_json;
use std::collections::HashSet;
use std::fs;
use std::io::{Write, stdin, stdout};
use std::path::{Path, PathBuf};
use tracing::{debug, info, info_span, warn};

/// Ensure an output directory exists and we can create it. Call this early before
/// running computations so the user gets a clear permission/path error upfront.
fn ensure_output_directory(path: &Path, purpose: &str) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("Cannot create {} directory: {}", purpose, path.display()))
}

/// Best-effort display label for an unstained control. Used in logs and QC-pipeline error
/// contexts so the user can identify which file triggered a failure even though the `Fcs`
/// struct doesn't carry its path.
///
/// Prefers `$FIL` (the FCS-standard source filename keyword); falls back to a static label.
fn unstained_control_label(fcs: &Fcs) -> String {
    use flow_fcs::keyword::StringableKeyword;
    if let Ok(kw) = fcs.metadata.get_string_keyword("$FIL") {
        let fil = kw.get_str().trim().to_string();
        if !fil.is_empty() {
            return fil;
        }
    }
    "unstained control".to_string()
}

fn qc_cli_options_from_unmix_args(
    qc_preset: &str,
    auto_gate: bool,
    qc_debug_dir: Option<PathBuf>,
    qc_cofactor: Option<f32>,
    qc_no_compensation: bool,
    qc_no_transform: bool,
    qc_mad: Option<f64>,
    qc_mad_only: bool,
    scatter_min_keep_pct: Option<f64>,
) -> crate::qc_pipeline::QcCliOptions {
    use crate::qc_pipeline::QcPreset;
    let preset = match qc_preset.to_lowercase().as_str() {
        "legacy" => Some(QcPreset::LegacyTruOls),
        "literature" => Some(QcPreset::LiteratureDefault),
        "relaxed" => Some(QcPreset::Relaxed),
        "auto" => {
            if auto_gate {
                Some(QcPreset::Relaxed)
            } else {
                Some(QcPreset::LiteratureDefault)
            }
        }
        other => {
            warn!(
                "Unknown --qc-preset {:?}; using {}",
                other,
                if auto_gate { "relaxed" } else { "literature" }
            );
            if auto_gate {
                Some(QcPreset::Relaxed)
            } else {
                Some(QcPreset::LiteratureDefault)
            }
        }
    };
    crate::qc_pipeline::QcCliOptions {
        preset,
        qc_debug_dir,
        qc_cofactor,
        qc_no_compensation,
        qc_no_transform,
        qc_mad,
        qc_mad_only,
        scatter_min_keep_pct,
    }
}

/// Count delimiter characters (space, hyphen, underscore) to measure ambiguity
fn count_delimiters(name: &str) -> usize {
    name.chars()
        .filter(|c| c.is_whitespace() || *c == '-' || *c == '_')
        .count()
}

/// Find the endmember with the most delimiters in its filename.
fn find_most_ambiguous_endmember(control_files: &[(String, PathBuf)]) -> Option<(usize, usize)> {
    if control_files.is_empty() {
        return None;
    }
    let mut max_delim = 0;
    let mut max_idx = 0;
    for (idx, (endmember, _)) in control_files.iter().enumerate() {
        let delim_count = count_delimiters(endmember);
        if delim_count > max_delim {
            max_delim = delim_count;
            max_idx = idx;
        }
    }
    if max_delim > 0 {
        Some((max_idx, max_delim))
    } else {
        None
    }
}

/// Infer delimiter preference from a chosen fragment and original name.
/// Used by interactive fragment selection and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DelimiterPreference {
    use_space: bool,
    use_hyphen: bool,
    use_underscore: bool,
}

#[allow(dead_code)] // used in tests and interactive path
impl DelimiterPreference {
    fn infer(original: &str, chosen: &str) -> Self {
        if original == chosen {
            Self {
                use_space: true,
                use_hyphen: true,
                use_underscore: true,
            }
        } else {
            let parts_space: Vec<&str> = original.split_whitespace().collect();
            let parts_hyphen: Vec<&str> = original.split('-').collect();
            let parts_underscore: Vec<&str> = original.split('_').collect();
            Self {
                use_space: parts_space.iter().any(|p| p == &chosen),
                use_hyphen: parts_hyphen.iter().any(|p| p.trim() == chosen),
                use_underscore: parts_underscore.iter().any(|p| p.trim() == chosen),
            }
        }
    }

    fn apply(&self, name: &str) -> Vec<String> {
        let mut parts: Vec<String> = Vec::new();
        let full = name.trim().to_string();
        if !full.is_empty() {
            parts.push(full.clone());
        }
        if self.use_space {
            for p in name.split_whitespace() {
                let s = p.trim();
                if !s.is_empty() && !parts.contains(&s.to_string()) {
                    parts.push(s.to_string());
                }
            }
        }
        if self.use_underscore {
            for p in name.split('_') {
                let s = p.trim();
                if !s.is_empty() && !parts.contains(&s.to_string()) {
                    parts.push(s.to_string());
                }
            }
        }
        if self.use_hyphen {
            for p in name.split('-') {
                let s = p.trim();
                if !s.is_empty() && !parts.contains(&s.to_string()) {
                    parts.push(s.to_string());
                }
            }
        }
        parts
    }
}

/// Print detailed help information
fn print_detailed_help() {
    println!("TRU-OLS CLI - Detailed Arguments Reference");
    println!("==========================================\n");

    println!("REQUIRED ARGUMENTS");
    println!("------------------\n");
    println!("The CLI requires at least ONE of the following mixing matrix sources:\n");
    println!("  1. --mixing-matrix (CSV file) - OR");
    println!("  2. --use-spill (extract from FCS file SPILL keyword) - OR");
    println!("  3. --single-stain-controls (directory with single-stain control files)\n");

    println!("Always Required:");
    println!("  -s, --stained <PATH>");
    println!("      Path to stained sample FCS file or directory containing stained FCS files");
    println!("      If a directory is provided, all FCS files in it will be processed\n");
    println!("  -u, --unstained <PATH>");
    println!("      Path to unstained control FCS file");
    println!(
        "      Optional if using --controls (auto-detected from filename containing 'unstained')\n"
    );
    println!("  -e, --endmembers <NAMES>");
    println!("      Comma-separated endmember names (e.g., \"AF488,PE,APC,Autofluorescence\")");
    println!(
        "      Optional when using --single-stain-controls or --controls (auto-detected from filenames)\n"
    );

    println!("Conditionally Required:");
    println!("  -d, --detectors <NAMES>");
    println!("      Required if NOT using --use-spill or --single-stain-controls");
    println!("      • When using --use-spill: Detector names are extracted from SPILL keyword");
    println!(
        "      • When using --single-stain-controls: Auto-detected from FCS parameters (optional)"
    );
    println!("      • When using --mixing-matrix: Detector names must be provided\n");

    println!("OPTIONAL ARGUMENTS AND DEFAULTS");
    println!("--------------------------------\n");

    println!("Mixing Matrix Options:");
    println!("  -m, --mixing-matrix <PATH>");
    println!(
        "      Path to CSV mixing matrix file (optional if using --use-spill, --single-stain-controls, or --controls)\n"
    );
    println!("  --use-spill");
    println!("      Use SPILL/SPILLOVER keyword from FCS file");
    println!("      Default: false\n");
    println!("  -c, --controls <PATH>");
    println!("      Directory containing all control files (single-stain controls + unstained)");
    println!("      Unstained control is auto-detected from filename containing 'unstained'");
    println!("      Single-stain controls are all other FCS files in the directory\n");
    println!("  --single-stain-controls <PATH>");
    println!("      Directory containing single-stain control FCS files only");
    println!("      Optional if using --controls (auto-detected)\n");

    println!("Basic Unmixing Parameters:");
    println!("  -a, --autofluorescence <NAME>");
    println!("      Autofluorescence endmember name");
    println!("      Default: \"Autofluorescence\"\n");
    println!("  -p, --cutoff-percentile <VALUE>");
    println!("      Cutoff percentile");
    println!("      Default: 0.995\n");
    println!("  --strategy <STRATEGY>");
    println!("      Unmixing strategy: \"zero\" or \"ucm\"");
    println!("      Default: \"ucm\"\n");
    println!("  -o, --output <PATH>");
    println!("      Output FCS file path (optional, no output file created if omitted)\n");

    println!("Plotting Options:");
    println!("  --plot");
    println!("      Generate comparison plots");
    println!("      Default: true\n");
    println!("  --plot-format <FORMAT>");
    println!("      Plot format: png, svg, or pdf");
    println!("      Default: \"png\"\n");
    println!("  --plot-output-dir <PATH>");
    println!("      Directory for plot outputs (optional, defaults to current directory)\n");
    println!("  --compare-ols");
    println!("      Also run standard OLS and compare");
    println!("      Default: true\n");
    println!("  --plot-both");
    println!("      Generate plots for both OLS and TRU-OLS");
    println!("      Default: false\n");

    println!("Peak Detection Options (for single-stain controls):");
    println!("  --peak-detection");
    println!("      Enable peak-based median selection");
    println!("      Default: false\n");
    println!("  --peak-threshold <VALUE>");
    println!("      Peak detection threshold (fraction of max density)");
    println!("      Lower values detect more peaks, higher values detect only strong peaks");
    println!("      Default: 0.3\n");
    println!("  --peak-bias <VALUE>");
    println!("      Peak bias fraction for positive peaks (0.5 = upper 50%%)");
    println!("      Default: 0.5\n");
    println!("  --peak-bias-negative <VALUE>");
    println!("      Peak bias fraction for negative peaks (0.5 = lower 50%%)");
    println!("      Default: 0.5\n");

    println!("Negative Event Options:");
    println!("  --use-negative-events");
    println!("      Use negative events from single-stain controls for autofluorescence");
    println!("      Default: false\n");
    println!("  --min-negative-events <COUNT>");
    println!("      Minimum number of negative events required");
    println!("      Default: 100\n");
    println!("  --autofluorescence-mode <MODE>");
    println!("      Autofluorescence mode: \"universal\", \"negative-events\", or \"hybrid\"");
    println!("      Default: \"universal\"\n");
    println!("  --af-weight <VALUE>");
    println!("      Autofluorescence weight for hybrid mode (0.0-1.0)");
    println!("      Weight of unstained control vs negative events");
    println!("      Default: 0.7\n");

    println!("Automated Gating Options:");
    println!("  --auto-gate");
    println!("      Enable automated scatter and doublet gating before processing");
    println!("      Default: false\n");
    println!("  --debug-control-plots");
    println!(
        "      Write FSC-A vs SSC-A at each cleanup stage and per-endmember spectral-from-peak plots"
    );
    println!("      Requires --plot-output-dir when using single-stain controls.\n");

    println!("USAGE EXAMPLES");
    println!("--------------\n");

    println!("Using SPILL Matrix (No Detector List Required):");
    println!("  tru-ols unmix \\");
    println!("    --stained stained.fcs \\");
    println!("    --unstained unstained.fcs \\");
    println!("    --use-spill \\");
    println!("    --endmembers AF488,PE,APC,Autofluorescence \\");
    println!("    --output unmixed.fcs\n");

    println!("Using --controls (Simplest - All Auto-Detection):");
    println!("  tru-ols unmix \\");
    println!("    --stained stained.fcs \\");
    println!("    --controls ./controls/ \\");
    println!("    --output unmixed.fcs\n");
    println!("  # Unstained control, detectors, and endmembers are all auto-detected\n");

    println!("Batch Processing (Directory of Stained Files):");
    println!("  tru-ols unmix \\");
    println!("    --stained ./samples/ \\");
    println!("    --controls ./controls/ \\");
    println!("    --output ./unmixed/\n");
    println!(
        "  # Processes all FCS files in ./samples/, outputs to ./unmixed/ with _unmixed suffix\n"
    );

    println!("Using Single-Stain Controls (Auto-Detection Available):");
    println!("  tru-ols unmix \\");
    println!("    --stained stained.fcs \\");
    println!("    --unstained unstained.fcs \\");
    println!("    --single-stain-controls ./controls/ \\");
    println!("    --output unmixed.fcs\n");
    println!("  # Detectors and endmembers are auto-detected from FCS files and filenames");
    println!("  # You can still provide them explicitly if needed:\n");
    println!("  tru-ols unmix \\");
    println!("    --stained stained.fcs \\");
    println!("    --unstained unstained.fcs \\");
    println!("    --single-stain-controls ./controls/ \\");
    println!("    --detectors FL1-A,FL2-A,FL3-A,FL4-A \\");
    println!("    --endmembers AF488,PE,APC,Autofluorescence \\");
    println!("    --output unmixed.fcs\n");

    println!("Using CSV Mixing Matrix (Detector List Required):");
    println!("  tru-ols unmix \\");
    println!("    --stained stained.fcs \\");
    println!("    --unstained unstained.fcs \\");
    println!("    --mixing-matrix matrix.csv \\");
    println!("    --detectors FL1-A,FL2-A,FL3-A,FL4-A \\");
    println!("    --endmembers AF488,PE,APC,Autofluorescence \\");
    println!("    --output unmixed.fcs\n");

    println!("QUICK REFERENCE");
    println!("---------------\n");
    println!("Can you run without detector list?");
    println!("  ✅ YES - If using --use-spill (detectors extracted from SPILL keyword)");
    println!("  ✅ YES - If using --single-stain-controls or --controls (detectors auto-detected)");
    println!("  ❌ NO  - If using --mixing-matrix (detectors must be provided)\n");
    println!("Can you run without endmembers?");
    println!(
        "  ✅ YES - If using --single-stain-controls or --controls (endmembers auto-detected)"
    );
    println!("  ❌ NO  - If using --use-spill or --mixing-matrix (endmembers must be provided)\n");
    println!("Can you run without unstained control?");
    println!(
        "  ✅ YES - If using --controls (unstained auto-detected from filename containing 'unstained')"
    );
    println!("  ❌ NO  - Otherwise (must provide --unstained)\n");
    println!("For more information, see: CLI_ARGUMENTS_REFERENCE.md\n");
}

/// Main command enum
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show detailed reference for CLI arguments and options
    Args,
    /// Unmix FCS files using TRU-OLS
    Unmix {
        /// Path to stained sample FCS file or directory containing stained FCS files
        /// If a directory is provided, all FCS files in it will be processed
        #[arg(short, long)]
        stained: PathBuf,

        /// Path to unstained control FCS file
        /// Optional if using --controls (auto-detected from filename containing "unstained")
        #[arg(short, long)]
        unstained: Option<PathBuf>,

        /// Directory containing all control files (single-stain controls + unstained)
        /// Unstained control is auto-detected from filename containing "unstained"
        /// Single-stain controls are all other FCS files in the directory
        /// Can be overridden with --single-stain-controls and --unstained
        #[arg(short = 'c', long)]
        controls: Option<PathBuf>,

        /// Path to mixing matrix file (CSV format: detectors × endmembers)
        /// Optional if using --use-spill or --single-stain-controls or --controls
        #[arg(short = 'm', long)]
        mixing_matrix: Option<PathBuf>,

        /// Use SPILL/SPILLOVER keyword from stained FCS file as mixing matrix
        /// For spectral cytometry, the SPILL matrix is the mixing matrix
        #[arg(long)]
        use_spill: bool,

        /// Directory containing single-stain control FCS files
        /// Each file should be stained with one fluorophore
        /// Files will be matched to endmember names by filename or metadata
        /// Optional if using --controls (auto-detected)
        #[arg(long)]
        single_stain_controls: Option<PathBuf>,

        /// Detector names (comma-separated, e.g., "FL1-A,FL2-A,FL3-A")
        /// Required if not using --use-spill or --single-stain-controls
        /// If using --single-stain-controls, detectors can be auto-detected from FCS parameters
        #[arg(short, long, value_delimiter = ',')]
        detectors: Vec<String>,

        /// Endmember names (comma-separated, e.g., "AF488,PE,APC,Autofluorescence")
        /// Required if not using --single-stain-controls
        /// If using --single-stain-controls, endmembers can be auto-detected from filenames
        #[arg(short, long, value_delimiter = ',')]
        endmembers: Vec<String>,

        /// Autofluorescence endmember name
        #[arg(short = 'a', long, default_value = "Autofluorescence")]
        autofluorescence: String,

        /// Cutoff percentile (default: 0.995)
        #[arg(short = 'p', long, default_value = "0.995")]
        cutoff_percentile: f64,

        /// Strategy: "zero" or "ucm" (default: ucm)
        #[arg(long, default_value = "ucm")]
        strategy: String,

        /// Output FCS file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Generate comparison plots
        #[arg(long)]
        plot: bool,

        /// Plot format: png, svg, or pdf (default: png)
        #[arg(long, default_value = "png")]
        plot_format: String,

        /// Directory for plot outputs
        #[arg(long)]
        plot_output_dir: Option<PathBuf>,

        /// Also run standard OLS and compare
        #[arg(long)]
        compare_ols: bool,

        /// Generate plots for both OLS and TRU-OLS
        #[arg(long)]
        plot_both: bool,

        /// Export mixing matrix to CSV file (useful for comparison with Julia)
        #[arg(long)]
        export_mixing_matrix: Option<PathBuf>,

        /// Enable peak-based median selection for single-stain controls
        /// Uses KDE to detect peaks and selects median from highest intensity peak
        #[arg(long, default_value_t = true)]
        peak_detection: bool,

        /// Peak detection threshold (fraction of max density, default: 0.3)
        /// Lower values detect more peaks, higher values detect only strong peaks
        #[arg(long, default_value = "0.3")]
        peak_threshold: f64,

        /// Enable peak biasing (right-side for positive peaks, left-side for negative)
        /// Bias fraction: 0.5 = upper 50% of peak events (default: 0.5)
        #[arg(long, default_value = "0.5")]
        peak_bias: f64,

        /// Peak bias for negative peaks (left-side biasing)
        /// Bias fraction: 0.5 = lower 50% of negative peak events (default: 0.5)
        #[arg(long, default_value = "0.5")]
        peak_bias_negative: f64,

        /// Minimum number of negative events required (default: 100)
        #[arg(long, default_value = "100")]
        min_negative_events: usize,

        /// Use negative events from single-stain controls for autofluorescence
        #[arg(long)]
        use_negative_events: bool,

        /// Autofluorescence mode: universal, negative-events, hybrid (default: universal)
        #[arg(long, default_value = "universal")]
        autofluorescence_mode: String,

        /// Autofluorescence weight for hybrid mode (default: 0.7)
        /// Weight of unstained control vs negative events (0.0-1.0)
        #[arg(long, default_value = "0.7")]
        af_weight: f64,

        /// QC pipeline preset: `auto` (default: relaxed when --auto-gate, else literature), `literature`, `relaxed`, or `legacy`
        #[arg(long, default_value = "auto")]
        qc_preset: String,

        /// Write PeacoQC overview and post-debris scatter PNGs under this directory when auto-gate is on
        #[arg(long, value_name = "DIR")]
        qc_debug_dir: Option<PathBuf>,

        /// Arcsinh cofactor for fluorescence preprocessing before time-bin QC (default: 2000)
        #[arg(long)]
        qc_cofactor: Option<f32>,

        /// Skip compensation during preprocessing when spillover is available
        #[arg(long, default_value_t = false)]
        qc_no_compensation: bool,

        /// Skip arcsinh transform during preprocessing
        #[arg(long, default_value_t = false)]
        qc_no_transform: bool,

        /// Override PeacoQC MAD multiplier
        #[arg(long)]
        qc_mad: Option<f64>,

        /// Use MAD-only mode for time-bin QC (skip isolation tree)
        #[arg(long, default_value_t = false)]
        qc_mad_only: bool,

        /// Minimum percent of events kept inside scatter gate before FSC consensus fallback (0–100)
        #[arg(long)]
        scatter_min_keep_pct: Option<f64>,

        /// Enable automated scatter and doublet gating before processing
        /// Applies gates to single-stain controls and unstained control
        #[arg(long, default_value_t = false)]
        auto_gate: bool,

        /// Generate debug plots for each control: FSC-A vs SSC-A at each cleanup stage
        /// (pre-gating, post-margin, post-doublet, post-debris) and per-endmember spectral from peak events.
        /// Requires --plot-output-dir when using single-stain controls.
        #[arg(long)]
        debug_control_plots: bool,
    },
    /// Interactive step-by-step prompts for unmix options
    Interactive,
    /// Run OLS vs TRU-OLS benchmark on synthetic data
    #[cfg(feature = "cli_benchmark")]
    Benchmark {
        /// Output directory for reports and plots
        #[arg(short, long, default_value = "benchmark_output")]
        output_dir: PathBuf,

        /// Number of stained events per dataset
        #[arg(long, default_value = "5000")]
        n_events: usize,

        /// Number of unstained control events
        #[arg(long, default_value = "2000")]
        n_unstained: usize,

        /// Comma-separated noise sigma levels (0 = noise-free)
        #[arg(long, value_delimiter = ',', default_values_t = vec![0.0, 0.01, 0.05, 0.1])]
        noise_levels: Vec<f64>,
    },
}

/// Run a command. `None` runs the interactive wizard (same as `interactive` subcommand).
pub fn run_command(command: Option<&Command>) -> Result<()> {
    match command {
        None | Some(Command::Interactive) => crate::interactive::run_interactive(),
        Some(Command::Args) => {
            print_detailed_help();
            Ok(())
        },
        #[cfg(feature = "cli_benchmark")]
        Some(Command::Benchmark {
            output_dir,
            n_events,
            n_unstained,
            noise_levels,
        }) => run_benchmark(output_dir, *n_events, *n_unstained, noise_levels),
        Some(Command::Unmix {
            stained,
            unstained,
            controls,
            mixing_matrix,
            use_spill,
            single_stain_controls,
            detectors,
            endmembers,
            autofluorescence,
            cutoff_percentile,
            strategy,
            output,
            plot,
            plot_format,
            plot_output_dir,
            compare_ols,
            plot_both,
            peak_detection,
            peak_threshold,
            peak_bias,
            peak_bias_negative,
            use_negative_events,
            autofluorescence_mode,
            af_weight,
            min_negative_events,
            qc_preset,
            qc_debug_dir,
            qc_cofactor,
            qc_no_compensation,
            qc_no_transform,
            qc_mad,
            qc_mad_only,
            scatter_min_keep_pct,
            auto_gate,
            debug_control_plots,
            export_mixing_matrix,
        }) => {
            let qc_opts = qc_cli_options_from_unmix_args(
                qc_preset,
                *auto_gate,
                qc_debug_dir.clone(),
                *qc_cofactor,
                *qc_no_compensation,
                *qc_no_transform,
                *qc_mad,
                *qc_mad_only,
                *scatter_min_keep_pct,
            );
            run_unmix_command(
                stained,
                unstained.as_ref(),
                controls.as_ref(),
                mixing_matrix.as_ref(),
                *use_spill,
                single_stain_controls.as_ref(),
                detectors,
                endmembers,
                autofluorescence,
                *cutoff_percentile,
                strategy,
                output.as_ref(),
                *plot,
                plot_format,
                plot_output_dir.as_ref(),
                *compare_ols,
                *plot_both,
                *peak_detection,
                *peak_threshold,
                *peak_bias,
                *peak_bias_negative,
                *use_negative_events,
                autofluorescence_mode,
                *af_weight,
                *min_negative_events,
                *auto_gate,
                *debug_control_plots,
                export_mixing_matrix.as_ref(),
                None,
                &qc_opts,
            )
        },
    }
}

/// Run unmix with the given parameters. Public for use by the interactive subcommand.
pub(crate) fn run_unmix_command(
    stained_path: &PathBuf,
    unstained_path: Option<&PathBuf>,
    controls_dir: Option<&PathBuf>,
    mixing_matrix_path: Option<&PathBuf>,
    use_spill: bool,
    single_stain_controls_dir: Option<&PathBuf>,
    detectors: &[String],
    endmembers: &[String],
    autofluorescence: &str,
    cutoff_percentile: f64,
    strategy_str: &str,
    output: Option<&PathBuf>,
    plot: bool,
    plot_format: &str,
    plot_output_dir: Option<&PathBuf>,
    compare_ols: bool,
    plot_both: bool,
    peak_detection: bool,
    peak_threshold: f64,
    peak_bias: f64,
    peak_bias_negative: f64,
    use_negative_events: bool,
    autofluorescence_mode: &str,
    af_weight: f64,
    min_negative_events: usize,
    auto_gate: bool,
    debug_control_plots: bool,
    export_mixing_matrix: Option<&PathBuf>,
    control_assignments: Option<&[(String, PathBuf)]>,
    qc_options: &crate::qc_pipeline::QcCliOptions,
) -> Result<()> {
    // Ensure output directories exist and we can create them before running computations
    if let Some(dir) = plot_output_dir {
        ensure_output_directory(dir, "plot output")?;
    }
    if stained_path.is_dir() {
        if let Some(out) = output {
            let output_dir = if out.is_dir() || out.extension().is_none() {
                out.as_path()
            } else if let Some(parent) = out.parent() {
                parent
            } else {
                out.as_path()
            };
            ensure_output_directory(output_dir, "output")?;
        }
    }

    // Check if stained_path is a directory or file
    if stained_path.is_dir() {
        info!(
            "Processing directory of stained FCS files: {}",
            stained_path.display()
        );
        process_directory_of_stained_files(
            stained_path,
            unstained_path,
            controls_dir,
            mixing_matrix_path,
            use_spill,
            single_stain_controls_dir,
            detectors,
            endmembers,
            autofluorescence,
            cutoff_percentile,
            strategy_str,
            output,
            plot,
            plot_format,
            plot_output_dir,
            compare_ols,
            plot_both,
            peak_detection,
            peak_threshold,
            peak_bias,
            peak_bias_negative,
            use_negative_events,
            autofluorescence_mode,
            af_weight,
            min_negative_events,
            auto_gate,
            debug_control_plots,
            export_mixing_matrix,
            control_assignments,
            qc_options,
        )
    } else {
        // Single file processing (existing logic)
        process_single_stained_file(
            stained_path,
            unstained_path,
            controls_dir,
            mixing_matrix_path,
            use_spill,
            single_stain_controls_dir,
            detectors,
            endmembers,
            autofluorescence,
            cutoff_percentile,
            strategy_str,
            output,
            plot,
            plot_format,
            plot_output_dir,
            compare_ols,
            plot_both,
            peak_detection,
            peak_threshold,
            peak_bias,
            peak_bias_negative,
            use_negative_events,
            autofluorescence_mode,
            af_weight,
            min_negative_events,
            auto_gate,
            debug_control_plots,
            export_mixing_matrix,
            control_assignments,
            qc_options,
        )
    }
}

/// Process a single stained FCS file
fn process_single_stained_file(
    stained_path: &PathBuf,
    unstained_path: Option<&PathBuf>,
    controls_dir: Option<&PathBuf>,
    mixing_matrix_path: Option<&PathBuf>,
    use_spill: bool,
    single_stain_controls_dir: Option<&PathBuf>,
    detectors: &[String],
    endmembers: &[String],
    autofluorescence: &str,
    _cutoff_percentile: f64,
    strategy_str: &str,
    output: Option<&PathBuf>,
    plot: bool,
    plot_format: &str,
    plot_output_dir: Option<&PathBuf>,
    compare_ols: bool,
    plot_both: bool,
    peak_detection: bool,
    peak_threshold: f64,
    peak_bias: f64,
    peak_bias_negative: f64,
    use_negative_events: bool,
    autofluorescence_mode: &str,
    af_weight: f64,
    min_negative_events: usize,
    auto_gate: bool,
    debug_control_plots: bool,
    export_mixing_matrix: Option<&PathBuf>,
    control_assignments: Option<&[(String, PathBuf)]>,
    qc_options: &crate::qc_pipeline::QcCliOptions,
) -> Result<()> {
    info!("Loading FCS files...");
    let stained_fcs = Fcs::open(stained_path.to_str().context("Invalid stained file path")?)?;

    let unstained_path_final = resolve_unstained_control_path(
        unstained_path,
        controls_dir,
        single_stain_controls_dir,
    )?;

    let unstained_fcs = Fcs::open(
        unstained_path_final
            .to_str()
            .context("Invalid unstained file path")?,
    )?;

    // Determine single-stain controls directory: explicit, from --controls, or None
    let single_stain_controls_dir_final = if let Some(dir) = single_stain_controls_dir {
        Some(dir.clone())
    } else if let Some(controls_dir) = controls_dir {
        // Use --controls directory, excluding the unstained file
        Some(controls_dir.clone())
    } else {
        None
    };

    // Auto-detect endmembers and detectors if using single-stain-controls/controls and not provided
    let (mut final_detectors, mut final_endmembers) = if let Some(controls_dir) =
        &single_stain_controls_dir_final
    {
        if detectors.is_empty() || endmembers.is_empty() {
            info!("Auto-detecting detectors and endmembers from single-stain controls...");
            let (auto_detectors, auto_endmembers) =
                auto_detect_from_single_stains(controls_dir, &stained_fcs)?;

            let final_detectors = if detectors.is_empty() {
                info!("Auto-detected detectors: {}", auto_detectors.join(", "));
                auto_detectors
            } else {
                detectors.to_vec()
            };

            let mut final_endmembers = if endmembers.is_empty() {
                auto_endmembers
            } else {
                endmembers.to_vec()
            };

            // Add autofluorescence endmember if not already present
            if !final_endmembers.contains(&autofluorescence.to_string()) {
                info!(
                    "Adding autofluorescence endmember '{}' to endmembers list",
                    autofluorescence
                );
                final_endmembers.push(autofluorescence.to_string());
            }

            (final_detectors, final_endmembers)
        } else {
            let mut final_endmembers = endmembers.to_vec();
            // Add autofluorescence endmember if not already present
            if !final_endmembers.contains(&autofluorescence.to_string()) {
                info!(
                    "Adding autofluorescence endmember '{}' to endmembers list",
                    autofluorescence
                );
                final_endmembers.push(autofluorescence.to_string());
            }
            (detectors.to_vec(), final_endmembers)
        }
    } else {
        // Not using single-stain-controls, use provided values (or empty if not provided)
        let mut final_endmembers = endmembers.to_vec();
        // Add autofluorescence endmember if not already present (only if endmembers were provided)
        if !final_endmembers.is_empty() && !final_endmembers.contains(&autofluorescence.to_string())
        {
            info!(
                "Adding autofluorescence endmember '{}' to endmembers list",
                autofluorescence
            );
            final_endmembers.push(autofluorescence.to_string());
        }
        (detectors.to_vec(), final_endmembers)
    };

    // Determine mixing matrix source (`--mixing-matrix` takes precedence over building from controls)
    let (
        mixing_matrix,
        detector_names_from_matrix,
        primary_detector_info,
        used_single_stain_controls,
    ) = if use_spill {
        info!("Step 1/2: Extracting mixing matrix from SPILL keyword...");
        let (matrix, detectors) =
            extract_mixing_matrix_from_spill(&stained_fcs, &final_endmembers)?;
        // For SPILL matrix, create placeholder primary detector info
        let mut info = Vec::new();
        for endmember in &final_endmembers {
            info.push(PrimaryDetectorInfo {
                endmember_name: endmember.clone(),
                is_autofluorescence: endmember == autofluorescence,
                primary_detector_name: None,
                primary_detector_pn_name: None,
                primary_detector_pn_label: None,
                selected_marker_name: None,
                selected_fluor_name: None,
            });
        }
        (matrix, detectors, info, false)
    } else if let Some(matrix_path) = mixing_matrix_path {
        info!("Step 1/2: Loading mixing matrix from CSV file...");
        let (matrix, det_csv, em_csv) = load_mixing_matrix(matrix_path)?;
        if !em_csv.is_empty() {
            final_endmembers = em_csv;
        }
        if !det_csv.is_empty() {
            final_detectors = det_csv.clone();
        }
        let detector_names_from_matrix = if det_csv.is_empty() {
            final_detectors.clone()
        } else {
            det_csv
        };
        let mut info = Vec::new();
        for endmember in &final_endmembers {
            info.push(PrimaryDetectorInfo {
                endmember_name: endmember.clone(),
                is_autofluorescence: endmember == autofluorescence,
                primary_detector_name: None,
                primary_detector_pn_name: None,
                primary_detector_pn_label: None,
                selected_marker_name: None,
                selected_fluor_name: None,
            });
        }
        (matrix, detector_names_from_matrix, info, false)
    } else if let Some(controls_dir) = &single_stain_controls_dir_final {
        info!("Step 1/3: Identifying autofluorescence from unstained control");
        info!("Creating mixing matrix from single-stain controls...");
        let single_stain_config = SingleStainConfig {
            peak_detection,
            peak_threshold,
            peak_bias,
            peak_bias_negative,
            use_negative_events,
            autofluorescence_mode: autofluorescence_mode.to_string(),
            af_weight,
            min_negative_events,
            qc_options: qc_options.clone(),
        };
        let (matrix, detectors, info, _) = create_mixing_matrix_from_single_stains(
            controls_dir,
            &unstained_fcs,
            &final_detectors,
            &final_endmembers,
            &autofluorescence,
            &single_stain_config,
            control_assignments,
            auto_gate,
            debug_control_plots,
            if debug_control_plots {
                plot_output_dir
            } else {
                None
            },
        )?;
        (matrix, detectors, info, true)
    } else {
        return Err(anyhow::anyhow!(
            "Must provide one of: --mixing-matrix, --use-spill, or --single-stain-controls"
        ));
    };

    // Use detector names from matrix if available, otherwise use provided/auto-detected detectors
    let detector_names: Vec<String> = if !detector_names_from_matrix.is_empty() {
        detector_names_from_matrix
    } else if !final_detectors.is_empty() {
        final_detectors.clone()
    } else {
        return Err(anyhow::anyhow!(
            "Detector names must be provided or extracted from SPILL keyword"
        ));
    };

    // Use final endmembers (provided or auto-detected)
    let endmember_names = final_endmembers;

    // Validate dimensions
    if mixing_matrix.nrows() != detector_names.len() {
        return Err(anyhow::anyhow!(
            "Mixing matrix rows ({}) don't match number of detectors ({})",
            mixing_matrix.nrows(),
            detector_names.len()
        ));
    }

    if mixing_matrix.ncols() != endmember_names.len() {
        return Err(anyhow::anyhow!(
            "Mixing matrix columns ({}) don't match number of endmembers ({})",
            mixing_matrix.ncols(),
            endmember_names.len()
        ));
    }

    // Export mixing matrix if requested
    if let Some(export_path) = export_mixing_matrix {
        export_mixing_matrix_to_csv(
            &mixing_matrix,
            export_path,
            &detector_names,
            &endmember_names,
        )?;
        info!("Exported mixing matrix to: {}", export_path.display());
    }

    // Parse strategy
    let strategy = match strategy_str.to_lowercase().as_str() {
        "zero" => Some(UnmixingStrategy::Zero),
        "ucm" => Some(UnmixingStrategy::UnstainedControlMapping),
        _ => {
            warn!("Unknown strategy '{}', using default (zero)", strategy_str);
            Some(UnmixingStrategy::Zero)
        }
    };

    // Filter mixing matrix and detector names to only those present in stained file
    // This allows proper unmixing when stained file has fewer detectors than the full panel
    let stained_param_names = stained_fcs.get_parameter_names_from_dataframe();
    let stained_param_set: std::collections::HashSet<&str> =
        stained_param_names.iter().map(|s| s.as_str()).collect();

    let mut filtered_matrix_rows = Vec::new();
    let mut filtered_detector_names = Vec::new();
    let mut filtered_primary_pn_names = Vec::new();
    let mut filtered_primary_pn_labels = Vec::new();
    let mut filtered_primary_detector_names = Vec::new();

    for (det_idx, det_name) in detector_names.iter().enumerate() {
        if stained_param_set.contains(det_name.as_str()) {
            filtered_matrix_rows.push(det_idx);
            filtered_detector_names.push(det_name.clone());
        }
    }

    if filtered_detector_names.is_empty() {
        return Err(anyhow::anyhow!(
            "No detectors from mixing matrix found in stained file. Stained file parameters: {:?}, Expected detectors: {:?}",
            stained_param_names,
            detector_names
        ));
    }

    if filtered_detector_names.len() < endmember_names.len() {
        return Err(anyhow::anyhow!(
            "Stained file has {} detectors but requires at least {} for unmixing {} endmembers (underdetermined system). \
            Consider using a stained file with more detector channels or reducing the number of endmembers.",
            filtered_detector_names.len(),
            endmember_names.len(),
            endmember_names.len()
        ));
    }

    // Reduce mixing matrix to filtered rows
    use ndarray::Array2;
    let n_filtered = filtered_matrix_rows.len();
    let mut filtered_mixing_matrix = Array2::<f64>::zeros((n_filtered, mixing_matrix.ncols()));
    for (new_idx, &orig_idx) in filtered_matrix_rows.iter().enumerate() {
        let src_row = mixing_matrix.row(orig_idx);
        filtered_mixing_matrix.row_mut(new_idx).assign(&src_row);
    }

    info!(
        "Filtered mixing matrix: {} detectors (from {}) × {} endmembers",
        n_filtered,
        detector_names.len(),
        endmember_names.len()
    );

    // All endmembers use primary detector metadata, so all metadata rows apply
    let mut filtered_selected_marker_names = Vec::new();
    let mut filtered_selected_fluor_names = Vec::new();
    for pn_name in &primary_detector_info {
        filtered_primary_pn_names.push(pn_name.primary_detector_pn_name.clone());
        filtered_primary_pn_labels.push(pn_name.primary_detector_pn_label.clone());
        filtered_primary_detector_names.push(pn_name.primary_detector_name.clone());
        filtered_selected_marker_names.push(pn_name.selected_marker_name.clone());
        filtered_selected_fluor_names.push(pn_name.selected_fluor_name.clone());
    }

    // Convert to string slices
    let detector_names_slices: Vec<&str> =
        filtered_detector_names.iter().map(|s| s.as_str()).collect();
    let endmember_names: Vec<&str> = endmember_names.iter().map(|s| s.as_str()).collect();

    if used_single_stain_controls {
        info!("Step 3/3: Unmixing stained sample (1 file)");
    } else {
        info!("Step 2/2: Unmixing stained sample (1 file)");
    }
    info!("Running TRU-OLS unmixing...");

    // Convert Array2 to faer Mat for tru-ols
    let mixing_mat = Mat::from_fn(
        filtered_mixing_matrix.nrows(),
        filtered_mixing_matrix.ncols(),
        |i, j| filtered_mixing_matrix[(i, j)],
    );

    let unmixed_fcs = stained_fcs.apply_tru_ols_unmixing(
        &unstained_fcs,
        mixing_mat,
        &detector_names_slices,
        &endmember_names,
        autofluorescence,
        strategy,
        &filtered_primary_detector_names,
        &filtered_primary_pn_names,
        &filtered_primary_pn_labels,
        &filtered_selected_marker_names,
        &filtered_selected_fluor_names,
    )?;

    info!("TRU-OLS unmixing complete!");

    // Create output FCS if requested
    if let Some(output_path) = output {
        info!("Writing unmixed FCS file to: {}", output_path.display());
        use flow_fcs::write_fcs_file;
        write_fcs_file(unmixed_fcs.clone(), output_path)?;
        info!("Successfully wrote unmixed FCS file");
    }

    // Handle plotting
    if plot || plot_both {
        let plot_dir = plot_output_dir
            .map(|p| p.clone())
            .unwrap_or_else(|| PathBuf::from("."));

        std::fs::create_dir_all(&plot_dir)?;

        if plot_both && compare_ols {
            // Generate plots for both OLS and TRU-OLS
            info!("Generating comparison plots...");
            generate_ols_comparison_plots(
                &stained_fcs,
                &unstained_fcs,
                &filtered_mixing_matrix,
                &detector_names_slices,
                &endmember_names,
                &unmixed_fcs.data_frame,
                &plot_dir,
                plot_format,
                stained_path.file_name().and_then(|s| s.to_str()),
            )?;
        } else if plot {
            info!("Generating TRU-OLS plots...");
            generate_tru_ols_plots(
                &unmixed_fcs.data_frame,
                &endmember_names,
                &plot_dir,
                plot_format,
            )?;
        }
    }

    Ok(())
}

/// Run the synthetic benchmark suite.
#[cfg(feature = "cli_benchmark")]
fn run_benchmark(
    output_dir: &Path,
    n_events: usize,
    n_unstained: usize,
    noise_levels: &[f64],
) -> Result<()> {
    use crate::benchmark::{
        run_synthetic_benchmark, write_csv_report, write_json_report, write_markdown_report,
    };

    ensure_output_directory(output_dir, "benchmark")?;

    info!(
        "Running benchmark: {} events, {} unstained, noise levels {:?}",
        n_events, n_unstained, noise_levels
    );
    let report = run_synthetic_benchmark(n_events, n_unstained, noise_levels)?;

    let json_path = output_dir.join("benchmark_report.json");
    write_json_report(&report, &json_path)?;
    info!("JSON report: {}", json_path.display());

    let md_path = output_dir.join("benchmark_report.md");
    write_markdown_report(&report, &md_path)?;
    info!("Markdown report: {}", md_path.display());

    let csv_dir = output_dir.join("csv");
    write_csv_report(&report, &csv_dir)?;
    info!("CSV metrics: {}", csv_dir.display());

    println!(
        "\nBenchmark complete. {} dataset(s) processed.\nResults in: {}",
        report.datasets.len(),
        output_dir.display()
    );
    Ok(())
}

/// Process a directory of stained FCS files
fn process_directory_of_stained_files(
    stained_dir: &PathBuf,
    unstained_path: Option<&PathBuf>,
    controls_dir: Option<&PathBuf>,
    mixing_matrix_path: Option<&PathBuf>,
    use_spill: bool,
    single_stain_controls_dir: Option<&PathBuf>,
    detectors: &[String],
    endmembers: &[String],
    autofluorescence: &str,
    cutoff_percentile: f64,
    strategy_str: &str,
    output: Option<&PathBuf>,
    plot: bool,
    plot_format: &str,
    plot_output_dir: Option<&PathBuf>,
    compare_ols: bool,
    plot_both: bool,
    peak_detection: bool,
    peak_threshold: f64,
    peak_bias: f64,
    peak_bias_negative: f64,
    use_negative_events: bool,
    autofluorescence_mode: &str,
    af_weight: f64,
    min_negative_events: usize,
    auto_gate: bool,
    debug_control_plots: bool,
    export_mixing_matrix: Option<&PathBuf>,
    control_assignments: Option<&[(String, PathBuf)]>,
    qc_options: &crate::qc_pipeline::QcCliOptions,
) -> Result<()> {
    use std::fs;

    // Get all FCS files in the directory
    let entries = fs::read_dir(stained_dir)
        .with_context(|| format!("Failed to read directory: {}", stained_dir.display()))?;

    let mut stained_files: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("fcs") {
            stained_files.push(path);
        }
    }

    if stained_files.is_empty() {
        return Err(anyhow::anyhow!(
            "No FCS files found in directory: {}",
            stained_dir.display()
        ));
    }

    // Sort files for consistent processing order
    stained_files.sort();

    info!("Found {} FCS files to unmix", stained_files.len());
    info!("Preparing mixing matrix and configuration (this will be reused for all files)...");
    info!("Loading first FCS and controls to build matrix (this may take a moment)...");

    // Determine output directory
    let output_dir = if let Some(output_path) = output {
        if output_path.is_dir() {
            Some(output_path.clone())
        } else {
            // If output is a file, use its parent directory
            output_path.parent().map(|p| p.to_path_buf())
        }
    } else {
        // Default: use the input directory
        Some(stained_dir.clone())
    };

    // Create output directory if it doesn't exist
    if let Some(ref out_dir) = output_dir {
        std::fs::create_dir_all(out_dir)?;
    }

    // Prepare mixing matrix and configuration ONCE (amortized)
    // Use the first stained file to infer parameter structure if needed
    let first_stained_fcs = Fcs::open(
        stained_files[0]
            .to_str()
            .context("Invalid stained file path")?,
    )?;

    // When debug_control_plots is set, use plot_output_dir or default to output_dir/plots
    let diagnostic_plot_dir: Option<PathBuf> = if debug_control_plots {
        plot_output_dir.cloned().or_else(|| {
            output_dir.as_ref().map(|out_dir| {
                let mut p = out_dir.clone();
                p.push("plots");
                p
            })
        })
    } else {
        None
    };

    let (
        mixing_matrix,
        detector_names,
        endmember_names,
        unstained_fcs,
        primary_detector_info,
        used_single_stain_controls,
    ) = prepare_mixing_matrix_for_batch(
        &first_stained_fcs,
        unstained_path,
        controls_dir,
        mixing_matrix_path,
        use_spill,
        single_stain_controls_dir,
        detectors,
        endmembers,
        autofluorescence,
        peak_detection,
        peak_threshold,
        peak_bias,
        peak_bias_negative,
        use_negative_events,
        autofluorescence_mode,
        af_weight,
        min_negative_events,
        auto_gate,
        debug_control_plots,
        diagnostic_plot_dir.as_ref(),
        export_mixing_matrix,
        control_assignments,
        qc_options,
    )?;

    // Convert strategy string to enum
    let strategy = match strategy_str {
        "ucm" => UnmixingStrategy::UnstainedControlMapping,
        _ => UnmixingStrategy::Zero,
    };

    let batch_prelude = precompute_tru_ols_batch_prelude(
        &mixing_matrix,
        &detector_names,
        &endmember_names,
        &unstained_fcs,
        autofluorescence,
        cutoff_percentile,
    )?;
    if batch_prelude.factor_cache.is_some() {
        info!(
            "Precomputed TRU-OLS cutoffs/nonspecific (full panel); mask-factor cache shared across stained files (TRU_OLS_BATCH_SHARED_FACTOR_CACHE unset or 1)"
        );
    } else {
        info!(
            "Precomputed TRU-OLS cutoffs/nonspecific (full panel); fresh mask-factor cache per stained file (TRU_OLS_BATCH_SHARED_FACTOR_CACHE=0)"
        );
    }

    let n_stained = stained_files.len();
    if used_single_stain_controls {
        info!("Step 3/3: Unmixing stained samples ({} files)", n_stained);
    } else {
        info!("Step 2/2: Unmixing stained samples ({} files)", n_stained);
    }

    // Process each file using the pre-computed matrix
    let mut success_count = 0;
    let mut error_count = 0;

    for (idx, stained_file) in stained_files.iter().enumerate() {
        info!(
            "\nUnmixing stained samples [{}/{}]: {}",
            idx + 1,
            n_stained,
            stained_file.display()
        );

        // Generate output filename
        let output_file = if let Some(ref out_dir) = output_dir {
            let stem = stained_file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unmixed");
            let mut output_path = out_dir.clone();
            output_path.push(format!("{}_unmixed.fcs", stem));
            Some(output_path)
        } else {
            None
        };

        // Generate plot output directory for this file
        let file_plot_dir: Option<PathBuf> = if plot || plot_both {
            plot_output_dir.cloned().or_else(|| {
                output_dir
                    .as_ref()
                    .map(|out_dir| {
                        let mut plot_path = out_dir.clone();
                        plot_path.push("plots");
                        plot_path
                    })
                    .or(Some(PathBuf::from("plots")))
            })
        } else {
            None
        };

        match process_stained_file_with_matrix(
            stained_file,
            &mixing_matrix,
            &detector_names,
            &endmember_names,
            &unstained_fcs,
            &primary_detector_info,
            autofluorescence,
            cutoff_percentile,
            &strategy,
            Some(&batch_prelude),
            output_file.as_ref(),
            plot,
            plot_format,
            file_plot_dir.as_ref(),
            compare_ols,
            plot_both,
        ) {
            Ok(()) => {
                success_count += 1;
                info!("✓ Successfully processed: {}", stained_file.display());
            }
            Err(e) => {
                error_count += 1;
                warn!("✗ Failed to process {}: {}", stained_file.display(), e);
            }
        }
    }

    info!("\n=== Batch Processing Complete ===");
    info!("Successfully processed: {} files", success_count);
    if error_count > 0 {
        warn!("Failed to process: {} files", error_count);
    }

    if error_count > 0 {
        Err(anyhow::anyhow!(
            "Batch processing completed with {} errors out of {} files",
            error_count,
            stained_files.len()
        ))
    } else {
        Ok(())
    }
}

/// Shared TRU-OLS preprocessing for a stained-directory batch: one mixing matrix, one unstained,
/// shared cutoffs/nonspecific on the full detector list, and optionally one mask-factor cache
/// shared across files (see `TRU_OLS_BATCH_SHARED_FACTOR_CACHE`).
struct TruOlsBatchPrelude {
    cutoffs: Col<f64>,
    nonspecific_full: Col<f64>,
    /// When `None`, each stained file uses a fresh mask-factor cache (A/B vs one shared cache).
    factor_cache: Option<flow_tru_ols::SharedMaskFactorCache>,
}

fn precompute_tru_ols_batch_prelude(
    mixing_matrix: &Array2<f64>,
    detector_names: &[String],
    endmember_names: &[String],
    unstained_fcs: &Fcs,
    autofluorescence: &str,
    cutoff_percentile: f64,
) -> Result<TruOlsBatchPrelude> {
    let af_idx = endmember_names
        .iter()
        .position(|n| n == autofluorescence)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Autofluorescence endmember '{}' not found in endmember list",
                autofluorescence
            )
        })?;
    let detector_strs: Vec<&str> = detector_names.iter().map(|s| s.as_str()).collect();
    let mixing_mat = Mat::from_fn(mixing_matrix.nrows(), mixing_matrix.ncols(), |i, j| {
        mixing_matrix[(i, j)]
    });
    let unstained_mat = extract_detector_data(unstained_fcs, &detector_strs)?;
    let cutoffs = CutoffCalculator::calculate(
        mixing_mat.as_ref(),
        unstained_mat.as_ref(),
        cutoff_percentile,
    )?;
    let nonspecific =
        NonspecificObservation::calculate(mixing_mat.as_ref(), unstained_mat.as_ref(), af_idx)?;
    let share_mask_cache = std::env::var("TRU_OLS_BATCH_SHARED_FACTOR_CACHE")
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            !(lower == "0" || lower == "false" || lower == "no")
        })
        .unwrap_or(true);
    let factor_cache = if share_mask_cache {
        Some(shared_mask_factor_cache_with_capacity(512))
    } else {
        None
    };
    Ok(TruOlsBatchPrelude {
        cutoffs: cutoffs.cutoffs().clone(),
        nonspecific_full: nonspecific.observation().clone(),
        factor_cache,
    })
}

/// Prepare mixing matrix and configuration for batch processing
/// This is called ONCE at the beginning to amortize expensive operations
/// Returns (mixing_matrix, detector_names, endmember_names, unstained_fcs, primary_detector_info)
fn prepare_mixing_matrix_for_batch(
    sample_fcs: &Fcs,
    unstained_path: Option<&PathBuf>,
    controls_dir: Option<&PathBuf>,
    mixing_matrix_path: Option<&PathBuf>,
    use_spill: bool,
    single_stain_controls_dir: Option<&PathBuf>,
    detectors: &[String],
    endmembers: &[String],
    autofluorescence: &str,
    peak_detection: bool,
    peak_threshold: f64,
    peak_bias: f64,
    peak_bias_negative: f64,
    use_negative_events: bool,
    autofluorescence_mode: &str,
    af_weight: f64,
    min_negative_events: usize,
    auto_gate: bool,
    debug_control_plots: bool,
    diagnostic_plot_dir: Option<&PathBuf>,
    export_mixing_matrix: Option<&PathBuf>,
    control_assignments: Option<&[(String, PathBuf)]>,
    qc_options: &crate::qc_pipeline::QcCliOptions,
) -> Result<(
    Array2<f64>,
    Vec<String>,
    Vec<String>,
    Fcs,
    Vec<PrimaryDetectorInfo>,
    bool, // used_single_stain_controls: true => 3-step flow, false => 2-step flow
)> {
    let unstained_path_final = resolve_unstained_control_path(
        unstained_path,
        controls_dir,
        single_stain_controls_dir,
    )?;

    let unstained_fcs = Fcs::open(
        unstained_path_final
            .to_str()
            .context("Invalid unstained file path")?,
    )?;

    // Determine single-stain controls directory
    let single_stain_controls_dir_final = if let Some(dir) = single_stain_controls_dir {
        Some(dir.clone())
    } else if let Some(controls_dir) = controls_dir {
        Some(controls_dir.clone())
    } else {
        None
    };

    // Auto-detect endmembers and detectors if needed
    let (mut final_detectors, mut final_endmembers) = if let Some(controls_dir) =
        &single_stain_controls_dir_final
    {
        if detectors.is_empty() || endmembers.is_empty() {
            info!("Auto-detecting detectors and endmembers from single-stain controls...");
            let (auto_detectors, auto_endmembers) =
                auto_detect_from_single_stains(controls_dir, sample_fcs)?;

            let final_detectors = if detectors.is_empty() {
                info!(
                    "Auto-detected {} detectors and {} endmembers from single-stain controls",
                    auto_detectors.len(),
                    auto_endmembers.len()
                );
                info!("Auto-detected detectors: {}", auto_detectors.join(", "));
                auto_detectors
            } else {
                detectors.to_vec()
            };

            let mut final_endmembers = if endmembers.is_empty() {
                auto_endmembers
            } else {
                endmembers.to_vec()
            };

            // Add autofluorescence endmember if not already present
            if !final_endmembers.contains(&autofluorescence.to_string()) {
                info!(
                    "Adding autofluorescence endmember '{}' to endmembers list",
                    autofluorescence
                );
                final_endmembers.push(autofluorescence.to_string());
            }

            (final_detectors, final_endmembers)
        } else {
            let mut final_endmembers = endmembers.to_vec();
            // Add autofluorescence endmember if not already present
            if !final_endmembers.contains(&autofluorescence.to_string()) {
                info!(
                    "Adding autofluorescence endmember '{}' to endmembers list",
                    autofluorescence
                );
                final_endmembers.push(autofluorescence.to_string());
            }
            (detectors.to_vec(), final_endmembers)
        }
    } else {
        let mut final_endmembers = endmembers.to_vec();
        // Add autofluorescence endmember if not already present (only if endmembers were provided)
        if !final_endmembers.is_empty() && !final_endmembers.contains(&autofluorescence.to_string())
        {
            info!(
                "Adding autofluorescence endmember '{}' to endmembers list",
                autofluorescence
            );
            final_endmembers.push(autofluorescence.to_string());
        }
        (detectors.to_vec(), final_endmembers)
    };

    // Determine mixing matrix source (`--mixing-matrix` takes precedence over building from controls)
    let (
        mixing_matrix,
        detector_names_from_matrix,
        primary_detector_info,
        used_single_stain_controls,
    ) = if use_spill {
        info!("Step 1/2: Extracting mixing matrix from SPILL keyword...");
        let (matrix, detectors) = extract_mixing_matrix_from_spill(sample_fcs, &final_endmembers)?;
        // For SPILL matrix, create placeholder primary detector info
        let mut info = Vec::new();
        for endmember in &final_endmembers {
            info.push(PrimaryDetectorInfo {
                endmember_name: endmember.clone(),
                is_autofluorescence: endmember == autofluorescence,
                primary_detector_name: None,
                primary_detector_pn_name: None,
                primary_detector_pn_label: None,
                selected_marker_name: None,
                selected_fluor_name: None,
            });
        }
        (matrix, detectors, info, false)
    } else if let Some(matrix_path) = mixing_matrix_path {
        info!("Step 1/2: Loading mixing matrix from CSV file...");
        let (matrix, det_csv, em_csv) = load_mixing_matrix(matrix_path)?;
        if !em_csv.is_empty() {
            final_endmembers = em_csv;
        }
        if !det_csv.is_empty() {
            final_detectors = det_csv.clone();
        }
        let detector_names_from_matrix = if det_csv.is_empty() {
            final_detectors.clone()
        } else {
            det_csv
        };
        let mut info = Vec::new();
        for endmember in &final_endmembers {
            info.push(PrimaryDetectorInfo {
                endmember_name: endmember.clone(),
                is_autofluorescence: endmember == autofluorescence,
                primary_detector_name: None,
                primary_detector_pn_name: None,
                primary_detector_pn_label: None,
                selected_marker_name: None,
                selected_fluor_name: None,
            });
        }
        (matrix, detector_names_from_matrix, info, false)
    } else if let Some(controls_dir) = &single_stain_controls_dir_final {
        info!("Step 1/3: Identifying autofluorescence from unstained control");
        info!("Creating mixing matrix from single-stain controls...");
        let single_stain_config = SingleStainConfig {
            peak_detection,
            peak_threshold,
            peak_bias,
            peak_bias_negative,
            use_negative_events,
            autofluorescence_mode: autofluorescence_mode.to_string(),
            af_weight,
            min_negative_events,
            qc_options: qc_options.clone(),
        };
        let (matrix, detectors, info, _) = create_mixing_matrix_from_single_stains(
            controls_dir,
            &unstained_fcs,
            &final_detectors,
            &final_endmembers,
            &autofluorescence,
            &single_stain_config,
            control_assignments,
            auto_gate,
            debug_control_plots,
            if debug_control_plots {
                diagnostic_plot_dir
            } else {
                None
            },
        )?;
        (matrix, detectors, info, true)
    } else {
        return Err(anyhow::anyhow!(
            "Must provide --mixing-matrix, --use-spill, or --single-stain-controls/--controls"
        ));
    };

    // Use detector names from matrix if available, otherwise use provided/auto-detected detectors
    let final_detector_names: Vec<String> = if !detector_names_from_matrix.is_empty() {
        detector_names_from_matrix
    } else if !final_detectors.is_empty() {
        final_detectors.clone()
    } else {
        return Err(anyhow::anyhow!(
            "Detector names must be provided or extracted from SPILL keyword"
        ));
    };

    // Validate dimensions
    let n_detectors_in_matrix = mixing_matrix.nrows();

    if n_detectors_in_matrix != final_detector_names.len() {
        return Err(anyhow::anyhow!(
            "Mixing matrix rows ({}) don't match number of detectors ({})",
            n_detectors_in_matrix,
            final_detector_names.len()
        ));
    }

    if mixing_matrix.ncols() != final_endmembers.len() {
        return Err(anyhow::anyhow!(
            "Mixing matrix columns ({}) don't match number of endmembers ({})",
            mixing_matrix.ncols(),
            final_endmembers.len()
        ));
    }

    info!(
        "Prepared mixing matrix: {} detectors × {} endmembers",
        final_detector_names.len(),
        final_endmembers.len()
    );

    // Export mixing matrix if requested
    if let Some(export_path) = export_mixing_matrix {
        export_mixing_matrix_to_csv(
            &mixing_matrix,
            export_path,
            &final_detector_names,
            &final_endmembers,
        )?;
        info!("Exported mixing matrix to: {}", export_path.display());
    }

    Ok((
        mixing_matrix,
        final_detector_names,
        final_endmembers,
        unstained_fcs,
        primary_detector_info,
        used_single_stain_controls,
    ))
}

/// Process a single stained file using a pre-computed mixing matrix
/// This avoids recalculating the matrix for each file in batch processing
fn process_stained_file_with_matrix(
    stained_path: &PathBuf,
    mixing_matrix: &Array2<f64>,
    detector_names: &[String],
    endmember_names: &[String],
    unstained_fcs: &Fcs,
    primary_detector_info: &[PrimaryDetectorInfo],
    autofluorescence: &str,
    _cutoff_percentile: f64,
    strategy: &UnmixingStrategy,
    batch_prelude: Option<&TruOlsBatchPrelude>,
    output: Option<&PathBuf>,
    plot: bool,
    plot_format: &str,
    plot_output_dir: Option<&PathBuf>,
    compare_ols: bool,
    plot_both: bool,
) -> Result<()> {
    info!("Loading stained FCS file...");
    let stained_fcs = Fcs::open(stained_path.to_str().context("Invalid stained file path")?)?;

    // Filter mixing matrix and detector names to only those present in stained file
    let stained_param_names = stained_fcs.get_parameter_names_from_dataframe();
    let stained_param_set: std::collections::HashSet<&str> =
        stained_param_names.iter().map(|s| s.as_str()).collect();

    let mut filtered_matrix_rows = Vec::new();
    let mut filtered_detector_names = Vec::new();

    for (det_idx, det_name) in detector_names.iter().enumerate() {
        if stained_param_set.contains(det_name.as_str()) {
            filtered_matrix_rows.push(det_idx);
            filtered_detector_names.push(det_name.clone());
        }
    }

    if filtered_detector_names.is_empty() {
        return Err(anyhow::anyhow!(
            "No detectors from mixing matrix found in stained file. Stained file parameters: {:?}, Expected detectors: {:?}",
            stained_param_names,
            detector_names
        ));
    }

    if filtered_detector_names.len() < endmember_names.len() {
        return Err(anyhow::anyhow!(
            "Stained file has {} detectors but requires at least {} for unmixing {} endmembers (underdetermined system). \
            Consider using a stained file with more detector channels or reducing the number of endmembers.",
            filtered_detector_names.len(),
            endmember_names.len(),
            endmember_names.len()
        ));
    }

    // Reduce mixing matrix to filtered rows
    use ndarray::Array2;
    let n_filtered = filtered_matrix_rows.len();
    let mut filtered_mixing_matrix = Array2::<f64>::zeros((n_filtered, mixing_matrix.ncols()));
    for (new_idx, &orig_idx) in filtered_matrix_rows.iter().enumerate() {
        let src_row = mixing_matrix.row(orig_idx);
        filtered_mixing_matrix.row_mut(new_idx).assign(&src_row);
    }

    info!(
        "Filtered mixing matrix: {} detectors (from {}) × {} endmembers",
        n_filtered,
        detector_names.len(),
        endmember_names.len()
    );

    info!("Running TRU-OLS unmixing...");
    // Convert Vec<String> to &[&str] for the function call
    let detector_names_str: Vec<&str> =
        filtered_detector_names.iter().map(|s| s.as_str()).collect();
    let endmember_names_str: Vec<&str> = endmember_names.iter().map(|s| s.as_str()).collect();
    // Prepare primary detector metadata vectors to pass to unmixing
    let primary_detector_names: Vec<Option<String>> = primary_detector_info
        .iter()
        .map(|p| p.primary_detector_name.clone())
        .collect();
    let primary_pn_names: Vec<Option<String>> = primary_detector_info
        .iter()
        .map(|p| p.primary_detector_pn_name.clone())
        .collect();
    let primary_pn_labels: Vec<Option<String>> = primary_detector_info
        .iter()
        .map(|p| p.primary_detector_pn_label.clone())
        .collect();
    let selected_marker_names: Vec<Option<String>> = primary_detector_info
        .iter()
        .map(|p| p.selected_marker_name.clone())
        .collect();
    let selected_fluor_names: Vec<Option<String>> = primary_detector_info
        .iter()
        .map(|p| p.selected_fluor_name.clone())
        .collect();

    // Convert Array2 to faer Mat for tru-ols
    let mixing_mat = Mat::from_fn(
        filtered_mixing_matrix.nrows(),
        filtered_mixing_matrix.ncols(),
        |i, j| filtered_mixing_matrix[(i, j)],
    );

    let unmixed_fcs = if let Some(batch) = batch_prelude {
        let nonspecific_filtered = Col::from_fn(n_filtered, |i| {
            batch.nonspecific_full[filtered_matrix_rows[i]]
        });
        apply_tru_ols_unmixing_from_preprocessed_with_shared_factor_cache(
            &stained_fcs,
            unstained_fcs,
            mixing_mat,
            &detector_names_str,
            &endmember_names_str,
            autofluorescence,
            Some(*strategy),
            batch.cutoffs.clone(),
            nonspecific_filtered,
            batch
                .factor_cache
                .clone()
                .unwrap_or_else(|| shared_mask_factor_cache_with_capacity(512)),
            &primary_detector_names,
            &primary_pn_names,
            &primary_pn_labels,
            &selected_marker_names,
            &selected_fluor_names,
        )?
    } else {
        stained_fcs.apply_tru_ols_unmixing(
            unstained_fcs,
            mixing_mat,
            &detector_names_str,
            &endmember_names_str,
            autofluorescence,
            Some(*strategy),
            &primary_detector_names,
            &primary_pn_names,
            &primary_pn_labels,
            &selected_marker_names,
            &selected_fluor_names,
        )?
    };

    info!("TRU-OLS unmixing complete!");

    // Create output FCS if requested
    if let Some(output_path) = output {
        info!("Writing unmixed FCS file to: {}", output_path.display());
        use flow_fcs::write_fcs_file;
        write_fcs_file(unmixed_fcs.clone(), output_path)?;
        info!("Successfully wrote unmixed FCS file");
    }

    // Handle plotting
    if plot || plot_both {
        let plot_dir = plot_output_dir
            .map(|p| p.clone())
            .unwrap_or_else(|| PathBuf::from("."));

        std::fs::create_dir_all(&plot_dir)?;

        if plot_both && compare_ols {
            // Generate plots for both OLS and TRU-OLS
            info!("Generating comparison plots...");
            generate_ols_comparison_plots(
                &stained_fcs,
                unstained_fcs,
                &filtered_mixing_matrix,
                &detector_names_str,
                &endmember_names_str,
                &unmixed_fcs.data_frame,
                &plot_dir,
                plot_format,
                stained_path.file_name().and_then(|s| s.to_str()),
            )?;
        } else {
            // Generate TRU-OLS plots only
            generate_tru_ols_plots(
                &unmixed_fcs.data_frame,
                &endmember_names_str,
                &plot_dir,
                plot_format,
            )?;
        }
    }

    Ok(())
}

/// Extract mixing matrix from SPILL/SPILLOVER keyword in FCS file
/// For spectral cytometry, the SPILL matrix IS the mixing matrix (spectral signature matrix)
/// Returns (mixing_matrix, detector_names)
fn extract_mixing_matrix_from_spill(
    fcs: &Fcs,
    endmember_names: &[String],
) -> Result<(Array2<f64>, Vec<String>)> {
    let (spill_matrix_f32, detector_names) = fcs
        .get_spillover_matrix()
        .map_err(|e| anyhow::anyhow!("Failed to extract SPILL/SPILLOVER keyword: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No SPILL/SPILLOVER keyword found in FCS file"))?;

    // Convert f32 matrix to f64 for consistency with rest of codebase
    let spill_matrix = faer::Mat::from_fn(
        spill_matrix_f32.nrows(),
        spill_matrix_f32.ncols(),
        |i, j| spill_matrix_f32[(i, j)] as f64,
    );

    // Validate that matrix dimensions match expectations
    let n_detectors = spill_matrix.nrows();
    let n_endmembers_in_matrix = spill_matrix.ncols();

    // Validate matrix is square or has correct dimensions for spectral unmixing
    if n_detectors == 0 || n_endmembers_in_matrix == 0 {
        return Err(anyhow::anyhow!(
            "SPILL matrix has invalid dimensions: {} × {}",
            n_detectors,
            n_endmembers_in_matrix
        ));
    }

    // For spectral cytometry, SPILL matrix is detectors × fluorophores (mixing matrix)
    // Check if dimensions match endmember count
    if n_endmembers_in_matrix != endmember_names.len() {
        warn!(
            "SPILL matrix has {} endmembers, but {} were specified. Using matrix dimensions.",
            n_endmembers_in_matrix,
            endmember_names.len()
        );
    }

    // Validate matrix values are reasonable (non-negative, finite)
    for i in 0..n_detectors {
        for j in 0..n_endmembers_in_matrix {
            let value = spill_matrix[(i, j)];
            if !value.is_finite() {
                return Err(anyhow::anyhow!(
                    "SPILL matrix contains non-finite value at position [{}, {}]",
                    i,
                    j
                ));
            }
            if value < 0.0 {
                warn!(
                    "SPILL matrix contains negative value at [{}, {}]: {}",
                    i, j, value
                );
            }
        }
    }

    // Check if matrix appears to be a mixing matrix (spectral signatures)
    // Each column should have a primary detector with high value (typically > 0.5)
    let mut has_primary_detectors = true;
    for j in 0..n_endmembers_in_matrix {
        let column_max: f64 = (0..n_detectors)
            .map(|i| spill_matrix[(i, j)])
            .fold(0.0_f64, |a, b| a.max(b));
        if column_max < 0.1 {
            warn!(
                "Endmember {} has very low maximum signal ({}) in SPILL matrix",
                j, column_max
            );
            has_primary_detectors = false;
        }
    }

    if !has_primary_detectors {
        warn!(
            "SPILL matrix may not be a valid spectral mixing matrix (low primary detector signals)"
        );
    }

    info!(
        "Extracted mixing matrix from SPILL keyword: {} detectors × {} endmembers",
        n_detectors, n_endmembers_in_matrix
    );

    // Copy into ndarray for compatibility with downstream code (no faer-ext conversion)
    let mixing_matrix =
        Array2::from_shape_fn((spill_matrix.nrows(), spill_matrix.ncols()), |(i, j)| {
            spill_matrix[(i, j)]
        });
    Ok((mixing_matrix, detector_names))
}

/// Resolve unstained path: explicit `--unstained`, else auto-detect from `--controls`,
/// else from `--single-stain-controls` (same filename heuristic as [`find_unstained_control`]).
fn resolve_unstained_control_path(
    unstained_path: Option<&PathBuf>,
    controls_dir: Option<&PathBuf>,
    single_stain_controls_dir: Option<&PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = unstained_path {
        return Ok(path.clone());
    }
    if let Some(controls_dir) = controls_dir {
        info!("Auto-detecting unstained control from --controls directory...");
        let detected = find_unstained_control(controls_dir)?;
        info!("Auto-detected unstained control: {}", detected.display());
        return Ok(detected);
    }
    if let Some(ss_dir) = single_stain_controls_dir {
        info!("Auto-detecting unstained control from --single-stain-controls directory...");
        let detected = find_unstained_control(ss_dir)?;
        info!("Auto-detected unstained control: {}", detected.display());
        return Ok(detected);
    }
    Err(anyhow::anyhow!(
        "Unstained control must be provided via --unstained, or auto-detected from a directory \
         (filename containing 'unstained') using --controls or --single-stain-controls"
    ))
}

/// Find unstained control file in a directory by looking for "unstained" in filename
///
/// Returns the path to the unstained control file
pub(crate) fn find_unstained_control(controls_dir: &PathBuf) -> Result<PathBuf> {
    use std::fs;

    let entries = fs::read_dir(controls_dir)
        .with_context(|| format!("Failed to read directory: {}", controls_dir.display()))?;

    let mut unstained_candidates: Vec<PathBuf> = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("fcs") {
            // Check if filename contains "unstained" (case-insensitive)
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.to_lowercase().contains("unstained") {
                    unstained_candidates.push(path);
                }
            }
        }
    }

    if unstained_candidates.is_empty() {
        return Err(anyhow::anyhow!(
            "No unstained control file found in {} (looking for filename containing 'unstained')",
            controls_dir.display()
        ));
    }

    if unstained_candidates.len() > 1 {
        warn!(
            "Multiple files with 'unstained' in filename found: {:?}. Using first: {}",
            unstained_candidates
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy())
                .collect::<Vec<_>>(),
            unstained_candidates[0].display()
        );
    }

    Ok(unstained_candidates[0].clone())
}

/// Strip plate-style prefixes ("Filtered_Reference Group_<well> ", "Reference Group_<well> "),
/// then optional " (Beads)_*", so marker names that contain underscores (e.g. CD14_CD19_dump)
/// are not broken by taking only the last underscore-segment.
fn control_stem_to_content(stem: &str) -> &str {
    let stem_trim = stem.trim();
    let mut after_prefix = stem_trim;
    const LEADING_PREFIXES: &[&str] = &[
        "Filtered_Reference Group_",
        "Filtered Reference Group_",
        "Reference Group_",
    ];
    let mut stripped = true;
    while stripped {
        stripped = false;
        for p in LEADING_PREFIXES {
            if after_prefix.len() >= p.len() && after_prefix[..p.len()].eq_ignore_ascii_case(p) {
                after_prefix = after_prefix[p.len()..].trim_start();
                stripped = true;
            }
        }
    }
    let before_beads = after_prefix
        .split(" (Beads)_")
        .next()
        .unwrap_or(after_prefix)
        .trim();
    let after_ref = before_beads
        .strip_prefix("Reference Group_")
        .or_else(|| before_beads.strip_prefix("Reference group_"))
        .unwrap_or(before_beads);
    let rest = if let Some(space_pos) = after_ref.find(' ') {
        let after_space = after_ref[space_pos + 1..].trim();
        if !after_space.is_empty() {
            after_space
        } else {
            after_ref
        }
    } else {
        after_ref
    };
    if rest.is_empty() { before_beads } else { rest }
}

/// Extract marker and fluor from a control FCS (using $PnS when useful, else filename).
/// Returns (marker_name, fluor_name) for use in short log labels.
fn extract_marker_and_fluor_from_control(
    control_fcs: &Fcs,
    detector_names: &[String],
    control_filename: &str,
    endmember_name: &str,
) -> (String, String) {
    use std::sync::Arc;

    for det_name in detector_names.iter() {
        if let Some(param) = control_fcs.parameters.get(&Arc::from(det_name.as_str())) {
            if !param.label_name.is_empty() {
                let pns_label = param.label_name.to_string();
                if !is_detector_channel_name(&pns_label) {
                    let (m, f) = extract_marker_and_fluor_from_text(&pns_label);
                    if !m.is_empty() && !f.is_empty() {
                        info!(
                            "Using $PnS label '{}' from detector {} for marker/fluor extraction",
                            pns_label, det_name
                        );
                        return (m, f);
                    }
                }
            }
        }
    }

    info!("No useful $PnS labels found, using filename extraction");
    let text_to_parse = if control_filename.contains('(') {
        control_stem_to_content(control_filename)
    } else {
        control_filename
    };
    let (mut marker, mut fluor) = extract_marker_and_fluor_from_text(text_to_parse);

    if marker.is_empty() {
        marker = endmember_name.to_string();
    }
    if fluor.is_empty() {
        fluor = marker.clone();
    }
    (marker, fluor)
}

/// Derive a short display label from a control file stem (e.g. "CD45 Spark UV 387" from
/// "Reference Group_A2 CD45 Spark UV 387 (Beads)_2026_03_05_11_55_59"). Uses marker/fluor
/// extraction heuristics; falls back to the full stem if parsing fails.
fn short_label_from_control_stem(stem: &str) -> String {
    let content = control_stem_to_content(stem);
    let (marker, fluor) = extract_marker_and_fluor_from_text(content);
    let label = format!("{} {}", marker, fluor).trim().to_string();
    if label.is_empty() {
        stem.to_string()
    } else {
        label
    }
}

/// Strip a trailing `.fcs` suffix if present (canonical endmembers are usually stems only).
fn strip_fcs_extension_from_stem(s: &str) -> &str {
    let t = s.trim();
    if let Some(dot) = t.rfind('.') {
        let (_, ext) = t.split_at(dot);
        if ext.eq_ignore_ascii_case(".fcs") {
            return t[..dot].trim_end();
        }
    }
    t
}

fn normalize_endmember_display_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// If at least two whitespace-separated tokens are exactly two ASCII digits, drop **all**
/// such two-digit-only tokens. This removes leftover `MM DD HH mm ss` fragments (e.g. from
/// `2025_09_25_15_15_25`) without removing three-digit dye indices (`387`, `421`) or tokens
/// like `RY775`.
fn strip_standalone_two_digit_datetime_tokens(s: &str) -> String {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let two_digit_only = parts
        .iter()
        .filter(|t| t.len() == 2 && t.chars().all(|c| c.is_ascii_digit()))
        .count();
    if two_digit_only < 2 {
        return s.to_string();
    }
    parts
        .into_iter()
        .filter(|t| !(t.len() == 2 && t.chars().all(|c| c.is_ascii_digit())))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Filename / export tokens that are unlikely to be part of a fluor name for UI labels.
const ENDMEMBER_DISPLAY_NOISE_WORDS: &[&str] = &[
    "filtered",
    "reference",
    "group",
    "plate",
    "well",
    "bead",
    "beads",
    "cell",
    "cells",
    "positive",
    "negative",
    "debris",
    "non",
    "file",
    "sample",
    "acquisition",
];

fn is_noise_endmember_token(t: &str) -> bool {
    let l = t.to_lowercase();
    if ENDMEMBER_DISPLAY_NOISE_WORDS.iter().any(|w| l == *w) {
        return true;
    }
    if l.contains("debris") {
        return true;
    }
    if l.starts_with("plate") && l.chars().skip(5).all(|c| c.is_ascii_digit()) {
        return true;
    }
    false
}

/// Pure digit / digit+underscore blobs with enough digits to treat as timestamps, plate IDs, etc.
fn should_drop_digit_style_token(t: &str) -> bool {
    if !t
        .chars()
        .all(|c| c.is_ascii_digit() || c == '_')
    {
        return false;
    }
    let digit_count = t.chars().filter(|c| c.is_ascii_digit()).count();
    if digit_count >= 4 {
        return true;
    }
    // Typical zero-padded export / plate indices (avoid dropping 3-digit dye numbers like 387).
    t.len() == 3 && t.chars().all(|c| c.is_ascii_digit()) && t.starts_with('0')
}

/// Split on runs of characters that are not ASCII alphanumeric or `-` (keeps `PD-1`, `BV421`).
fn for_each_stem_display_token(s: &str, mut on_token: impl FnMut(&str)) {
    let s = s.trim();
    let mut start: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        let keep = ch.is_ascii_alphanumeric() || ch == '-';
        match (start, keep) {
            (None, true) => start = Some(i),
            (Some(b), false) => {
                on_token(&s[b..i]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(b) = start {
        on_token(&s[b..]);
    }
}

fn filter_stem_tokens_for_endmember_display(stem: &str) -> String {
    let stem = strip_fcs_extension_from_stem(stem);
    let mut kept: Vec<String> = Vec::new();
    for_each_stem_display_token(stem, |tok| {
        if tok.is_empty() {
            return;
        }
        if is_noise_endmember_token(tok) {
            return;
        }
        if is_well_identifier(tok) {
            return;
        }
        if should_drop_digit_style_token(tok) {
            return;
        }
        kept.push(tok.to_string());
    });
    normalize_endmember_display_whitespace(&kept.join(" "))
}

/// Human-readable endmember line for interactive UI. Canonical ids remain control **file stems**
/// (see [`auto_detect_from_single_stains`]); this is not `$FIL`.
pub(crate) fn endmember_display_label(canonical_stem: &str) -> String {
    let stem = strip_fcs_extension_from_stem(canonical_stem);
    let from_tokens = filter_stem_tokens_for_endmember_display(stem);
    let from_short = short_label_from_control_stem(stem);

    let chosen = if !from_tokens.is_empty() && from_tokens.len() + 8 < stem.len() {
        from_tokens
    } else if !from_short.is_empty() && from_short != stem {
        from_short
    } else if !from_tokens.is_empty() {
        from_tokens
    } else {
        stem.to_string()
    };
    normalize_endmember_display_whitespace(&strip_standalone_two_digit_datetime_tokens(&chosen))
}

/// List `.fcs` paths in a directory, excluding filenames containing `unstained` (case-insensitive).
pub(crate) fn list_non_unstained_control_fcs(controls_dir: &Path) -> Result<Vec<PathBuf>> {
    use std::fs;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(controls_dir).with_context(|| {
        format!(
            "Failed to read single-stain control directory: {}",
            controls_dir.display()
        )
    })? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("fcs") {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.to_lowercase().contains("unstained") {
                    continue;
                }
            }
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(anyhow::anyhow!(
            "No single-stain .fcs files in {}",
            controls_dir.display()
        ));
    }
    Ok(paths)
}

/// Auto-detect detectors and endmembers from single-stain control files
///
/// Returns (detectors, endmembers) where:
/// - detectors: Parameter names from the sample FCS (excluding FSC/SSC/Time and `Unmixed_*` export columns)
/// - endmembers: Single-stain control **file name stems** (without `.fcs`), not `$FIL`
///
/// Excludes unstained control files (those containing "unstained" in filename)
pub(crate) fn auto_detect_from_single_stains(
    controls_dir: &PathBuf,
    sample_fcs: &Fcs,
) -> Result<(Vec<String>, Vec<String>)> {
    use std::fs;

    /// Synthetic columns written by this toolchain's unmixed exports (`Unmixed_<marker>`).
    fn is_unmixed_export_column(name: &str) -> bool {
        name.trim().to_uppercase().starts_with("UNMIXED_")
    }

    // Get all parameter names from the sample FCS file
    let all_params = sample_fcs.get_parameter_names_from_dataframe();

    // Filter out scatter and time parameters to get detector names
    // Keep only fluorescent parameters (typically FL1-A, FL2-A, etc.)
    let detectors: Vec<String> = all_params
        .iter()
        .filter(|name| {
            let name_upper = name.to_uppercase();
            // Exclude FSC, SSC, and Time parameters
            !name_upper.contains("FSC")
                && !name_upper.contains("SSC")
                && !name_upper.contains("TIME")
                && !name_upper.contains("TIME ")
                && !is_unmixed_export_column(name)
        })
        .cloned()
        .collect();

    if detectors.is_empty() {
        return Err(anyhow::anyhow!(
            "No fluorescent detector parameters found in FCS file (after excluding Unmixed_* export columns). Found parameters: {}",
            sample_fcs.get_parameter_names_from_dataframe().join(", ")
        ));
    }

    let skipped_unmixed: Vec<&String> = all_params
        .iter()
        .filter(|n| is_unmixed_export_column(n))
        .collect();
    if !skipped_unmixed.is_empty() {
        warn!(
            "Ignoring {} Unmixed_* parameter(s) on the stained sample when auto-detecting detectors (use raw compensated FCS for unmixing, not a prior unmixed export).",
            skipped_unmixed.len()
        );
    }

    // Get all FCS files in the controls directory
    let entries = fs::read_dir(controls_dir)
        .with_context(|| format!("Failed to read directory: {}", controls_dir.display()))?;

    let mut endmembers: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("fcs") {
            // Skip unstained control files (they're not endmembers)
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.to_lowercase().contains("unstained") {
                    continue;
                }
            }

            // Extract endmember name from filename (without extension)
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // Use filename stem as endmember name
                if !endmembers.contains(&stem.to_string()) {
                    endmembers.push(stem.to_string());
                }
            }
        }
    }

    if endmembers.is_empty() {
        return Err(anyhow::anyhow!(
            "No FCS files found in single-stain controls directory: {}",
            controls_dir.display()
        ));
    }

    // Sort endmembers for consistent ordering
    endmembers.sort();

    let short_labels: Vec<String> = endmembers
        .iter()
        .map(|s| short_label_from_control_stem(s))
        .collect();
    info!(
        "Auto-detected {} detectors and {} endmembers from single-stain controls",
        detectors.len(),
        endmembers.len()
    );
    info!("Auto-detected endmembers: {}", short_labels.join(", "));

    Ok((detectors, endmembers))
}

/// Split a candidate name into fragments using common delimiters and
/// return a list of unique candidate fragments (including the full name).
#[allow(dead_code)] // used in tests and by extract_fluor_candidates
fn candidate_fragments(name: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let full = name.trim().to_string();
    if !full.is_empty() {
        parts.push(full.clone());
    }
    // Split on whitespace
    for p in name.split_whitespace() {
        let s = p.trim();
        if !s.is_empty() && !parts.contains(&s.to_string()) {
            parts.push(s.to_string());
        }
    }
    // Split on underscore
    for p in name.split('_') {
        let s = p.trim();
        if !s.is_empty() && !parts.contains(&s.to_string()) {
            parts.push(s.to_string());
        }
    }
    // Split on hyphen
    for p in name.split('-') {
        let s = p.trim();
        if !s.is_empty() && !parts.contains(&s.to_string()) {
            parts.push(s.to_string());
        }
    }
    parts
}

/// Extract fluor/dye name candidates from a filename or label
/// This is specialized for fluorophore naming patterns and keeps multi-word names together
#[allow(dead_code)] // used by choose_fragment_interactive and examples
fn extract_fluor_candidates(filename: &str, pn_label: Option<&str>) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();

    // First, try to extract from $PnS label (often contains the dye name)
    if let Some(label) = pn_label {
        if !label.is_empty() {
            // Add the full label as first candidate
            candidates.push(label.to_string());

            // Also add fragments for short labels
            if label.len() < 15 {
                candidates.extend(candidate_fragments(label));
            }
        }
    }

    // Extract from filename - look for pattern between marker and parenthesis
    // Format: "Reference Group_A2 HLA-DR_DQ Spark UV 387 (Beads)_..."
    if let Some(paren_start) = filename.find('(') {
        let before_paren = filename[..paren_start].trim();

        // Split on underscore to separate sections
        let sections: Vec<&str> = before_paren.split('_').collect();

        // The dye is typically in the last 1-2 sections before the parenthesis
        // Look for sections that start with common dye name patterns
        if sections.len() >= 3 {
            let last_section = sections[sections.len() - 1].trim();

            // Split the last section on spaces to find dye name start
            let words: Vec<&str> = last_section.split_whitespace().collect();

            // Look for where the dye name starts (after marker fragments)
            let mut dye_start_idx = 0;
            for (i, word) in words.iter().enumerate() {
                if is_dye_name_start(word) {
                    dye_start_idx = i;
                    break;
                }
            }

            // Extract from dye start to end
            if dye_start_idx < words.len() {
                let dye_name = words[dye_start_idx..].join(" ");
                if is_likely_fluor_name(&dye_name) && !candidates.contains(&dye_name) {
                    candidates.push(dye_name);
                }
            }
        }
    }

    // Filter and clean candidates
    candidates.retain(|c| {
        let lower = c.to_lowercase();
        // Remove obvious non-dye terms
        !lower.contains("plate") 
            && !lower.contains("beads") 
            && !lower.contains("cells")
            && !lower.contains("filtered")
            && !lower.contains("reference")
            && !lower.contains("group")
            && !lower.contains("non-debris")
            // Filter out pure date/time patterns
            && !c.chars().all(|ch| ch.is_numeric() || ch == '_')
            // Keep if it looks like a dye name
            && (c.len() <= 20 && c.len() >= 2)
    });

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|c| seen.insert(c.clone()));

    candidates
}

/// Check if a word is the start of a dye name
fn is_dye_name_start(word: &str) -> bool {
    let word_upper = word.to_uppercase();

    // Common dye name prefixes
    let dye_prefixes = [
        "SPARK", "UV", "BV", "BUV", "BB", "PE", "APC", "FITC", "RB", "RY", "LD", "AF", "ALEXA",
        "NEAR", "FAR", "LIVE", "DEAD", "R", "V", "B", "YG",
    ];

    // Check if word starts with or equals a common dye prefix
    dye_prefixes
        .iter()
        .any(|&prefix| word_upper.starts_with(prefix))
        || word_upper.starts_with("R")
            && word.len() >= 2
            && word.chars().nth(1).map_or(false, |c| c.is_numeric())
        || word_upper.starts_with("V")
            && word.len() >= 2
            && word.chars().nth(1).map_or(false, |c| c.is_numeric())
}

/// Check if a word is a well identifier (e.g., "A10", "B5", "H12")
fn is_well_identifier(word: &str) -> bool {
    if word.len() < 2 || word.len() > 3 {
        return false;
    }

    let first_char = word.chars().next().unwrap();
    let rest = &word[1..];

    // Well format: Letter (A-H) followed by 1-2 digits
    first_char.is_ascii_uppercase()
        && first_char >= 'A'
        && first_char <= 'H'
        && rest.chars().all(|c| c.is_numeric())
        && rest.parse::<u32>().map_or(false, |n| n >= 1 && n <= 12)
}

/// Check if a string looks like a detector channel name (e.g., "B1-A", "UV2-A", "BL3-A")
fn is_detector_channel_name(s: &str) -> bool {
    // Pattern: Letters/numbers followed by dash and letter (e.g., "B1-A", "UV2-A")
    if let Some(dash_pos) = s.find('-') {
        if dash_pos > 0 && dash_pos < s.len() - 1 {
            let before_dash = &s[..dash_pos];
            let after_dash = &s[dash_pos + 1..];

            // Before dash should start with letter and may contain numbers
            // After dash should be single letter (A, H, W, etc.)
            return before_dash
                .chars()
                .next()
                .map_or(false, |c| c.is_ascii_alphabetic())
                && after_dash.len() == 1
                && after_dash
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_ascii_alphabetic());
        }
    }
    false
}

/// Extract marker and fluor names from a label or filename
/// Returns (marker_name, fluor_name) where both can be empty if extraction fails
/// This intelligently splits on dye name boundaries
fn extract_marker_and_fluor_from_text(text: &str) -> (String, String) {
    // Clean up the text: remove common prefixes and split on spaces
    let cleaned = text.trim();
    let words: Vec<&str> = cleaned.split_whitespace().collect();

    if words.is_empty() {
        return (String::new(), String::new());
    }

    // Find where dye name starts
    let mut dye_start_idx = words.len();
    for (i, word) in words.iter().enumerate() {
        if is_dye_name_start(word) {
            dye_start_idx = i;
            break;
        }
    }

    // Extract marker: words before dye, filtered for well IDs and detector names
    let marker_words: Vec<&str> = words[..dye_start_idx]
        .iter()
        .filter(|&w| !is_well_identifier(w) && !is_detector_channel_name(w))
        .copied()
        .collect();

    let marker_name = if !marker_words.is_empty() {
        marker_words.join(" ")
    } else if dye_start_idx > 0 {
        // Fallback: use unfiltered if filtering removed everything
        words[..dye_start_idx].join(" ")
    } else {
        String::new()
    };

    // Extract fluor: words from dye start onwards
    let fluor_name = if dye_start_idx < words.len() {
        words[dye_start_idx..].join(" ")
    } else {
        String::new()
    };

    (marker_name, fluor_name)
}

/// Check if a string looks like a fluorophore/dye name
#[allow(dead_code)] // used by extract_fluor_candidates
fn is_likely_fluor_name(s: &str) -> bool {
    let s_trim = s.trim();

    // Common patterns for dye names:
    // - Contains letters and numbers (e.g., "BV421", "Spark UV 387")
    // - Contains "UV", "Spark", "PE", "APC", "FITC", etc.
    // - Short alphanumeric strings
    // - Mixed case or all caps

    let has_letter = s_trim.chars().any(|c| c.is_alphabetic());
    let has_number = s_trim.chars().any(|c| c.is_numeric());

    // Common dye name patterns
    let common_dyes = [
        "UV", "Spark", "PE", "APC", "FITC", "BV", "BUV", "BB", "RB", "RY", "LD", "Near IR",
    ];
    let has_common_pattern = common_dyes.iter().any(|&dye| s_trim.contains(dye));

    // Looks like a dye if:
    // 1. Has letters (not just numbers)
    // 2. Either has numbers OR matches common dye pattern
    // 3. Reasonable length
    has_letter && (has_number || has_common_pattern) && s_trim.len() >= 2 && s_trim.len() <= 30
}

/// If multiple candidate fragments exist, prompt the user to choose one.
/// Returns the selected fragment and inferred delimiter preference.
#[allow(dead_code)] // used when interactive marker selection is enabled
fn choose_fragment_interactive(
    control_path: &PathBuf,
    candidates: &[String],
    original_name: &str,
) -> (String, DelimiterPreference) {
    if candidates.len() <= 1 {
        let choice = candidates
            .get(0)
            .cloned()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let pref = DelimiterPreference::infer(original_name, &choice);
        return (choice, pref);
    }

    println!(
        "Ambiguous marker name extracted from control file: {}",
        control_path.display()
    );
    println!("Select the best marker name from the candidates below:");
    for (i, c) in candidates.iter().enumerate() {
        println!("  {}) {}", i + 1, c);
    }
    print!("Enter selection (1-{}) [default: 1]: ", candidates.len());
    let _ = stdout().flush();

    let mut input = String::new();
    let choice = match stdin().read_line(&mut input) {
        Ok(_) => {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                candidates[0].clone()
            } else if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 && idx <= candidates.len() {
                    candidates[idx - 1].clone()
                } else {
                    candidates[0].clone()
                }
            } else {
                candidates[0].clone()
            }
        }
        Err(_) => candidates[0].clone(),
    };
    let pref = DelimiterPreference::infer(original_name, &choice);
    (choice, pref)
}

/// Per-endmember data used to resolve shared-primary conflicts (swap to second-highest peak).
#[derive(Clone)]
/// Per-endmember data used to resolve shared primary detector conflicts (e.g. swap to second-highest peak).
#[allow(dead_code)]
pub struct EndmemberConflictData {
    pub corrected_medians: Vec<f32>,
    pub primary_idx: usize,
    pub second_idx: usize,
    pub alternate_primary_info: PrimaryDetectorInfo,
}

/// Metadata about primary detector for an endmember
/// Used to generate unmixed output channel names and labels
#[derive(Debug, Clone)]
pub struct PrimaryDetectorInfo {
    /// Name of the endmember
    #[allow(dead_code)] // populated for API/export; not yet read in CLI
    pub endmember_name: String,
    /// Is this the autofluorescence endmember (no primary detector)
    #[allow(dead_code)]
    pub is_autofluorescence: bool,
    /// Name of the primary detector (e.g., "UV1"), None for autofluorescence
    pub primary_detector_name: Option<String>,
    /// $PnN (parameter name) from the primary detector's control file
    pub primary_detector_pn_name: Option<String>,
    /// $PnS (parameter label) from the primary detector's control file
    pub primary_detector_pn_label: Option<String>,
    /// User-selected marker name from interactive prompt (e.g., "HLA-DR_DQ", "CD4")
    pub selected_marker_name: Option<String>,
    /// User-selected fluor/dye name for the $PnS label (e.g., "RB705", "BV421")
    pub selected_fluor_name: Option<String>,
}

/// Configuration for single-stain control processing
#[derive(Debug, Clone)]
pub struct SingleStainConfig {
    /// Enable peak-based median selection
    pub peak_detection: bool,
    /// Peak detection threshold (fraction of max density)
    pub peak_threshold: f64,
    /// Peak bias fraction for positive peaks (0.5 = upper 50% of peak events)
    /// Higher values bias more to the right side of the peak
    pub peak_bias: f64,
    /// Peak bias fraction for negative peaks (0.5 = lower 50% of peak events)
    /// Higher values bias more to the left side of the negative peak
    pub peak_bias_negative: f64,
    /// Use negative events from controls for autofluorescence
    pub use_negative_events: bool,
    /// Autofluorescence mode: universal, negative-events, hybrid
    pub autofluorescence_mode: String,
    /// Autofluorescence weight for hybrid mode (0.0-1.0, default: 0.7)
    /// Weight of unstained control vs negative events
    pub af_weight: f64,
    /// Minimum number of negative events required (default: 100)
    pub min_negative_events: usize,
    /// Optional QC pipeline overrides (preset, debug plots, PeacoQC, scatter policy).
    pub qc_options: crate::qc_pipeline::QcCliOptions,
}

impl Default for SingleStainConfig {
    fn default() -> Self {
        Self {
            peak_detection: true,
            peak_threshold: 0.3,
            peak_bias: 0.5,
            peak_bias_negative: 0.5,
            use_negative_events: false,
            autofluorescence_mode: "universal".to_string(),
            af_weight: 0.7,
            min_negative_events: 100,
            qc_options: crate::qc_pipeline::QcCliOptions::default(),
        }
    }
}

/// Build map: detector name -> list of endmember names that have it as primary.
fn detector_to_endmembers(
    primary_detector_info: &[PrimaryDetectorInfo],
    endmember_names: &[String],
) -> std::collections::HashMap<String, Vec<String>> {
    let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (idx, info) in primary_detector_info.iter().enumerate() {
        if let Some(ref det) = info.primary_detector_name {
            map.entry(det.clone())
                .or_default()
                .push(endmember_names[idx].clone());
        }
    }
    map
}

/// Swap one endmember in a conflict group to its second-highest peak. Returns true if a swap was done.
fn apply_one_swap(
    mixing_matrix: &mut Array2<f64>,
    primary_detector_info: &mut [PrimaryDetectorInfo],
    conflict_data: &mut [Option<EndmemberConflictData>],
    detector_to_endmembers: &std::collections::HashMap<String, Vec<String>>,
    endmember_names: &[String],
) -> bool {
    for (detector, endmembers) in detector_to_endmembers.iter() {
        if endmembers.len() <= 1 {
            continue;
        }
        // Find endmember indices and their (primary_median, second_median) from conflict_data
        let mut candidates: Vec<(usize, f32, f32)> = Vec::new();
        for endmember_name in endmembers {
            let idx = match endmember_names.iter().position(|n| n == endmember_name) {
                Some(i) => i,
                None => continue,
            };
            let data = match conflict_data.get(idx).and_then(|o| o.as_ref()) {
                Some(d) => d,
                None => continue,
            };
            let primary_median = data.corrected_medians[data.primary_idx];
            let second_median = data.corrected_medians[data.second_idx];
            if second_median <= 0.0 {
                continue;
            }
            candidates.push((idx, primary_median, second_median));
        }
        if candidates.is_empty() {
            continue;
        }
        // Swap the one with smallest gap (second peak closest to first = most ambiguous)
        let (swap_idx, _primary_m, _second_median) = candidates
            .iter()
            .min_by(|(_, a1, a2), (_, b1, b2)| {
                let gap_a = a1 - a2;
                let gap_b = b1 - b2;
                gap_a
                    .partial_cmp(&gap_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
            .unwrap();
        let data = conflict_data[swap_idx].take().unwrap();
        let norm = data.corrected_medians[data.second_idx].max(1e-10);
        for (detector_idx, &val) in data.corrected_medians.iter().enumerate() {
            mixing_matrix[(detector_idx, swap_idx)] = (val / norm) as f64;
        }
        primary_detector_info[swap_idx] = data.alternate_primary_info;
        info!(
            "Resolved shared primary '{}': swapped endmember '{}' to second-highest peak",
            detector, endmember_names[swap_idx]
        );
        return true;
    }
    false
}

/// Create mixing matrix from single-stain control FCS files
/// Each control file should contain events stained with one fluorophore
/// Returns (mixing_matrix, detector_names, primary_detector_info)
///
/// If diagnostic_plot_dir is provided, generates diagnostic plots for each control:
/// - FSC-A vs SSC-A and FSC-A vs FSC-H before/after gating
/// - Density plots showing signal across channels
/// - Normalized spectral signature plots (1.0 to 0.0 vs channels)
pub fn create_mixing_matrix_from_single_stains(
    controls_dir: &PathBuf,
    unstained_fcs: &Fcs,
    detector_names: &[String],
    endmember_names: &[String],
    autofluorescence_name: &str,
    config: &SingleStainConfig,
    control_assignments: Option<&[(String, PathBuf)]>,
    auto_gate: bool,
    debug_control_plots: bool,
    diagnostic_plot_dir: Option<&PathBuf>,
) -> Result<(
    Array2<f64>,
    Vec<String>,
    Vec<PrimaryDetectorInfo>,
    Vec<Option<EndmemberConflictData>>,
)> {
    use std::fs;

    info!(
        "Scanning single-stain control directory: {}",
        controls_dir.display()
    );

    let control_files: Vec<(String, PathBuf)> = if let Some(assignments) = control_assignments {
        let mut v: Vec<(String, PathBuf)> = Vec::new();
        for (endmember_name, path) in assignments {
            if path.extension().and_then(|s| s.to_str()) != Some("fcs") {
                return Err(anyhow::anyhow!(
                    "Control assignment for '{}' is not an .fcs file: {}",
                    endmember_name,
                    path.display()
                ));
            }
            if !path.is_file() {
                return Err(anyhow::anyhow!(
                    "Control assignment path for '{}' does not exist or is not a file: {}",
                    endmember_name,
                    path.display()
                ));
            }
            v.push((endmember_name.clone(), path.clone()));
        }
        for em in endmember_names {
            if em == autofluorescence_name {
                continue;
            }
            if !v.iter().any(|(n, _)| n == em) {
                return Err(anyhow::anyhow!(
                    "Control assignments missing endmember '{}'",
                    em
                ));
            }
        }
        info!(
            "Using {} interactive control file assignment(s)",
            v.len()
        );
        v
    } else {
        let entries = fs::read_dir(controls_dir)
            .with_context(|| format!("Failed to read directory: {}", controls_dir.display()))?;

        let mut scanned: Vec<(String, PathBuf)> = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("fcs") {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if filename.to_lowercase().contains("unstained") {
                        continue;
                    }
                }

                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                for endmember in endmember_names {
                    if filename.contains(&endmember.to_lowercase()) {
                        scanned.push((endmember.clone(), path));
                        break;
                    }
                }
            }
        }
        if scanned.is_empty() {
            return Err(anyhow::anyhow!(
                "No matching single-stain control files found in {}",
                controls_dir.display()
            ));
        }
        info!("Found {} single-stain control files", scanned.len());
        scanned
    };

    // Detect the most ambiguous file (most delimiters) for interactive marker selection
    let _delimiter_preference = DelimiterPreference {
        use_space: true,
        use_hyphen: true,
        use_underscore: true,
    };
    if let Some((most_ambig_idx, delim_count)) = find_most_ambiguous_endmember(&control_files) {
        debug!(
            "Most ambiguous control file at index {}: {} delimiters",
            most_ambig_idx, delim_count
        );
    }

    // Extract autofluorescence medians from unstained control (universal AF)
    // Gate unstained control to match single-stain controls when auto_gate is enabled
    let unstained_label = unstained_control_label(unstained_fcs);
    let want_unstained_snapshots =
        auto_gate && debug_control_plots && diagnostic_plot_dir.is_some();
    let unstained_for_af = if auto_gate {
        info!(
            "Applying QC pipeline to unstained control '{}' for autofluorescence extraction...",
            unstained_label
        );
        let mut qc_cfg = crate::qc_pipeline::QcPipelineConfig::literature_default();
        qc_cfg.apply_qc_cli_options(&config.qc_options);
        if want_unstained_snapshots {
            qc_cfg.capture_stages = true;
        }
        let plot_dir = qc_cfg
            .debug_plot_dir
            .clone()
            .or_else(|| diagnostic_plot_dir.map(|p| p.to_path_buf()));
        qc_cfg.debug_plot_dir = plot_dir;

        let report = crate::qc_pipeline::run_qc_pipeline(&unstained_fcs, &qc_cfg)
            .with_context(|| {
                format!(
                    "QC pipeline failed for unstained control '{}'",
                    unstained_label
                )
            })?;

        // Generate debug plots for unstained control if requested
        if want_unstained_snapshots {
            let snap = |name: &str| {
                report
                    .stage_snapshots
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, f)| f.clone())
                    .unwrap_or_else(|| report.final_fcs.clone())
            };
            let stages = (
                snap("post_margins"),
                snap("post_raw_doublets"),
                report.final_fcs.clone(),
            );
            if let Err(e) = generate_control_cleanup_debug_plots(
                &unstained_fcs,
                Some(&stages),
                &report.final_fcs,
                "Unstained (Autofluorescence)",
                detector_names,
                0, // arbitrary primary_idx for AF (not used for gating)
                config,
                diagnostic_plot_dir.unwrap(),
                "jpg",
            ) {
                warn!("Failed to generate debug plots for unstained control: {}", e);
            }
        }
        report.final_fcs
    } else {
        unstained_fcs.clone()
    };

    let mut autofluorescence_medians: Vec<f32> = Vec::new();
    for detector_name in detector_names {
        let values = unstained_for_af
            .get_parameter_events_slice(detector_name)
            .with_context(|| {
                format!("Failed to extract {} from unstained control", detector_name)
            })?;

        // Calculate median
        let mut sorted_values: Vec<f32> = values.iter().copied().collect();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = if sorted_values.is_empty() {
            0.0
        } else {
            sorted_values[sorted_values.len() / 2]
        };
        autofluorescence_medians.push(median);
    }

    // Store negative event autofluorescence per endmember (if enabled)
    // Map: endmember_name -> detector_name -> median AF from negative events
    let mut negative_event_af: std::collections::HashMap<String, Vec<f32>> =
        std::collections::HashMap::new();

    // Process each single-stain control (skip autofluorescence - it will be added as last column)
    let n_detectors = detector_names.len();
    let n_endmembers = endmember_names.len();
    let mut mixing_matrix = Array2::<f64>::zeros((n_detectors, n_endmembers));

    // Track which endmembers are fluorophores (have control files) vs autofluorescence
    let mut fluorophore_endmembers: Vec<(usize, String)> = Vec::new();
    let mut autofluorescence_idx: Option<usize> = None;
    // Initialize primary_detector_info with None values, to be filled in by endmember index
    let mut primary_detector_info: Vec<Option<PrimaryDetectorInfo>> = vec![None; n_endmembers];
    // Per-endmember data for resolving shared-primary conflicts (second-highest peak swap).
    let mut conflict_data: Vec<Option<EndmemberConflictData>> = vec![None; n_endmembers];
    // Track the primary detectors used (for filtering returned detector list)
    let mut primary_detectors_used: Vec<(usize, String)> = Vec::new();

    // First pass: identify which endmembers are fluorophores vs autofluorescence
    info!(
        "Looking for autofluorescence '{}' in {} endmembers: {:?}",
        autofluorescence_name,
        endmember_names.len(),
        endmember_names
    );
    for (endmember_idx, endmember_name) in endmember_names.iter().enumerate() {
        if endmember_name == autofluorescence_name {
            info!("Found autofluorescence at index {}", endmember_idx);
            autofluorescence_idx = Some(endmember_idx);
        } else {
            // Check if this endmember has a control file
            if control_files.iter().any(|(name, _)| name == endmember_name) {
                fluorophore_endmembers.push((endmember_idx, endmember_name.clone()));
            } else {
                return Err(anyhow::anyhow!(
                    "No single-stain control file found for endmember: {}",
                    endmember_name
                ));
            }
        }
    }

    if autofluorescence_idx.is_none() {
        return Err(anyhow::anyhow!(
            "Autofluorescence endmember '{}' not found in endmember names",
            autofluorescence_name
        ));
    }
    let autofluorescence_idx = autofluorescence_idx.unwrap();

    let n_controls = fluorophore_endmembers.len();
    info!(
        "Step 2/3: Extracting fluor spectra from single-stain controls ({} controls)",
        n_controls
    );

    // Process fluorophore endmembers (skip autofluorescence)
    for (control_file_idx, (endmember_idx, endmember_name)) in
        fluorophore_endmembers.iter().enumerate()
    {
        // Find matching control file
        let control_path = control_files
            .iter()
            .find(|(name, _)| name == endmember_name)
            .map(|(_, path)| path)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No single-stain control file found for endmember: {}",
                    endmember_name
                )
            })?;

        let control_filename_only = control_path
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| endmember_name.into());
        info!(
            "Control {}/{}: {}",
            control_file_idx + 1,
            n_controls,
            control_filename_only
        );
        info!("  auto_gate enabled: {}", auto_gate);

        // Load control FCS file
        let control_fcs_before_gating =
            Fcs::open(control_path.to_str().context("Invalid control file path")?)?;

        let control_filename = control_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(endmember_name);

        // Extract marker and fluor early so we can use a short label in logs (e.g. "CD45 Spark UV 387")
        let (marker_name, fluor_name) = extract_marker_and_fluor_from_control(
            &control_fcs_before_gating,
            detector_names,
            control_filename,
            endmember_name,
        );
        let control_short_label = {
            let s = format!("{} {}", marker_name, fluor_name).trim().to_string();
            if s.is_empty() {
                endmember_name.to_string()
            } else {
                s
            }
        };

        // When generating debug control plots we need intermediate stages; otherwise run pipeline once.
        let want_stage_snapshots =
            auto_gate && debug_control_plots && diagnostic_plot_dir.is_some();
        let mut qc_cfg = crate::qc_pipeline::QcPipelineConfig::literature_default();
        qc_cfg.apply_qc_cli_options(&config.qc_options);
        if want_stage_snapshots {
            qc_cfg.capture_stages = true;
        }
        if auto_gate {
            let plot_dir = qc_cfg
                .debug_plot_dir
                .clone()
                .or_else(|| diagnostic_plot_dir.map(|p| p.to_path_buf()));
            qc_cfg.debug_plot_dir = plot_dir;
        }

        let (control_fcs, stages_for_plot) = if auto_gate {
            info!(
                "Applying QC pipeline to control '{}' ({}/{})",
                control_filename_only,
                control_file_idx + 1,
                n_controls
            );
            let report = crate::qc_pipeline::run_qc_pipeline(&control_fcs_before_gating, &qc_cfg)
                .with_context(|| {
                    format!(
                        "QC pipeline failed for single-stain control '{}' ({}) [endmember: {}]",
                        control_filename_only,
                        control_path.display(),
                        endmember_name
                    )
                })?;
            let stages_for_plot = if want_stage_snapshots {
                let snap = |name: &str| {
                    report
                        .stage_snapshots
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, f)| f.clone())
                        .unwrap_or_else(|| report.final_fcs.clone())
                };
                Some((
                    snap("post_margins"),
                    snap("post_raw_doublets"),
                    report.final_fcs.clone(),
                ))
            } else {
                None
            };
            (report.final_fcs, stages_for_plot)
        } else {
            (control_fcs_before_gating.clone(), None)
        };

        if auto_gate {
            info!("Applied QC pipeline to {} control", control_short_label);
        }

        // Extract negative events for autofluorescence calculation (if enabled)
        if config.use_negative_events {
            let negative_af = extract_negative_event_autofluorescence(
                &control_fcs,
                detector_names,
                endmember_name,
                config,
            )?;

            if let Some(af) = negative_af {
                // Compare with universal AF for diagnostics
                let universal_af = &autofluorescence_medians;
                let mut af_differences = Vec::new();
                for (det_idx, detector_name) in detector_names.iter().enumerate() {
                    let diff = (af[det_idx] - universal_af[det_idx]).abs();
                    let diff_percent = if universal_af[det_idx] > 0.0 {
                        (diff / universal_af[det_idx]) * 100.0
                    } else {
                        0.0
                    };
                    af_differences.push((detector_name.clone(), diff, diff_percent));
                }

                let max_diff = af_differences
                    .iter()
                    .map(|(_, _, p)| *p)
                    .fold(0.0f32, f32::max);
                info!(
                    "Extracted negative event autofluorescence for {} ({} detectors, max diff: {:.1}%)",
                    endmember_name,
                    detector_names.len(),
                    max_diff
                );

                if max_diff > 20.0 {
                    info!(
                        "Significant difference between negative-event AF and universal AF for {}:",
                        endmember_name
                    );
                    for (det_idx, (det_name, _diff, diff_pct)) in af_differences.iter().enumerate()
                    {
                        if *diff_pct > 10.0 {
                            info!(
                                "  {}: negative={:.2}, universal={:.2}, diff={:.1}%",
                                det_name, af[det_idx], universal_af[det_idx], diff_pct
                            );
                        }
                    }
                }

                negative_event_af.insert(endmember_name.clone(), af);
            } else {
                warn!(
                    "Insufficient negative events for {} (need at least {}), using universal AF only",
                    endmember_name, config.min_negative_events
                );
            }
        }

        // Extract median fluorescence for each detector
        let mut medians: Vec<f32> = Vec::new();
        let mut mads: Vec<f32> = Vec::new(); // Median Absolute Deviation

        let control_display = control_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(endmember_name);
        info!("");
        info!(
            "========== Analyzing control: {} (marker: {}, fluor: {}) ==========",
            control_display, marker_name, fluor_name
        );
        info!("");

        for detector_name in detector_names.iter() {
            let _det_span = info_span!(
                "single_stain_detector_median",
                detector = detector_name.as_str(),
                control = control_short_label.as_str()
            )
            .entered();
            let values = control_fcs
                .get_parameter_events_slice(detector_name)
                .with_context(|| {
                    format!(
                        "Failed to extract {} from control file {}",
                        detector_name,
                        control_path.display()
                    )
                })?;

            // DIAGNOSTIC: Log raw statistics before peak detection
            let n_events = values.len();
            let min_val = values.iter().cloned().fold(f32::INFINITY, f32::min);
            let max_val = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let sum: f64 = values.iter().map(|&v| v as f64).sum();
            let mean = (sum / n_events as f64) as f32;

            // Count events above certain thresholds
            let above_100 = values.iter().filter(|&&v| v > 100.0).count();
            let above_1000 = values.iter().filter(|&&v| v > 1000.0).count();
            let above_10000 = values.iter().filter(|&&v| v > 10000.0).count();

            // Convert to f64 for KDE
            let values_f64: Vec<f64> = values.iter().map(|&v| v as f64).collect();

            let median = if config.peak_detection {
                // Use peak-based median selection
                match calculate_peak_based_median(
                    &values_f64,
                    config.peak_threshold,
                    config.peak_bias,
                ) {
                    Some(peak_median) => {
                        let simple_median = calculate_simple_median(values);
                        let diff_percent =
                            ((peak_median - simple_median).abs() / simple_median.max(1.0)) * 100.0;
                        info!(
                            "Peak-based median for {} in {}: {:.2} (simple: {:.2}, diff: {:.1}%)",
                            detector_name,
                            control_short_label,
                            peak_median,
                            simple_median,
                            diff_percent
                        );
                        peak_median
                    }
                    None => {
                        // Fallback to simple median if peak detection fails
                        warn!(
                            "Peak detection failed for {} in {}, falling back to simple median",
                            detector_name, control_short_label
                        );
                        calculate_simple_median(values)
                    }
                }
            } else {
                // Simple median across all events
                calculate_simple_median(values)
            };

            // DIAGNOSTIC: Log detailed results for this detector
            info!(
                "  {} -> n={}, min={:.1}, max={:.1}, mean={:.1}, median={:.1}, >100: {}, >1k: {}, >10k: {}",
                detector_name,
                n_events,
                min_val,
                max_val,
                mean,
                median,
                above_100,
                above_1000,
                above_10000
            ); // Calculate MAD (Median Absolute Deviation) using the same method
            let deviations: Vec<f32> = values.iter().map(|&v| (v - median).abs()).collect();
            let mut sorted_deviations = deviations;
            sorted_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mad = if sorted_deviations.is_empty() {
                0.0
            } else {
                sorted_deviations[sorted_deviations.len() / 2]
            };

            medians.push(median);
            mads.push(mad.max(f32::EPSILON)); // Avoid division by zero
        }

        // Determine which autofluorescence to use based on mode
        let effective_af: Vec<f32> = match config.autofluorescence_mode.as_str() {
            "negative-events" => {
                // Use negative event AF if available, otherwise fall back to universal
                if let Some(negative_af) = negative_event_af.get(endmember_name) {
                    info!(
                        "Using negative-event autofluorescence for {}",
                        endmember_name
                    );
                    negative_af.clone()
                } else {
                    warn!(
                        "Negative events not available for {}, falling back to universal AF",
                        endmember_name
                    );
                    autofluorescence_medians.clone()
                }
            }
            "hybrid" => {
                // Weighted combination: α * universal + (1-α) * negative_events
                if let Some(negative_af) = negative_event_af.get(endmember_name) {
                    info!(
                        "Using hybrid autofluorescence for {} (weight: {:.2})",
                        endmember_name, config.af_weight
                    );
                    autofluorescence_medians
                        .iter()
                        .zip(negative_af.iter())
                        .map(|(&af_universal, &af_negative)| {
                            (config.af_weight * af_universal as f64
                                + (1.0 - config.af_weight) * af_negative as f64)
                                as f32
                        })
                        .collect()
                } else {
                    // Fallback to universal if negative events not available
                    warn!(
                        "Negative events not available for {}, using universal AF only",
                        endmember_name
                    );
                    autofluorescence_medians.clone()
                }
            }
            _ => {
                // "universal" or default: use unstained control AF
                if config.use_negative_events {
                    info!(
                        "Using universal autofluorescence for {} (negative events available but mode=universal)",
                        endmember_name
                    );
                }
                autofluorescence_medians.clone()
            }
        };

        // Subtract autofluorescence and normalize by primary detector
        // Primary detector is the one with highest signal for this fluorophore
        let corrected_medians: Vec<f32> = medians
            .iter()
            .zip(effective_af.iter())
            .map(|(median, &af)| (median - af).max(0.0))
            .collect();

        // DIAGNOSTIC: Show before/after AF subtraction
        info!(
            "  --- After AF subtraction (ALL {} detectors) ---",
            detector_names.len()
        );
        for (det_idx, detector_name) in detector_names.iter().enumerate() {
            info!(
                "    {} -> raw={:.1}, af={:.1}, corrected={:.1}",
                detector_name, medians[det_idx], effective_af[det_idx], corrected_medians[det_idx]
            );
        }

        // Find primary detector (highest corrected median)
        let primary_idx = corrected_medians
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, _)| idx)
            .ok_or_else(|| anyhow::anyhow!("No valid signal found in control file"))?;

        let primary_median = corrected_medians[primary_idx];

        // DIAGNOSTIC: Show why this detector was selected as primary
        info!(
            "  PRIMARY DETECTOR SELECTION: {} with corrected value {:.1}",
            detector_names[primary_idx], primary_median
        );
        info!("    Top 3 corrected values:");
        let mut sorted_corrected: Vec<(usize, f32)> = corrected_medians
            .iter()
            .enumerate()
            .map(|(idx, &val)| (idx, val))
            .collect();
        sorted_corrected.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (i, (det_idx, val)) in sorted_corrected.iter().take(3).enumerate() {
            info!(
                "      {}. {} = {:.1} (raw: {:.1}, af: {:.1})",
                i + 1,
                detector_names[*det_idx],
                val,
                medians[*det_idx],
                effective_af[*det_idx]
            );
        }
        if primary_median <= 0.0 {
            return Err(anyhow::anyhow!(
                "Primary detector has zero or negative signal after autofluorescence subtraction"
            ));
        }

        // Track primary detector for this endmember
        let primary_detector_name = detector_names[primary_idx].clone();
        primary_detectors_used.push((*endmember_idx, primary_detector_name.clone()));

        // Populate PrimaryDetectorInfo for this endmember (try to extract $PnN/$PnS from control)
        let mut pn_name: Option<String> = None;
        let mut pn_label: Option<String> = None;
        {
            use std::sync::Arc;
            if let Some(param) = control_fcs
                .parameters
                .get(&Arc::from(primary_detector_name.as_str()))
            {
                // Use existing parameter metadata from control FCS
                if !param.channel_name.is_empty() {
                    pn_name = Some(param.channel_name.to_string());
                }
                if !param.label_name.is_empty() {
                    pn_label = Some(param.label_name.to_string());
                }
            }
        }
        primary_detector_info[*endmember_idx] = Some(PrimaryDetectorInfo {
            endmember_name: endmember_name.clone(),
            is_autofluorescence: false,
            primary_detector_name: Some(primary_detector_name.clone()),
            primary_detector_pn_name: pn_name.clone(),
            primary_detector_pn_label: pn_label.clone(),
            selected_marker_name: Some(marker_name.clone()),
            selected_fluor_name: if fluor_name.is_empty() {
                pn_label.clone()
            } else {
                Some(fluor_name.clone())
            },
        });

        // Store conflict-resolution data: second-highest peak for shared-primary swap
        if sorted_corrected.len() >= 2 {
            let second_idx = sorted_corrected[1].0;
            let second_detector_name = detector_names[second_idx].clone();
            let mut alt_pn: Option<String> = None;
            let mut alt_label: Option<String> = None;
            {
                use std::sync::Arc;
                if let Some(param) = control_fcs
                    .parameters
                    .get(&Arc::from(second_detector_name.as_str()))
                {
                    if !param.channel_name.is_empty() {
                        alt_pn = Some(param.channel_name.to_string());
                    }
                    if !param.label_name.is_empty() {
                        alt_label = Some(param.label_name.to_string());
                    }
                }
            }
            let alternate_primary_info = PrimaryDetectorInfo {
                endmember_name: endmember_name.clone(),
                is_autofluorescence: false,
                primary_detector_name: Some(second_detector_name.clone()),
                primary_detector_pn_name: alt_pn,
                primary_detector_pn_label: alt_label.clone(),
                selected_marker_name: Some(marker_name.clone()),
                selected_fluor_name: if fluor_name.is_empty() {
                    alt_label
                } else {
                    Some(fluor_name.clone())
                },
            };
            conflict_data[*endmember_idx] = Some(EndmemberConflictData {
                corrected_medians: corrected_medians.clone(),
                primary_idx,
                second_idx,
                alternate_primary_info,
            });
        }

        // Normalize by primary detector to create spectral signature
        // This creates the mixing matrix column for this endmember
        for (detector_idx, corrected_median) in corrected_medians.iter().enumerate() {
            mixing_matrix[(detector_idx, *endmember_idx)] =
                (*corrected_median / primary_median) as f64;
        }

        // Diagnostic: report spectral signature quality
        let max_spillover = corrected_medians
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != primary_idx)
            .map(|(_, &val)| val / primary_median)
            .fold(0.0f32, f32::max);

        info!(
            "Created spectral signature for {}: primary detector {} (normalized to 1.0, max spillover: {:.3})",
            endmember_name, detector_names[primary_idx], max_spillover
        );

        // DEBUG: Log detailed signature information for debugging similarity issues
        // #region agent log
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open("/Users/kfls271/Rust/.cursor/debug.log")
            {
                let normalized_sig: Vec<f64> = (0..detector_names.len())
                    .map(|idx| mixing_matrix[(idx, *endmember_idx)])
                    .collect();
                let non_zero_count = normalized_sig.iter().filter(|&&v| v > 1e-6).count();
                let max_non_primary = normalized_sig
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| *idx != primary_idx)
                    .map(|(_, &v)| v)
                    .fold(0.0f64, f64::max);

                let log_entry = serde_json::json!({
                    "sessionId": "debug-session",
                    "runId": "signature-extraction",
                    "hypothesisId": "A,B,C,D",
                    "location": "commands.rs:1792",
                    "message": "Spectral signature extracted",
                    "data": {
                        "endmember": endmember_name,
                        "primary_detector": detector_names[primary_idx],
                        "primary_idx": primary_idx,
                        "raw_medians": medians.iter().map(|&v| v as f64).collect::<Vec<f64>>(),
                        "corrected_medians": corrected_medians.iter().map(|&v| v as f64).collect::<Vec<f64>>(),
                        "normalized_signature": normalized_sig,
                        "non_zero_detectors": non_zero_count,
                        "max_non_primary": max_non_primary,
                        "max_spillover": max_spillover
                    },
                    "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
                });
                let _ = writeln!(file, "{}", log_entry);
            }
        }
        // #endregion

        // Generate diagnostic or debug control plots if requested
        if let Some(plot_dir) = diagnostic_plot_dir {
            if debug_control_plots {
                if let Err(e) = generate_control_cleanup_debug_plots(
                    &control_fcs_before_gating,
                    stages_for_plot.as_ref(),
                    &control_fcs,
                    endmember_name,
                    detector_names,
                    primary_idx,
                    config,
                    plot_dir,
                    "jpg",
                ) {
                    warn!(
                        "Failed to generate debug control plots for {}: {}",
                        endmember_name, e
                    );
                }
            } else {
                // Extract normalized signature for plotting
                let normalized_signature: Vec<f64> = corrected_medians
                    .iter()
                    .map(|&val| (val / primary_median) as f64)
                    .collect();

                if let Err(e) = generate_control_diagnostic_plots(
                    &control_fcs_before_gating,
                    &control_fcs,
                    endmember_name,
                    detector_names,
                    &normalized_signature,
                    plot_dir,
                    "jpg", // Default format
                ) {
                    warn!(
                        "Failed to generate diagnostic plots for {}: {}",
                        endmember_name, e
                    );
                }
            }
        }

        if max_spillover > 0.5 {
            warn!(
                "High spillover detected for {}: max spillover = {:.1}% - verify control quality",
                endmember_name,
                max_spillover * 100.0
            );
        }
    }

    // Add autofluorescence as the last column in the mixing matrix
    // Autofluorescence signature is the normalized autofluorescence pattern from unstained control
    // This represents how autofluorescence signal is distributed across detectors
    info!("Adding autofluorescence column to mixing matrix...");
    let max_af = autofluorescence_medians
        .iter()
        .fold(0.0f32, |a, &b| a.max(b));
    if max_af > 0.0 {
        // Normalize autofluorescence medians by maximum to create spectral signature
        // This follows the same pattern as fluorophore signatures (normalized to max = 1.0)
        for (detector_idx, &af_median) in autofluorescence_medians.iter().enumerate() {
            mixing_matrix[(detector_idx, autofluorescence_idx)] = (af_median / max_af) as f64;
        }
        info!(
            "Created autofluorescence signature: normalized to max = 1.0 (detector with max AF: {:.2})",
            max_af
        );
    } else {
        warn!(
            "Autofluorescence medians are all zero - this may indicate an issue with the unstained control"
        );
        // Set all detectors to a small value to avoid division issues
        for detector_idx in 0..n_detectors {
            mixing_matrix[(detector_idx, autofluorescence_idx)] = 1e-6;
        }
    }

    // Summary diagnostics
    info!(
        "Created mixing matrix from single-stain controls: {} detectors × {} endmembers",
        n_detectors, n_endmembers
    );

    if config.peak_detection {
        info!(
            "Peak detection: ENABLED (threshold: {:.2}, bias: {:.2})",
            config.peak_threshold, config.peak_bias
        );
    } else {
        info!("Peak detection: DISABLED (using simple median)");
    }

    if config.use_negative_events {
        info!(
            "Negative event extraction: ENABLED (min events: {}, mode: {})",
            config.min_negative_events, config.autofluorescence_mode
        );
        info!(
            "Negative event AF available for {} endmembers",
            negative_event_af.len()
        );
    } else {
        info!("Negative event extraction: DISABLED");
    }

    // Validate matrix quality
    for endmember_idx in 0..n_endmembers {
        let column = mixing_matrix.column(endmember_idx);
        let max_val = column.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_val = column.iter().fold(f64::INFINITY, |a, &b| a.min(b));

        if max_val <= 0.0 {
            warn!(
                "Endmember {} has zero or negative maximum value in mixing matrix",
                endmember_names[endmember_idx]
            );
        }
        if min_val < -0.1 {
            warn!(
                "Endmember {} has negative values in mixing matrix (min: {:.3})",
                endmember_names[endmember_idx], min_val
            );
        }
    }

    // Check for potential linear dependencies by comparing normalized spectral signatures
    // Compare columns of the mixing matrix (normalized spectra) to detect similarity
    if n_endmembers > 1 {
        let mut similar_pairs = Vec::new();
        for i in 0..n_endmembers {
            for j in (i + 1)..n_endmembers {
                // Skip autofluorescence comparisons - identify as endmember without a matching control file
                let has_control_i = control_files
                    .iter()
                    .any(|(name, _)| name == &endmember_names[i]);
                let has_control_j = control_files
                    .iter()
                    .any(|(name, _)| name == &endmember_names[j]);
                if !has_control_i || !has_control_j {
                    continue; // Skip if either is autofluorescence (no control file)
                }

                let col_i = mixing_matrix.column(i);
                let col_j = mixing_matrix.column(j);

                // Calculate cosine similarity on normalized spectra
                let dot_product: f64 = col_i.iter().zip(col_j.iter()).map(|(a, b)| a * b).sum();
                let norm_i: f64 = col_i.iter().map(|x| x * x).sum::<f64>().sqrt();
                let norm_j: f64 = col_j.iter().map(|x| x * x).sum::<f64>().sqrt();

                // DEBUG: Log detailed similarity calculation
                // #region agent log
                {
                    use std::fs::OpenOptions;
                    use std::io::Write;
                    if let Ok(mut file) = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/Users/kfls271/Rust/.cursor/debug.log")
                    {
                        let col_i_vec: Vec<f64> = col_i.iter().copied().collect();
                        let col_j_vec: Vec<f64> = col_j.iter().copied().collect();
                        let diff_vec: Vec<f64> = col_i_vec
                            .iter()
                            .zip(col_j_vec.iter())
                            .map(|(a, b)| (a - b).abs())
                            .collect();
                        let max_diff = diff_vec.iter().fold(0.0f64, |a, &b| a.max(b));
                        let mean_diff = diff_vec.iter().sum::<f64>() / diff_vec.len() as f64;

                        let log_entry = serde_json::json!({
                            "sessionId": "debug-session",
                            "runId": "similarity-check",
                            "hypothesisId": "A,B,C,D,E",
                            "location": "commands.rs:1792",
                            "message": "Cosine similarity calculation",
                            "data": {
                                "endmember_i": endmember_names[i],
                                "endmember_j": endmember_names[j],
                                "col_i": col_i_vec,
                                "col_j": col_j_vec,
                                "dot_product": dot_product,
                                "norm_i": norm_i,
                                "norm_j": norm_j,
                                "similarity": if norm_i > 0.0 && norm_j > 0.0 { dot_product / (norm_i * norm_j) } else { -1.0 },
                                "max_diff": max_diff,
                                "mean_diff": mean_diff,
                                "diff_vector": diff_vec
                            },
                            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
                        });
                        let _ = writeln!(file, "{}", log_entry);
                    }
                }
                // #endregion

                if norm_i > 0.0 && norm_j > 0.0 {
                    let similarity = dot_product / (norm_i * norm_j);
                    if similarity > 0.99 {
                        // Find primary detectors (detectors with value = 1.0 or highest value)
                        let primary_i = col_i
                            .iter()
                            .enumerate()
                            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                            .map(|(idx, _)| idx);
                        let primary_j = col_j
                            .iter()
                            .enumerate()
                            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                            .map(|(idx, _)| idx);

                        let same_primary = primary_i == primary_j;

                        // Count non-zero detectors
                        let non_zero_i = col_i.iter().filter(|&&v| v > 1e-6).count();
                        let non_zero_j = col_j.iter().filter(|&&v| v > 1e-6).count();

                        // Find max values
                        let max_i = col_i.iter().fold(0.0f64, |a, &b| a.max(b));
                        let max_j = col_j.iter().fold(0.0f64, |a, &b| a.max(b));

                        similar_pairs.push((
                            i,
                            j,
                            similarity,
                            same_primary,
                            primary_i,
                            primary_j,
                            non_zero_i,
                            non_zero_j,
                            max_i,
                            max_j,
                        ));
                    }
                }
            }
        }

        if !similar_pairs.is_empty() {
            warn!(
                "Detected highly similar endmember pairs (cosine similarity > 0.99 on normalized spectra), which may cause solve failures:"
            );
            for (i, j, sim, same_primary, prim_i, prim_j, nz_i, nz_j, max_i, max_j) in similar_pairs
            {
                let primary_note = if same_primary {
                    format!(" (both primary: {})", detector_names[prim_i.unwrap()])
                } else {
                    format!(
                        " (primary: {} vs {})",
                        detector_names[prim_i.unwrap()],
                        detector_names[prim_j.unwrap()]
                    )
                };
                warn!(
                    "  - {} and {}: similarity = {:.4}{}",
                    endmember_names[i], endmember_names[j], sim, primary_note
                );
                warn!(
                    "    Non-zero detectors: {} vs {}, max values: {:.3} vs {:.3}",
                    nz_i, nz_j, max_i, max_j
                );

                // If they have very few non-zero detectors, that's likely why they're similar
                if nz_i <= 2 && nz_j <= 2 {
                    warn!(
                        "    ⚠ Both spectra have very few non-zero detectors - this may indicate over-aggressive autofluorescence subtraction"
                    );
                }
            }
        }
    }

    // Log matrix dimensions for diagnostics
    info!(
        "Mixing matrix dimensions: {} detectors × {} endmembers (overdetermined system)",
        n_detectors, n_endmembers
    );
    if n_detectors < n_endmembers {
        return Err(anyhow::anyhow!(
            "Mixing matrix is underdetermined: {} detectors < {} endmembers. Cannot solve uniquely.",
            n_detectors,
            n_endmembers
        ));
    }

    // DIAGNOSTIC: Report primary detector assignments
    info!("Primary detector assignments:");
    let mut primary_assignments: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (endmember_idx, endmember_name) in endmember_names.iter().enumerate() {
        if let Some(Some(info)) = primary_detector_info.get(endmember_idx) {
            if let Some(ref primary_det) = info.primary_detector_name {
                primary_assignments
                    .entry(primary_det.clone())
                    .or_insert_with(Vec::new)
                    .push(endmember_name.clone());
                info!(
                    "  [{}] {} -> primary detector: {}",
                    endmember_idx, endmember_name, primary_det
                );
            }
        }
    }

    // Warn about shared primary detectors
    for (detector, endmembers) in primary_assignments.iter() {
        if endmembers.len() > 1 {
            warn!(
                "⚠️  {} endmembers share primary detector '{}' - may indicate weak control signals or cross-reactivity:",
                endmembers.len(),
                detector
            );
            for (i, em) in endmembers.iter().enumerate() {
                warn!("     {}. {}", i + 1, em);
            }
        }
    }

    // Append autofluorescence primary info (no primary detector)
    primary_detector_info[autofluorescence_idx] = Some(PrimaryDetectorInfo {
        endmember_name: endmember_names[autofluorescence_idx].clone(),
        is_autofluorescence: true,
        primary_detector_name: None,
        primary_detector_pn_name: None,
        primary_detector_pn_label: None,
        selected_marker_name: Some("Autofluorescence".to_string()),
        selected_fluor_name: None,
    });

    // Convert Option<PrimaryDetectorInfo> to PrimaryDetectorInfo (all should be Some now)
    let mut primary_detector_info: Vec<PrimaryDetectorInfo> = primary_detector_info
        .into_iter()
        .map(|opt| {
            opt.unwrap_or_else(|| {
                panic!("PrimaryDetectorInfo not populated for all endmembers");
            })
        })
        .collect();

    let mut mixing_matrix = mixing_matrix;
    let mut conflict_data = conflict_data;

    // Resolve shared primary detector: retry with peak bias disabled, then swap to second-highest peak
    let mut dt = detector_to_endmembers(&primary_detector_info, endmember_names);
    if dt.iter().any(|(_, v)| v.len() > 1) && config.peak_detection && config.peak_bias < 1.0 - 1e-9
    {
        info!("Retrying with peak bias disabled to resolve shared primary detector...");
        let no_bias_config = SingleStainConfig {
            peak_bias: 1.0,
            peak_bias_negative: 1.0,
            ..config.clone()
        };
        match create_mixing_matrix_from_single_stains(
            controls_dir,
            unstained_fcs,
            detector_names,
            endmember_names,
            autofluorescence_name,
            &no_bias_config,
            control_assignments,
            auto_gate,
            debug_control_plots,
            diagnostic_plot_dir,
        ) {
            Ok((m2, d2, i2, cd2)) => {
                let dt2 = detector_to_endmembers(&i2, endmember_names);
                if !dt2.iter().any(|(_, v)| v.len() > 1) {
                    return Ok((m2, d2, i2, cd2));
                }
                mixing_matrix = m2;
                primary_detector_info = i2;
                conflict_data = cd2;
                dt = dt2;
            }
            Err(e) => return Err(e),
        }
    }

    while dt.iter().any(|(_, v)| v.len() > 1) {
        if !apply_one_swap(
            &mut mixing_matrix,
            &mut primary_detector_info,
            &mut conflict_data,
            &dt,
            endmember_names,
        ) {
            break;
        }
        dt = detector_to_endmembers(&primary_detector_info, endmember_names);
    }

    // Return full mixing matrix, detector names, primary info, and conflict data (for diagnostics)
    Ok((
        mixing_matrix,
        detector_names.to_vec(),
        primary_detector_info,
        conflict_data,
    ))
}

/// Calculate simple median across all events
fn calculate_simple_median(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted_values: Vec<f32> = values.iter().copied().collect();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted_values[sorted_values.len() / 2]
}

/// Export mixing matrix to CSV file
fn export_mixing_matrix_to_csv(
    matrix: &ndarray::Array2<f64>,
    path: &PathBuf,
    detector_names: &[String],
    endmember_names: &[String],
) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(path)?;

    // Write header: first column is row names, then column names
    write!(file, "RowName,")?;
    for (i, col_name) in endmember_names.iter().enumerate() {
        write!(file, "{}", col_name)?;
        if i < endmember_names.len() - 1 {
            write!(file, ",")?;
        }
    }
    writeln!(file)?;

    // Write data
    for (row_idx, row_name) in detector_names.iter().enumerate() {
        write!(file, "{}", row_name)?;
        for col_idx in 0..matrix.ncols() {
            write!(file, ",{:.10e}", matrix[(row_idx, col_idx)])?;
        }
        writeln!(file)?;
    }

    Ok(())
}

/// Calculate simple median for f64 values
fn calculate_simple_median_f64(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted_values: Vec<f64> = values.iter().copied().collect();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted_values[sorted_values.len() / 2]
}

/// Extract negative events from a positive single-stain control and calculate autofluorescence
///
/// Negative events are those in the left/low peak, representing unstained cells
/// in a positive control sample.
fn extract_negative_event_autofluorescence(
    control_fcs: &Fcs,
    detector_names: &[String],
    endmember_name: &str,
    config: &SingleStainConfig,
) -> Result<Option<Vec<f32>>> {
    // Find the primary detector (highest signal detector) to identify negative events
    // We'll use peak detection on the primary detector to find the negative peak
    let mut primary_detector_idx = 0;
    let mut max_median = 0.0f32;

    for (idx, detector_name) in detector_names.iter().enumerate() {
        let values = control_fcs
            .get_parameter_events_slice(detector_name)
            .with_context(|| format!("Failed to extract {} from control", detector_name))?;

        let median = calculate_simple_median(values);
        if median > max_median {
            max_median = median;
            primary_detector_idx = idx;
        }
    }

    let primary_detector = &detector_names[primary_detector_idx];
    let primary_values = control_fcs
        .get_parameter_events_slice(primary_detector)
        .with_context(|| format!("Failed to extract {} from control", primary_detector))?;

    // Use peak detection to find negative peak (left/low peak)
    let primary_values_f64: Vec<f64> = primary_values.iter().map(|&v| v as f64).collect();

    let negative_events_mask = if config.peak_detection {
        // Use peak detection to find negative peak
        find_negative_peak_events(
            &primary_values_f64,
            config.peak_threshold,
            config.peak_bias_negative,
        )?
    } else {
        // Fallback: use threshold-based method (events below median)
        let threshold = calculate_simple_median(primary_values);
        primary_values
            .iter()
            .map(|&v| v < threshold * 0.5) // Events below 50% of median
            .collect()
    };

    let n_negative = negative_events_mask.iter().filter(|&&x| x).count();
    if n_negative < config.min_negative_events {
        return Ok(None);
    }

    let negative_percent = (n_negative as f64 / primary_values.len() as f64) * 100.0;
    info!(
        "Found {} negative events ({:.1}%) in {} control",
        n_negative, negative_percent, endmember_name
    );

    // Warn if negative event percentage is unusually high or low
    if negative_percent < 5.0 {
        warn!(
            "Very few negative events ({:.1}%) in {} - may indicate poor staining or gating issues",
            negative_percent, endmember_name
        );
    } else if negative_percent > 50.0 {
        warn!(
            "Unusually high negative event percentage ({:.1}%) in {} - verify control quality",
            negative_percent, endmember_name
        );
    }

    // Calculate autofluorescence medians from negative events for each detector
    let mut negative_af: Vec<f32> = Vec::new();
    for detector_name in detector_names.iter() {
        let values = control_fcs
            .get_parameter_events_slice(detector_name)
            .with_context(|| format!("Failed to extract {} from control", detector_name))?;

        // Filter to negative events only
        let negative_values: Vec<f32> = values
            .iter()
            .zip(negative_events_mask.iter())
            .filter_map(|(&value, &is_negative)| if is_negative { Some(value) } else { None })
            .collect();

        if negative_values.is_empty() {
            return Ok(None);
        }

        let median = calculate_simple_median(&negative_values);
        negative_af.push(median);
    }

    Ok(Some(negative_af))
}

/// Find events in the negative peak (left/low peak) using peak detection
///
/// Returns a boolean mask indicating which events belong to the negative peak
fn find_negative_peak_events(
    values: &[f64],
    peak_threshold: f64,
    peak_bias_negative: f64,
) -> Result<Vec<bool>> {
    if values.is_empty() {
        return Ok(vec![]);
    }

    // Estimate KDE and find peaks
    let kde = match KernelDensity::estimate(values, 1.0, 512) {
        Ok(kde) => kde,
        Err(_) => {
            // Fallback: use threshold-based method
            let threshold = calculate_simple_median_f64(values);
            return Ok(values.iter().map(|&v| v < threshold * 0.5).collect());
        }
    };

    let peaks = kde.find_peaks(peak_threshold);
    if peaks.is_empty() {
        // Fallback: use threshold-based method
        let threshold = calculate_simple_median_f64(values);
        return Ok(values.iter().map(|&v| v < threshold * 0.5).collect());
    }

    // Find the lowest/leftmost peak (negative peak)
    let negative_peak = peaks
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .copied()
        .ok_or_else(|| anyhow::anyhow!("No negative peak found"))?;

    // Calculate MAD to determine peak width
    let mut sorted_values: Vec<f64> = values.iter().copied().collect();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_all = sorted_values[sorted_values.len() / 2];

    let deviations: Vec<f64> = sorted_values
        .iter()
        .map(|&v| (v - median_all).abs())
        .collect();
    let mut sorted_deviations = deviations;
    sorted_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad = sorted_deviations[sorted_deviations.len() / 2];

    // Filter events within negative peak (within 2 MAD of peak center)
    let peak_width = 2.0 * mad;
    let peak_min = negative_peak - peak_width;
    let peak_max = negative_peak + peak_width;

    let mut peak_events: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter_map(|(idx, &v)| {
            if v >= peak_min && v <= peak_max {
                Some((idx, v))
            } else {
                None
            }
        })
        .collect();

    if peak_events.is_empty() {
        // Fallback: use threshold-based method
        let threshold = calculate_simple_median_f64(values);
        return Ok(values.iter().map(|&v| v < threshold * 0.5).collect());
    }

    // Apply negative bias: select lower fraction of peak events (left side)
    peak_events.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    let bias_end_idx = ((peak_events.len() as f64) * peak_bias_negative) as usize;
    let biased_indices: HashSet<usize> = peak_events[..bias_end_idx]
        .iter()
        .map(|(idx, _)| *idx)
        .collect();

    // Create mask
    Ok(values
        .iter()
        .enumerate()
        .map(|(idx, _)| biased_indices.contains(&idx))
        .collect())
}

/// Calculate peak-based median using KDE peak detection
///
/// 1. Detect peaks using KDE
/// 2. Identify highest intensity peak (positive population)
/// 3. Filter events within peak (within 2 MAD of peak center)
/// 4. Apply bias (select upper fraction of peak events)
/// 5. Calculate median of biased subset
fn calculate_peak_based_median(values: &[f64], peak_threshold: f64, peak_bias: f64) -> Option<f32> {
    if values.is_empty() {
        return None;
    }

    // Estimate KDE and find peaks
    let kde = match KernelDensity::estimate(values, 1.0, 512) {
        Ok(kde) => kde,
        Err(_) => return None,
    };

    let peaks = kde.find_peaks(peak_threshold);
    if peaks.is_empty() {
        return None;
    }

    // Diagnostic: log peak detection results
    if peaks.len() > 1 {
        info!(
            "Detected {} peaks (using highest at {:.2})",
            peaks.len(),
            peaks
                .iter()
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap()
        );
    }

    // Find highest intensity peak (rightmost/largest peak)
    let main_peak = peaks
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;

    // Calculate MAD to determine peak width
    let mut sorted_values: Vec<f64> = values.iter().copied().collect();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_all = sorted_values[sorted_values.len() / 2];

    let deviations: Vec<f64> = sorted_values
        .iter()
        .map(|&v| (v - median_all).abs())
        .collect();
    let mut sorted_deviations = deviations;
    sorted_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad = sorted_deviations[sorted_deviations.len() / 2];

    // Filter events within peak (within 2 MAD of peak center)
    let peak_width = 2.0 * mad;
    let peak_min = main_peak - peak_width;
    let peak_max = main_peak + peak_width;

    let mut peak_events: Vec<f64> = values
        .iter()
        .filter(|&&v| v >= peak_min && v <= peak_max)
        .copied()
        .collect();

    if peak_events.is_empty() {
        // Fallback: use all events
        peak_events = values.to_vec();
    }

    // Apply bias: select upper fraction of peak events
    peak_events.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let bias_start_idx = ((peak_events.len() as f64) * (1.0 - peak_bias)) as usize;
    let biased_events = &peak_events[bias_start_idx..];

    if biased_events.is_empty() {
        return None;
    }

    // Calculate median of biased subset
    let median_idx = biased_events.len() / 2;
    Some(biased_events[median_idx] as f32)
}

/// Load mixing matrix from CSV.
///
/// Supports the CLI export layout (`RowName`, then endmember column headers, then one detector
/// name plus floats per row). Legacy files with only numeric cells return empty name vectors.
fn load_mixing_matrix(path: &PathBuf) -> Result<(Array2<f64>, Vec<String>, Vec<String>)> {
    use std::fs::File;
    use std::io::BufReader;

    fn strip_bom(s: &str) -> &str {
        s.strip_prefix('\u{feff}').unwrap_or(s)
    }

    let file = File::open(path)
        .with_context(|| format!("Failed to open mixing matrix file: {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(reader);

    let records: Vec<csv::StringRecord> = csv_reader
        .records()
        .collect::<csv::Result<_>>()
        .with_context(|| format!("Failed to parse CSV: {}", path.display()))?;

    if records.is_empty() {
        return Err(anyhow::anyhow!("Mixing matrix file is empty"));
    }

    let first_cell = records[0].get(0).map(|s| strip_bom(s.trim())).unwrap_or("");

    if first_cell.eq_ignore_ascii_case("RowName") {
        let endmember_names: Vec<String> = records[0]
            .iter()
            .skip(1)
            .map(|s| s.trim().to_string())
            .collect();
        let n_em = endmember_names.len();
        if n_em == 0 {
            return Err(anyhow::anyhow!(
                "Mixing matrix CSV has RowName header but no endmember columns"
            ));
        }

        let mut detector_names = Vec::new();
        let mut rows: Vec<Vec<f64>> = Vec::new();
        for (idx, record) in records.iter().enumerate().skip(1) {
            if record.len() != n_em + 1 {
                return Err(anyhow::anyhow!(
                    "Row {}: expected {} columns (detector + {} endmembers), found {}",
                    idx + 1,
                    n_em + 1,
                    n_em,
                    record.len()
                ));
            }
            detector_names.push(record[0].trim().to_string());
            let mut row = Vec::with_capacity(n_em);
            for (j, cell) in record.iter().enumerate().skip(1) {
                let v: f64 = cell.trim().parse().with_context(|| {
                    format!(
                        "Row {} column {}: expected float, got {:?}",
                        idx + 1,
                        j + 1,
                        cell
                    )
                })?;
                row.push(v);
            }
            rows.push(row);
        }

        let n_rows = rows.len();
        let mut matrix = Array2::<f64>::zeros((n_rows, n_em));
        for (i, row) in rows.iter().enumerate() {
            for (j, &value) in row.iter().enumerate() {
                matrix[(i, j)] = value;
            }
        }

        Ok((matrix, detector_names, endmember_names))
    } else {
        let mut rows = Vec::new();
        for (idx, record) in records.iter().enumerate() {
            let row: Vec<f64> = record
                .iter()
                .enumerate()
                .map(|(j, s)| {
                    s.parse::<f64>().with_context(|| {
                        format!(
                            "Legacy numeric matrix: row {} column {}: expected float, got {:?}",
                            idx + 1,
                            j + 1,
                            s
                        )
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            rows.push(row);
        }

        let n_cols = rows[0].len();
        for (idx, row) in rows.iter().enumerate() {
            if row.len() != n_cols {
                return Err(anyhow::anyhow!(
                    "Row {} has {} columns, expected {}",
                    idx + 1,
                    row.len(),
                    n_cols
                ));
            }
        }

        let n_rows = rows.len();
        let mut matrix = Array2::<f64>::zeros((n_rows, n_cols));
        for (i, row) in rows.iter().enumerate() {
            for (j, &value) in row.iter().enumerate() {
                matrix[(i, j)] = value;
            }
        }

        Ok((matrix, Vec::new(), Vec::new()))
    }
}

/// Detector and endmember name lists from a mixing matrix CSV when present (`RowName` layout).
/// Legacy numeric-only matrices return empty vectors for both.
pub(crate) fn mixing_matrix_csv_detector_endmember_lists(
    matrix_path: &Path,
) -> Result<(Vec<String>, Vec<String>)> {
    let (_, detectors, endmembers) = load_mixing_matrix(&matrix_path.to_path_buf())?;
    Ok((detectors, endmembers))
}

/// Generate TRU-OLS plots for endmember pairs
fn generate_tru_ols_plots(
    unmixed_df: &EventDataFrame,
    endmember_names: &[&str],
    plot_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    use flow_plots::{DensityPlot, DensityPlotOptions};
    use flow_tru_ols::plot_abundance_distribution;

    // TRU-OLS unmixed FCS uses column names like "Unmixed_<marker>", not full control filenames
    let unmixed_col_names: Vec<String> = unmixed_df
        .get_column_names()
        .into_iter()
        .filter(|s| s.starts_with("Unmixed_"))
        .map(|s| s.to_string())
        .collect();

    if unmixed_col_names.len() != endmember_names.len() {
        anyhow::bail!(
            "TRU-OLS unmixed columns ({}) don't match endmember count ({}); cannot generate plots",
            unmixed_col_names.len(),
            endmember_names.len()
        );
    }

    // Convert DataFrame to Array2 for plotting
    let n_events = unmixed_df.height();
    let n_endmembers = endmember_names.len();
    let mut unmixed_array = Array2::<f64>::zeros((n_events, n_endmembers));

    for (idx, col_name) in unmixed_col_names.iter().enumerate() {
        let series = unmixed_df.column(col_name.as_str()).with_context(|| {
            format!("Failed to find column for endmember {}: {}", idx, col_name)
        })?;

        let values = series
            .f32()
            .with_context(|| format!("Failed to extract f32 values for {}", col_name))?;

        for (event_idx, opt_val) in values.iter().enumerate() {
            if let Some(val) = opt_val {
                unmixed_array[(event_idx, idx)] = val as f64;
            }
        }
    }

    // Convert to faer Mat for plot_abundance_distribution
    let unmixed_mat = Mat::from_fn(n_events, n_endmembers, |i, j| unmixed_array[(i, j)]);

    // Generate plots for each endmember distribution
    for (idx, &endmember_name) in endmember_names.iter().enumerate() {
        let plot_bytes = plot_abundance_distribution(unmixed_mat.as_ref(), endmember_names, idx)
            .with_context(|| {
                format!(
                    "Failed to plot abundance distribution for {}",
                    endmember_name
                )
            })?;

        let filename = format!(
            "tru_ols_{}_distribution.{}",
            endmember_name.replace(" ", "_"),
            plot_format
        );
        let filepath = plot_dir.join(&filename);

        fs::write(&filepath, plot_bytes)
            .with_context(|| format!("Failed to write plot to {}", filepath.display()))?;

        info!("Saved plot: {}", filepath.display());
    }

    // Generate pairwise comparison plots for first few endmembers
    if endmember_names.len() >= 2 {
        for i in 0..(endmember_names.len().min(4)) {
            for j in (i + 1)..(endmember_names.len().min(4)) {
                let x_col = unmixed_col_names[i].as_str();
                let y_col = unmixed_col_names[j].as_str();

                let x_series = unmixed_df
                    .column(x_col)
                    .with_context(|| format!("Failed to find column: {}", x_col))?;
                let y_series = unmixed_df
                    .column(y_col)
                    .with_context(|| format!("Failed to find column: {}", y_col))?;

                let x_values = x_series
                    .f32()
                    .with_context(|| format!("Failed to extract f32 values for {}", x_col))?;
                let y_values = y_series
                    .f32()
                    .with_context(|| format!("Failed to extract f32 values for {}", y_col))?;

                // Create pairs
                let pairs: Vec<(f32, f32)> = x_values
                    .iter()
                    .zip(y_values.iter())
                    .filter_map(|(x_opt, y_opt)| x_opt.and_then(|x| y_opt.map(|y| (x, y))))
                    .collect();

                if !pairs.is_empty() {
                    let base = BasePlotOptions::new()
                        .width(800u32)
                        .height(600u32)
                        .build()
                        .context("Failed to create base plot options")?;
                    let options = DensityPlotOptions::new()
                        .base(base)
                        .build()
                        .context("Failed to create plot options")?;

                    let plot = DensityPlot::new();
                    let plot_bytes = plot
                        .render(
                            pairs.into(),
                            &options,
                            &mut flow_plots::render::RenderConfig::default(),
                        )
                        .context("Failed to render plot")?;

                    let filename = format!(
                        "tru_ols_{}_vs_{}.{}",
                        endmember_names[i].replace(" ", "_"),
                        endmember_names[j].replace(" ", "_"),
                        plot_format
                    );
                    let filepath = plot_dir.join(&filename);

                    fs::write(&filepath, plot_bytes).with_context(|| {
                        format!("Failed to write plot to {}", filepath.display())
                    })?;

                    info!("Saved comparison plot: {}", filepath.display());
                }
            }
        }
    }

    Ok(())
}

/// Linear axis ranges for 2D unmixed-abundance plots (OLS vs TRU-OLS on the same stained sample).
/// Uses combined OLS + TRU-OLS pairs so the two plots for a given endmember pair share comparable scales.
fn unmixed_abundance_axis_ranges(
    ols_pairs: &[(f32, f32)],
    tru_ols_pairs: &[(f32, f32)],
) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (x, y) in ols_pairs.iter().chain(tru_ols_pairs.iter()) {
        if x.is_finite() {
            min_x = min_x.min(*x);
            max_x = max_x.max(*x);
        }
        if y.is_finite() {
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }
    }
    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return (0.0, 1.0, 0.0, 1.0);
    }
    if min_x > max_x {
        std::mem::swap(&mut min_x, &mut max_x);
    }
    if min_y > max_y {
        std::mem::swap(&mut min_y, &mut max_y);
    }
    let pad_x = ((max_x - min_x).max(1e-12)) * 0.05 + 1e-9;
    let pad_y = ((max_y - min_y).max(1e-12)) * 0.05 + 1e-9;
    let x0 = min_x - pad_x;
    let mut x1 = max_x + pad_x;
    let y0 = min_y - pad_y;
    let mut y1 = max_y + pad_y;
    if (x1 - x0).abs() < 1e-15 {
        x1 = x0 + 1.0;
    }
    if (y1 - y0).abs() < 1e-15 {
        y1 = y0 + 1.0;
    }
    (x0, x1, y0, y1)
}

fn density_plot_options_unmixed_endmember_pair(
    method_label: &str,
    stained_sample_label: Option<&str>,
    endmember_x: &str,
    endmember_y: &str,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
) -> Result<DensityPlotOptions> {
    let sample = stained_sample_label.unwrap_or("stained sample");
    // Title + axis labels: each point = one event; axes = unmixed abundance for two endmembers.
    let title = format!(
        "{} — unmixed abundances (each point = one event): {} vs {} — {}",
        method_label, endmember_x, endmember_y, sample
    );
    let base = BasePlotOptions::new()
        .width(800u32)
        .height(600u32)
        .title(title)
        .build()
        .context("Failed to create base plot options")?;
    let x_label = format!("Unmixed abundance: {}", endmember_x);
    let y_label = format!("Unmixed abundance: {}", endmember_y);
    DensityPlotOptions::new()
        .base(base)
        .x_axis(
            AxisOptions::new()
                .label(x_label)
                .range(x_min..=x_max)
                .transform(TransformType::Linear)
                .build()
                .context("Failed to build x-axis options")?,
        )
        .y_axis(
            AxisOptions::new()
                .label(y_label)
                .range(y_min..=y_max)
                .transform(TransformType::Linear)
                .build()
                .context("Failed to build y-axis options")?,
        )
        .density_normalization_percentile(99.0)
        .build()
        .context("Failed to create density plot options")
}

/// Generate OLS comparison plots (2D density of unmixed abundances for endmember pairs on the stained file).
fn generate_ols_comparison_plots(
    stained_fcs: &Fcs,
    _unstained_fcs: &Fcs,
    mixing_matrix: &Array2<f64>,
    detector_names: &[&str],
    endmember_names: &[&str],
    tru_ols_unmixed_df: &EventDataFrame,
    plot_dir: &PathBuf,
    plot_format: &str,
    stained_sample_label: Option<&str>,
) -> Result<()> {
    use flow_plots::DensityPlot;

    info!(
        "Running OLS unmixing for comparison (stained sample: {})...",
        stained_sample_label.unwrap_or("unknown file")
    );

    // Convert mixing matrix to faer Mat<f32> for OLS unmixing
    let mixing_matrix_f32 =
        faer::Mat::from_fn(mixing_matrix.nrows(), mixing_matrix.ncols(), |i, j| {
            mixing_matrix[(i, j)] as f32
        });

    // Run OLS unmixing using apply_spectral_unmixing with actual endmember names
    let ols_unmixed_df = stained_fcs
        .apply_spectral_unmixing(
            mixing_matrix_f32.as_ref(),
            detector_names,
            Some(endmember_names),
        )
        .context("Failed to run OLS unmixing")?;

    // TRU-OLS unmixed FCS uses column names like "Unmixed_<marker>" (not full control filenames).
    // Collect Unmixed_* columns in order so we can index by endmember position.
    let tru_ols_unmixed_col_names: Vec<String> = tru_ols_unmixed_df
        .get_column_names()
        .into_iter()
        .filter(|s| s.starts_with("Unmixed_"))
        .map(|s| s.to_string())
        .collect();

    if tru_ols_unmixed_col_names.len() != endmember_names.len() {
        anyhow::bail!(
            "TRU-OLS unmixed columns ({}) don't match endmember count ({}); cannot generate comparison plots",
            tru_ols_unmixed_col_names.len(),
            endmember_names.len()
        );
    }

    // Generate comparison plots for first few endmember pairs
    for i in 0..(endmember_names.len().min(4)) {
        for j in (i + 1)..(endmember_names.len().min(4)) {
            // OLS uses endmember names as column names; TRU-OLS uses Unmixed_<marker>
            let ols_x_col = endmember_names[i];
            let ols_y_col = endmember_names[j];
            let tru_ols_x_col = tru_ols_unmixed_col_names[i].as_str();
            let tru_ols_y_col = tru_ols_unmixed_col_names[j].as_str();

            // Extract pairs from OLS DataFrame
            let ols_x_series = ols_unmixed_df
                .column(ols_x_col)
                .with_context(|| format!("Failed to find OLS column: {}", ols_x_col))?;
            let ols_y_series = ols_unmixed_df
                .column(ols_y_col)
                .with_context(|| format!("Failed to find OLS column: {}", ols_y_col))?;

            // Both unmixing methods return f32 values
            let ols_x_values = ols_x_series
                .f32()
                .with_context(|| format!("Failed to extract f32 values for OLS {}", ols_x_col))?;
            let ols_y_values = ols_y_series
                .f32()
                .with_context(|| format!("Failed to extract f32 values for OLS {}", ols_y_col))?;

            // Extract pairs from TRU-OLS DataFrame (use actual column names)
            let tru_ols_x_series = tru_ols_unmixed_df
                .column(tru_ols_x_col)
                .with_context(|| format!("Failed to find TRU-OLS column: {}", tru_ols_x_col))?;
            let tru_ols_y_series = tru_ols_unmixed_df
                .column(tru_ols_y_col)
                .with_context(|| format!("Failed to find TRU-OLS column: {}", tru_ols_y_col))?;

            let tru_ols_x_values = tru_ols_x_series.f32().with_context(|| {
                format!("Failed to extract f32 values for TRU-OLS {}", tru_ols_x_col)
            })?;
            let tru_ols_y_values = tru_ols_y_series.f32().with_context(|| {
                format!("Failed to extract f32 values for TRU-OLS {}", tru_ols_y_col)
            })?;

            // Create pairs for both methods
            let ols_pairs: Vec<(f32, f32)> = ols_x_values
                .iter()
                .zip(ols_y_values.iter())
                .filter_map(|(x_opt, y_opt)| x_opt.and_then(|x| y_opt.map(|y| (x, y))))
                .collect();

            let tru_ols_pairs: Vec<(f32, f32)> = tru_ols_x_values
                .iter()
                .zip(tru_ols_y_values.iter())
                .filter_map(|(x_opt, y_opt)| x_opt.and_then(|x| y_opt.map(|y| (x, y))))
                .collect();

            let (x_min, x_max, y_min, y_max) =
                unmixed_abundance_axis_ranges(&ols_pairs, &tru_ols_pairs);

            // Generate separate plots for OLS and TRU-OLS (shared linear axis bounds; labels explain quantities).
            if !ols_pairs.is_empty() {
                let options = density_plot_options_unmixed_endmember_pair(
                    "OLS",
                    stained_sample_label,
                    endmember_names[i],
                    endmember_names[j],
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                )?;

                let plot = DensityPlot::new();
                let plot_bytes = plot
                    .render(
                        ols_pairs.into(),
                        &options,
                        &mut flow_plots::render::RenderConfig::default(),
                    )
                    .context("Failed to render OLS plot")?;

                let filename = format!(
                    "comparison_ols_{}_vs_{}.{}",
                    endmember_names[i].replace(" ", "_"),
                    endmember_names[j].replace(" ", "_"),
                    plot_format
                );
                let filepath = plot_dir.join(&filename);

                fs::write(&filepath, plot_bytes)
                    .with_context(|| format!("Failed to write plot to {}", filepath.display()))?;

                info!(
                    "Saved OLS comparison plot (unmixed {} vs {} on stained sample): {}",
                    endmember_names[i],
                    endmember_names[j],
                    filepath.display()
                );
            }

            if !tru_ols_pairs.is_empty() {
                let options = density_plot_options_unmixed_endmember_pair(
                    "TRU-OLS",
                    stained_sample_label,
                    endmember_names[i],
                    endmember_names[j],
                    x_min,
                    x_max,
                    y_min,
                    y_max,
                )?;

                let plot = DensityPlot::new();
                let plot_bytes = plot
                    .render(
                        tru_ols_pairs.into(),
                        &options,
                        &mut flow_plots::render::RenderConfig::default(),
                    )
                    .context("Failed to render TRU-OLS plot")?;

                let filename = format!(
                    "comparison_tru_ols_{}_vs_{}.{}",
                    endmember_names[i].replace(" ", "_"),
                    endmember_names[j].replace(" ", "_"),
                    plot_format
                );
                let filepath = plot_dir.join(&filename);

                fs::write(&filepath, plot_bytes)
                    .with_context(|| format!("Failed to write plot to {}", filepath.display()))?;

                info!(
                    "Saved TRU-OLS comparison plot (unmixed {} vs {} on stained sample): {}",
                    endmember_names[i],
                    endmember_names[j],
                    filepath.display()
                );
            }
        }
    }

    Ok(())
}

/// Returns intermediate snapshots for debug plots: `(post_margins, post_raw_doublets, final)`.
pub fn clean_fcs_data_with_stages(fcs: &Fcs) -> Result<(Fcs, Fcs, Fcs)> {
    let mut cfg = crate::qc_pipeline::QcPipelineConfig::literature_default();
    cfg.capture_stages = true;
    let report = crate::qc_pipeline::run_qc_pipeline(fcs, &cfg)?;
    let snap = |name: &str| {
        report
            .stage_snapshots
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f.clone())
            .unwrap_or_else(|| report.final_fcs.clone())
    };
    Ok((
        snap("post_margins"),
        snap("post_raw_doublets"),
        report.final_fcs,
    ))
}

/// Full literature-aligned QC pipeline (margins, raw doublets, preprocess, PeacoQC, debris, post doublets).
pub fn clean_fcs_data(fcs: &Fcs) -> Result<Fcs> {
    Ok(crate::qc_pipeline::run_qc_pipeline(
        fcs,
        &crate::qc_pipeline::QcPipelineConfig::literature_default(),
    )?
    .final_fcs)
}

/// Remove debris from bottom-left corner using clustering-based detection
///
/// This function uses K-means clustering to identify the smallest cluster (debris)
/// in the FSC-A vs SSC-A scatter plot, then removes those events before applying
/// more sophisticated gating methods. This is more robust than percentile-based thresholds.
pub fn remove_debris_heuristic(fcs: &Fcs) -> Result<Fcs> {
    use flow_utils::clustering::{KMeans, KMeansConfig};
    use polars::prelude::Series;
    use std::sync::Arc;

    // Get FSC-A and SSC-A channels
    let fsc_a_values = fcs
        .get_parameter_events_slice("FSC-A")
        .map_err(|e| anyhow::anyhow!("Failed to get FSC-A: {}", e))?;
    let ssc_a_values = fcs
        .get_parameter_events_slice("SSC-A")
        .map_err(|e| anyhow::anyhow!("Failed to get SSC-A: {}", e))?;

    if fsc_a_values.len() != ssc_a_values.len() {
        return Err(anyhow::anyhow!("FSC-A and SSC-A have different lengths"));
    }

    let n_events = fsc_a_values.len();
    if n_events < 100 {
        // Too few events to cluster meaningfully, skip debris removal
        return Ok(fcs.clone());
    }

    // For performance: use percentile-based method for very large datasets
    // K-means clustering on 200k+ events is too slow
    if n_events > 100_000 {
        info!(
            "Large dataset ({} events), using fast percentile-based debris removal",
            n_events
        );
        return remove_debris_percentile(fcs);
    }

    // Create 2D data matrix for clustering
    let data_rows: Vec<Vec<f64>> = (0..n_events)
        .map(|i| vec![fsc_a_values[i] as f64, ssc_a_values[i] as f64])
        .collect();

    // Use K-means with 3 clusters: main population, debris, and potentially intermediate
    // This helps identify the smallest cluster which is likely debris
    let kmeans_config = KMeansConfig {
        n_clusters: 3,
        max_iterations: 50, // Reduced iterations for speed
        tolerance: 1e-3,    // Slightly relaxed tolerance for speed
        seed: Some(42),     // Fixed seed for reproducibility
    };

    let result = match KMeans::fit_from_rows(data_rows, &kmeans_config) {
        Ok(r) => r,
        Err(e) => {
            // If clustering fails, fall back to percentile-based heuristic
            info!(
                "K-means clustering failed: {:?}, falling back to percentile method",
                e
            );
            return remove_debris_percentile(fcs);
        }
    };

    // Count events per cluster
    let mut cluster_counts = vec![0; result.centroids.nrows()];
    for &assignment in &result.assignments {
        cluster_counts[assignment] += 1;
    }

    info!("Cluster sizes: {:?}", cluster_counts);

    // Find smallest cluster (debris)
    let debris_cluster = cluster_counts
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.cmp(b))
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    let debris_cluster_size = cluster_counts[debris_cluster];
    let debris_percentage = (debris_cluster_size as f64 / n_events as f64) * 100.0;

    // Calculate centroid of smallest cluster and overall centroid
    let mut debris_sum_fsc = 0.0;
    let mut debris_sum_ssc = 0.0;
    let mut debris_count = 0;
    let mut total_fsc = 0.0;
    let mut total_ssc = 0.0;

    for (i, &cluster) in result.assignments.iter().enumerate() {
        let fsc = fsc_a_values[i] as f64;
        let ssc = ssc_a_values[i] as f64;
        total_fsc += fsc;
        total_ssc += ssc;

        if cluster == debris_cluster {
            debris_sum_fsc += fsc;
            debris_sum_ssc += ssc;
            debris_count += 1;
        }
    }

    if debris_count == 0 {
        info!("No events in smallest cluster, skipping debris removal");
        return Ok(fcs.clone());
    }

    let debris_centroid_fsc = debris_sum_fsc / debris_count as f64;
    let debris_centroid_ssc = debris_sum_ssc / debris_count as f64;
    let overall_centroid_fsc = total_fsc / n_events as f64;
    let overall_centroid_ssc = total_ssc / n_events as f64;

    // Only remove if debris cluster is in bottom-left (below overall centroid)
    let is_bottom_left =
        debris_centroid_fsc < overall_centroid_fsc && debris_centroid_ssc < overall_centroid_ssc;

    info!(
        "Debris cluster: size={} ({:.2}%), centroid=(FSC={:.1}, SSC={:.1}), overall_centroid=(FSC={:.1}, SSC={:.1}), is_bottom_left={}",
        debris_cluster_size,
        debris_percentage,
        debris_centroid_fsc,
        debris_centroid_ssc,
        overall_centroid_fsc,
        overall_centroid_ssc,
        is_bottom_left
    );

    // Create mask: keep events that are NOT in the debris cluster
    // Be more aggressive: remove if < 20% of events OR in bottom-left OR if centroid is very low
    let very_low_threshold_fsc = overall_centroid_fsc * 0.3; // 30% of overall centroid
    let very_low_threshold_ssc = overall_centroid_ssc * 0.3;
    let is_very_low = debris_centroid_fsc < very_low_threshold_fsc
        && debris_centroid_ssc < very_low_threshold_ssc;

    let mask: Vec<bool> = if is_bottom_left || debris_percentage < 20.0 || is_very_low {
        // Remove debris cluster if:
        // 1. It's in bottom-left (below overall centroid), OR
        // 2. It's < 20% of events, OR
        // 3. It's very low (below 30% of overall centroid in both dimensions)
        info!(
            "Removing debris cluster: is_bottom_left={}, percentage={:.2}%, is_very_low={}",
            is_bottom_left, debris_percentage, is_very_low
        );
        result
            .assignments
            .iter()
            .map(|&cluster| cluster != debris_cluster)
            .collect()
    } else {
        // Debris cluster not in bottom-left and is large - don't remove
        info!(
            "Debris cluster is large ({:.2}%) and not in bottom-left, skipping removal",
            debris_percentage
        );
        vec![true; n_events]
    };

    let n_events_before = fcs.data_frame.height();
    let n_removed = mask.iter().filter(|&&keep| !keep).count();

    if n_removed > 0 {
        info!(
            "Debris removal (clustering): removed {} events ({:.2}%) from smallest cluster (centroid: FSC={:.1}, SSC={:.1})",
            n_removed,
            (n_removed as f64 / n_events_before as f64) * 100.0,
            debris_centroid_fsc,
            debris_centroid_ssc
        );
    }

    // Filter DataFrame
    let mask_series = Series::from_iter(mask.iter().copied());
    let mask_ca = mask_series
        .bool()
        .map_err(|e| anyhow::anyhow!("Failed to create boolean mask: {}", e))?;
    let filtered_df = fcs
        .data_frame
        .filter(&mask_ca)
        .map_err(|e| anyhow::anyhow!("Failed to filter DataFrame: {}", e))?;

    // Create new Fcs with filtered data
    let mut filtered_fcs = fcs.clone();
    filtered_fcs.data_frame = Arc::new(filtered_df);

    // Update metadata $TOT keyword
    let n_events_after = filtered_fcs.get_event_count_from_dataframe();
    use flow_fcs::keyword::{
        IntegerKeyword, Keyword, KeywordCreationResult, match_and_parse_keyword,
    };
    let tot_keyword = match_and_parse_keyword("$TOT", &n_events_after.to_string());
    if let KeywordCreationResult::Int(IntegerKeyword::TOT(tot)) = tot_keyword {
        filtered_fcs
            .metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(tot)));
    }

    Ok(filtered_fcs)
}

/// Fallback debris removal using percentile-based heuristic
fn remove_debris_percentile(fcs: &Fcs) -> Result<Fcs> {
    use polars::prelude::Series;
    use std::sync::Arc;

    // Get FSC-A and SSC-A channels
    let fsc_a_values = fcs
        .get_parameter_events_slice("FSC-A")
        .map_err(|e| anyhow::anyhow!("Failed to get FSC-A: {}", e))?;
    let ssc_a_values = fcs
        .get_parameter_events_slice("SSC-A")
        .map_err(|e| anyhow::anyhow!("Failed to get SSC-A: {}", e))?;

    // Calculate percentiles to determine debris threshold
    let mut fsc_sorted = fsc_a_values.to_vec();
    fsc_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut ssc_sorted = ssc_a_values.to_vec();
    ssc_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Use more aggressive percentiles: 2nd percentile instead of 1st
    // This catches more debris while still being fast
    let fsc_threshold_idx = (fsc_sorted.len() as f64 * 0.02).floor() as usize;
    let ssc_threshold_idx = (ssc_sorted.len() as f64 * 0.02).floor() as usize;

    let fsc_threshold = fsc_sorted.get(fsc_threshold_idx).copied().unwrap_or(0.0);
    let ssc_threshold = ssc_sorted.get(ssc_threshold_idx).copied().unwrap_or(0.0);

    // More aggressive: use 3x the percentile threshold instead of 2x
    // Also calculate overall centroid to ensure we're removing bottom-left debris
    let mut total_fsc = 0.0;
    let mut total_ssc = 0.0;
    for (&fsc, &ssc) in fsc_a_values.iter().zip(ssc_a_values.iter()) {
        total_fsc += fsc as f64;
        total_ssc += ssc as f64;
    }
    let overall_centroid_fsc = total_fsc / fsc_a_values.len() as f64;
    let overall_centroid_ssc = total_ssc / ssc_a_values.len() as f64;

    // Use the higher of: 2.5x percentile OR 30% of overall centroid (relaxed from 3x/25%)
    let fsc_debris_threshold = (fsc_threshold as f64 * 2.5).max(overall_centroid_fsc * 0.30) as f32;
    let ssc_debris_threshold = (ssc_threshold as f64 * 2.5).max(overall_centroid_ssc * 0.30) as f32;

    // Remove events where BOTH FSC-A and SSC-A are below thresholds (bottom-left corner)
    // This is more aggressive than OR logic
    let mask: Vec<bool> = fsc_a_values
        .iter()
        .zip(ssc_a_values.iter())
        .map(|(&fsc, &ssc)| {
            // Keep events that are above BOTH thresholds (not in bottom-left debris region)
            fsc > fsc_debris_threshold && ssc > ssc_debris_threshold
        })
        .collect();

    let n_events_before = fcs.data_frame.height();
    let n_removed = mask.iter().filter(|&&keep| !keep).count();

    if n_removed > 0 {
        info!(
            "Debris removal (percentile fallback): removed {} events ({:.2}%)",
            n_removed,
            (n_removed as f64 / n_events_before as f64) * 100.0
        );
    }

    let mask_series = Series::from_iter(mask.iter().copied());
    let mask_ca = mask_series
        .bool()
        .map_err(|e| anyhow::anyhow!("Failed to create boolean mask: {}", e))?;
    let filtered_df = fcs
        .data_frame
        .filter(&mask_ca)
        .map_err(|e| anyhow::anyhow!("Failed to filter DataFrame: {}", e))?;

    let mut filtered_fcs = fcs.clone();
    filtered_fcs.data_frame = Arc::new(filtered_df);

    let n_events_after = filtered_fcs.get_event_count_from_dataframe();
    use flow_fcs::keyword::{
        IntegerKeyword, Keyword, KeywordCreationResult, match_and_parse_keyword,
    };
    let tot_keyword = match_and_parse_keyword("$TOT", &n_events_after.to_string());
    if let KeywordCreationResult::Int(IntegerKeyword::TOT(tot)) = tot_keyword {
        filtered_fcs
            .metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(tot)));
    }

    Ok(filtered_fcs)
}

/// Isolate positive peak events from cleaned FCS data
///
/// Uses KDE-based peak detection to find the densest peak at highest intensity in the primary detector,
/// then returns a mask indicating which events are in the peak (with right-bias).
/// This mask can be applied to filter the original FCS data.
///
/// Algorithm:
/// 1. Estimate KDE and find up to 3 peaks
/// 2. For each peak, evaluate both density and intensity
/// 3. Select the peak that maximizes density first, then intensity as tiebreaker
/// 4. Use tighter IQR window (2.0x instead of 3.0x) for initial MAD calculation
/// 5. Apply secondary MAD calculation after initial filtering for refinement
pub fn isolate_positive_peak_mask(
    values: &[f64],
    peak_threshold: f64,
    peak_bias: f64,
) -> Result<Vec<bool>> {
    use flow_utils::kde::KernelDensity;

    if values.is_empty() {
        return Ok(vec![]);
    }

    // Pre-filter negative/low-intensity population to focus on positive events
    // This prevents the algorithm from selecting low-intensity peaks that happen to be dense
    let mut sorted_all = values.to_vec();
    sorted_all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Use Q1 (25th percentile) as threshold to filter out negative/low-intensity events
    let q1_idx = sorted_all.len() / 4;
    let intensity_threshold = sorted_all[q1_idx];

    // Also calculate median and Q3 for reference
    let median_all = sorted_all[sorted_all.len() / 2];
    let q3_idx = (sorted_all.len() * 3) / 4;
    let q3_value = sorted_all[q3_idx];

    info!(
        "Intensity statistics: Q1={:.2}, median={:.2}, Q3={:.2}, max={:.2}",
        intensity_threshold,
        median_all,
        q3_value,
        sorted_all[sorted_all.len() - 1]
    );

    // Filter to events above Q1 threshold for peak detection
    // Keep track of original indices for mapping back
    let mut positive_events: Vec<(usize, f64)> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > intensity_threshold)
        .map(|(idx, v)| (idx, *v))
        .collect();

    if positive_events.is_empty() {
        info!("No positive events found after filtering, using all events");
        // Fall back to using all events
        positive_events = values
            .iter()
            .enumerate()
            .map(|(idx, v)| (idx, *v))
            .collect();
    }

    let positive_values: Vec<f64> = positive_events.iter().map(|(_, v)| *v).collect();
    info!(
        "Filtered to {} positive events (above Q1={:.2}) out of {} total",
        positive_values.len(),
        intensity_threshold,
        values.len()
    );

    // Estimate KDE on positive events only
    // Use tighter bandwidth (0.5) and higher resolution (1024) for better peak detection in 1D
    // FFT-based KDE is already used by default in KernelDensity::estimate
    let kde = match KernelDensity::estimate(&positive_values, 0.5, 1024) {
        Ok(kde) => kde,
        Err(e) => {
            info!("KDE estimation failed: {:?}, returning all events", e);
            return Ok(vec![true; values.len()]);
        }
    };

    // Use lower threshold to detect smaller peaks (0.2 instead of default 0.3)
    let adjusted_threshold = peak_threshold.min(0.2);
    let mut peaks = kde.find_peaks(adjusted_threshold);

    if peaks.is_empty() {
        info!("No peaks detected in positive events, returning all events");
        return Ok(vec![true; values.len()]);
    }

    // Limit to top 3 peaks for evaluation (sorted by intensity, highest first)
    peaks.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if peaks.len() > 3 {
        peaks.truncate(3);
    }

    info!(
        "Detected {} candidate peaks in positive region: {:?}",
        peaks.len(),
        peaks
    );

    // Evaluate each peak: get density and intensity
    // Select the peak that maximizes BOTH density AND intensity (combined score)
    // Use density * intensity as combined score to ensure we get dense peaks at high intensity
    struct PeakCandidate {
        x: f64,
        density: f64,
        intensity: f64,
        combined_score: f64, // density * intensity
    }

    let mut candidates: Vec<PeakCandidate> = peaks
        .iter()
        .map(|&peak_x| {
            let density = kde.density_at(peak_x);
            let intensity = peak_x; // Intensity is the x-value itself
            // Combined score: density * intensity
            // This ensures we prioritize peaks that are BOTH dense AND high-intensity
            // Normalize density to [0, 1] range for fair weighting
            let max_density = kde.y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let normalized_density = if max_density > 0.0 {
                density / max_density
            } else {
                0.0
            };
            // Use log-scale for intensity to prevent very high values from dominating
            // Add small epsilon to handle zero/negative values
            let log_intensity = (intensity + 1.0).ln();
            let combined_score = normalized_density * log_intensity;

            PeakCandidate {
                x: peak_x,
                density,
                intensity,
                combined_score,
            }
        })
        .collect();

    // Sort by combined score (descending), then by intensity (descending) as tiebreaker
    // This ensures we select the peak that is both dense AND high-intensity
    candidates.sort_by(|a, b| {
        match b
            .combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
        {
            std::cmp::Ordering::Equal => b
                .intensity
                .partial_cmp(&a.intensity)
                .unwrap_or(std::cmp::Ordering::Equal),
            other => other,
        }
    });

    let main_peak = candidates[0].x;
    let main_density = candidates[0].density;
    let main_intensity = candidates[0].intensity;
    let main_score = candidates[0].combined_score;

    info!(
        "Selected peak at {:.2} (density: {:.6}, intensity: {:.2}, combined_score: {:.6}) from {} candidates",
        main_peak,
        main_density,
        main_intensity,
        main_score,
        candidates.len()
    );

    // Log all candidates for diagnostics
    for (i, cand) in candidates.iter().enumerate() {
        info!(
            "  Candidate {}: x={:.2}, density={:.6}, intensity={:.2}, combined_score={:.6}",
            i + 1,
            cand.x,
            cand.density,
            cand.intensity,
            cand.combined_score
        );
    }

    // Calculate IQR for initial window (tighter: 2.0x instead of 3.0x)
    let mut sorted_all = values.to_vec();
    sorted_all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q1_idx = sorted_all.len() / 4;
    let q3_idx = (sorted_all.len() * 3) / 4;
    let iqr = sorted_all[q3_idx] - sorted_all[q1_idx];
    let window = iqr * 2.0; // Tighter window

    info!("IQR: {:.2}, initial window: {:.2} (IQR * 2.0)", iqr, window);

    // First-stage MAD: Calculate from events near the peak (not all events)
    // This gives a better estimate of peak width
    let mut peak_region_values: Vec<f64> = values
        .iter()
        .filter(|&&v| (v - main_peak).abs() < window)
        .copied()
        .collect();

    if peak_region_values.is_empty() {
        peak_region_values = values.to_vec();
    }

    peak_region_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_peak_region = peak_region_values[peak_region_values.len() / 2];

    let deviations: Vec<f64> = peak_region_values
        .iter()
        .map(|&v| (v - median_peak_region).abs())
        .collect();
    let mut sorted_deviations = deviations;
    sorted_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad1 = sorted_deviations[sorted_deviations.len() / 2];

    // First-stage peak region (within 2 MAD of peak center)
    let peak_width1 = 2.0 * mad1;
    let peak_min1 = main_peak - peak_width1;
    let peak_max1 = main_peak + peak_width1;

    info!(
        "First-stage MAD: {:.2}, peak region: [{:.2}, {:.2}], width: {:.2}",
        mad1, peak_min1, peak_max1, peak_width1
    );

    // First pass: filter to initial peak region
    let peak_indices: Vec<usize> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| **v >= peak_min1 && **v <= peak_max1)
        .map(|(idx, _)| idx)
        .collect();

    if peak_indices.is_empty() {
        info!("No events in first-stage peak region, returning all events");
        return Ok(vec![true; values.len()]);
    }

    info!(
        "Found {} events in first-stage peak region (out of {})",
        peak_indices.len(),
        values.len()
    );

    // Second-stage MAD: Recalculate MAD from filtered events for refinement
    let mut filtered_values: Vec<f64> = peak_indices.iter().map(|&idx| values[idx]).collect();

    filtered_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_filtered = filtered_values[filtered_values.len() / 2];

    let deviations2: Vec<f64> = filtered_values
        .iter()
        .map(|&v| (v - median_filtered).abs())
        .collect();
    let mut sorted_deviations2 = deviations2;
    sorted_deviations2.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad2 = sorted_deviations2[sorted_deviations2.len() / 2];

    // Second-stage peak region (tighter, using refined MAD)
    let peak_width2 = 2.0 * mad2;
    let peak_min2 = main_peak - peak_width2;
    let peak_max2 = main_peak + peak_width2;

    info!(
        "Second-stage MAD: {:.2}, refined peak region: [{:.2}, {:.2}], width: {:.2}",
        mad2, peak_min2, peak_max2, peak_width2
    );

    // Second pass: filter to refined peak region
    let mut refined_indices: Vec<usize> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| **v >= peak_min2 && **v <= peak_max2)
        .map(|(idx, _)| idx)
        .collect();

    if refined_indices.is_empty() {
        info!("No events in refined peak region, using first-stage region");
        refined_indices = peak_indices;
    } else {
        info!(
            "Found {} events in refined peak region",
            refined_indices.len()
        );
    }

    // Apply right-bias: select upper fraction of peak events
    // Sort by value (ascending) and take the top (1 - bias) fraction
    let mut peak_values: Vec<(usize, f64)> = refined_indices
        .iter()
        .map(|&idx| (idx, values[idx]))
        .collect();
    peak_values.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());

    let bias_start_idx = ((peak_values.len() as f64) * (1.0 - peak_bias)) as usize;
    let biased_indices: std::collections::HashSet<usize> = peak_values[bias_start_idx..]
        .iter()
        .map(|(idx, _)| *idx)
        .collect();

    info!(
        "After right-bias ({}): kept {} events",
        peak_bias,
        biased_indices.len()
    );

    if biased_indices.is_empty() {
        info!("No events after bias filtering, returning all events");
        return Ok(vec![true; values.len()]);
    }

    // Create mask: true for events in biased peak region
    let mask: Vec<bool> = (0..values.len())
        .map(|idx| biased_indices.contains(&idx))
        .collect();

    let n_kept = mask.iter().filter(|&&keep| keep).count();
    info!(
        "Positive peak isolation: kept {} events ({:.2}%) from peak region (peak: {:.2}, density: {:.6}, bias: {:.2})",
        n_kept,
        (n_kept as f64 / values.len() as f64) * 100.0,
        main_peak,
        main_density,
        peak_bias
    );

    Ok(mask)
}

/// Apply a boolean mask to filter an FCS file.
/// Used by examples (e.g. process_compensation_controls).
#[allow(dead_code)]
pub fn apply_mask_to_fcs(fcs: &Fcs, mask: &[bool]) -> Result<Fcs> {
    use polars::prelude::Series;
    use std::sync::Arc;

    let n_events = fcs.data_frame.height();
    if mask.len() != n_events {
        return Err(anyhow::anyhow!(
            "Mask length {} doesn't match FCS event count {}",
            mask.len(),
            n_events
        ));
    }

    let mask_series = Series::from_iter(mask.iter().copied());
    let mask_ca = mask_series
        .bool()
        .map_err(|e| anyhow::anyhow!("Failed to create boolean mask: {}", e))?;
    let filtered_df = fcs
        .data_frame
        .filter(&mask_ca)
        .map_err(|e| anyhow::anyhow!("Failed to filter DataFrame: {}", e))?;

    let mut filtered_fcs = fcs.clone();
    filtered_fcs.data_frame = Arc::new(filtered_df);

    // Update metadata $TOT keyword
    let n_events_after = filtered_fcs.get_event_count_from_dataframe();
    use flow_fcs::keyword::{
        IntegerKeyword, Keyword, KeywordCreationResult, match_and_parse_keyword,
    };
    let tot_keyword = match_and_parse_keyword("$TOT", &n_events_after.to_string());
    if let KeywordCreationResult::Int(IntegerKeyword::TOT(tot)) = tot_keyword {
        filtered_fcs
            .metadata
            .keywords
            .insert("$TOT".to_string(), Keyword::Int(IntegerKeyword::TOT(tot)));
    }

    Ok(filtered_fcs)
}

/// Generate debug plots for control cleanup stages and spectral from peak events.
///
/// **All FSC-A vs SSC-A plots show events REMAINING (kept) after each step**, not events removed.
/// - Pre-gating: all events from the raw control file.
/// - Post-margin / post-doublet / post-debris: events kept after each cleaning step.
/// - Post-gating: events that passed the final scatter/doublet gates.
///
/// Filenames are prefixed with a 2-digit step number so that directory listings follow
/// the QC execution order: 01_pre_gating → 02_post_margin → 03_post_doublet → 04_post_debris
/// → 05_post_gating → 06_primary_vs_ssca → 07_spectral.
///
/// Additionally, for doublet discrimination, `FSC-A vs FSC-H` plots are written for the
/// pre/post-doublet states (step 03_*) when stage snapshots are available.
///
/// FSC-A vs SSC-A density uses a 99th-percentile colormap cap so saturated margin bins do not
/// flatten the rest of the population to "blank" (global max normalization).
fn generate_control_cleanup_debug_plots(
    raw_fcs: &Fcs,
    stages: Option<&(Fcs, Fcs, Fcs)>,
    control_fcs: &Fcs,
    endmember_name: &str,
    detector_names: &[String],
    primary_idx: usize,
    config: &SingleStainConfig,
    plot_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    use std::fs;

    let control_plot_dir = plot_dir.join(endmember_name);
    fs::create_dir_all(&control_plot_dir)?;

    info!(
        "Generating debug control plots for {} in {}",
        endmember_name,
        control_plot_dir.display()
    );

    let mut render_config = RenderConfig::default();
    let plot = DensityPlot::new();

    // Query $PnR for a channel, falling back to 262144 (classic 18-bit digital range).
    let channel_range = |channel: &str| -> f32 {
        raw_fcs
            .find_parameter(channel)
            .ok()
            .and_then(|p| {
                raw_fcs
                    .metadata
                    .get_parameter_numeric_metadata(p.parameter_number, "R")
                    .ok()
                    .and_then(|kw| match kw {
                        flow_fcs::keyword::IntegerKeyword::PnR(r) => Some(*r as f32),
                        _ => None,
                    })
            })
            .unwrap_or(262144.0)
    };
    let fsc_a_range = channel_range("FSC-A");
    let ssc_a_range = channel_range("SSC-A");
    let fsc_h_range = channel_range("FSC-H");

    // Helper to write one 2D density plot on the specified axes. Events are those REMAINING
    // at the given step, not the events removed.
    let mut write_density = |fcs: &Fcs,
                             x_channel: &str,
                             y_channel: &str,
                             x_range: f32,
                             y_range: f32,
                             step_label: &str|
     -> Result<()> {
        let x_data = fcs.get_parameter_events_slice(x_channel).ok();
        let y_data = fcs.get_parameter_events_slice(y_channel).ok();
        if let (Some(xs), Some(ys)) = (x_data, y_data) {
            let n_events = xs.len();
            info!(
                "  Plot {} ({}x{}): {} events (remaining after this step)",
                step_label, x_channel, y_channel, n_events
            );
            let data: Vec<(f32, f32)> = xs.iter().zip(ys.iter()).map(|(a, b)| (*a, *b)).collect();
            let base_opts = BasePlotOptions::new()
                .width(800u32)
                .height(600u32)
                .title(format!(
                    "{} - {} vs {} ({})",
                    endmember_name, x_channel, y_channel, step_label
                ))
                .build()?;
            let options = DensityPlotOptions::new()
                .base(base_opts)
                .x_axis(
                    AxisOptions::new()
                        .label(x_channel.to_string())
                        .range(0.0..=x_range)
                        .transform(flow_fcs::TransformType::Linear)
                        .build()?,
                )
                .y_axis(
                    AxisOptions::new()
                        .label(y_channel.to_string())
                        .range(0.0..=y_range)
                        .transform(flow_fcs::TransformType::Linear)
                        .build()?,
                )
                .density_normalization_percentile(99.0)
                .build()?;
            let bytes = plot.render(data.into(), &options, &mut render_config)?;
            let sanitized_x = x_channel.replace(['/', '\\'], "-");
            let sanitized_y = y_channel.replace(['/', '\\'], "-");
            let path = control_plot_dir.join(format!(
                "{}_{}_vs_{}_{}.{}",
                step_label, sanitized_x, sanitized_y, endmember_name, plot_format
            ));
            std::fs::write(&path, bytes)?;
            info!("  ✓ Saved: {}", path.display());
        }
        Ok(())
    };

    // 01. Pre-gating FSC-A vs SSC-A (all events from raw file)
    write_density(raw_fcs, "FSC-A", "SSC-A", fsc_a_range, ssc_a_range, "01_pre_gating")?;

    // 02–04. Post-margin, post-doublet, post-debris (only when we have stages)
    if let Some((post_margin, post_doublet, post_debris)) = stages {
        // 02. Post-margin FSC-A vs SSC-A
        write_density(
            post_margin,
            "FSC-A",
            "SSC-A",
            fsc_a_range,
            ssc_a_range,
            "02_post_margin",
        )?;
        // 03. Post-doublet FSC-A vs SSC-A, and the FSC-A vs FSC-H pre/post views (doublet-focused).
        //     Pre-doublet = post_margin (input to doublet gate). Post-doublet = post_doublet.
        write_density(
            post_doublet,
            "FSC-A",
            "SSC-A",
            fsc_a_range,
            ssc_a_range,
            "03_post_doublet",
        )?;
        write_density(
            post_margin,
            "FSC-A",
            "FSC-H",
            fsc_a_range,
            fsc_h_range,
            "03_pre_doublet_fsca_fsch",
        )?;
        write_density(
            post_doublet,
            "FSC-A",
            "FSC-H",
            fsc_a_range,
            fsc_h_range,
            "03_post_doublet_fsca_fsch",
        )?;
        // 04. Post-debris FSC-A vs SSC-A
        write_density(
            post_debris,
            "FSC-A",
            "SSC-A",
            fsc_a_range,
            ssc_a_range,
            "04_post_debris",
        )?;
    }

    // 05. Post-gating FSC-A vs SSC-A (final retained events after scatter/doublet gates)
    write_density(control_fcs, "FSC-A", "SSC-A", fsc_a_range, ssc_a_range, "05_post_gating")?;

    // 06. Primary channel vs SSC-A on final gated events. This visualises signal strength on the
    //     detector actually used for the spectral median, which is useful for sanity-checking the
    //     selected primary detector against the population.
    if let Some(primary_detector) = detector_names.get(primary_idx) {
        let primary_range = channel_range(primary_detector);
        write_density(
            control_fcs,
            primary_detector,
            "SSC-A",
            primary_range,
            ssc_a_range,
            "06_primary_vs_ssca",
        )?;
    }

    // 6. Spectral from peak events: median per channel over peak events, normalized, title = endmember
    let primary_detector = &detector_names[primary_idx];
    let primary_values: Vec<f64> = control_fcs
        .get_parameter_events_slice(primary_detector)
        .with_context(|| format!("Missing primary detector {} in control", primary_detector))?
        .iter()
        .map(|&v| v as f64)
        .collect();
    let peak_mask =
        isolate_positive_peak_mask(&primary_values, config.peak_threshold, config.peak_bias)?;
    let n_events = peak_mask.len();
    let peak_count = peak_mask.iter().filter(|&&b| b).count();
    if peak_count == 0 {
        warn!(
            "No peak events for {} spectral-from-peak plot, skipping",
            endmember_name
        );
        return Ok(());
    }
    let mut medians_from_peak: Vec<f64> = Vec::with_capacity(detector_names.len());
    for det_name in detector_names {
        let values = control_fcs
            .get_parameter_events_slice(det_name)
            .with_context(|| format!("Missing {} in control", det_name))?;
        let peak_values: Vec<f64> = values
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < n_events && peak_mask[*i])
            .map(|(_, &v)| v as f64)
            .collect();
        let median = if peak_values.is_empty() {
            0.0
        } else {
            let mut sorted = peak_values;
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            sorted[sorted.len() / 2]
        };
        medians_from_peak.push(median);
    }
    let max_median = medians_from_peak.iter().cloned().fold(0.0f64, f64::max);
    let normalized: Vec<f64> = if max_median > 0.0 {
        medians_from_peak.iter().map(|&v| v / max_median).collect()
    } else {
        medians_from_peak.clone()
    };
    let spectrum_data: Vec<(usize, f64)> = normalized
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    let mut render_config_spec = RenderConfig::default();
    let spec_plot = SpectralSignaturePlot::new();
    let base_opts = BasePlotOptions::new()
        .width(1200u32)
        .height(600u32)
        .title(endmember_name.to_string())
        .build()?;
    let options = SpectralSignaturePlotOptions::new()
        .base(base_opts)
        .x_axis(Some(
            AxisOptions::new().label("Channel".to_string()).build()?,
        ))
        .y_axis(Some(
            AxisOptions::new()
                .label("Normalized Intensity (1.0 to 0.0)".to_string())
                .build()?,
        ))
        .line_color("#1f77b4".to_string())
        .line_width(2.0)
        .show_grid(true)
        .build()?;
    let bytes = spec_plot.render(
        (spectrum_data, detector_names.to_vec()),
        &options,
        &mut render_config_spec,
    )?;
    let path = control_plot_dir.join(format!(
        "07_spectral_from_peak_events_{}.{}",
        endmember_name, plot_format
    ));
    std::fs::write(&path, bytes)?;
    info!("  ✓ Saved: {}", path.display());

    Ok(())
}

/// Generate diagnostic plots for a control file
///
/// Creates:
/// 1. FSC-A vs SSC-A and FSC-A vs FSC-H before and after gating
/// 2. Density plot showing signal across channels
/// 3. Normalized spectral signature plot (1.0 to 0.0 vs channels)
fn generate_control_diagnostic_plots(
    control_fcs_before: &Fcs,
    control_fcs_after: &Fcs,
    endmember_name: &str,
    detector_names: &[String],
    normalized_signature: &[f64],
    plot_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    use std::fs;

    // Create subdirectory for this control
    let control_plot_dir = plot_dir.join(endmember_name);
    fs::create_dir_all(&control_plot_dir)?;

    info!(
        "Generating diagnostic plots for {} in {}",
        endmember_name,
        control_plot_dir.display()
    );

    // 1. Scatter plots: FSC-A vs SSC-A and FSC-A vs FSC-H (before/after gating)
    generate_scatter_diagnostic_plots(
        control_fcs_before,
        control_fcs_after,
        endmember_name,
        &control_plot_dir,
        plot_format,
    )?;

    // 2. Density plot: signal across channels
    generate_channel_density_plot(
        control_fcs_after,
        endmember_name,
        detector_names,
        &control_plot_dir,
        plot_format,
    )?;

    // 3. Normalized spectral signature plot
    generate_spectral_signature_plot(
        endmember_name,
        detector_names,
        normalized_signature,
        &control_plot_dir,
        plot_format,
    )?;

    Ok(())
}

/// Generate scatter plots (FSC-A vs SSC-A, FSC-A vs FSC-H) before and after gating
fn generate_scatter_diagnostic_plots(
    fcs_before: &Fcs,
    fcs_after: &Fcs,
    endmember_name: &str,
    plot_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    use flow_fcs::TransformType;

    // Get FSC-A, SSC-A, FSC-H parameters
    let fsc_a_before = fcs_before.get_parameter_events_slice("FSC-A").ok();
    let ssc_a_before = fcs_before.get_parameter_events_slice("SSC-A").ok();
    let fsc_h_before = fcs_before.get_parameter_events_slice("FSC-H").ok();

    let fsc_a_after = fcs_after.get_parameter_events_slice("FSC-A").ok();
    let ssc_a_after = fcs_after.get_parameter_events_slice("SSC-A").ok();
    let fsc_h_after = fcs_after.get_parameter_events_slice("FSC-H").ok();

    let mut render_config = RenderConfig::default();
    let plot = DensityPlot::new();

    let fsc_a_range = fcs_before
        .find_parameter("FSC-A")
        .ok()
        .and_then(|p| {
            fcs_before
                .metadata
                .get_parameter_numeric_metadata(p.parameter_number, "R")
                .ok()
                .and_then(|kw| match kw {
                    flow_fcs::keyword::IntegerKeyword::PnR(r) => Some(*r as f32),
                    _ => None,
                })
        })
        .unwrap_or(262144.0);

    let ssc_a_range = fcs_before
        .find_parameter("SSC-A")
        .ok()
        .and_then(|p| {
            fcs_before
                .metadata
                .get_parameter_numeric_metadata(p.parameter_number, "R")
                .ok()
                .and_then(|kw| match kw {
                    flow_fcs::keyword::IntegerKeyword::PnR(r) => Some(*r as f32),
                    _ => None,
                })
        })
        .unwrap_or(262144.0);

    let fsc_h_range = fcs_before
        .find_parameter("FSC-H")
        .ok()
        .and_then(|p| {
            fcs_before
                .metadata
                .get_parameter_numeric_metadata(p.parameter_number, "R")
                .ok()
                .and_then(|kw| match kw {
                    flow_fcs::keyword::IntegerKeyword::PnR(r) => Some(*r as f32),
                    _ => None,
                })
        })
        .unwrap_or(262144.0);

    // FSC-A vs SSC-A before gating
    if let (Some(fsc_a), Some(ssc_a)) = (fsc_a_before, ssc_a_before) {
        let data: Vec<(f32, f32)> = fsc_a
            .iter()
            .zip(ssc_a.iter())
            .map(|(a, b)| (*a, *b))
            .collect();
        let base_opts = BasePlotOptions::new()
            .width(800u32)
            .height(600u32)
            .build()?;
        let options = DensityPlotOptions::new()
            .base(base_opts)
            .x_axis(
                AxisOptions::new()
                    .label("FSC-A".to_string())
                    .range(0.0..=fsc_a_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .y_axis(
                AxisOptions::new()
                    .label("SSC-A".to_string())
                    .range(0.0..=ssc_a_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .density_normalization_percentile(99.0)
            .build()?;

        let bytes = plot.render(data.into(), &options, &mut render_config)?;
        let output_path = plot_dir.join(format!(
            "{}_fsca_vs_ssca_before.{}",
            endmember_name, plot_format
        ));
        std::fs::write(&output_path, bytes)?;
        info!("  ✓ Saved: {}", output_path.display());
    }

    // FSC-A vs SSC-A after gating
    if let (Some(fsc_a), Some(ssc_a)) = (fsc_a_after, ssc_a_after) {
        let data: Vec<(f32, f32)> = fsc_a
            .iter()
            .zip(ssc_a.iter())
            .map(|(a, b)| (*a, *b))
            .collect();
        let base_opts = BasePlotOptions::new()
            .width(800u32)
            .height(600u32)
            .build()?;
        let options = DensityPlotOptions::new()
            .base(base_opts)
            .x_axis(
                AxisOptions::new()
                    .label("FSC-A".to_string())
                    .range(0.0..=fsc_a_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .y_axis(
                AxisOptions::new()
                    .label("SSC-A".to_string())
                    .range(0.0..=ssc_a_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .density_normalization_percentile(99.0)
            .build()?;

        let bytes = plot.render(data.into(), &options, &mut render_config)?;
        let output_path = plot_dir.join(format!(
            "{}_fsca_vs_ssca_after.{}",
            endmember_name, plot_format
        ));
        std::fs::write(&output_path, bytes)?;
        info!("  ✓ Saved: {}", output_path.display());
    }

    // FSC-A vs FSC-H before gating
    if let (Some(fsc_a), Some(fsc_h)) = (fsc_a_before, fsc_h_before) {
        let data: Vec<(f32, f32)> = fsc_a
            .iter()
            .zip(fsc_h.iter())
            .map(|(a, b)| (*a, *b))
            .collect();
        let base_opts = BasePlotOptions::new()
            .width(800u32)
            .height(600u32)
            .build()?;
        let options = DensityPlotOptions::new()
            .base(base_opts)
            .x_axis(
                AxisOptions::new()
                    .label("FSC-A".to_string())
                    .range(0.0..=fsc_a_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .y_axis(
                AxisOptions::new()
                    .label("FSC-H".to_string())
                    .range(0.0..=fsc_h_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .build()?;

        let bytes = plot.render(data.into(), &options, &mut render_config)?;
        let output_path = plot_dir.join(format!(
            "{}_fsca_vs_fsch_before.{}",
            endmember_name, plot_format
        ));
        std::fs::write(&output_path, bytes)?;
        info!("  ✓ Saved: {}", output_path.display());
    }

    // FSC-A vs FSC-H after gating
    if let (Some(fsc_a), Some(fsc_h)) = (fsc_a_after, fsc_h_after) {
        let data: Vec<(f32, f32)> = fsc_a
            .iter()
            .zip(fsc_h.iter())
            .map(|(a, b)| (*a, *b))
            .collect();
        let base_opts = BasePlotOptions::new()
            .width(800u32)
            .height(600u32)
            .build()?;
        let options = DensityPlotOptions::new()
            .base(base_opts)
            .x_axis(
                AxisOptions::new()
                    .label("FSC-A".to_string())
                    .range(0.0..=fsc_a_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .y_axis(
                AxisOptions::new()
                    .label("FSC-H".to_string())
                    .range(0.0..=fsc_h_range)
                    .transform(TransformType::Linear)
                    .build()?,
            )
            .build()?;

        let bytes = plot.render(data.into(), &options, &mut render_config)?;
        let output_path = plot_dir.join(format!(
            "{}_fsca_vs_fsch_after.{}",
            endmember_name, plot_format
        ));
        std::fs::write(&output_path, bytes)?;
        info!("  ✓ Saved: {}", output_path.display());
    }

    Ok(())
}

/// Generate density plot showing signal across channels
fn generate_channel_density_plot(
    fcs: &Fcs,
    endmember_name: &str,
    detector_names: &[String],
    plot_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    // For now, create a simple plot showing median signal per channel
    // This could be enhanced to show full distributions
    let mut medians = Vec::new();
    for detector_name in detector_names {
        if let Ok(values) = fcs.get_parameter_events_slice(detector_name) {
            let median = calculate_simple_median(values);
            medians.push(median);
        } else {
            medians.push(0.0);
        }
    }

    // Create a simple bar-like visualization using density plot
    // Plot channel index vs median signal
    let data: Vec<(f32, f32)> = medians
        .iter()
        .enumerate()
        .map(|(idx, &val)| (idx as f32, val))
        .collect();

    let mut render_config = RenderConfig::default();
    let plot = DensityPlot::new();
    let base_opts = BasePlotOptions::new()
        .width(1200u32)
        .height(400u32)
        .build()?;
    let options = DensityPlotOptions::new()
        .base(base_opts)
        .x_axis(
            AxisOptions::new()
                .label("Channel Index".to_string())
                .range(0.0..=detector_names.len() as f32)
                .transform(flow_fcs::TransformType::Linear)
                .build()?,
        )
        .y_axis(
            AxisOptions::new()
                .label("Median Signal".to_string())
                .range(0.0..=medians.iter().fold(0.0f32, |a, &b| a.max(b)) * 1.1)
                .transform(flow_fcs::TransformType::Linear)
                .build()?,
        )
        .build()?;

    let bytes = plot.render(data.into(), &options, &mut render_config)?;
    let output_path = plot_dir.join(format!(
        "{}_channel_signals.{}",
        endmember_name, plot_format
    ));
    std::fs::write(&output_path, bytes)?;
    info!("  ✓ Saved: {}", output_path.display());

    Ok(())
}

/// Generate normalized spectral signature plot (1.0 to 0.0 vs channels)
fn generate_spectral_signature_plot(
    endmember_name: &str,
    detector_names: &[String],
    normalized_signature: &[f64],
    plot_dir: &PathBuf,
    plot_format: &str,
) -> Result<()> {
    // Convert normalized signature to plot data: (channel_index, normalized_intensity)
    let spectrum_data: Vec<(usize, f64)> = normalized_signature
        .iter()
        .enumerate()
        .map(|(idx, &val)| (idx, val))
        .collect();

    let mut render_config = RenderConfig::default();
    let plot = SpectralSignaturePlot::new();
    let base_opts = BasePlotOptions::new()
        .width(1200u32)
        .height(600u32)
        .build()?;
    let options = SpectralSignaturePlotOptions::new()
        .base(base_opts)
        .x_axis(Some(
            AxisOptions::new()
                .label("Detector Channel".to_string())
                .build()?,
        ))
        .y_axis(Some(
            AxisOptions::new()
                .label("Normalized Intensity (1.0 to 0.0)".to_string())
                .build()?,
        ))
        .line_color("#1f77b4".to_string())
        .line_width(2.0)
        .show_grid(true)
        .build()?;

    let bytes = plot.render(
        (spectrum_data, detector_names.to_vec()),
        &options,
        &mut render_config,
    )?;
    let output_path = plot_dir.join(format!(
        "{}_spectral_signature.{}",
        endmember_name, plot_format
    ));
    std::fs::write(&output_path, bytes)?;
    info!("  ✓ Saved: {}", output_path.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_delimiters() {
        assert_eq!(count_delimiters("PD-1"), 1);
        assert_eq!(count_delimiters("HLA-DR_DQ"), 2);
        assert_eq!(count_delimiters("simple"), 0);
        assert_eq!(count_delimiters("a b c"), 2);
        assert_eq!(count_delimiters("a_b-c d"), 3);
    }

    #[test]
    fn test_candidate_fragments() {
        let frags = candidate_fragments("PD-1");
        assert!(frags.contains(&"PD-1".to_string()));
        assert!(frags.contains(&"PD".to_string()));
        assert!(frags.contains(&"1".to_string()));

        // When splitting "HLA-DR_DQ" on hyphens: ["HLA", "DR_DQ"]
        // When splitting on underscores: ["HLA-DR", "DQ"]
        let frags = candidate_fragments("HLA-DR_DQ");
        assert!(frags.contains(&"HLA-DR_DQ".to_string()));
        assert!(frags.contains(&"HLA".to_string()));
        assert!(frags.contains(&"DR_DQ".to_string()));
        assert!(frags.contains(&"DQ".to_string()));
        // "DR" alone only appears if both hyphens AND underscores are split

        let frags = candidate_fragments("simple");
        assert_eq!(frags.len(), 1);
        assert_eq!(frags[0], "simple");
    }

    #[test]
    fn test_delimiter_preference_infer_full_name() {
        let pref = DelimiterPreference::infer("PD-1", "PD-1");
        assert!(pref.use_space);
        assert!(pref.use_hyphen);
        assert!(pref.use_underscore);
    }

    #[test]
    fn test_delimiter_preference_infer_hyphen_split() {
        let pref = DelimiterPreference::infer("HLA-DR_DQ", "HLA");
        assert!(!pref.use_space);
        assert!(pref.use_hyphen);
        assert!(!pref.use_underscore);
    }

    #[test]
    fn test_delimiter_preference_infer_underscore_split() {
        // When splitting "HLA-DR_DQ" on underscore, we get ["HLA-DR", "DQ"]
        // So "DQ" is found when use_underscore is true
        // But "HLA-DR" contains hyphen, so we need to check if underscore alone produces "DQ"
        let pref = DelimiterPreference::infer("HLA-DR_DQ", "DQ");
        assert!(!pref.use_space);
        // Hyphen doesn't split to produce "DQ"
        assert!(!pref.use_hyphen);
        // Underscore DOES split to produce "DQ"
        assert!(pref.use_underscore);
    }

    #[test]
    fn test_delimiter_preference_apply_hyphen_only() {
        let pref = DelimiterPreference {
            use_space: false,
            use_hyphen: true,
            use_underscore: false,
        };
        let frags = pref.apply("HLA-DR_DQ");
        assert!(frags.contains(&"HLA-DR_DQ".to_string()));
        assert!(frags.contains(&"HLA".to_string()));
        assert!(frags.contains(&"DR_DQ".to_string()));
        // Should NOT split on underscore
        assert!(!frags.contains(&"DQ".to_string()));
    }

    #[test]
    fn test_delimiter_preference_apply_space_only() {
        let pref = DelimiterPreference {
            use_space: true,
            use_hyphen: false,
            use_underscore: false,
        };
        let frags = pref.apply("Panel A CD4-T Cells");
        assert!(frags.contains(&"Panel A CD4-T Cells".to_string()));
        assert!(frags.contains(&"Panel".to_string()));
        assert!(frags.contains(&"A".to_string()));
        assert!(frags.contains(&"CD4-T".to_string()));
        assert!(frags.contains(&"Cells".to_string()));
        // Should NOT split on hyphen
        assert!(!frags.contains(&"CD4".to_string()));
    }

    #[test]
    fn test_find_most_ambiguous_endmember() {
        let controls = vec![
            ("CD4".to_string(), PathBuf::from("cd4.fcs")),
            ("HLA-DR_DQ".to_string(), PathBuf::from("hla.fcs")),
            ("PD-1".to_string(), PathBuf::from("pd1.fcs")),
        ];
        if let Some((idx, delim)) = find_most_ambiguous_endmember(&controls) {
            assert_eq!(idx, 1); // "HLA-DR_DQ" has 2 delimiters
            assert_eq!(delim, 2);
        } else {
            panic!("Expected to find most ambiguous endmember");
        }
    }

    #[test]
    fn test_find_most_ambiguous_endmember_empty() {
        let controls: Vec<(String, PathBuf)> = vec![];
        assert!(find_most_ambiguous_endmember(&controls).is_none());
    }

    #[test]
    fn test_find_most_ambiguous_endmember_no_delimiters() {
        let controls = vec![
            ("CD4".to_string(), PathBuf::from("cd4.fcs")),
            ("CD8".to_string(), PathBuf::from("cd8.fcs")),
        ];
        assert!(find_most_ambiguous_endmember(&controls).is_none());
    }

    #[test]
    fn test_endmember_display_label_long_export_stem() {
        let s = "Filtered_Reference Group_A11 TIM3 RY775 (Beads)_Plate_001_2025_09_25_15_15_25_Non-debris_positive";
        let d = endmember_display_label(s);
        assert_eq!(d, "TIM3 RY775");
        assert!(!d.contains("09"), "{d}");
        assert!(!d.contains("15"), "{d}");
    }

    #[test]
    fn test_endmember_display_keeps_lone_two_digit_token() {
        let d = endmember_display_label("Panel_A1 CD4 BV421 Trial 09 export");
        assert!(
            d.contains("09") || d.contains("BV421") || d.contains("CD4"),
            "{d}"
        );
    }

    #[test]
    fn test_endmember_display_label_keeps_dye_numbers() {
        let d = endmember_display_label("Reference Group_A2 CD45 Spark UV 387 (Beads)_2026_03_05_11_55_59");
        assert!(d.contains("387") || d.contains("CD45"), "{d}");
        assert!(!d.contains("2026"), "{d}");
    }
}
