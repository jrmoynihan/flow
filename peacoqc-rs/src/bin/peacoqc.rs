// Integration Example: Adding PeacoQC to Your Tauri Commands
//
// This file shows how to add PeacoQC as a command in your existing Tauri app
// by reusing your open_files() infrastructure

use peacoqc_rs::qc::*;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

// Your existing imports (adjust paths as needed)
// use crate::fcs::{Fcs, file::open_from_str};

/// Report structure to send back to frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct PeacoQCReport {
    pub filename: String,
    pub n_events_before: usize,
    pub n_events_after: usize,
    pub percentage_removed: f64,
    pub it_percentage: Option<f64>,
    pub mad_percentage: Option<f64>,
    pub consecutive_percentage: f64,
    pub n_bins: usize,
    pub events_per_bin: usize,
    pub weird_channels: WeirdChannelsSummary,
    pub processing_time_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeirdChannelsSummary {
    pub increasing: Vec<String>,
    pub decreasing: Vec<String>,
    pub both: Vec<String>,
}

/// Example command to add to your commands.rs
/// 
/// Usage in your Tauri app:
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri'
/// 
/// const result = await invoke('run_peacoqc', {
///   filePath: '/path/to/file.fcs',
///   channels: ['FSC-A', 'SSC-A', 'FL1-A'],
///   qcMode: 'all',
///   madThreshold: 6.0,
///   itLimit: 0.6,
/// })
/// ```
/*
#[command(rename_all = "snake_case")]
pub async fn run_peacoqc(
    app_handle: AppHandle,
    file_path: &str,
    channels: Option<Vec<String>>,
    qc_mode: Option<String>,
    mad_threshold: Option<f64>,
    it_limit: Option<f64>,
    consecutive_bins: Option<usize>,
    remove_margins: Option<bool>,
    remove_doublets: Option<bool>,
    output_path: Option<String>,
) -> Result<PeacoQCReport, CommandError> {
    use std::time::Instant;
    
    let start_time = Instant::now();
    
    println!("🧬 Starting PeacoQC analysis for: {}", file_path);
    
    // Step 1: Load FCS using your existing infrastructure
    let fcs = open_from_str(file_path)?;
    let n_events_before = fcs.data_frame.height();
    
    // Emit progress
    app_handle.emit("peacoqc_progress", json!({
        "stage": "loaded",
        "message": format!("Loaded {} events", n_events_before),
    }))?;
    
    // Step 2: Auto-detect channels if not specified
    let channels = channels.unwrap_or_else(|| {
        fcs.parameters.values()
            .filter(|p| p.is_fluorescence())  // Your existing method!
            .map(|p| p.channel_name.to_string())
            .collect()
    });
    
    println!("📊 Analyzing {} channels: {:?}", channels.len(), channels);
    
    // Step 3: Remove margins if requested
    let fcs = if remove_margins.unwrap_or(true) {
        app_handle.emit("peacoqc_progress", json!({
            "stage": "margins",
            "message": "Removing margin events...",
        }))?;
        
        let margin_config = MarginConfig::from_fcs_metadata(
            &fcs.metadata,
            &fcs.parameters,
            channels.clone(),
        )?;
        
        let margin_result = remove_margins(&fcs, &margin_config)?;
        println!("  ✓ Margins: removed {:.2}%", margin_result.percentage_removed);
        
        fcs.filter(&margin_result.mask)?
    } else {
        fcs
    };
    
    // Step 4: Remove doublets if requested
    let fcs = if remove_doublets.unwrap_or(true) {
        app_handle.emit("peacoqc_progress", json!({
            "stage": "doublets",
            "message": "Removing doublets...",
        }))?;
        
        let doublet_result = remove_doublets(&fcs, &DoubletConfig::default())?;
        println!("  ✓ Doublets: removed {:.2}%", doublet_result.percentage_removed);
        
        fcs.filter(&doublet_result.mask)?
    } else {
        fcs
    };
    
    // Step 5: Run PeacoQC
    app_handle.emit("peacoqc_progress", json!({
        "stage": "peacoqc",
        "message": "Running PeacoQC analysis...",
    }))?;
    
    let qc_mode = match qc_mode.as_deref().unwrap_or("all") {
        "all" => QCMode::All,
        "it" => QCMode::IsolationTree,
        "mad" => QCMode::MAD,
        "none" => QCMode::None,
        _ => QCMode::All,
    };
    
    let peacoqc_config = PeacoQCConfig {
        channels: channels.clone(),
        determine_good_cells: qc_mode,
        mad: mad_threshold.unwrap_or(6.0),
        it_limit: it_limit.unwrap_or(0.6),
        consecutive_bins: consecutive_bins.unwrap_or(5),
        ..Default::default()
    };
    
    let peacoqc_result = peacoqc(&fcs, &peacoqc_config)?;
    
    println!("  ✓ PeacoQC: removed {:.2}%", peacoqc_result.percentage_removed);
    if let Some(it_pct) = peacoqc_result.it_percentage {
        println!("    - IT: {:.2}%", it_pct);
    }
    if let Some(mad_pct) = peacoqc_result.mad_percentage {
        println!("    - MAD: {:.2}%", mad_pct);
    }
    println!("    - Consecutive: {:.2}%", peacoqc_result.consecutive_percentage);
    
    // Step 6: Apply filter
    let clean_fcs = fcs.filter(&peacoqc_result.good_cells)?;
    let n_events_after = clean_fcs.data_frame.height();
    
    // Step 7: Save if output path specified
    if let Some(output) = output_path {
        app_handle.emit("peacoqc_progress", json!({
            "stage": "saving",
            "message": "Saving cleaned FCS file...",
        }))?;
        
        clean_fcs.write_to_file(&output)?;
        println!("💾 Saved cleaned FCS to: {}", output);
    }
    
    // Step 8: Create report
    let processing_time = start_time.elapsed().as_millis();
    
    let report = PeacoQCReport {
        filename: file_path.to_string(),
        n_events_before,
        n_events_after,
        percentage_removed: peacoqc_result.percentage_removed,
        it_percentage: peacoqc_result.it_percentage,
        mad_percentage: peacoqc_result.mad_percentage,
        consecutive_percentage: peacoqc_result.consecutive_percentage,
        n_bins: peacoqc_result.n_bins,
        events_per_bin: peacoqc_result.events_per_bin,
        weird_channels: WeirdChannelsSummary {
            increasing: vec![], // TODO: Add monotonic detection
            decreasing: vec![],
            both: vec![],
        },
        processing_time_ms: processing_time,
    };
    
    // Save report if requested
    if let Some(report_path) = &args.report {
        let report_json = serde_json::to_string_pretty(&report)?;
        std::fs::write(report_path, report_json)?;
        println!("📄 Saved report to: {}", report_path.display());
    }
    
    // Final progress event
    app_handle.emit("peacoqc_complete", &report)?;
    
    println!("\n✅ PeacoQC complete!");
    println!("   Events: {} → {} ({:.2}% removed)", 
             n_events_before, 
             n_events_after, 
             report.percentage_removed);
    println!("   Time: {:.2}s", processing_time as f64 / 1000.0);
    
    Ok(report)
}
*/

// Standalone CLI (without Tauri)
fn main() -> Result<()> {
    let args = Cli::parse();
    
    println!("🧬 PeacoQC - Flow Cytometry Quality Control");
    println!("============================================\n");
    
    println!("📂 Input file: {}", args.input.display());
    
    if let Some(ref output) = args.output {
        println!("💾 Output file: {}", output.display());
    }
    
    println!("\n⚙️  Configuration:");
    println!("   Mode: {}", args.qc_mode);
    println!("   MAD threshold: {}", args.mad);
    println!("   IT limit: {}", args.it_limit);
    println!("   Consecutive bins: {}", args.consecutive_bins);
    println!("   Remove margins: {}", args.remove_margins);
    println!("   Remove doublets: {}", args.remove_doublets);
    
    println!("\n⚠️  To run PeacoQC, you need to:");
    println!("   1. Implement PeacoQCData trait on your Fcs struct");
    println!("   2. Load FCS with your Fcs::open()");
    println!("   3. Call peacoqc(&fcs, &config)");
    println!("\nSee TAURI_INTEGRATION.md for complete example");
    
    Ok(())
}
