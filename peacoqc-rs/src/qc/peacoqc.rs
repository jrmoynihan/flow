use polars::prelude::*;
use crate::error::{PeacoQCError, Result};
use crate::PeacoQCData;
use crate::qc::peaks::{determine_peaks_all_channels, PeakDetectionConfig, ChannelPeakFrame};
use crate::qc::mad::{mad_outlier_method, MADConfig, MADResult};
use crate::qc::consecutive::{remove_short_regions, ConsecutiveConfig};
use crate::qc::isolation_tree::{isolation_tree_detect, IsolationTreeConfig, IsolationTreeResult};
use std::collections::HashMap;

/// Quality control mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QCMode {
    /// Use both Isolation Tree and MAD methods
    All,
    /// Use only Isolation Tree
    IsolationTree,
    /// Use only MAD method
    MAD,
    /// No quality control, only peak detection
    None,
}

/// Main PeacoQC configuration
#[derive(Debug, Clone)]
pub struct PeacoQCConfig {
    /// Channels to analyze
    pub channels: Vec<String>,
    
    /// Quality control mode
    pub determine_good_cells: QCMode,
    
    /// Minimum events per bin
    pub min_cells: usize,
    
    /// Maximum number of bins
    pub max_bins: usize,
    
    /// Events per bin (auto-calculated if None)
    pub events_per_bin: Option<usize>,
    
    /// MAD threshold
    pub mad: f64,
    
    /// Isolation Tree limit
    pub it_limit: f64,
    
    /// Consecutive bins threshold
    pub consecutive_bins: usize,
    
    /// Remove zeros before peak detection
    pub remove_zeros: bool,
    
    /// Peak removal threshold
    pub peak_removal: f64,
    
    /// Minimum bins for peak detection
    pub min_nr_bins_peakdetection: f64,
    
    /// Force Isolation Tree minimum bins
    pub force_it: usize,
}

impl Default for PeacoQCConfig {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            determine_good_cells: QCMode::All,
            min_cells: 150,
            max_bins: 500,
            events_per_bin: None,
            mad: 6.0,
            it_limit: 0.6,
            consecutive_bins: 5,
            remove_zeros: false,
            peak_removal: 1.0 / 3.0,
            min_nr_bins_peakdetection: 10.0,
            force_it: 150,
        }
    }
}

/// PeacoQC result
#[derive(Debug)]
pub struct PeacoQCResult {
    /// Boolean mask of good cells (true = keep, false = remove)
    pub good_cells: Vec<bool>,
    
    /// Percentage of cells removed
    pub percentage_removed: f64,
    
    /// IT percentage (if used)
    pub it_percentage: Option<f64>,
    
    /// MAD percentage (if used)
    pub mad_percentage: Option<f64>,
    
    /// Consecutive cells percentage
    pub consecutive_percentage: f64,
    
    /// Peak detection results per channel
    pub peaks: HashMap<String, ChannelPeakFrame>,
    
    /// Number of bins used
    pub n_bins: usize,
    
    /// Events per bin
    pub events_per_bin: usize,
}

