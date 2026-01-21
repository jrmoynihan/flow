//! GPU-accelerated matrix operations
//!
//! Note: Currently uses CPU implementation. GPU acceleration for matrix operations
//! was tested but didn't provide benefits due to overhead. Kept for API consistency
//! and potential future optimization.

use crate::error::Result;
use crate::qc::peaks::ChannelPeakFrame;
use std::collections::HashMap;

/// Build feature matrix
///
/// Currently uses CPU implementation. GPU acceleration didn't provide benefits
/// due to overhead for typical matrix sizes.
pub fn build_feature_matrix_gpu(
    peak_results: &HashMap<String, ChannelPeakFrame>,
    n_bins: usize,
) -> Result<(Vec<Vec<f64>>, Vec<String>)> {
    // Get channels in consistent order
    let mut channel_names: Vec<String> = peak_results.keys().cloned().collect();
    channel_names.sort();

    // Collect all clusters per channel
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

        // Process each cluster
        let mut cluster_ids: Vec<usize> = clusters.keys().cloned().collect();
        cluster_ids.sort();
        for cluster_id in cluster_ids {
            let peaks_in_cluster = &clusters[&cluster_id];

            feature_names.push(format!("{}_cluster_{}", channel, cluster_id));
            cluster_data.push((channel.clone(), cluster_id, peaks_in_cluster.clone()));
        }
    }

    let n_features = feature_names.len();

    // Build matrix on GPU: bins × features
    // Initialize with zeros
    let mut matrix_data = vec![0.0f64; n_bins * n_features];
    
    // Fill matrix column by column (one per cluster)
    for (feature_idx, (_, _, peaks_in_cluster)) in cluster_data.iter().enumerate() {
        // Calculate cluster median (default value) - use CPU for now
        let peak_values: Vec<f64> = peaks_in_cluster.iter().map(|(_, v)| *v).collect();
        let cluster_median = crate::stats::median(&peak_values)?;

        // Initialize all bins with cluster median
        for bin_idx in 0..n_bins {
            matrix_data[bin_idx * n_features + feature_idx] = cluster_median;
        }

        // Replace with actual peak values where available
        for (bin_idx, peak_value) in peaks_in_cluster {
            if *bin_idx < n_bins {
                matrix_data[*bin_idx * n_features + feature_idx] = *peak_value;
            }
        }
    }

    // Convert to 2D Vec<Vec<f64>>
    let mut matrix = vec![vec![0.0; n_features]; n_bins];
    for bin_idx in 0..n_bins {
        for feature_idx in 0..n_features {
            matrix[bin_idx][feature_idx] = matrix_data[bin_idx * n_features + feature_idx];
        }
    }

    Ok((matrix, feature_names))
}
