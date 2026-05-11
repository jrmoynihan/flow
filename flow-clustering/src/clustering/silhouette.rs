//! Silhouette score for cluster quality evaluation.
//!
//! Measures how similar a data point is to its own cluster versus the nearest
//! neighboring cluster. Scores range from -1 (misclassified) to +1 (well-clustered).

use super::{ClusteringError, ClusteringResult};

/// Silhouette analysis result for a dataset.
#[derive(Debug, Clone)]
pub struct SilhouetteResult {
    /// Per-point silhouette scores in \[-1, +1\].
    pub scores: Vec<f64>,
    /// Mean silhouette score across all points.
    pub mean_score: f64,
    /// Per-cluster mean silhouette score, indexed by cluster label.
    pub cluster_scores: Vec<f64>,
}

/// Compute the silhouette score for every point.
///
/// # Arguments
/// * `data` - Row-major data matrix, shape `(n_points, n_dims)`
/// * `labels` - Cluster label for each point (0-based, contiguous)
///
/// # Returns
/// [`SilhouetteResult`] with per-point scores and summary statistics.
pub fn silhouette_scores(
    data: &[Vec<f64>],
    labels: &[usize],
) -> ClusteringResult<SilhouetteResult> {
    let n = data.len();
    if n == 0 {
        return Err(ClusteringError::EmptyData);
    }
    if n != labels.len() {
        return Err(ClusteringError::ValidationFailed(format!(
            "data length ({}) != labels length ({})",
            n,
            labels.len()
        )));
    }

    let n_clusters = labels.iter().copied().max().unwrap_or(0) + 1;
    if n_clusters < 2 {
        return Err(ClusteringError::InvalidConfig(
            "silhouette score requires at least 2 clusters".into(),
        ));
    }

    // Pre-compute cluster membership lists
    let mut cluster_members: Vec<Vec<usize>> = vec![Vec::new(); n_clusters];
    for (i, &label) in labels.iter().enumerate() {
        cluster_members[label].push(i);
    }

    let mut scores = vec![0.0_f64; n];

    for i in 0..n {
        let ci = labels[i];
        let same_cluster = &cluster_members[ci];

        // a(i): mean distance to other points in same cluster
        let a_i = if same_cluster.len() <= 1 {
            0.0
        } else {
            let sum: f64 = same_cluster
                .iter()
                .filter(|&&j| j != i)
                .map(|&j| euclidean_dist(&data[i], &data[j]))
                .sum();
            sum / (same_cluster.len() - 1) as f64
        };

        // b(i): min mean distance to any *other* cluster
        let mut b_i = f64::INFINITY;
        for (ck, members) in cluster_members.iter().enumerate() {
            if ck == ci || members.is_empty() {
                continue;
            }
            let mean_dist: f64 = members
                .iter()
                .map(|&j| euclidean_dist(&data[i], &data[j]))
                .sum::<f64>()
                / members.len() as f64;
            if mean_dist < b_i {
                b_i = mean_dist;
            }
        }

        scores[i] = if a_i.max(b_i) == 0.0 {
            0.0
        } else {
            (b_i - a_i) / a_i.max(b_i)
        };
    }

    let mean_score = scores.iter().sum::<f64>() / n as f64;

    let mut cluster_scores = vec![0.0_f64; n_clusters];
    for (ck, members) in cluster_members.iter().enumerate() {
        if members.is_empty() {
            continue;
        }
        cluster_scores[ck] = members.iter().map(|&i| scores[i]).sum::<f64>() / members.len() as f64;
    }

    Ok(SilhouetteResult {
        scores,
        mean_score,
        cluster_scores,
    })
}

/// Compute silhouette scores using a deterministic stride-based subsample for large datasets.
///
/// Picks every `ceil(n / max_points)`-th row to get at most `max_points` samples.
pub fn silhouette_scores_sampled(
    data: &[Vec<f64>],
    labels: &[usize],
    max_points: usize,
) -> ClusteringResult<SilhouetteResult> {
    if data.len() <= max_points {
        return silhouette_scores(data, labels);
    }

    let stride = (data.len() + max_points - 1) / max_points;
    let sampled_data: Vec<Vec<f64>> = data.iter().step_by(stride).cloned().collect();
    let sampled_labels: Vec<usize> = labels.iter().step_by(stride).copied().collect();
    silhouette_scores(&sampled_data, &sampled_labels)
}

#[inline]
fn euclidean_dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_well_separated_clusters() {
        let data: Vec<Vec<f64>> = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.1],
            vec![0.2, 0.0],
            vec![10.0, 10.0],
            vec![10.1, 10.1],
            vec![10.2, 10.0],
        ];
        let labels = vec![0, 0, 0, 1, 1, 1];
        let result = silhouette_scores(&data, &labels).unwrap();
        assert!(
            result.mean_score > 0.9,
            "expected >0.9, got {}",
            result.mean_score
        );
    }

    #[test]
    fn overlapping_clusters_low_score() {
        let data: Vec<Vec<f64>> = vec![
            vec![0.0],
            vec![0.5],
            vec![1.0],
            vec![0.5],
            vec![1.0],
            vec![1.5],
        ];
        let labels = vec![0, 0, 0, 1, 1, 1];
        let result = silhouette_scores(&data, &labels).unwrap();
        assert!(result.mean_score < 0.5);
    }

    #[test]
    fn single_cluster_error() {
        let data: Vec<Vec<f64>> = vec![vec![0.0], vec![1.0]];
        let labels = vec![0, 0];
        assert!(silhouette_scores(&data, &labels).is_err());
    }

    #[test]
    fn sampled_runs_on_small_data() {
        let data: Vec<Vec<f64>> = vec![vec![0.0], vec![0.1], vec![10.0], vec![10.1]];
        let labels = vec![0, 0, 1, 1];
        let result = silhouette_scores_sampled(&data, &labels, 100).unwrap();
        assert!(result.mean_score > 0.5);
    }
}
