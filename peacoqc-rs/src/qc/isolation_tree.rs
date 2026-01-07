use crate::error::{PeacoQCError, Result};
use crate::qc::peaks::ChannelPeakFrame;
use std::collections::HashMap;
use rand::Rng;

/// Configuration for Isolation Tree anomaly detection
#[derive(Debug, Clone)]
pub struct IsolationTreeConfig {
    /// Anomaly score threshold (default: 0.6)
    /// Higher values = more strict (fewer outliers detected)
    pub it_limit: f64,
    
    /// Minimum number of bins required to run IT (default: 150)
    pub force_it: usize,
    
    /// Number of trees in the forest (default: 100)
    pub n_trees: usize,
    
    /// Sample size for each tree (default: 256)
    pub sample_size: usize,
}

impl Default for IsolationTreeConfig {
    fn default() -> Self {
        Self {
            it_limit: 0.6,
            force_it: 150,
            n_trees: 100,
            sample_size: 256,
        }
    }
}

/// Result of Isolation Tree analysis
#[derive(Debug)]
pub struct IsolationTreeResult {
    /// Boolean mask indicating outlier bins (true = outlier)
    pub outlier_bins: Vec<bool>,
    
    /// Anomaly scores per bin (0-1, higher = more anomalous)
    pub anomaly_scores: Vec<f64>,
    
    /// Tree statistics for diagnostics
    pub tree_stats: TreeStats,
}

#[derive(Debug)]
pub struct TreeStats {
    pub n_trees: usize,
    pub n_bins: usize,
    pub n_features: usize,
    pub mean_score: f64,
    pub threshold: f64,
}

/// Detect anomalous bins using Isolation Forest
///
/// # Algorithm
/// 1. Build feature matrix from peak values (bins × channels)
/// 2. Train isolation forest on peak matrix
/// 3. Score each bin based on average path length
/// 4. Mark bins with score > threshold as outliers
///
/// Isolation forests work by:
/// - Randomly selecting features and split values
/// - Anomalies are isolated earlier (shorter path length)
/// - Anomaly score = 2^(-E(h(x))/c(n))
///   where E(h(x)) is expected path length, c(n) is average path length
///
/// # Arguments
/// * `peak_results` - Peak detection results per channel
/// * `n_bins` - Total number of bins
/// * `config` - Isolation Tree configuration
pub fn isolation_tree_detect(
    peak_results: &HashMap<String, ChannelPeakFrame>,
    n_bins: usize,
    config: &IsolationTreeConfig,
) -> Result<IsolationTreeResult> {
    // Check if we have enough bins
    if n_bins < config.force_it {
        return Err(PeacoQCError::InsufficientData {
            min: config.force_it,
            actual: n_bins,
        });
    }
    
    if peak_results.is_empty() {
        return Err(PeacoQCError::NoPeaksDetected);
    }
    
    // Build feature matrix: bins × channels
    let feature_matrix = build_feature_matrix(peak_results, n_bins)?;
    let n_features = feature_matrix[0].len();
    
    eprintln!("Running Isolation Forest: {} bins, {} features", n_bins, n_features);
    
    // Train isolation forest and get anomaly scores
    let anomaly_scores = isolation_forest_scores(&feature_matrix, config)?;
    
    // Calculate statistics
    let mean_score = anomaly_scores.iter().sum::<f64>() / anomaly_scores.len() as f64;
    
    // Mark outliers based on threshold
    let outlier_bins: Vec<bool> = anomaly_scores.iter()
        .map(|&score| score > config.it_limit)
        .collect();
    
    let n_outliers = outlier_bins.iter().filter(|&&x| x).count();
    eprintln!("IT detected {} outlier bins ({:.1}%)", 
              n_outliers, 
              (n_outliers as f64 / n_bins as f64) * 100.0);
    
    Ok(IsolationTreeResult {
        outlier_bins,
        anomaly_scores,
        tree_stats: TreeStats {
            n_trees: config.n_trees,
            n_bins,
            n_features,
            mean_score,
            threshold: config.it_limit,
        },
    })
}

