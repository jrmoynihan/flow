//! MAD (Median Absolute Deviation) outlier detection
//!
//! This module implements the MAD-based outlier detection from R's PeacoQC package.
//! Key feature: applies smoothing before MAD calculation to reduce sensitivity to local noise.
//!
//! R's smooth.spline is approximated using kernel smoothing (Gaussian kernel) which
//! provides similar noise reduction characteristics.

use crate::error::{PeacoQCError, Result};
use crate::qc::peaks::{ChannelPeakFrame, PeakInfo};
use crate::stats::median_mad::{MAD_SCALE_FACTOR, median_mad_scaled};
use crate::stats::spline::smooth_spline;
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
/// Uses cubic smoothing spline matching R's smooth.spline implementation.
/// The smooth_param (spar) controls the smoothing strength.
fn smooth_peak_trajectory(peak_values: &[f64], smooth_param: f64) -> Vec<f64> {
    let n = peak_values.len();

    if n < 3 || smooth_param <= 0.0 {
        // Not enough points for smoothing or smoothing disabled, return original
        return peak_values.to_vec();
    }

    // Create x values (indices 1..n, matching R's seq_along)
    // Note: For seq_along, x values are equally spaced (1, 2, 3, ..., n)
    // This means h[i] = 1.0 for all i, so the penalty matrix simplifies
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();

    // Debug logging
    if std::env::var("PEACOQC_DEBUG_SPLINE").is_ok() {
        let y_min = peak_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let y_max = peak_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let y_range = y_max - y_min;
        eprintln!(
            "Smoothing trajectory: n={}, y_range={:.4}, spar={:.3}, first={:.4}, last={:.4}",
            n, y_range, smooth_param,
            peak_values.first().copied().unwrap_or(0.0),
            peak_values.last().copied().unwrap_or(0.0)
        );
    }

    // Apply smoothing spline (matching R's smooth.spline)
    match smooth_spline(&x, peak_values, smooth_param) {
        Ok(smoothed) => {
            if std::env::var("PEACOQC_DEBUG_SPLINE").is_ok() {
                // Check how much smoothing occurred
                let max_diff = peak_values.iter().zip(smoothed.iter())
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0f64, f64::max);
                let mean_diff = peak_values.iter().zip(smoothed.iter())
                    .map(|(a, b)| (a - b).abs())
                    .sum::<f64>() / n as f64;
                eprintln!(
                    "Smoothing result: max_diff={:.4}, mean_diff={:.4}, smoothed_range={:.4}",
                    max_diff, mean_diff,
                    smoothed.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) -
                    smoothed.iter().fold(f64::INFINITY, |a, &b| a.min(b))
                );
            }
            smoothed
        },
        Err(e) => {
            eprintln!("Spline smoothing failed: {:?}, returning original", e);
            // Fallback to original if spline fails
            peak_values.to_vec()
        }
    }
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

    // Debug logging for MAD thresholds
    if std::env::var("PEACOQC_DEBUG_MAD").is_ok() {
        let n_outliers = smoothed.iter()
            .filter(|&&y| y > upper_interval || y < lower_interval)
            .count();
        eprintln!(
            "MAD thresholds: median={:.4}, mad={:.4}, upper={:.4}, lower={:.4}, outliers={}/{}",
            median, mad, upper_interval, lower_interval, n_outliers, smoothed.len()
        );
        // Show first few values and their outlier status
        for (i, &y) in smoothed.iter().take(10).enumerate() {
            let is_outlier = y > upper_interval || y < lower_interval;
            let deviation = if y > median {
                (y - median) / mad
            } else {
                (median - y) / mad
            };
            eprintln!(
                "  [{}] smoothed={:.4}, deviation={:.2} MADs, outlier={}",
                i, y, deviation, is_outlier
            );
        }
    }

    // 4. Mark outliers (values outside interval on smoothed trajectory)
    let outliers: Vec<bool> = smoothed
        .iter()
        .map(|&y| y > upper_interval || y < lower_interval)
        .collect();

    Ok(outliers)
}

