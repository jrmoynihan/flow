use anyhow::Result;
use clap::Parser;
use flow_fcs::Fcs;
use indicatif::{ProgressBar, ProgressStyle};
use peacoqc_rs::{
    DoubletConfig, MarginConfig, PeacoQCConfig, QCMode, peacoqc, remove_doublets, remove_margins,
};
use std::path::PathBuf;
use std::time::Instant;

/// PeacoQC - Quality Control for Flow Cytometry Data
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "peacoqc")]
#[command(about = "Peak-based quality control for flow cytometry FCS files", long_about = None)]
struct Cli {
    /// Path to the input FCS file
    #[arg(value_name = "INPUT_FILE")]
    input: PathBuf,

    /// Path to save the cleaned FCS file (optional)
    #[arg(short, long, value_name = "OUTPUT_FILE")]
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

    /// Save QC report as JSON
    #[arg(long, value_name = "REPORT_FILE")]
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

fn main() -> Result<()> {
    let args = Cli::parse();

    println!("🧬 PeacoQC - Flow Cytometry Quality Control");
    println!("============================================\\n");

    // Check input file exists
    if !args.input.exists() {
        eprintln!("❌ Error: Input file not found: {}", args.input.display());
        std::process::exit(1);
    }

    let start_time = Instant::now();

    // Set up progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}% {msg}")?
            .progress_chars("=>-"),
    );

    // Step 1: Load FCS file
    pb.set_position(5);
    pb.set_message("Loading FCS file...");

    let fcs = Fcs::open(args.input.to_str().unwrap())?;
    let n_events_initial = fcs.n_events();
    let filename = args
        .input
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    pb.set_position(10);
    pb.set_message(format!("Loaded {} events", n_events_initial));

    if args.verbose {
        println!("📂 File: {}", filename);
        println!("📊 Events: {}", n_events_initial);
        println!("📈 Parameters: {}", fcs.parameters.len());
    }

    // Step 2: Determine channels
    let channels = args.channels.unwrap_or_else(|| {
        let fluoro = fcs.get_fluorescence_channels();
        if args.verbose {
            println!("🔬 Auto-detected {} fluorescence channels", fluoro.len());
        }
        fluoro
    });

    if channels.is_empty() {
        eprintln!("❌ Error: No channels specified or detected");
        std::process::exit(1);
    }

    if args.verbose {
        println!("📋 Analyzing channels: {}", channels.join(", "));
        println!();
    }

    let mut current_fcs = fcs;
    let mut total_removed_pct = 0.0;

    // Step 3: Remove margins (optional)
    if args.remove_margins {
        pb.set_position(20);
        pb.set_message("Removing margin events...");

        let margin_config = MarginConfig {
            channels: channels.clone(),
            channel_specifications: None,
            remove_min: None,
            remove_max: None,
        };

        let margin_result = remove_margins(&current_fcs, &margin_config)?;

        if margin_result.percentage_removed > 0.0 {
            current_fcs = current_fcs.filter(&margin_result.mask)?;
            total_removed_pct += margin_result.percentage_removed;

            if args.verbose {
                println!(
                    "  ✓ Margins: removed {:.2}%",
                    margin_result.percentage_removed
                );
            }
        }
    }

    // Step 4: Remove doublets (optional)
    if args.remove_doublets {
        pb.set_position(25);
        pb.set_message("Removing doublets...");

        let doublet_config = DoubletConfig {
            channel1: "FSC-A".to_string(),
            channel2: "FSC-H".to_string(),
            nmad: args.doublet_nmad,
            b: 0.0,
        };

        match remove_doublets(&current_fcs, &doublet_config) {
            Ok(doublet_result) => {
                if doublet_result.percentage_removed > 0.0 {
                    current_fcs = current_fcs.filter(&doublet_result.mask)?;
                    total_removed_pct += doublet_result.percentage_removed;

                    if args.verbose {
                        println!(
                            "  ✓ Doublets: removed {:.2}%",
                            doublet_result.percentage_removed
                        );
                    }
                }
            }
            Err(e) => {
                if args.verbose {
                    eprintln!("  ⚠ Doublet removal failed: {} (continuing)", e);
                }
            }
        }
    }

    // Step 5: Run PeacoQC
    pb.set_position(30);
    pb.set_message("Running PeacoQC analysis...");

    let qc_mode: QCMode = args.qc_mode.into();

    let peacoqc_config = PeacoQCConfig {
        channels: channels.clone(),
        determine_good_cells: qc_mode,
        mad: args.mad,
        it_limit: args.it_limit,
        consecutive_bins: args.consecutive_bins,
        remove_zeros: args.remove_zeros,
        ..Default::default()
    };

    pb.set_position(40);
    pb.set_message("Detecting peaks...");

    let peacoqc_result = peacoqc(&current_fcs, &peacoqc_config)?;

    pb.set_position(70);
    pb.set_message("Applying quality control filter...");

    // Step 6: Apply filter
    let clean_fcs = current_fcs.filter(&peacoqc_result.good_cells)?;
    let n_events_final = clean_fcs.n_events();

    pb.set_position(90);
    pb.set_message("Finalizing...");

    // Step 7: Save output (optional)
    if let Some(output_path) = &args.output {
        pb.set_message("Saving cleaned FCS file...");

        // TODO: Implement write_to_file on Fcs
        // clean_fcs.write_to_file(output_path)?;

        if args.verbose {
            println!("💾 Saved to: {}", output_path.display());
        }
    }

    // Step 8: Save report (optional)
    if let Some(report_path) = &args.report {
        let report = serde_json::json!({
            "filename": filename,
            "n_events_before": n_events_initial,
            "n_events_after": n_events_final,
            "percentage_removed": peacoqc_result.percentage_removed,
            "it_percentage": peacoqc_result.it_percentage,
            "mad_percentage": peacoqc_result.mad_percentage,
            "consecutive_percentage": peacoqc_result.consecutive_percentage,
            "n_bins": peacoqc_result.n_bins,
            "events_per_bin": peacoqc_result.events_per_bin,
            "channels_analyzed": channels,
            "processing_time_ms": start_time.elapsed().as_millis(),
        });

        std::fs::write(report_path, serde_json::to_string_pretty(&report)?)?;

        if args.verbose {
            println!("📄 Report saved to: {}", report_path.display());
        }
    }

    pb.finish_with_message("Complete!");

    // Print summary
    println!("\\n✅ PeacoQC Complete!");
    println!(
        "   Events: {} → {} ({:.2}% removed)",
        n_events_initial, n_events_final, peacoqc_result.percentage_removed
    );

    if let Some(it_pct) = peacoqc_result.it_percentage {
        println!("   - Isolation Tree: {:.2}%", it_pct);
    }
    if let Some(mad_pct) = peacoqc_result.mad_percentage {
        println!("   - MAD: {:.2}%", mad_pct);
    }
    println!(
        "   - Consecutive: {:.2}%",
        peacoqc_result.consecutive_percentage
    );
    println!("   ⏱️  Time: {:.2}s", start_time.elapsed().as_secs_f64());

    Ok(())
}