/// Build feature matrix from peak detection results
/// 
/// Returns: Vec<Vec<f64>> where outer vec is bins, inner vec is features (one per channel)
fn build_feature_matrix(
    peak_results: &HashMap<String, ChannelPeakFrame>,
    n_bins: usize,
) -> Result<Vec<Vec<f64>>> {
    // Get channels in consistent order
    let mut channels: Vec<&String> = peak_results.keys().collect();
    channels.sort();
    
    // Initialize matrix with NaN (will be replaced with median if bin has no peak)
    let mut matrix = vec![vec![f64::NAN; channels.len()]; n_bins];
    
    // Fill in peak values
    for (channel_idx, channel) in channels.iter().enumerate() {
        let peak_frame = &peak_results[*channel];
        
        // Map bin → peak value (using cluster median)
        let mut bin_peaks: HashMap<usize, Vec<f64>> = HashMap::new();
        for peak in &peak_frame.peaks {
            bin_peaks.entry(peak.bin)
                .or_insert_with(Vec::new)
                .push(peak.peak_value);
        }
        
        // Calculate representative value per bin
        for (bin_idx, peaks) in bin_peaks {
            if !peaks.is_empty() {
                let median = crate::stats::median(&peaks)?;
                if bin_idx < n_bins {
                    matrix[bin_idx][channel_idx] = median;
                }
            }
        }
    }
    
    // Replace NaN with channel median
    for channel_idx in 0..channels.len() {
        let values: Vec<f64> = matrix.iter()
            .map(|row| row[channel_idx])
            .filter(|x| x.is_finite())
            .collect();
        
        if !values.is_empty() {
            let channel_median = crate::stats::median(&values)?;
            for row in &mut matrix {
                if !row[channel_idx].is_finite() {
                    row[channel_idx] = channel_median;
                }
            }
        }
    }
    
    Ok(matrix)
}

/// Compute anomaly scores using Isolation Forest algorithm
/// 
/// Implements a simplified but effective isolation forest:
/// - Builds multiple isolation trees
/// - Each tree recursively partitions data
/// - Anomaly score based on average path length
fn isolation_forest_scores(
    data: &[Vec<f64>],
    config: &IsolationTreeConfig,
) -> Result<Vec<f64>> {
    use rand::seq::SliceRandom;
    
    let n_samples = data.len();
    let n_features = data[0].len();
    
    // Expected path length for given sample size (average case)
    let c_n = average_path_length(config.sample_size.min(n_samples));
    
    let mut rng = rand::thread_rng();
    let mut path_lengths = vec![0.0; n_samples];
    
    // Build multiple trees
    for _tree_idx in 0..config.n_trees {
        // Sample data for this tree
        let indices: Vec<usize> = (0..n_samples).collect();
        let sampled_indices: Vec<usize> = indices.choose_multiple(&mut rng, config.sample_size.min(n_samples))
            .copied()
            .collect();
        
        let sampled_data: Vec<&Vec<f64>> = sampled_indices.iter()
            .map(|&i| &data[i])
            .collect();
        
        // Build isolation tree
        let tree = build_isolation_tree(&sampled_data, n_features, 0, 10, &mut rng);
        
        // Score all samples
        for (i, sample) in data.iter().enumerate() {
            let depth = score_sample(&tree, sample);
            path_lengths[i] += depth;
        }
    }
    
    // Average path lengths across trees
    for pl in &mut path_lengths {
        *pl /= config.n_trees as f64;
    }
    
    // Convert to anomaly scores: s = 2^(-E(h(x))/c(n))
    let anomaly_scores: Vec<f64> = path_lengths.iter()
        .map(|&pl| 2.0_f64.powf(-pl / c_n))
        .collect();
    
    Ok(anomaly_scores)
}

/// Simple Isolation Tree node
#[derive(Debug)]
enum IsolationNode {
    Internal {
        feature: usize,
        split_value: f64,
        left: Box<IsolationNode>,
        right: Box<IsolationNode>,
    },
    Leaf {
        size: usize,
    },
}