/// Detect outlier bins using Median Absolute Deviation per cluster
///
/// # Algorithm (matches R's MADOutlierMethod)
/// R processes each peak cluster separately:
/// 1. For each channel and each cluster, build a full-length trajectory (length = n_bins)
///    - Fill with cluster median for all bins
///    - Replace with actual peak values where cluster appears
/// 2. Filter to only bins that passed IT (existing_outliers)
/// 3. Apply smoothing spline to each cluster trajectory
/// 4. Calculate MAD on smoothed values and mark outliers
/// 5. A bin is an outlier if ANY cluster marks it as such
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

    // Build per-cluster trajectories (matching R's ExtractPeakValues)
    // Structure: (channel, cluster) -> Vec<f64> (full-length trajectory)
    let mut cluster_trajectories: Vec<(String, usize, Vec<f64>)> = Vec::new();

    // Get channel names in sorted order for consistent processing
    let mut channel_names: Vec<&String> = peak_results.keys().collect();
    channel_names.sort();

    for channel in channel_names {
        let peak_frame = &peak_results[channel];

        // Group peaks by cluster
        let mut clusters: HashMap<usize, Vec<&PeakInfo>> = HashMap::new();
        for peak in &peak_frame.peaks {
            clusters.entry(peak.cluster).or_default().push(peak);
        }

        // Build trajectory for each cluster
        for (cluster_id, cluster_peaks) in clusters {
            // Calculate cluster median (for filling missing bins)
            let n_peaks = cluster_peaks.len();
            let cluster_values: Vec<f64> = cluster_peaks.iter().map(|p| p.peak_value).collect();
            let cluster_median = crate::stats::median(&cluster_values)
                .unwrap_or_else(|_| cluster_values.iter().sum::<f64>() / cluster_values.len() as f64);

            // Build full-length trajectory: start with median, then fill actual values
            let mut trajectory = vec![cluster_median; n_bins];
            for peak in &cluster_peaks {
                if peak.bin < n_bins {
                    trajectory[peak.bin] = peak.peak_value;
                }
            }

            // Debug logging for trajectory building
            if std::env::var("PEACOQC_DEBUG_TRAJECTORY").is_ok() {
                let n_bins_with_peaks = trajectory.iter()
                    .take(n_bins)
                    .filter(|&&v| (v - cluster_median).abs() > 1e-10)
                    .count();
                eprintln!(
                    "Trajectory: channel={}, cluster={}, n_peaks={}, cluster_median={:.4}, bins_with_peaks={}/{}",
                    channel, cluster_id, n_peaks, cluster_median, n_bins_with_peaks, n_bins
                );
                // Show first 5 and last 5 trajectory values
                if n_bins > 10 {
                    eprintln!("  First 5: {:?}", &trajectory[0..5]);
                    eprintln!("  Last 5: {:?}", &trajectory[n_bins-5..n_bins]);
                }
            }

            cluster_trajectories.push((channel.clone(), cluster_id, trajectory));
        }
    }

    if cluster_trajectories.is_empty() {
        return Err(PeacoQCError::NoPeaksDetected);
    }

    // Process each cluster trajectory
    // R: to_remove_bins_df <- apply(peak_frame, 2, MADOutliers, MAD)
    let mut outlier_bins_per_cluster: Vec<Vec<bool>> = Vec::new();
    let mut contribution = HashMap::new();

    for (channel, _cluster_id, trajectory) in &cluster_trajectories {
        // Filter to bins that passed IT (matching R: peak_frame <- peaks[outlier_bins, , drop = FALSE])
        let filtered_trajectory: Vec<f64> = trajectory
            .iter()
            .enumerate()
            .filter_map(|(bin_idx, &value)| {
                if bin_idx < existing_outliers.len() && existing_outliers[bin_idx] {
                    Some(value)
                } else {
                    None
                }
            })
            .collect();

        // Debug logging for filtered trajectory
        if std::env::var("PEACOQC_DEBUG_TRAJECTORY").is_ok() {
            eprintln!(
                "Filtered trajectory: channel={}, cluster={}, original_len={}, filtered_len={}",
                channel, _cluster_id, trajectory.len(), filtered_trajectory.len()
            );
            if filtered_trajectory.len() > 10 {
                eprintln!("  First 5 filtered: {:?}", &filtered_trajectory[0..5]);
                eprintln!("  Last 5 filtered: {:?}", &filtered_trajectory[filtered_trajectory.len()-5..]);
            }
        }

        if filtered_trajectory.len() < 3 {
            continue;
        }

        // Apply MAD outlier detection with smoothing
        let cluster_outliers = mad_outliers_single_channel(
            &filtered_trajectory,
            config.mad_threshold,
            config.smooth_param,
        )?;

        // Map back to full bin indices
        let mut full_outliers = vec![false; n_bins];
        let mut filtered_idx = 0;
        for bin_idx in 0..n_bins {
            if bin_idx < existing_outliers.len() && existing_outliers[bin_idx] {
                if filtered_idx < cluster_outliers.len() && cluster_outliers[filtered_idx] {
                    full_outliers[bin_idx] = true;
                }
                filtered_idx += 1;
            }
        }

        // Track contribution per channel (sum across all clusters)
        let n_outliers: usize = full_outliers.iter().filter(|&&x| x).count();
        
        outlier_bins_per_cluster.push(full_outliers.clone());
        let contrib_pct = (n_outliers as f64 / n_bins as f64) * 100.0;
        contribution
            .entry(channel.clone())
            .and_modify(|e| *e += contrib_pct)
            .or_insert(contrib_pct);
    }

    // Combine: a bin is an outlier if ANY cluster marks it
    // R: outlier_bins_MAD <- apply(to_remove_bins_df, 1, any)
    let mut outlier_bins = vec![false; n_bins];
    for cluster_outliers in &outlier_bins_per_cluster {
        for (bin_idx, &is_outlier) in cluster_outliers.iter().enumerate() {
            if is_outlier {
                outlier_bins[bin_idx] = true;
            }
        }
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
