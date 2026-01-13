use crate::PeacoQCData;
use crate::error::{PeacoQCError, Result};
use crate::stats::median;

/// Configuration for monotonic channel detection
///
/// Matches the original R implementation parameters:
/// - Uses kernel smoothing with bandwidth=50 (like R's ksmooth)
/// - Checks if 75% of smoothed values are in cummax/cummin
#[derive(Debug, Clone)]
pub struct MonotonicConfig {
    /// Kernel smoothing bandwidth (default: 50.0, matching R's ksmooth bandwidth)
    pub bandwidth: f64,

    /// Minimum fraction of points that must satisfy monotonicity (default: 0.75)
    /// In R: checks if length(which(condition)) > (3/4)*length(values)
    pub monotonic_threshold: f64,
}

impl Default for MonotonicConfig {
    fn default() -> Self {
        Self {
            bandwidth: 50.0,           // Matches R's ksmooth bandwidth parameter
            monotonic_threshold: 0.75, // Matches R's 3/4 threshold
        }
    }
}

/// Result of monotonic channel detection
#[derive(Debug, Clone)]
pub struct MonotonicResult {
    /// Channels with increasing trend
    pub increasing: Vec<String>,

    /// Channels with decreasing trend
    pub decreasing: Vec<String>,

    /// Channels with problematic monotonic behavior when both increasing and decreasing channels are detected
    /// This field is populated when the dataset has both increasing AND decreasing channels, indicating unstable conditions.
    /// In such cases, this contains all channels showing monotonic behavior (union of increasing and decreasing).
    /// When only one type exists, this field is empty.
    pub both: Vec<String>,

    /// Per-channel correlation coefficients
    pub correlations: std::collections::HashMap<String, f64>,
}

impl MonotonicResult {
    /// Check if any channels show monotonic behavior
    pub fn has_issues(&self) -> bool {
        !self.increasing.is_empty() || !self.decreasing.is_empty() || !self.both.is_empty()
    }

    /// Get a summary message
    pub fn summary(&self) -> String {
        if !self.has_issues() {
            "No increasing or decreasing channels detected".to_string()
        } else {
            let mut parts = Vec::new();
            if !self.increasing.is_empty() {
                parts.push(format!("Increasing: {}", self.increasing.join(", ")));
            }
            if !self.decreasing.is_empty() {
                parts.push(format!("Decreasing: {}", self.decreasing.join(", ")));
            }
            if !self.both.is_empty() {
                parts.push(format!("Both: {}", self.both.join(", ")));
            }
            parts.join("; ")
        }
    }
}

