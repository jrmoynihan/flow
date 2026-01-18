//! SD-based Isolation Tree (matches R's isolationTreeSD)
//!
//! This implementation matches the R PeacoQC package's isolation tree algorithm,
//! which is fundamentally different from standard Isolation Forests:
//!
//! - **Single tree**, not a forest
//! - **Deterministic** (no randomness applied)
//! - Uses **standard deviation reduction** as gain metric
//! - Finds the **largest homogeneous group** (inliers), not outliers
//!
//! From the PeacoQC paper:
//! > "Only one tree is used and no randomness is applied, as we are not concerned
//! > about overfitting issues or the generation of a model since the model only
//! > has to be used for this specific sample."

use crate::error::{PeacoQCError, Result};
use crate::qc::peaks::ChannelPeakFrame;
use rayon::prelude::*;
use std::collections::HashMap;

/// Configuration for SD-based Isolation Tree
#[derive(Debug, Clone)]
pub struct IsolationTreeConfig {
    /// Gain threshold (default: 0.6)
    ///
    /// **Tradeoff**: By lowering the IT limit, the algorithm will be more strict
    /// and outliers will be removed sooner.
    pub it_limit: f64,

    /// Minimum number of bins required to run IT (default: 150)
    ///
    /// The isolation tree can be sensitive to a low number of bins and is by
    /// default not used when less than `force_it` bins are available, as it
    /// can remove too much of the data.
    pub force_it: usize,
}

impl Default for IsolationTreeConfig {
    fn default() -> Self {
        Self {
            it_limit: 0.6,
            force_it: 150,
        }
    }
}

/// Result of Isolation Tree analysis
#[derive(Debug)]
pub struct IsolationTreeResult {
    /// Boolean mask indicating outlier bins (true = outlier, false = good)
    /// Bins in the largest homogeneous node are marked as good (false)
    pub outlier_bins: Vec<bool>,

    /// Tree structure for diagnostics
    pub tree: Vec<TreeNode>,

    /// Statistics
    pub stats: TreeStats,
}

/// A node in the SD-based isolation tree
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: usize,
    pub left_child: Option<usize>,
    pub right_child: Option<usize>,
    pub gain: Option<f64>,
    pub split_column: Option<String>,
    pub split_value: Option<f64>,
    pub depth: usize,
    pub path_length: Option<f64>,
    pub n_datapoints: usize,
}

#[derive(Debug)]
pub struct TreeStats {
    pub n_bins: usize,
    pub n_features: usize,
    pub max_depth: usize,
    pub largest_node_size: usize,
    pub largest_node_id: usize,
}

/// Euler-Mascheroni constant for avgPL calculation
const EULER_MASCHERONI: f64 = 0.5772156649;

/// Average path length in a BST with n nodes (matches R's avgPL)
///
/// ```r
/// avgPL <- function(n_datapoints){
///     if (n_datapoints -1 == 0){
///         AVG <- 0
///     } else {
///         AVG <- 2*(log(n_datapoints - 1) +  0.5772156649) -
///             (2*(n_datapoints -1))/(n_datapoints)
///     }
///     return (AVG)
/// }
/// ```
fn avg_path_length(n: usize) -> f64 {
    if n <= 1 {
        0.0
    } else {
        let n_f = n as f64;
        2.0 * ((n_f - 1.0).ln() + EULER_MASCHERONI) - (2.0 * (n_f - 1.0)) / n_f
    }
}

/// Calculate standard deviation (sample SD, matching R's stats::sd)
fn std_dev(data: &[f64]) -> f64 {
    if data.len() <= 1 {
        return 0.0;
    }
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    variance.sqrt()
}

