//! MAD (Median Absolute Deviation) outlier detection
//!
//! This module implements the MAD-based outlier detection from R's PeacoQC package.
//! Key feature: applies smoothing before MAD calculation to reduce sensitivity to local noise.
//!
//! R's smooth.spline is approximated using kernel smoothing (Gaussian kernel) which
//! provides similar noise reduction characteristics.

use crate::error::{PeacoQCError, Result};
use crate::qc::peaks::ChannelPeakFrame;
use crate::stats::median_mad::{MAD_SCALE_FACTOR, median_mad_scaled};
use std::collections::HashMap;

/// Configuration for MAD outlier detection
#[derive(Debug, Clone)]
pub struct MADConfig {
    /// MAD threshold multiplier (default: 6.0)
    ///
    /// **Tradeoff**: The lower the number of MADs allowed, the more strict
    /// the algorithm will be and more cells will be removed.
    pub mad_threshold: f64,

    /// Smoothing parameter (default: 0.5)
    /// Higher values = more smoothing. Matches R's smooth.spline spar parameter.
    /// The smoothing is implemented using a Gaussian kernel with bandwidth
    /// proportional to this parameter.
    pub smooth_param: f64,
}

impl Default for MADConfig {
    fn default() -> Self {
        Self {
            mad_threshold: 6.0,
            smooth_param: 0.5, // R default: spar=0.5
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

/// Apply smoothing to peak trajectory before MAD detection
///
/// R equivalent:
/// ```r
/// kernel <- stats::smooth.spline(seq_along(peak), peak, spar=0.5)
/// ```
///
/// We approximate R's smooth.spline using local kernel smoothing.
/// The smooth_param controls the bandwidth - lower values = less smoothing.
/// At smooth_param = 0, returns original values (no smoothing).
fn smooth_peak_trajectory(peak_values: &[f64], smooth_param: f64) -> Vec<f64> {
    let n = peak_values.len();

    if n < 4 || smooth_param <= 0.0 {
        // Not enough points for smoothing or smoothing disabled, return original
        return peak_values.to_vec();
    }

    // Bandwidth for local kernel smoothing
    // R's spar=0.5 corresponds to moderate smoothing
    // We use a local window approach similar to LOESS
    let half_window = ((n as f64 * smooth_param * 0.5).ceil() as usize)
        .max(1)
        .min(n / 4);

    let mut smoothed = Vec::with_capacity(n);

    for i in 0..n {
        // Local window around point i
        let start = i.saturating_sub(half_window);
        let end = (i + half_window + 1).min(n);

        let mut sum_weights = 0.0;
        let mut weighted_sum = 0.0;

        for j in start..end {
            // Triangular kernel weight (local, preserves peaks better than Gaussian)
            let dist = (i as f64 - j as f64).abs() / (half_window as f64 + 1.0);
            let weight = (1.0 - dist).max(0.0);

            sum_weights += weight;
            weighted_sum += weight * peak_values[j];
        }

        if sum_weights > 0.0 {
            smoothed.push(weighted_sum / sum_weights);
        } else {
            smoothed.push(peak_values[i]);
        }
    }

    smoothed
}

/// MAD outlier detection for a single peak trajectory
///
/// R equivalent (MADOutliers function):
/// ```r
/// MADOutliers <- function(peak, MAD) {
///     kernel <- stats::smooth.spline(seq_along(peak), peak, spar=0.5)
///     median_peak <- stats::median(kernel$y, na.rm=TRUE)
///     mad_peak <- stats::mad(kernel$y)
///     upper_interval <- stats::median(median_peak, na.rm=TRUE)+MAD*(mad_peak)
///     lower_interval <- stats::median(median_peak, na.rm=TRUE)-MAD*(mad_peak)
///     outliers <- ifelse(kernel$y > upper_interval, TRUE,
///                         ifelse(kernel$y < lower_interval, TRUE, FALSE))
///     return(outliers)
/// }
/// ```
fn mad_outliers_single_channel(
    peak_values: &[f64],
    mad_threshold: f64,
    smooth_param: f64,
) -> Result<Vec<bool>> {
    if peak_values.len() < 3 {
        return Ok(vec![false; peak_values.len()]);
    }

    // 1. Apply smoothing (approximates R's smooth.spline)
    let smoothed = smooth_peak_trajectory(peak_values, smooth_param);

    // 2. Calculate median and MAD on smoothed values (with R's scale factor)
    let (median, mad) = median_mad_scaled(&smoothed)?;

    if mad == 0.0 {
        return Ok(vec![false; peak_values.len()]);
    }

    // 3. Calculate intervals
    let upper_interval = median + mad_threshold * mad;
    let lower_interval = median - mad_threshold * mad;

    // 4. Mark outliers (values outside interval on smoothed trajectory)
    let outliers: Vec<bool> = smoothed
        .iter()
        .map(|&y| y > upper_interval || y < lower_interval)
        .collect();

    Ok(outliers)
}

/// Detect outlier bins using Median Absolute Deviation per channel
///
/// # Algorithm (matches R's MADOutlierMethod)
/// For each channel:
/// 1. Extract peak values per bin (ordered by bin index)
/// 2. Apply smoothing to the peak trajectory (approximates R's smooth.spline)
/// 3. Calculate median and MAD on smoothed values (with 1.4826 scale factor)
/// 4. Mark bins as outliers if smoothed value is outside median ± MAD_threshold * MAD
/// 5. A bin is an outlier if ANY channel marks it as such
///
/// # Arguments
/// * `peak_results` - Peak detection results per channel
/// * `existing_outliers` - Boolean mask where true = bin passed IT (still candidate for MAD)
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

    // Get channel names in sorted order for consistent processing
    let mut channel_names: Vec<&String> = peak_results.keys().collect();
    channel_names.sort();

    // Process each channel
    for channel in channel_names {
        let peak_frame = &peak_results[channel];

        // Extract peaks per bin (using cluster medians)
        let mut bin_peak_map: HashMap<usize, Vec<f64>> = HashMap::new();
        for peak in &peak_frame.peaks {
            bin_peak_map
                .entry(peak.bin)
                .or_default()
                .push(peak.peak_value);
        }

        // Build ordered peak trajectory
        // Only include bins that are in existing_outliers (passed IT)
        let mut bin_indices: Vec<usize> = bin_peak_map
            .keys()
            .filter(|&&bin_idx| bin_idx < existing_outliers.len() && existing_outliers[bin_idx])
            .copied()
            .collect();
        bin_indices.sort();

        if bin_indices.len() < 3 {
            contribution.insert(channel.clone(), 0.0);
            continue;
        }

        // Get representative peak value per bin (median if multiple peaks)
        let peak_values: Vec<f64> = bin_indices
            .iter()
            .map(|&bin_idx| {
                let peaks = &bin_peak_map[&bin_idx];
                if peaks.len() == 1 {
                    peaks[0]
                } else {
                    // Use median for multiple peaks
                    crate::stats::median(peaks).unwrap_or(peaks[0])
                }
            })
            .collect();

        // Apply MAD outlier detection with smoothing
        let channel_outliers =
            mad_outliers_single_channel(&peak_values, config.mad_threshold, config.smooth_param)?;

        // Map back to bin indices and mark outliers
        let mut n_outliers_in_channel = 0;
        for (i, &is_outlier) in channel_outliers.iter().enumerate() {
            if is_outlier {
                let bin_idx = bin_indices[i];
                outlier_bins[bin_idx] = true;
                n_outliers_in_channel += 1;
            }
        }

        // Calculate contribution percentage
        let contrib_pct = (n_outliers_in_channel as f64 / n_bins as f64) * 100.0;
        contribution.insert(channel.clone(), contrib_pct);
    }

    let total_outliers = outlier_bins.iter().filter(|&&x| x).count();
    eprintln!(
        "MAD detected {} outlier bins ({:.1}%) using scale factor {}",
        total_outliers,
        (total_outliers as f64 / n_bins as f64) * 100.0,
        MAD_SCALE_FACTOR
    );

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
    fn test_smooth_peak_trajectory() {
        // Create data with linear trend
        let peak_values: Vec<f64> = (0..20).map(|i| 100.0 + (i as f64) * 2.0).collect();

        let smoothed = smooth_peak_trajectory(&peak_values, 0.5);

        assert_eq!(smoothed.len(), peak_values.len());
        // Smoothed values should preserve the general trend
        // First should be lower than last
        assert!(smoothed[0] < smoothed[19], "Trend should be preserved");
        // Middle values should be reasonable
        let mid = smoothed[10];
        assert!(
            mid > 100.0 && mid < 150.0,
            "Mid value {} should be reasonable",
            mid
        );
    }

    #[test]
    fn test_mad_outliers_single_channel() {
        // Create data with a VERY large outlier (needs to be extreme to detect with MAD=6)
        let mut peak_values: Vec<f64> = (0..50).map(|i| 100.0 + (i as f64) * 0.1).collect();
        // Make outlier extremely large (much more than 6 MADs)
        peak_values[25] = 10000.0; // Extreme outlier

        // Use less smoothing (lower param = less smoothing effect)
        let outliers = mad_outliers_single_channel(&peak_values, 3.0, 0.2).unwrap();

        assert_eq!(outliers.len(), peak_values.len());
        // With a very extreme outlier and stricter threshold, it should be detected
        let n_outliers: usize = outliers.iter().filter(|&&x| x).count();
        assert!(
            n_outliers > 0,
            "Should detect at least one outlier near the extreme spike"
        );
    }

    #[test]
    fn test_mad_outliers() {
        // Create synthetic peak data with extreme outlier bin
        let mut peaks = Vec::new();
        for bin in 0..50 {
            let peak_value = if bin == 25 {
                10000.0 // Extreme outlier
            } else {
                100.0 + (bin as f64) * 0.5
            };
            peaks.push(PeakInfo {
                bin,
                peak_value,
                cluster: 1,
            });
        }

        let mut peak_results = HashMap::new();
        peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

        // All bins passed IT (all true)
        let existing_outliers = vec![true; 50];
        // Use stricter threshold for testing
        let config = MADConfig {
            mad_threshold: 3.0, // More strict than default 6.0
            smooth_param: 0.2,  // Less smoothing
        };

        let result = mad_outlier_method(&peak_results, &existing_outliers, 50, &config).unwrap();

        // Should detect outlier(s) near bin 25 with extreme value
        let n_outliers = result.outlier_bins.iter().filter(|&&x| x).count();
        assert!(
            n_outliers > 0,
            "Should detect outlier bins near the extreme spike at bin 25"
        );
        assert!(result.contribution.get("FL1-A").unwrap() > &0.0);
    }

    #[test]
    fn test_mad_no_outliers_stable_data() {
        // Create perfectly stable synthetic data - should have no outliers
        let peaks: Vec<PeakInfo> = (0..50)
            .map(|bin| PeakInfo {
                bin,
                peak_value: 100.0, // All same value
                cluster: 1,
            })
            .collect();

        let mut peak_results = HashMap::new();
        peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

        let existing_outliers = vec![true; 50];
        let config = MADConfig::default();

        let result = mad_outlier_method(&peak_results, &existing_outliers, 50, &config).unwrap();

        // Stable data should have no outliers (MAD = 0)
        let n_outliers = result.outlier_bins.iter().filter(|&&x| x).count();
        assert_eq!(n_outliers, 0, "Stable data should have no outliers");
    }
}