/// Detect channels with monotonic increasing or decreasing trends
///
/// Monotonic trends in flow cytometry channels indicate technical problems:
/// - Increasing: Possible accumulation, clog developing
/// - Decreasing: Possible depletion, pressure loss
///
/// # Algorithm (matches R implementation)
/// 1. Split data into bins
/// 2. Calculate median value per bin
/// 3. Apply kernel smoothing to bin medians (like R's ksmooth with bandwidth=50)
/// 4. Check if cummax(smoothed) == smoothed (non-decreasing) for increasing
/// 5. Check if cummin(smoothed) == smoothed (non-increasing) for decreasing
/// 6. Flag as monotonic if >75% of values satisfy the condition
///
/// # Arguments
/// * `fcs` - FCS file data
/// * `channels` - Channels to check
/// * `breaks` - Bin boundaries (start, end) indices
/// * `config` - Configuration with bandwidth and threshold
pub fn find_increasing_decreasing_channels<T: PeacoQCData>(
    fcs: &T,
    channels: &[String],
    breaks: &[(usize, usize)],
    config: &MonotonicConfig,
) -> Result<MonotonicResult> {
    let mut increasing = Vec::new();
    let mut decreasing = Vec::new();
    let mut correlations = std::collections::HashMap::new();

    eprintln!(
        "Checking {} channels for monotonic trends...",
        channels.len()
    );

    for channel in channels {
        // Get channel data
        let data = fcs.get_channel_f64(channel)?;

        // Calculate median per bin (matching R: vapply(breaks, function(x) median(channel_data[x])))
        let mut bin_medians = Vec::new();
        for (start, end) in breaks {
            if *end <= data.len() {
                let bin_data = &data[*start..*end];
                if !bin_data.is_empty() {
                    if let Ok(med) = median(bin_data) {
                        bin_medians.push(med);
                    }
                }
            }
        }

        if bin_medians.len() < 3 {
            continue; // Need at least 3 points
        }

        // Apply kernel smoothing (like R's ksmooth)
        // R: smoothed <- stats::ksmooth(seq_along(channel_medians), channel_medians,
        //                               x.points=seq_along(channel_medians), bandwidth=50)
        let bin_indices: Vec<f64> = (0..bin_medians.len()).map(|i| i as f64).collect();
        let smoothed = kernel_smooth(&bin_indices, &bin_medians, &bin_indices, config.bandwidth)?;

        // Store correlation for reference (using Spearman as additional metric)
        let correlation = spearman_correlation(&bin_indices, &bin_medians)?;
        correlations.insert(channel.clone(), correlation);

        // Check for increasing: cummax(smoothed) == smoothed (all values are non-decreasing)
        // R: increasing <- cummax(smoothed$y) == smoothed$y
        let increasing_mask: Vec<bool> = smoothed
            .iter()
            .scan(f64::NEG_INFINITY, |max, &val| {
                *max = val.max(*max);
                Some((*max - val).abs() < 1e-10) // Check if val equals cummax (within tolerance)
            })
            .collect();

        let increasing_count = increasing_mask.iter().filter(|&&x| x).count();
        let is_increasing =
            increasing_count as f64 > config.monotonic_threshold * smoothed.len() as f64;

        // Check for decreasing: cummin(smoothed) == smoothed (all values are non-increasing)
        // R: decreasing <- cummin(smoothed$y) == smoothed$y
        let decreasing_mask: Vec<bool> = smoothed
            .iter()
            .scan(f64::INFINITY, |min, &val| {
                *min = val.min(*min);
                Some((*min - val).abs() < 1e-10) // Check if val equals cummin (within tolerance)
            })
            .collect();

        let decreasing_count = decreasing_mask.iter().filter(|&&x| x).count();
        let is_decreasing =
            decreasing_count as f64 > config.monotonic_threshold * smoothed.len() as f64;

        // Classify channel (matching R logic: if/else if, not both)
        // R: if (length(which(increasing)) > (3/4)*length(increasing)) ...
        //    else if (length(which(decreasing)) > (3/4)*length(decreasing)) ...
        if is_increasing {
            increasing.push(channel.clone());
        } else if is_decreasing {
            decreasing.push(channel.clone());
        }
    }

    if !increasing.is_empty() {
        eprintln!("⚠️ Increasing channels detected: {:?}", increasing);
    }
    if !decreasing.is_empty() {
        eprintln!("⚠️ Decreasing channels detected: {:?}", decreasing);
    }

    // If both increasing and decreasing channels are detected, this indicates unstable conditions
    // This matches the original R implementation's "Increasing and decreasing channel" detection
    // When both types exist in the dataset, all problematic channels are included in 'both'
    let both: Vec<String> = if !increasing.is_empty() && !decreasing.is_empty() {
        let mut combined = increasing
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<String>>();
        combined.extend(decreasing.iter().cloned());
        combined.into_iter().collect()
    } else {
        Vec::new()
    };

    if !both.is_empty() {
        eprintln!("⚠️ Both increasing and decreasing channels detected - unstable conditions");
    }

    Ok(MonotonicResult {
        increasing,
        decreasing,
        both,
        correlations,
    })
}

/// Kernel smoothing (matching R's stats::ksmooth)
///
/// Smooths y values at x points using a normal kernel with given bandwidth
///
/// # Arguments
/// * `x` - Input x values (bin indices)
/// * `y` - Input y values (bin medians)
/// * `x_points` - Points at which to evaluate smoothed values (same as x in R)
/// * `bandwidth` - Bandwidth for kernel smoothing (default: 50.0 in R)
fn kernel_smooth(x: &[f64], y: &[f64], x_points: &[f64], bandwidth: f64) -> Result<Vec<f64>> {
    if x.len() != y.len() || x.len() < 2 {
        return Err(PeacoQCError::StatsError(
            "Invalid data for kernel smoothing".to_string(),
        ));
    }

    // R's ksmooth uses a normal (Gaussian) kernel
    // For each x_point, compute weighted average of y values
    let smoothed: Vec<f64> = x_points
        .iter()
        .map(|&x_target| {
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;

            for i in 0..x.len() {
                let distance = (x_target - x[i]) / bandwidth;
                // Normal kernel: exp(-0.5 * distance^2)
                let weight = (-0.5 * distance * distance).exp();
                weighted_sum += y[i] * weight;
                weight_sum += weight;
            }

            if weight_sum > 1e-10 {
                weighted_sum / weight_sum
            } else {
                y[x_points
                    .iter()
                    .position(|&v| (v - x_target).abs() < 1e-10)
                    .unwrap_or(0)]
            }
        })
        .collect();

    Ok(smoothed)
}

/// Calculate Spearman rank correlation coefficient
///
/// Spearman's rho measures monotonic relationship between two variables
/// rho = 1 - (6 * sum(d_i^2)) / (n * (n^2 - 1))
/// where d_i = rank difference for each pair
///
/// Note: This is used for correlation tracking but the main detection uses kernel smoothing
fn spearman_correlation(x: &[f64], y: &[f64]) -> Result<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return Err(PeacoQCError::StatsError(
            "Invalid data for correlation".to_string(),
        ));
    }

    let n = x.len();

    // Get ranks
    let x_ranks = rank_data(x);
    let y_ranks = rank_data(y);

    // Calculate sum of squared differences
    let d_squared_sum: f64 = x_ranks
        .iter()
        .zip(y_ranks.iter())
        .map(|(rx, ry)| (rx - ry).powi(2))
        .sum();

    // Spearman's rho formula
    let rho = 1.0 - (6.0 * d_squared_sum) / (n as f64 * ((n * n - 1) as f64));

    Ok(rho)
}