/// Main PeacoQC quality control function
///
/// # Algorithm
/// 1. Calculate optimal bin size
/// 2. Detect peaks per channel per bin
/// 3. Run Isolation Tree (optional)
/// 4. Run MAD outlier detection (optional)
/// 5. Filter consecutive bins
/// 6. Generate cell-level boolean mask
pub fn peacoqc<T: PeacoQCData>(fcs: &T, config: &PeacoQCConfig) -> Result<PeacoQCResult> {
    if config.channels.is_empty() {
        return Err(PeacoQCError::ConfigError(
            "No channels specified".to_string(),
        ));
    }
    
    let n_events = fcs.n_events();
    
    // Calculate events per bin
    let events_per_bin = config.events_per_bin.unwrap_or_else(|| {
        find_events_per_bin(n_events, config.min_cells, config.max_bins, 500)
    });
    
    let n_bins = (n_events + events_per_bin - 1) / events_per_bin;
    
    eprintln!("Starting PeacoQC analysis...");
    eprintln!("Events: {}, Bins: {}, Events per bin: {}", n_events, n_bins, events_per_bin);
    
    // Peak detection
    let peak_config = PeakDetectionConfig {
        events_per_bin,
        peak_removal: config.peak_removal,
        min_nr_bins_peakdetection: config.min_nr_bins_peakdetection,
        remove_zeros: config.remove_zeros,
    };
    
    let peaks = determine_peaks_all_channels(fcs, &config.channels, &peak_config)?;
    
    if peaks.is_empty() {
        return Err(PeacoQCError::NoPeaksDetected);
    }
    
    // Initialize outlier bins (all good initially)
    let mut outlier_bins = vec![false; n_bins];
    let mut it_percentage = None;
    let mut mad_percentage = None;
    
    // Run quality control methods
    match config.determine_good_cells {
        QCMode::All | QCMode::IsolationTree => {
            if n_bins >= config.force_it {
                let it_config = IsolationTreeConfig {
                    it_limit: config.it_limit,
                    force_it: config.force_it,
                    ..Default::default()
                };
                
                match isolation_tree_detect(&peaks, n_bins, &it_config) {
                    Ok(it_result) => {
                        outlier_bins = it_result.outlier_bins;
                        let n_it_outliers = outlier_bins.iter().filter(|&&x| x).count();
                        let it_pct = (n_it_outliers as f64 / n_bins as f64) * 100.0;
                        it_percentage = Some(it_pct);
                        
                        eprintln!("Isolation Tree analysis removed {:.2}% of the bins", it_pct);
                    }
                    Err(e) => {
                        eprintln!("Isolation Tree failed: {}, continuing with MAD only", e);
                    }
                }
            } else {
                eprintln!("Not enough bins ({}) for Isolation Tree (need {}), skipping IT", 
                          n_bins, config.force_it);
            }
        }
        _ => {}
    }
    
    // MAD method
    if config.determine_good_cells == QCMode::All || config.determine_good_cells == QCMode::MAD {
        let mad_config = MADConfig {
            mad_threshold: config.mad,
        };
        
        // For MAD, we want to consider all bins initially
        let all_good_bins = vec![true; n_bins];
        
        let mad_result = mad_outlier_method(&peaks, &all_good_bins, n_bins, &mad_config)?;
        
        // Combine with existing outliers
        for (i, &is_mad_outlier) in mad_result.outlier_bins.iter().enumerate() {
            if is_mad_outlier {
                outlier_bins[i] = true;
            }
        }
        
        let n_mad_outliers = mad_result.outlier_bins.iter().filter(|&&x| x).count();
        let mad_pct = (n_mad_outliers as f64 / n_bins as f64) * 100.0;
        mad_percentage = Some(mad_pct);
        
        eprintln!("MAD analysis removed {:.2}% of the bins", mad_pct);
    }
    
    // Consecutive bin filtering
    if config.determine_good_cells != QCMode::None {
        let consecutive_config = ConsecutiveConfig {
            consecutive_bins: config.consecutive_bins,
        };
        
        outlier_bins = remove_short_regions(&outlier_bins, &consecutive_config)?;
    }
    
    // Convert bin-level outliers to cell-level mask
    let good_cells = bin_mask_to_cell_mask(&outlier_bins, n_events, events_per_bin);
    
    let n_removed = good_cells.iter().filter(|&&x| !x).count();
    let percentage_removed = (n_removed as f64 / n_events as f64) * 100.0;
    let consecutive_percentage = percentage_removed - mad_percentage.unwrap_or(0.0);
    
    eprintln!("Total removed: {:.2}%", percentage_removed);
    
    if percentage_removed > 70.0 {
        eprintln!("WARNING: More than 70% of events removed!");
    }
    
    Ok(PeacoQCResult {
        good_cells,
        percentage_removed,
        it_percentage,
        mad_percentage,
        consecutive_percentage,
        peaks,
        n_bins,
        events_per_bin,
    })
}

/// Find optimal events per bin
fn find_events_per_bin(
    n_events: usize,
    min_cells: usize,
    max_bins: usize,
    step: usize,
) -> usize {
    let initial = n_events / max_bins;
    let mut events_per_bin = initial.max(min_cells);
    
    // Round up to nearest step
    events_per_bin = ((events_per_bin + step - 1) / step) * step;
    
    events_per_bin.max(min_cells)
}

/// Convert bin-level mask to cell-level mask
fn bin_mask_to_cell_mask(
    bin_mask: &[bool],
    n_events: usize,
    events_per_bin: usize,
) -> Vec<bool> {
    let mut cell_mask = vec![true; n_events];
    
    for (bin_idx, &is_bad) in bin_mask.iter().enumerate() {
        if is_bad {
            let start = bin_idx * events_per_bin;
            let end = ((bin_idx + 1) * events_per_bin).min(n_events);
            
            for i in start..end {
                cell_mask[i] = false;
            }
        }
    }
    
    cell_mask
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;
    use crate::fcs::ParameterMetadata;
    
    #[test]
    fn test_peacoqc_basic() {
        // Create synthetic data
        let mut data = Vec::new();
        for _ in 0..10000 {
            data.push(100.0 + (rand::random::<f64>() - 0.5) * 20.0);
        }
        
        let df = df![
            "FL1-A" => data,
        ].unwrap();
        
        let fcs = SimpleFcs {
            data_frame: df,
            parameter_metadata: HashMap::new(),
        };
        
        let config = PeacoQCConfig {
            channels: vec!["FL1-A".to_string()],
            determine_good_cells: QCMode::MAD,
            ..Default::default()
        };
        
        let result = peacoqc(&fcs, &config);
        
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.good_cells.len(), 10000);
    }
}
