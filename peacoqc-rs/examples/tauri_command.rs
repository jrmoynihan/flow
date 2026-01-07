// Example showing how to integrate PeacoQC into your existing Tauri command structure
// This demonstrates the complete flow from file loading to QC to saving

// NOTE: This is example code showing the pattern. You would add this to your
// existing commands.rs file in src-tauri/src/

use peacoqc_rs::qc::*;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::time::Instant;

// Assuming your imports look something like this:
// use crate::fcs::{Fcs, file::open_from_str};
// use tauri::{command, AppHandle};

/// PeacoQC report structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeacoQCReport {
    pub guid: String,
    pub filename: String,
    pub n_events_before: usize,
    pub n_events_after: usize,
    pub percentage_removed: f64,
    pub it_percentage: Option<f64>,
    pub mad_percentage: Option<f64>,
    pub consecutive_percentage: f64,
    pub n_bins: usize,
    pub events_per_bin: usize,
    pub channels_analyzed: Vec<String>,
    pub weird_channels: WeirdChannelsInfo,
    pub processing_time_ms: u128,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WeirdChannelsInfo {
    pub increasing: Vec<String>,
    pub decreasing: Vec<String>,
    pub both: Vec<String>,
    pub has_issues: bool,
}

/// Progress event payload
#[derive(Debug, Serialize, Clone)]
struct ProgressPayload {
    stage: String,
    progress: u8,
    message: String,
}