/// Detect anomalous bins using SD-based Isolation Tree
///
/// # Algorithm (from R's isolationTreeSD)
/// 1. Build feature matrix from peak values (bins × channels)
/// 2. Start with all bins in root node
/// 3. For each node, find best split using SD-gain metric
/// 4. Split maximizes: `gain = (base_sd - mean(sd_left, sd_right)) / base_sd`
/// 5. Only split if gain > gain_limit
/// 6. Continue until max_depth or no valid splits
/// 7. Find largest leaf node - these bins are "good"
/// 8. All other bins are outliers
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

    // Build feature matrix: bins × (channels × clusters)
    // Each cluster gets its own column, matching R's ExtractPeakValues
    let (feature_matrix, feature_names) = build_feature_matrix(peak_results, n_bins)?;
    let n_features = feature_matrix[0].len();

    eprintln!(
        "Running SD-based Isolation Tree: {} bins, {} features (clusters)",
        n_bins, n_features
    );

    // Build the SD-based isolation tree
    let (tree, selection) =
        build_isolation_tree_sd(&feature_matrix, &feature_names, config.it_limit)?;

    // Find the largest leaf node (node with most datapoints and a path_length)
    let largest_node = tree
        .iter()
        .filter(|node| node.path_length.is_some())
        .max_by_key(|node| node.n_datapoints)
        .ok_or_else(|| PeacoQCError::StatsError("No leaf nodes found".to_string()))?;

    let largest_node_id = largest_node.id;
    let largest_node_size = largest_node.n_datapoints;

    // Get the selection mask for the largest node
    // Bins in this node are "good" (not outliers)
    let good_bins = &selection[largest_node_id];

    // Create outlier mask (true = outlier = NOT in largest node)
    let outlier_bins: Vec<bool> = good_bins.iter().map(|&in_node| !in_node).collect();

    let n_outliers = outlier_bins.iter().filter(|&&x| x).count();
    eprintln!(
        "IT detected {} outlier bins ({:.1}%), largest node has {} bins",
        n_outliers,
        (n_outliers as f64 / n_bins as f64) * 100.0,
        largest_node_size
    );

    let max_depth = tree.iter().map(|n| n.depth).max().unwrap_or(0);

    Ok(IsolationTreeResult {
        outlier_bins,
        tree,
        stats: TreeStats {
            n_bins,
            n_features,
            max_depth,
            largest_node_size,
            largest_node_id,
        },
    })
}

/// Build feature matrix from peak detection results
///
/// This matches R's ExtractPeakValues and all_peaks construction:
/// - Each cluster gets its own column (one per cluster per channel)
/// - For each cluster, bins are filled with actual peak values where available
/// - Bins without peaks use the cluster median (default value)
///
/// Returns: (matrix, feature_names) where matrix is Vec<Vec<f64>> (bins × features)
/// Feature names are formatted as "{channel}_cluster_{cluster_id}"
pub fn build_feature_matrix(
    peak_results: &HashMap<String, ChannelPeakFrame>,
    n_bins: usize,
) -> Result<(Vec<Vec<f64>>, Vec<String>)> {
    // Get channels in consistent order
    let mut channel_names: Vec<String> = peak_results.keys().cloned().collect();
    channel_names.sort();

    // Collect all clusters per channel (matching R's ExtractPeakValues)
    let mut feature_names = Vec::new();
    let mut cluster_data: Vec<(String, usize, Vec<(usize, f64)>)> = Vec::new();

    for channel in &channel_names {
        let peak_frame = &peak_results[channel];

        // Group peaks by cluster
        let mut clusters: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
        for peak in &peak_frame.peaks {
            clusters
                .entry(peak.cluster)
                .or_default()
                .push((peak.bin, peak.peak_value));
        }

        // Process each cluster (matching R's ExtractPeakValues)
        let mut cluster_ids: Vec<usize> = clusters.keys().cloned().collect();
        cluster_ids.sort();
        for cluster_id in cluster_ids {
            let peaks_in_cluster = &clusters[&cluster_id];

            feature_names.push(format!("{}_cluster_{}", channel, cluster_id));
            cluster_data.push((channel.clone(), cluster_id, peaks_in_cluster.clone()));
        }
    }

    // Build matrix: bins × features (clusters)
    // Each column is a cluster trajectory
    let n_features = feature_names.len();
    let mut matrix = vec![vec![0.0; n_features]; n_bins];

    // Fill matrix column by column (one per cluster)
    for (feature_idx, (channel, cluster_id, peaks_in_cluster)) in cluster_data.iter().enumerate() {
        // Calculate cluster median (default value)
        let peak_values: Vec<f64> = peaks_in_cluster.iter().map(|(_, v)| *v).collect();
        let cluster_median = crate::stats::median(&peak_values)?;

        // Initialize all bins with cluster median
        for bin_idx in 0..n_bins {
            matrix[bin_idx][feature_idx] = cluster_median;
        }

        // Replace with actual peak values where available
        for (bin_idx, peak_value) in peaks_in_cluster {
            if *bin_idx < n_bins {
                matrix[*bin_idx][feature_idx] = *peak_value;
            }
        }
    }

    Ok((matrix, feature_names))
}