/// Build an isolation tree recursively
fn build_isolation_tree(
    data: &[&Vec<f64>],
    n_features: usize,
    depth: usize,
    max_depth: usize,
    rng: &mut impl Rng,
) -> IsolationNode {
    // Stop conditions
    if data.len() <= 1 || depth >= max_depth {
        return IsolationNode::Leaf { size: data.len() };
    }
    
    // Randomly select feature and split value
    let feature = rng.gen_range(0..n_features);
    
    let values: Vec<f64> = data.iter().map(|row| row[feature]).collect();
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    
    if (max_val - min_val).abs() < 1e-10 {
        return IsolationNode::Leaf { size: data.len() };
    }
    
    let split_value = rng.gen_range(min_val..max_val);
    
    // Partition data
    let (left_data, right_data): (Vec<&Vec<f64>>, Vec<&Vec<f64>>) = data.iter()
        .partition(|row| row[feature] < split_value);
    
    if left_data.is_empty() || right_data.is_empty() {
        return IsolationNode::Leaf { size: data.len() };
    }
    
    // Recursively build subtrees
    let left = Box::new(build_isolation_tree(&left_data, n_features, depth + 1, max_depth, rng));
    let right = Box::new(build_isolation_tree(&right_data, n_features, depth + 1, max_depth, rng));
    
    IsolationNode::Internal {
        feature,
        split_value,
        left,
        right,
    }
}

/// Score a sample by traversing the tree
fn score_sample(node: &IsolationNode, sample: &[f64]) -> f64 {
    match node {
        IsolationNode::Leaf { size } => {
            // Return depth + adjustment for remaining samples
            if *size > 1 {
                average_path_length(*size)
            } else {
                0.0
            }
        }
        IsolationNode::Internal { feature, split_value, left, right } => {
            if sample[*feature] < *split_value {
                1.0 + score_sample(left, sample)
            } else {
                1.0 + score_sample(right, sample)
            }
        }
    }
}

/// Average path length in a BST with n nodes
/// c(n) = 2H(n-1) - (2(n-1)/n)
/// where H(i) is the harmonic number
fn average_path_length(n: usize) -> f64 {
    if n <= 1 {
        return 0.0;
    }
    
    let n_f = n as f64;
    let harmonic = (2.0 * (n_f - 1.0)).ln() + 0.5772156649; // Euler-Mascheroni constant
    
    2.0 * harmonic - 2.0 * (n_f - 1.0) / n_f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qc::peaks::PeakInfo;
    
    #[test]
    fn test_isolation_tree_basic() {
        // Create synthetic peak data with one outlier bin
        let mut peaks = Vec::new();
        for bin in 0..20 {
            let peak_value = if bin == 10 {
                1000.0 // Outlier
            } else {
                100.0 + (bin as f64) * 2.0 // Normal trend
            };
            peaks.push(PeakInfo {
                bin,
                peak_value,
                cluster: 1,
            });
        }
        
        let mut peak_results = HashMap::new();
        peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });
        
        let config = IsolationTreeConfig {
            force_it: 10,
            ..Default::default()
        };
        
        let result = isolation_tree_detect(&peak_results, 20, &config).unwrap();
        
        // Should detect bin 10 as outlier
        assert!(result.outlier_bins[10], "Bin 10 should be detected as outlier");
        assert!(result.anomaly_scores[10] > config.it_limit);
    }
    
    #[test]
    fn test_average_path_length() {
        assert!((average_path_length(1) - 0.0).abs() < 1e-6);
        assert!(average_path_length(2) > 0.0);
        assert!(average_path_length(100) > average_path_length(10));
    }
    
    #[test]
    fn test_build_feature_matrix() {
        let mut peaks1 = Vec::new();
        let mut peaks2 = Vec::new();
        
        for bin in 0..5 {
            peaks1.push(PeakInfo {
                bin,
                peak_value: 100.0 + bin as f64,
                cluster: 1,
            });
            peaks2.push(PeakInfo {
                bin,
                peak_value: 200.0 + bin as f64,
                cluster: 1,
            });
        }
        
        let mut peak_results = HashMap::new();
        peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks: peaks1 });
        peak_results.insert("FL2-A".to_string(), ChannelPeakFrame { peaks: peaks2 });
        
        let matrix = build_feature_matrix(&peak_results, 5).unwrap();
        
        assert_eq!(matrix.len(), 5); // 5 bins
        assert_eq!(matrix[0].len(), 2); // 2 channels
        assert!(matrix[0][0] > 0.0);
        assert!(matrix[0][1] > 0.0);
    }
}
