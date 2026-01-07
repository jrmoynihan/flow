use polars::prelude::*;
use crate::error::{PeacoQCError, Result};
use crate::qc::peaks::ChannelPeakFrame;
use crate::stats::median_mad;
use std::collections::HashMap;

/// Configuration for MAD outlier detection
#[derive(Debug, Clone)]
pub struct MADConfig {
    /// MAD threshold multiplier (default: 6.0)
    pub mad_threshold: f64,
}

impl Default for MADConfig {
    fn default() -> Self {
        Self {
            mad_threshold: 6.0,
        }
    }
}

/// Result of MAD outlier detection
#[derive(Debug)]
pub struct MADResult {
    /// Boolean mask indicating outlier bins (true = outlier)
    pub outlier_bins: Vec<bool>,
    
    /// Percentage contribution of each channel to outlier detection
    pub contribution: HashMap<String, f64>,
}

/// Detect outlier bins using Median Absolute Deviation per channel
///
/// # Algorithm
/// For each channel:
/// 1. Extract peak values per bin
/// 2. Calculate median and MAD of peak values
/// 3. Mark bins as outliers if: |peak - median| > mad_threshold * MAD
/// 4. Combine outliers across channels
///
/// # Arguments
/// * `peak_results` - Peak detection results per channel
/// * `existing_outliers` - Existing outlier mask (e.g., from Isolation Tree)
/// * `n_bins` - Total number of bins
/// * `config` - MAD configuration
pub fn mad_outlier_method(
    peak_results: &HashMap<String, ChannelPeakFrame>,
    existing_outliers: &[bool],
    n_bins: usize,
    config: &MADConfig,
) -> Result<MADResult> {
    if peak_results.is_empty() {
        return Err(PeacoQCError::NoPeaksDetected);
    }
    
    let mut outlier_bins = vec![false; n_bins];
    let mut contribution = HashMap::new();
    
    // Process each channel
    for (channel, peak_frame) in peak_results {
        // Extract peaks per bin (using cluster medians)
        let mut bin_peak_map: HashMap<usize, Vec<f64>> = HashMap::new();
        for peak in &peak_frame.peaks {
            bin_peak_map.entry(peak.bin)
                .or_insert_with(Vec::new)
                .push(peak.peak_value);
        }
        
        // Get representative peak value per bin (median if multiple peaks)
        let mut bin_values: Vec<(usize, f64)> = Vec::new();
        for (bin_idx, peaks) in bin_peak_map {
            if !peaks.is_empty() {
                let peak_median = crate::stats::median(&peaks)?;
                bin_values.push((bin_idx, peak_median));
            }
        }
        
        if bin_values.len() < 3 {
            contribution.insert(channel.clone(), 0.0);
            continue;
        }
        
        // Calculate median and MAD across bins
        let values: Vec<f64> = bin_values.iter().map(|(_, v)| *v).collect();
        let (median, mad) = median_mad::median_mad(&values)?;
        
        if mad == 0.0 {
            contribution.insert(channel.clone(), 0.0);
            continue;
        }
        
        // Mark outlier bins
        let mut channel_outliers = 0;
        for (bin_idx, value) in bin_values {
            let deviation = (value - median).abs();
            if deviation > config.mad_threshold * mad {
                // Only mark as outlier if not already filtered by IT
                if bin_idx < existing_outliers.len() && existing_outliers[bin_idx] {
                    outlier_bins[bin_idx] = true;
                    channel_outliers += 1;
                }
            }
        }
        
        // Calculate contribution percentage
        let contrib_pct = if channel_outliers > 0 {
            (channel_outliers as f64 / n_bins as f64) * 100.0
        } else {
            0.0
        };
        contribution.insert(channel.clone(), contrib_pct);
    }
    
    Ok(MADResult {
        outlier_bins,
        contribution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qc::peaks::PeakInfo;
    
    #[test]
    fn test_mad_outliers() {
        // Create synthetic peak data with one outlier bin
        let mut peaks = Vec::new();
        for bin in 0..10 {
            let peak_value = if bin == 5 {
                1000.0 // Outlier
            } else {
                100.0 // Normal
            };
            peaks.push(PeakInfo {
                bin,
                peak_value,
                cluster: 1,
            });
        }
        
        let mut peak_results = HashMap::new();
        peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });
        
        let existing_outliers = vec![true; 10];
        let config = MADConfig::default();
        
        let result = mad_outlier_method(&peak_results, &existing_outliers, 10, &config).unwrap();
        
        // Should detect bin 5 as outlier
        assert!(result.outlier_bins[5]);
        assert!(result.contribution.get("FL1-A").unwrap() > &0.0);
    }
}
