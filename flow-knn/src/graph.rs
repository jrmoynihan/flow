//! Portable k-NN graph artifact.

use crate::config::DistanceMetric;
use crate::error::KnnError;

/// Neighbours of a single query point.
#[derive(Debug, Clone)]
pub struct NeighborList {
    /// Indices of k nearest neighbours (excluding self), ascending distance order.
    pub indices: Vec<u32>,
    /// Distances corresponding to each index.
    pub distances: Vec<f32>,
}

/// Portable k-nearest-neighbour graph: per-point indices and distances only.
#[derive(Debug, Clone)]
pub struct KnnGraph {
    pub neighbors: Vec<NeighborList>,
    pub n: usize,
    pub k: usize,
    pub metric: DistanceMetric,
    pub provenance: Option<String>,
}

impl KnnGraph {
    /// Minimum `k` for PaCMAP mid-near candidate window: `min(n_neighbors + 50, n − 1)`.
    pub fn required_k_for_pacmap(n: usize, n_neighbors: usize) -> usize {
        let n_nb = n_neighbors.min(n.saturating_sub(1));
        (n_nb + 50).min(n.saturating_sub(1))
    }

    /// Validate graph shape / metric for a consumer that needs `required_k` neighbours.
    pub fn validate(
        &self,
        data_n: usize,
        required_k: usize,
        metric: DistanceMetric,
    ) -> Result<(), KnnError> {
        if self.n != data_n || self.neighbors.len() != data_n {
            return Err(KnnError::GraphSizeMismatch {
                graph_n: self.n,
                neighbors_len: self.neighbors.len(),
                data_n,
            });
        }
        if self.k < required_k {
            return Err(KnnError::GraphInsufficientK {
                graph_k: self.k,
                required_k,
            });
        }
        if self.metric != metric {
            return Err(KnnError::GraphMetricMismatch {
                graph: self.metric,
                requested: metric,
            });
        }
        Ok(())
    }

    /// PaCMAP-oriented validation helper.
    pub fn validate_for_pacmap(
        &self,
        data_n: usize,
        n_neighbors: usize,
        config_metric: DistanceMetric,
    ) -> Result<(), KnnError> {
        self.validate(
            data_n,
            Self::required_k_for_pacmap(data_n, n_neighbors),
            config_metric,
        )
    }
}