/// Assign ranks to data (handles ties with average rank)
fn rank_data(data: &[f64]) -> Vec<f64> {
    let n = data.len();

    // Create index-value pairs
    let mut indexed: Vec<(usize, f64)> = data.iter().enumerate().map(|(i, &v)| (i, v)).collect();

    // Sort by value
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Assign ranks (1-based)
    let mut ranks = vec![0.0; n];
    let mut i = 0;

    while i < n {
        let value = indexed[i].1;
        let mut j = i;

        // Find all tied values
        while j < n && (indexed[j].1 - value).abs() < 1e-10 {
            j += 1;
        }

        // Average rank for ties
        let avg_rank = (i + j + 1) as f64 / 2.0;

        for k in i..j {
            ranks[indexed[k].0] = avg_rank;
        }

        i = j;
    }

    ranks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fcs::SimpleFcs;
    use polars::df;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_spearman_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];

        let corr = spearman_correlation(&x, &y).unwrap();
        assert!(
            (corr - 1.0).abs() < 1e-6,
            "Perfect positive correlation should be 1.0"
        );
    }

    #[test]
    fn test_spearman_perfect_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![10.0, 8.0, 6.0, 4.0, 2.0];

        let corr = spearman_correlation(&x, &y).unwrap();
        assert!(
            (corr + 1.0).abs() < 1e-6,
            "Perfect negative correlation should be -1.0"
        );
    }

    #[test]
    fn test_rank_data() {
        let data = vec![3.0, 1.0, 4.0, 1.0, 5.0];
        let ranks = rank_data(&data);

        // Expected ranks: [4, 1.5, 5, 1.5, 6] (ties get average rank)
        assert!((ranks[0] - 3.0).abs() < 1e-6);
        assert!((ranks[1] - 1.5).abs() < 1e-6);
        assert!((ranks[2] - 4.0).abs() < 1e-6);
        assert!((ranks[3] - 1.5).abs() < 1e-6);
        assert!((ranks[4] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_monotonic_detection() {
        // Create data with increasing trend
        let mut data = Vec::new();
        for i in 0..1000 {
            data.push(100.0 + i as f64 * 0.1); // Clear increasing trend
        }

        let df = Arc::new(
            df![
                "FL1-A" => data,
            ]
            .unwrap(),
        );

        let fcs = SimpleFcs {
            data_frame: df,
            parameter_metadata: HashMap::new(),
        };

        let breaks = (0..10)
            .map(|i| (i * 100, (i + 1) * 100))
            .collect::<Vec<_>>();

        let config = MonotonicConfig::default();

        let result =
            find_increasing_decreasing_channels(&fcs, &["FL1-A".to_string()], &breaks, &config)
                .unwrap();

        // Should detect increasing trend
        assert!(
            !result.increasing.is_empty(),
            "Should detect increasing channel"
        );
        assert!(result.has_issues());
        // When only increasing channels exist, 'both' should be empty
        assert!(result.both.is_empty());
    }

    #[test]
    fn test_monotonic_both_types() {
        // Create data with both increasing and decreasing trends in different channels
        let mut increasing_data = Vec::new();
        let mut decreasing_data = Vec::new();
        for i in 0..1000 {
            increasing_data.push(100.0 + i as f64 * 0.1); // Increasing trend
            decreasing_data.push(1000.0 - i as f64 * 0.1); // Decreasing trend
        }

        let df = Arc::new(
            df![
                "FL1-A" => increasing_data,
                "FL2-A" => decreasing_data,
            ]
            .unwrap(),
        );

        let fcs = SimpleFcs {
            data_frame: df,
            parameter_metadata: HashMap::new(),
        };

        let breaks = (0..10)
            .map(|i| (i * 100, (i + 1) * 100))
            .collect::<Vec<_>>();

        let config = MonotonicConfig::default();

        let result = find_increasing_decreasing_channels(
            &fcs,
            &["FL1-A".to_string(), "FL2-A".to_string()],
            &breaks,
            &config,
        )
        .unwrap();

        // Should detect both types
        assert!(
            !result.increasing.is_empty(),
            "Should detect increasing channel"
        );
        assert!(
            !result.decreasing.is_empty(),
            "Should detect decreasing channel"
        );
        // When both types exist, 'both' should contain all problematic channels
        assert!(
            !result.both.is_empty(),
            "Should populate 'both' when both types exist"
        );
        assert!(
            result.both.len() == 2,
            "Both should contain both problematic channels"
        );
        assert!(result.both.contains(&"FL1-A".to_string()));
        assert!(result.both.contains(&"FL2-A".to_string()));
    }
}
