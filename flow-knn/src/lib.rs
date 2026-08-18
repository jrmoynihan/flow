//! # flow-knn
//!
//! Algorithm-agnostic k-nearest-neighbour graphs for large-n scientific data.
//!
//! Build once with [`compute_knn`], reuse the [`KnnGraph`] across embedders
//! (PaCMAP, UMAP, …) without recomputing neighbours.

mod config;
mod error;
mod exact;
mod graph;
mod index;
mod io;
mod select;

#[cfg(feature = "gpu")]
mod gpu_ann;
#[cfg(feature = "ann-search")]
mod hnsw_ann;
#[cfg(feature = "hnsw")]
mod hnsw_usearch;

pub use config::{DistanceMetric, HnswParams, KnnMethod, Quantization};
#[cfg(feature = "gpu")]
pub use config::{IvfGpuParams, NnDescentGpuParams};
pub use error::KnnError;
pub use exact::exact_knn;
pub use graph::{KnnGraph, NeighborList};
pub use index::{AnnIndex, build_ann_index};
pub use io::{read_knn_graph, write_knn_graph};
#[cfg(feature = "gpu")]
pub use gpu_ann::gpu_adapter_available;
pub use select::{
    PerfRecord, RecommendOpts, builtin_matrix, load_matrix, parse_matrix_jsonl, recommend_method,
    recommend_method_with_matrix,
};

use config::KnnMethod as Method;

fn method_provenance(method: &Method) -> String {
    match method {
        #[cfg(feature = "hnsw")]
        Method::Hnsw(_) => "Hnsw".to_string(),
        Method::Exact => "Exact".to_string(),
        #[cfg(feature = "kdtree")]
        Method::KdTree => "KdTree".to_string(),
        #[cfg(feature = "ann-search")]
        Method::AnnSearchHnsw(_) => "AnnSearchHnsw".to_string(),
        #[cfg(feature = "gpu")]
        Method::GpuExact => "GpuExact".to_string(),
        #[cfg(feature = "gpu")]
        Method::GpuIvf(_) => "GpuIvf".to_string(),
        #[cfg(feature = "gpu")]
        Method::GpuNnDescent(_) => "GpuNnDescent".to_string(),
        Method::Annoy => "Annoy".to_string(),
    }
}

/// Compute k nearest neighbours for all `n` points in row-major `data`.
pub fn compute_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    method: &KnnMethod,
    metric: DistanceMetric,
) -> Result<KnnGraph, KnnError> {
    if n < 2 {
        return Err(KnnError::DatasetTooSmall { n });
    }
    if data.len() != n * d {
        return Err(KnnError::DimensionMismatch {
            len: data.len(),
            d,
        });
    }
    let k_capped = k.min(n - 1);
    let neighbors = match method {
        #[cfg(feature = "hnsw")]
        KnnMethod::Hnsw(params) => hnsw_usearch::hnsw_knn(data, n, d, k_capped, params, metric)?,
        KnnMethod::Exact => exact_knn(data, n, d, k_capped, metric)?,
        #[cfg(feature = "kdtree")]
        KnnMethod::KdTree => exact_knn(data, n, d, k_capped, metric)?,
        #[cfg(feature = "ann-search")]
        KnnMethod::AnnSearchHnsw(params) => {
            hnsw_ann::ann_search_hnsw_knn(data, n, d, k_capped, params, metric)?
        }
        #[cfg(feature = "gpu")]
        KnnMethod::GpuExact => gpu_ann::exact_gpu_knn(data, n, d, k_capped, metric)?,
        #[cfg(feature = "gpu")]
        KnnMethod::GpuIvf(params) => gpu_ann::ivf_gpu_knn(data, n, d, k_capped, params, metric)?,
        #[cfg(feature = "gpu")]
        KnnMethod::GpuNnDescent(params) => {
            gpu_ann::nndescent_gpu_knn(data, n, d, k_capped, params, metric)?
        }
        KnnMethod::Annoy => {
            return Err(KnnError::MethodNotImplemented {
                method: "Annoy".to_string(),
            });
        }
        #[allow(unreachable_patterns)]
        _ => {
            return Err(KnnError::MethodNotImplemented {
                method: "unknown (feature disabled)".to_string(),
            });
        }
    };

    Ok(KnnGraph {
        neighbors,
        n,
        k: k_capped,
        metric,
        provenance: Some(method_provenance(method)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid(n: usize) -> Vec<f32> {
        (0..n).flat_map(|i| [i as f32, 0.0]).collect()
    }

    #[test]
    fn exact_knn_nearest_neighbour() {
        let data = make_grid(10);
        let knn = exact_knn(&data, 10, 2, 2, DistanceMetric::Euclidean).unwrap();
        let nbrs = &knn[5].indices;
        assert!(nbrs.contains(&4u32) || nbrs.contains(&6u32));
    }

    #[test]
    fn compute_knn_returns_graph_metadata() {
        let data = make_grid(10);
        let graph = compute_knn(
            &data,
            10,
            2,
            3,
            &KnnMethod::Exact,
            DistanceMetric::Euclidean,
        )
        .unwrap();
        assert_eq!(graph.n, 10);
        assert_eq!(graph.k, 3);
        assert_eq!(graph.metric, DistanceMetric::Euclidean);
        assert_eq!(graph.provenance.as_deref(), Some("Exact"));
    }

    #[test]
    fn validate_rejects_mismatches() {
        let data = make_grid(80);
        let graph = compute_knn(
            &data,
            80,
            2,
            3,
            &KnnMethod::Exact,
            DistanceMetric::Euclidean,
        )
        .unwrap();
        assert!(matches!(
            graph.validate_for_pacmap(20, 5, DistanceMetric::Euclidean),
            Err(KnnError::GraphSizeMismatch { .. })
        ));
        assert!(matches!(
            graph.validate_for_pacmap(80, 5, DistanceMetric::Euclidean),
            Err(KnnError::GraphInsufficientK { .. })
        ));
    }

    #[cfg(feature = "gpu")]
    #[test]
    fn gpu_exact_runs_when_adapter_available() {
        if !gpu_adapter_available() {
            eprintln!("skip gpu_exact: no WGPU adapter");
            return;
        }
        let data = make_grid(64);
        let graph = compute_knn(
            &data,
            64,
            2,
            5,
            &KnnMethod::GpuExact,
            DistanceMetric::Euclidean,
        )
        .expect("gpu exact");
        assert_eq!(graph.n, 64);
        assert_eq!(graph.k, 5);
        assert_eq!(graph.provenance.as_deref(), Some("GpuExact"));
        assert_eq!(graph.neighbors[0].indices.len(), 5);
    }
}
