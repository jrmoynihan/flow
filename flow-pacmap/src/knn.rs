//! Re-export and PaCMAP-facing adapters for [`flow_knn`].

use crate::config::{DistanceMetric, KnnMethod};
use crate::error::PaCMAPError;
pub use flow_knn::{
    KnnGraph, NeighborList, PerfRecord, RecommendOpts, builtin_matrix, load_matrix,
    parse_matrix_jsonl, read_knn_graph, recommend_method, recommend_method_with_matrix,
    write_knn_graph,
};

/// Compute k nearest neighbours for all n points in `data` (n×d row-major).
pub fn compute_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    method: &KnnMethod,
    metric: DistanceMetric,
) -> Result<KnnGraph, PaCMAPError> {
    flow_knn::compute_knn(data, n, d, k, method, metric).map_err(map_knn_error)
}

pub(crate) fn map_knn_error(err: flow_knn::KnnError) -> PaCMAPError {
    use flow_knn::KnnError;
    match err {
        KnnError::DatasetTooSmall { n } => PaCMAPError::DatasetTooSmall { n },
        KnnError::DimensionMismatch { len, d } => PaCMAPError::DimensionMismatch { len, d },
        KnnError::MethodNotImplemented { method } => PaCMAPError::MethodNotImplemented { method },
        KnnError::Index(msg) => PaCMAPError::KnnIndex(msg),
        KnnError::GraphSizeMismatch {
            graph_n,
            neighbors_len,
            data_n,
        } => PaCMAPError::KnnGraphSizeMismatch {
            graph_n,
            neighbors_len,
            data_n,
        },
        KnnError::GraphInsufficientK {
            graph_k,
            required_k,
        } => PaCMAPError::KnnGraphInsufficientK {
            graph_k,
            required_k,
        },
        KnnError::GraphMetricMismatch { graph, requested } => PaCMAPError::KnnGraphMetricMismatch {
            graph,
            config: requested,
        },
        KnnError::GpuUnavailable(msg) => PaCMAPError::Gpu(msg),
        KnnError::Io(msg) => PaCMAPError::KnnIndex(format!("knn I/O: {msg}")),
    }
}

/// Validate a graph for PaCMAP, mapping [`flow_knn::KnnError`] into [`PaCMAPError`].
pub fn validate_knn_for_pacmap(
    graph: &KnnGraph,
    data_n: usize,
    n_neighbors: usize,
    metric: DistanceMetric,
) -> Result<(), PaCMAPError> {
    graph
        .validate_for_pacmap(data_n, n_neighbors, metric)
        .map_err(map_knn_error)
}