/// Run PeacoQC quality control
/// 
/// # Arguments
/// * `file_path` - Path to FCS file (must be already opened or will be opened)
/// * `channels` - Channels to analyze (None = auto-detect fluorescence)
/// * `qc_mode` - "all", "it", "mad", or "none"
/// * `mad_threshold` - MAD threshold (default: 6.0)
/// * `it_limit` - IT threshold (default: 0.6)
/// * `consecutive_bins` - Consecutive bins threshold (default: 5)
/// * `remove_margins` - Remove margin events first (default: true)
/// * `remove_doublets` - Remove doublets first (default: true)
/// * `output_path` - Where to save cleaned FCS (None = don't save)
/// 
/// # Example Frontend Call
/// ```typescript
/// const result = await invoke('run_peacoqc', {
///   filePath: '/data/sample.fcs',
///   channels: null, // Auto-detect
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
    doublet_nmad: Option<f64>,
    output_path: Option<String>,
) -> Result<PeacoQCReport, CommandError> {
    let start_time = Instant::now();
    
    // Helper to emit progress
    let emit_progress = |stage: &str, progress: u8, message: &str| {
        app_handle.emit("peacoqc_progress", ProgressPayload {
            stage: stage.to_string(),
            progress,
            message: message.to_string(),
        }).ok();
    };
    
    println!("🧬 Starting PeacoQC for: {}", file_path);
    emit_progress("loading", 5, "Loading FCS file...");
    
    // === STEP 1: Load FCS ===
    let mut fcs = open_from_str(file_path)?;
    let n_events_initial = fcs.data_frame.height();
    let filename = fcs.file_access.path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    
    let guid = fcs.metadata.get_string_keyword("GUID")
        .unwrap_or_else(|_| "unknown".to_string());
    
    emit_progress("loaded", 10, &format!("Loaded {} events", n_events_initial));
    
    // === STEP 2: Auto-detect channels ===
    let channels = channels.unwrap_or_else(|| {
        fcs.parameters.values()
            .filter(|p| p.is_fluorescence())
            .map(|p| p.channel_name.to_string())
            .collect()
    });
    
    println!("📊 Analyzing {} channels: {:?}", channels.len(), 
             if channels.len() <= 5 { 
                 channels.join(", ") 
             } else { 
                 format!("{}...", channels[..3].join(", ")) 
             });
    
    emit_progress("configured", 15, &format!("Analyzing {} channels", channels.len()));
    
    // === STEP 3: Remove margins (optional) ===
    let mut total_removed = 0.0;
    
    if remove_margins.unwrap_or(true) {
        emit_progress("margins", 20, "Removing margin events...");
        
        let margin_config = MarginConfig {
            channels: channels.clone(),
            channel_specifications: None,
            remove_min: None,
            remove_max: None,
        };
        
        let margin_result = remove_margins(&fcs, &margin_config)?;
        
        if margin_result.percentage_removed > 0.0 {
            fcs = fcs.filter(&margin_result.mask)?;
            total_removed += margin_result.percentage_removed;
            println!("  ✓ Margins: removed {:.2}%", margin_result.percentage_removed);
        }
    }
    
    // === STEP 4: Remove doublets (optional) ===
    if remove_doublets.unwrap_or(true) {
        emit_progress("doublets", 25, "Removing doublets...");
        
        let doublet_config = DoubletConfig {
            channel1: "FSC-A".to_string(),
            channel2: "FSC-H".to_string(),
            nmad: doublet_nmad.unwrap_or(4.0),
            b: 0.0,
        };
        
        match remove_doublets(&fcs, &doublet_config) {
            Ok(doublet_result) => {
                if doublet_result.percentage_removed > 0.0 {
                    fcs = fcs.filter(&doublet_result.mask)?;
                    total_removed += doublet_result.percentage_removed;
                    println!("  ✓ Doublets: removed {:.2}%", doublet_result.percentage_removed);
                }
            }
            Err(e) => {
                println!("  ⚠ Doublet removal failed: {} (continuing)", e);
            }
        }
    }
    
    // === STEP 5: Run PeacoQC ===
    emit_progress("peak_detection", 30, "Detecting peaks...");
    
    let qc_mode = match qc_mode.as_deref().unwrap_or("all") {
        "all" => QCMode::All,
        "it" | "isolation" => QCMode::IsolationTree,
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
    
    emit_progress("isolation_tree", 40, "Running Isolation Tree...");
    emit_progress("mad_analysis", 55, "Running MAD analysis...");
    
    let peacoqc_result = peacoqc(&fcs, &peacoqc_config)
        .map_err(|e| CommandError::from(format!("PeacoQC failed: {}", e)))?;
    
    println!("  ✓ PeacoQC: removed {:.2}%", peacoqc_result.percentage_removed);
    if let Some(it_pct) = peacoqc_result.it_percentage {
        println!("    - Isolation Tree: {:.2}%", it_pct);
    }
    if let Some(mad_pct) = peacoqc_result.mad_percentage {
        println!("    - MAD: {:.2}%", mad_pct);
    }
    println!("    - Consecutive: {:.2}%", peacoqc_result.consecutive_percentage);
    
    emit_progress("filtering", 70, "Applying quality control filter...");
    
    // === STEP 6: Apply filter ===
    let clean_fcs = fcs.filter(&peacoqc_result.good_cells)
        .map_err(|e| CommandError::from(format!("Filter failed: {}", e)))?;
    
    let n_events_after = clean_fcs.data_frame.height();
    
    // === STEP 7: Save (optional) ===
    if let Some(output) = output_path {
        emit_progress("saving", 85, "Saving cleaned FCS file...");
        
        clean_fcs.write_to_file(&output)
            .map_err(|e| CommandError::from(format!("Save failed: {}", e)))?;
        
        println!("💾 Saved cleaned FCS to: {}", output);
    }
    
    // === STEP 8: Update cache ===
    emit_progress("caching", 95, "Updating file cache...");
    add_file_to_cache(Arc::new(clean_fcs)).await?;
    
    // === STEP 9: Create report ===
    let processing_time = start_time.elapsed().as_millis();
    
    let report = PeacoQCReport {
        guid,
        filename,
        n_events_before: n_events_initial,
        n_events_after,
        percentage_removed: peacoqc_result.percentage_removed,
        it_percentage: peacoqc_result.it_percentage,
        mad_percentage: peacoqc_result.mad_percentage,
        consecutive_percentage: peacoqc_result.consecutive_percentage,
        n_bins: peacoqc_result.n_bins,
        events_per_bin: peacoqc_result.events_per_bin,
        channels_analyzed: channels,
        weird_channels: WeirdChannelsInfo {
            increasing: vec![],  // TODO: Add monotonic detection
            decreasing: vec![],
            both: vec![],
            has_issues: false,
        },
        processing_time_ms: processing_time,
    };
    
    emit_progress("complete", 100, "PeacoQC complete!");
    
    println!("✅ Complete! Processing time: {:.2}s", processing_time as f64 / 1000.0);
    println!("   Events: {} → {} ({:.2}% removed)", 
             n_events_initial, 
             n_events_after,
             report.percentage_removed);
    
    Ok(report)
}
*/

// Placeholder main for CLI
pub fn main() {
    println!("See TAURI_INTEGRATION.md for integration instructions");
}
