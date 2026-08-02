//! GPU k-NN via ann-search-rs (cubeCL / wgpu).
//!
//! Requires a working WGPU adapter. Exhaustive is exact on GPU; IVF / NN-Descent
//! are approximate (no GPU HNSW).

use crate::config::{DistanceMetric, IvfGpuParams, NnDescentGpuParams};
use crate::error::KnnError;
use crate::exact::exact_knn;
use crate::graph::NeighborList;
use ann_search_rs::{
    build_exhaustive_index_gpu, build_ivf_index_gpu, build_nndescent_index_gpu,
    query_exhaustive_index_gpu_self, query_ivf_index_gpu_self, query_nndescent_index_gpu_self,
};
use cubecl::prelude::Runtime;
use cubecl::wgpu::{WgpuDevice, WgpuRuntime};
use faer::Mat;
use std::sync::OnceLock;

fn dist_metric_str(metric: DistanceMetric) -> Result<&'static str, KnnError> {
    match metric {
        DistanceMetric::Euclidean | DistanceMetric::EuclideanSq => Ok("euclidean"),
        DistanceMetric::Cosine => Ok("cosine"),
        DistanceMetric::Manhattan => Err(KnnError::Index(
            "GPU kNN does not support Manhattan; use Exact or CPU HNSW".into(),
        )),
    }
}

fn to_neighbor_lists(
    n: usize,
    k: usize,
    indices: &[Vec<usize>],
    distances: &[Vec<f32>],
) -> Result<Vec<NeighborList>, KnnError> {
    if indices.len() != n || distances.len() != n {
        return Err(KnnError::Index(format!(
            "GPU self-query shape mismatch: got {}/{} lists for n={n}",
            indices.len(),
            distances.len()
        )));
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut idxs = Vec::with_capacity(k);
        let mut dists = Vec::with_capacity(k);
        for (j, &nbr) in indices[i].iter().enumerate() {
            if nbr == i {
                continue;
            }
            if idxs.len() >= k {
                break;
            }
            idxs.push(nbr as u32);
            dists.push(distances[i][j]);
        }
        out.push(NeighborList {
            indices: idxs,
            distances: dists,
        });
    }
    Ok(out)
}

/// True when a shared WGPU adapter initializes (smoke empty+read).
pub fn gpu_adapter_available() -> bool {
    try_shared_device().is_ok()
}

fn try_shared_device() -> Result<&'static WgpuDevice, String> {
    static INIT: OnceLock<Result<WgpuDevice, String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let device = WgpuDevice::default();
            let client = WgpuRuntime::client(&device);
            let handle = client.empty(4);
            let _ = client.read_one(handle);
            device
        }));
        std::panic::set_hook(prev);
        result.map_err(|payload| {
            if let Some(s) = payload.downcast_ref::<&str>() {
                format!("WGPU adapter init panicked: {s}")
            } else if let Some(s) = payload.downcast_ref::<String>() {
                format!("WGPU adapter init panicked: {s}")
            } else {
                "WGPU adapter init panicked".into()
            }
        })
    })
    .as_ref()
    .map_err(|s| s.clone())
}

fn device_or_err() -> Result<&'static WgpuDevice, KnnError> {
    try_shared_device().map_err(|e| KnnError::GpuUnavailable(e))
}

/// Exact (exhaustive) kNN on the GPU.
pub fn exact_gpu_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, KnnError> {
    if metric == DistanceMetric::Manhattan {
        return exact_knn(data, n, d, k, metric);
    }
    let dist = dist_metric_str(metric)?;
    let device = device_or_err()?.clone();
    let mat = Mat::from_fn(n, d, |i, j| data[i * d + j]);
    let k_fetch = k + 1;

    let index = build_exhaustive_index_gpu::<f32, WgpuRuntime>(mat.as_ref(), dist, device)
        .map_err(|e| KnnError::Index(e.to_string()))?;
    let (indices, distances_opt) =
        query_exhaustive_index_gpu_self(&index, k_fetch, true, false)
            .map_err(|e| KnnError::Index(e.to_string()))?;
    let distances = distances_opt
        .ok_or_else(|| KnnError::Index("exhaustive GPU returned no distances".into()))?;
    to_neighbor_lists(n, k, &indices, &distances)
}

/// IVF approximate kNN on the GPU (good default for large n).
pub fn ivf_gpu_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    params: &IvfGpuParams,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, KnnError> {
    if metric == DistanceMetric::Manhattan {
        return exact_knn(data, n, d, k, metric);
    }
    let dist = dist_metric_str(metric)?;
    let device = device_or_err()?.clone();
    let mat = Mat::from_fn(n, d, |i, j| data[i * d + j]);
    let k_fetch = k + 1;

    let index = build_ivf_index_gpu::<f32, WgpuRuntime>(
        mat.as_ref(),
        params.n_list,
        None,
        dist,
        42,
        false,
        device,
    )
    .map_err(|e| KnnError::Index(e.to_string()))?;

    let (indices, distances_opt) = query_ivf_index_gpu_self(
        &index,
        k_fetch,
        params.n_probes,
        None,
        true,
        false,
    )
    .map_err(|e| KnnError::Index(e.to_string()))?;
    let distances =
        distances_opt.ok_or_else(|| KnnError::Index("IVF GPU returned no distances".into()))?;
    to_neighbor_lists(n, k, &indices, &distances)
}

/// NN-Descent / CAGRA-style approximate kNN on the GPU.
pub fn nndescent_gpu_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    params: &NnDescentGpuParams,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, KnnError> {
    if metric == DistanceMetric::Manhattan {
        return exact_knn(data, n, d, k, metric);
    }
    let dist = dist_metric_str(metric)?;
    let device = device_or_err()?.clone();
    let mat = Mat::from_fn(n, d, |i, j| data[i * d + j]);
    let k_fetch = k + 1;
    // Graph degree should cover the requested neighbourhood.
    let graph_k = params.k.unwrap_or(k.max(30));

    let mut index = build_nndescent_index_gpu::<f32, WgpuRuntime>(
        mat.as_ref(),
        dist,
        Some(graph_k),
        params.k_build,
        None,
        params.n_trees,
        Some(params.delta),
        params.rho,
        None,
        42,
        false,
        true,
        device,
    )
    .map_err(|e| KnnError::Index(e.to_string()))?;

    let (indices, distances_opt) =
        query_nndescent_index_gpu_self(&mut index, k_fetch, None, true)
            .map_err(|e| KnnError::Index(e.to_string()))?;
    let distances = distances_opt
        .ok_or_else(|| KnnError::Index("NN-Descent GPU returned no distances".into()))?;
    to_neighbor_lists(n, k, &indices, &distances)
}