/// Build SD-based isolation tree (matches R's isolationTreeSD)
///
/// Returns: (tree_nodes, selection_matrix)
/// - tree_nodes: Vector of TreeNode describing the tree structure
/// - selection_matrix: Vec<Vec<bool>> where selection[node_id][bin_idx] = true if bin is in node
fn build_isolation_tree_sd(
    data: &[Vec<f64>],
    feature_names: &[String],
    initial_gain_limit: f64,
) -> Result<(Vec<TreeNode>, Vec<Vec<bool>>)> {
    let n_bins = data.len();
    let max_depth = (n_bins as f64).log2().ceil() as usize;

    // Initialize root node
    let mut tree = vec![TreeNode {
        id: 0,
        left_child: None,
        right_child: None,
        gain: None,
        split_column: None,
        split_value: None,
        depth: 0,
        path_length: None,
        n_datapoints: n_bins,
    }];

    // Selection matrix: selection[node_id][bin_idx] = true if bin is in this node
    let mut selection: Vec<Vec<bool>> = vec![vec![true; n_bins]];

    // Track which nodes still need to be split
    let mut nodes_to_split: Vec<usize> = vec![0];

    // The gain_limit can be updated after each successful split (R: line 363)
    let mut gain_limit = initial_gain_limit;

    while let Some(node_idx) = nodes_to_split.pop() {
        let node = &tree[node_idx];
        let depth = node.depth;

        // Get rows (bin indices) in this node
        let rows: Vec<usize> = selection[node_idx]
            .iter()
            .enumerate()
            .filter_map(|(i, &in_node)| if in_node { Some(i) } else { None })
            .collect();

        // Check stop conditions
        if rows.len() <= 3 || depth >= max_depth {
            // Make this a leaf node
            let path_length = avg_path_length(rows.len()) + depth as f64;
            tree[node_idx].path_length = Some(path_length);
            tree[node_idx].n_datapoints = rows.len();
            continue;
        }

        // Find best split across all columns
        let best_split = find_best_split_parallel(data, &rows, feature_names, gain_limit);

        match best_split {
            Some((col_idx, split_value, gain)) => {
                // Check if split actually separates data
                let left_rows: Vec<usize> = rows
                    .iter()
                    .filter(|&&r| data[r][col_idx] <= split_value)
                    .copied()
                    .collect();
                let right_rows: Vec<usize> = rows
                    .iter()
                    .filter(|&&r| data[r][col_idx] > split_value)
                    .copied()
                    .collect();

                if left_rows.is_empty()
                    || right_rows.is_empty()
                    || left_rows.len() == rows.len()
                    || right_rows.len() == rows.len()
                {
                    // Degenerate split - make leaf
                    let path_length = avg_path_length(rows.len()) + depth as f64;
                    tree[node_idx].path_length = Some(path_length);
                    tree[node_idx].n_datapoints = rows.len();
                    continue;
                }

                // Create child nodes
                let left_id = tree.len();
                let right_id = tree.len() + 1;

                // Update current node
                tree[node_idx].left_child = Some(left_id);
                tree[node_idx].right_child = Some(right_id);
                tree[node_idx].gain = Some(gain);
                tree[node_idx].split_column = Some(feature_names[col_idx].clone());
                tree[node_idx].split_value = Some(split_value);
                tree[node_idx].n_datapoints = rows.len();

                // Update gain_limit for next iterations (R: line 363)
                gain_limit = gain;

                // Create selection masks for children
                let mut left_selection = vec![false; n_bins];
                let mut right_selection = vec![false; n_bins];

                for &r in &left_rows {
                    left_selection[r] = true;
                }
                for &r in &right_rows {
                    right_selection[r] = true;
                }

                // Add child nodes
                tree.push(TreeNode {
                    id: left_id,
                    left_child: None,
                    right_child: None,
                    gain: None,
                    split_column: None,
                    split_value: None,
                    depth: depth + 1,
                    path_length: None,
                    n_datapoints: left_rows.len(),
                });

                tree.push(TreeNode {
                    id: right_id,
                    left_child: None,
                    right_child: None,
                    gain: None,
                    split_column: None,
                    split_value: None,
                    depth: depth + 1,
                    path_length: None,
                    n_datapoints: right_rows.len(),
                });

                selection.push(left_selection);
                selection.push(right_selection);

                // Add children to processing queue
                nodes_to_split.push(left_id);
                nodes_to_split.push(right_id);
            }
            None => {
                // No valid split found - make leaf
                let path_length = avg_path_length(rows.len()) + depth as f64;
                tree[node_idx].path_length = Some(path_length);
                tree[node_idx].n_datapoints = rows.len();
            }
        }
    }

    Ok((tree, selection))
}

