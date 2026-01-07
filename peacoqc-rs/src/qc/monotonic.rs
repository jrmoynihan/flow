use polars::prelude::*;
use crate::error::{PeacoQCError, Result};
use crate::PeacoQCData;
use crate::stats::median;

/// Configuration for monotonic channel detection
#[derive(Debug, Clone)]
pub struct MonotonicConfig {
    /// Correlation threshold for detecting trend (default: 0.7)
    /// Absolute correlation > threshold indicates monotonic channel
    pub correlation_threshold: f64,
}

impl Default for MonotonicConfig {
    fn default() -> Self {
        Self {
            correlation_threshold: 0.7,
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
    
    /// Channels with both increasing and decreasing (unstable)
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
/// # Algorithm
/// 1. Split data into bins
/// 2. Calculate median value per bin
/// 3. Compute Spearman rank correlation between bin index and median
/// 4. If |correlation| > threshold, flag as monotonic
///
/// # Arguments
/// * `fcs` - FCS file data
/// * `channels` - Channels to check
/// * `breaks` - Bin boundaries (start, end) indices
/// * `config` - Configuration with correlation threshold
pub fn find_increasing_decreasing_channels(
    fcs: &SimpleFcs,
    channels: &[String],
    breaks: &[(usize, usize)],
    config: &MonotonicConfig,
) -> Result<MonotonicResult> {
    let mut increasing = Vec::new();
    let mut decreasing = Vec::new();
    let mut both = Vec::new();
    let mut correlations = std::collections::HashMap::new();
    
    eprintln!("Checking {} channels for monotonic trends...", channels.len());
    
    for channel in channels {
        // Get channel data
        let series = fcs.column(channel)
            .ok_or_else(|| PeacoQCError::ChannelNotFound(channel.clone()))?;
        
        let values = series.f64()
            .map_err(|_| PeacoQCError::InvalidChannel(channel.clone()))?;
        
        let data: Vec<f64> = values.into_iter()
            .filter_map(|x| x)
            .collect();
        
        // Calculate median per bin
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
            continue; // Need at least 3 points for correlation
        }
        
        // Compute Spearman rank correlation
        let bin_indices: Vec<f64> = (0..bin_medians.len()).map(|i| i as f64).collect();
        let correlation = spearman_correlation(&bin_indices, &bin_medians)?;
        
        correlations.insert(channel.clone(), correlation);
        
        // Classify channel
        if correlation.abs() > config.correlation_threshold {
            if correlation > 0.0 {
                increasing.push(channel.clone());
            } else {
                decreasing.push(channel.clone());
            }
        }
    }
    
    if !increasing.is_empty() {
        eprintln!("⚠️ Increasing channels detected: {:?}", increasing);
    }
    if !decreasing.is_empty() {
        eprintln!("⚠️ Decreasing channels detected: {:?}", decreasing);
    }
    
    Ok(MonotonicResult {
        increasing,
        decreasing,
        both,
        correlations,
    })
}

/// Calculate Spearman rank correlation coefficient
///
/// Spearman's rho measures monotonic relationship between two variables
/// rho = 1 - (6 * sum(d_i^2)) / (n * (n^2 - 1))
/// where d_i = rank difference for each pair
fn spearman_correlation(x: &[f64], y: &[f64]) -> Result<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return Err(PeacoQCError::StatsError(
            "Invalid data for correlation".to_string()
        ));
    }
    
    let n = x.len();
    
    // Get ranks
    let x_ranks = rank_data(x);
    let y_ranks = rank_data(y);
    
    // Calculate sum of squared differences
    let d_squared_sum: f64 = x_ranks.iter()
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
    let mut indexed: Vec<(usize, f64)> = data.iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    
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
    use polars::df;
    use std::collections::HashMap;
    use crate::fcs::ParameterMetadata;
    
    #[test]
    fn test_spearman_perfect_positive() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        
        let corr = spearman_correlation(&x, &y).unwrap();
        assert!((corr - 1.0).abs() < 1e-6, "Perfect positive correlation should be 1.0");
    }
    
    #[test]
    fn test_spearman_perfect_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        
        let corr = spearman_correlation(&x, &y).unwrap();
        assert!((corr + 1.0).abs() < 1e-6, "Perfect negative correlation should be -1.0");
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
        
        let df = df![
            "FL1-A" => data,
        ].unwrap();
        
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
            &["FL1-A".to_string()],
            &breaks,
            &config,
        ).unwrap();
        
        // Should detect increasing trend
        assert!(!result.increasing.is_empty(), "Should detect increasing channel");
        assert!(result.has_issues());
    }
}
