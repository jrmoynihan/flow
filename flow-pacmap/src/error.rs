//! Error types for flow-pacmap.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PaCMAPError {
    #[error("Dataset too small: need at least 2 points, got {n}")]
    DatasetTooSmall { n: usize },

    #[error("Dimension mismatch: data slice length {len} is not divisible by d={d}")]
    DimensionMismatch { len: usize, d: usize },

    #[error("n={n} exceeds u32::MAX — indices would overflow pair storage")]
    DatasetTooLarge { n: usize },

    #[error("Pair count overflow: n={n} × k={k} would overflow usize")]
    PairCountOverflow { n: usize, k: usize },

    #[error(
        "Insufficient memory: embedding requires ~{required_bytes} bytes but only \
         {available_bytes} bytes are available"
    )]
    InsufficientMemory {
        required_bytes: usize,
        available_bytes: usize,
    },

    #[error("KNN method not implemented: {method}")]
    MethodNotImplemented { method: String },

    #[error("KNN index error: {0}")]
    KnnIndex(String),

    #[error(
        "KNN graph size mismatch: graph has n={graph_n} (neighbors len {neighbors_len}), \
         data has n={data_n}"
    )]
    KnnGraphSizeMismatch {
        graph_n: usize,
        neighbors_len: usize,
        data_n: usize,
    },

    #[error(
        "KNN graph k={graph_k} is too small for PaCMAP; need at least {required_k} \
         (n_neighbors + 50, capped by n − 1)"
    )]
    KnnGraphInsufficientK { graph_k: usize, required_k: usize },

    #[error("KNN graph metric {graph:?} does not match config metric {config:?}")]
    KnnGraphMetricMismatch {
        graph: crate::config::DistanceMetric,
        config: crate::config::DistanceMetric,
    },

    #[error("PCA failed: {0}")]
    Pca(String),

    #[error("GPU optimize error: {0}")]
    Gpu(String),

    #[error("Run cancelled by caller")]
    Cancelled,
}