/// Find the best split across all columns using SD-gain metric
/// Parallelized: each column is evaluated independently
fn find_best_split_parallel(
    data: &[Vec<f64>],
    rows: &[usize],
    _feature_names: &[String],
    gain_limit: f64,
) -> Option<(usize, f64, f64)> {
    let n_features = data[0].len();

    // Process each column in parallel
    let column_results: Vec<Option<(usize, f64, f64)>> = (0..n_features)
        .into_par_iter()
        .map(|col| find_best_split_for_column(data, rows, col, gain_limit))
        .collect();

    // Find the best split across all columns
    column_results
        .into_iter()
        .flatten()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
}

/// Find the best split for a single column (matches R's inner loop)
///
/// R implementation (lines 299-335):
/// ```r
/// for(col in seq_len(ncol(x))){
///     x_col <- sort(x[rows, col])
///     base_sd <- stats::sd(x_col)
///     for(i in seq_len((length(x_col)-1))){
///         sd_1 <- stats::sd(x_col[seq_len(i)])
///         sd_2 <- stats::sd(x_col[c((i+1):length(x_col))])
///         if (i == 1){ sd_1 <- 0 }
///         else if (i == length(x_col) - 1){ sd_2 <- 0 }
///         gain <- (base_sd - mean(c(sd_1, sd_2)))/base_sd
///     }
/// }
/// ```
fn find_best_split_for_column(
    data: &[Vec<f64>],
    rows: &[usize],
    col: usize,
    gain_limit: f64,
) -> Option<(usize, f64, f64)> {
    // Get and sort values for this column
    let mut values: Vec<f64> = rows.iter().map(|&r| data[r][col]).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = values.len();
    if n < 2 {
        return None;
    }

    let base_sd = std_dev(&values);
    if base_sd == 0.0 {
        return None;
    }

    let mut best_gain = gain_limit;
    let mut best_split_value = None;

    // Try all possible split points (R: line 309)
    for i in 1..n {
        let left = &values[..i];
        let right = &values[i..];

        // Calculate SDs with R's edge case handling (lines 316-319)
        let sd_1 = if i == 1 { 0.0 } else { std_dev(left) };
        let sd_2 = if i == n - 1 { 0.0 } else { std_dev(right) };

        // Gain formula (R: line 321)
        let mean_child_sd = (sd_1 + sd_2) / 2.0;
        let gain = (base_sd - mean_child_sd) / base_sd;

        if gain.is_finite() && gain >= best_gain {
            best_gain = gain;
            // R uses x_col[i] as split value (line 328), where i is 1-indexed
            // R's loop: for(i in seq_len((length(x_col)-1))) means i goes from 1 to n-1
            // R's split: left = x_col[1:i], right = x_col[(i+1):length(x_col)]
            // So split_v = x_col[i] is the last value in the left partition
            // In Rust, i goes from 1 to n-1 (0-indexed: 1..n)
            // So values[i-1] is the value at position i-1 (0-indexed) = position i (1-indexed)
            // This matches R's x_col[i]
            best_split_value = Some(values[i - 1]);
        }
    }

    best_split_value.map(|v| (col, v, best_gain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qc::peaks::PeakInfo;

    #[test]
    fn test_avg_path_length() {
        // R's avgPL function:
        // if (n-1 == 0) AVG <- 0
        // else AVG <- 2*(log(n-1) + 0.5772156649) - (2*(n-1))/n

        assert!((avg_path_length(1) - 0.0).abs() < 1e-6);

        // For n=2: 2*(ln(1) + 0.5772) - (2*1)/2 = 2*0.5772 - 1 = 0.1544
        let apl_2 = avg_path_length(2);
        assert!((apl_2 - 0.1544).abs() < 0.02, "avgPL(2) = {}", apl_2);

        assert!(avg_path_length(100) > avg_path_length(10));

        // Test that formula is monotonically increasing for n > 1
        let apl_10 = avg_path_length(10);
        let apl_100 = avg_path_length(100);
        assert!(apl_10 > 0.0, "avgPL(10) should be positive: {}", apl_10);
        assert!(apl_100 > apl_10, "avgPL should increase with n");
    }

    #[test]
    fn test_std_dev() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = std_dev(&data);
        // R: sd(c(2,4,4,4,5,5,7,9)) = 2.138
        assert!((sd - 2.138).abs() < 0.01, "sd = {}", sd);
    }

    #[test]
    fn test_isolation_tree_basic() {
        // Create synthetic peak data
        let mut peaks = Vec::new();
        for bin in 0..200 {
            let peak_value = if bin >= 50 && bin < 60 {
                1000.0 // Outlier region
            } else {
                100.0 + (bin as f64) * 0.5 // Normal trend
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
            force_it: 50,
            it_limit: 0.6,
        };

        let result = isolation_tree_detect(&peak_results, 200, &config).unwrap();

        // The outlier region (bins 50-59) should be marked as outliers
        // and the majority of bins should be marked as good
        let n_good = result.outlier_bins.iter().filter(|&&x| !x).count();
        assert!(
            n_good > 100,
            "Most bins should be good, but only {} are",
            n_good
        );
    }

    #[test]
    fn test_build_feature_matrix_old_behavior() {
        // This test documents the OLD (incorrect) behavior
        // It should now have one column per cluster, not per channel
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

        let (matrix, names) = build_feature_matrix(&peak_results, 5).unwrap();

        assert_eq!(matrix.len(), 5); // 5 bins
        // NEW: Should have 2 columns (one per cluster per channel)
        // Since each channel has 1 cluster, we get 2 columns total
        assert_eq!(matrix[0].len(), 2, "Should have 2 features (2 channels × 1 cluster each)");
        assert_eq!(names.len(), 2);
        assert!(matrix[0][0] > 0.0);
        assert!(matrix[0][1] > 0.0);
        
        // Verify feature names contain cluster information
        assert!(names[0].contains("_cluster_"), "Feature name should contain '_cluster_'");
        assert!(names[1].contains("_cluster_"), "Feature name should contain '_cluster_'");
    }
}
