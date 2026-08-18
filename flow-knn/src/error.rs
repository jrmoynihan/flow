//! Errors for k-NN graph construction.

use thiserror::Error;

use crate::config::DistanceMetric;

#[derive(Debug, Error)]
pub enum KnnError {
    #[error("Dataset too small: need at least 2 points, got {n}")]
    DatasetTooSmall { n: usize },

    #[error("Dimension mismatch: data slice length {len} is not divisible by d={d}")]
    DimensionMismatch { len: usize, d: usize },

    #[error("Query dimension {query_d} does not match index dimension {index_d}")]
    QueryDimensionMismatch { query_d: usize, index_d: usize },

    #[error("KNN method not implemented: {method}")]
    MethodNotImplemented { method: String },

    #[error("KNN index error: {0}")]
    Index(String),

    #[error("GPU adapter unavailable: {0}")]
    GpuUnavailable(String),

    #[error(
        "KNN graph size mismatch: graph has n={graph_n} (neighbors len {neighbors_len}), \
         data has n={data_n}"
    )]
    GraphSizeMismatch {
        graph_n: usize,
        neighbors_len: usize,
        data_n: usize,
    },

    #[error(
        "KNN graph k={graph_k} is too small; need at least {required_k} \
         (typically n_neighbors + 50, capped by n − 1)"
    )]
    GraphInsufficientK { graph_k: usize, required_k: usize },

    #[error("KNN graph metric {graph:?} does not match requested metric {requested:?}")]
    GraphMetricMismatch {
        graph: DistanceMetric,
        requested: DistanceMetric,
    },

    #[error("KNN graph I/O error: {0}")]
    Io(String),
}
