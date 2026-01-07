use clap::Parser;
use std::path::PathBuf;
use anyhow::Result;

/// PeacoQC - Quality Control for Flow Cytometry Data
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the input FCS file
    #[arg(short, long, value_name = "FILE")]
    pub input: PathBuf,
    
    /// Path to save the cleaned FCS file
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    
    /// Channels to analyze (comma-separated, e.g., "FSC-A,SSC-A,FL1-A")
    /// If not specified, all fluorescence channels will be analyzed
    #[arg(short, long, value_delimiter = ',')]
    pub channels: Option<Vec<String>>,
    
    /// Quality control mode: all, it, mad, none
    #[arg(short = 'm', long, default_value = "all")]
    pub qc_mode: String,
    
    /// MAD threshold (default: 6.0)
    /// Higher = less strict
    #[arg(long, default_value = "6.0")]
    pub mad: f64,
    
    /// Isolation Tree limit (default: 0.6)
    /// Higher = less strict
    #[arg(long, default_value = "0.6")]
    pub it_limit: f64,
    
    /// Consecutive bins threshold (default: 5)
    #[arg(long, default_value = "5")]
    pub consecutive_bins: usize,
    
    /// Remove zeros before peak detection
    #[arg(long)]
    pub remove_zeros: bool,
    
    /// Remove margin events before QC
    #[arg(long, default_value = "true")]
    pub remove_margins: bool,
    
    /// Remove doublets before QC
    #[arg(long, default_value = "true")]
    pub remove_doublets: bool,
    
    /// Doublet nmad threshold (default: 4.0)
    #[arg(long, default_value = "4.0")]
    pub doublet_nmad: f64,
    
    /// Save QC report as JSON
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,
    
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

impl Cli {
    pub fn parse_qc_mode(&self) -> Result<peacoqc_rs::qc::QCMode> {
        match self.qc_mode.to_lowercase().as_str() {
            "all" => Ok(peacoqc_rs::qc::QCMode::All),
            "it" | "isolation" | "isolationtree" => Ok(peacoqc_rs::qc::QCMode::IsolationTree),
            "mad" => Ok(peacoqc_rs::qc::QCMode::MAD),
            "none" | "false" => Ok(peacoqc_rs::qc::QCMode::None),
            _ => Err(anyhow::anyhow!("Invalid QC mode: {}. Use: all, it, mad, or none", self.qc_mode)),
        }
    }
}
