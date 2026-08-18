//! GPU k-NN via ann-search-rs (cubeCL / wgpu).
//!
//! Requires a working WGPU adapter. Exhaustive is exact on GPU; IVF / NN-Descent
//! are approximate (no GPU HNSW).

use crate::config::{DistanceMetric, IvfGpuParams, NnDescentGpuParams};
use crate::error::KnnError;
use crate::exact::exact_knn;
use crate::graph::NeighborList;
use ann_search_rs::gpu::LINE_SIZE;
use ann_search_rs::gpu::exhaustive_gpu::ExhaustiveIndexGpu;
use ann_search_rs::gpu::ivf_gpu::IvfIndexGpu;
use ann_search_rs::{
    build_exhaustive_index_gpu, build_ivf_index_gpu, build_nndescent_index_gpu,
    query_exhaustive_index_gpu, query_exhaustive_index_gpu_self, query_ivf_index_gpu,
    query_ivf_index_gpu_self, query_nndescent_index_gpu_self,
};
use cubecl::prelude::Runtime;
use cubecl::wgpu::{WgpuDevice, WgpuRuntime};
use faer::Mat;
use std::fmt;
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

fn row_major_to_mat(data: &[f32], n: usize, d: usize) -> Mat<f32> {
    Mat::from_fn(n, d, |i, j| data[i * d + j])
}

/// GPU indices pad features to [`LINE_SIZE`] and then `check_dim` against the
/// padded width. Trailing zeros keep Euclidean / cosine / Manhattan distances
/// unchanged versus the original `d`.
fn gpu_padded_d(d: usize) -> usize {
    d.next_multiple_of(LINE_SIZE)
}

fn row_major_to_gpu_mat(data: &[f32], n: usize, d: usize) -> Mat<f32> {
    let d_pad = gpu_padded_d(d);
    if d_pad == d {
        return row_major_to_mat(data, n, d);
    }
    Mat::from_fn(n, d_pad, |i, j| if j < d { data[i * d + j] } else { 0.0 })
}

/// Map query-vs-library GPU output to [`NeighborList`]s without dropping
/// database indices that happen to equal a query row id (queries are not the
/// indexed set).
fn gpu_query_to_neighbor_lists(
    n_queries: usize,
    indices: &[Vec<usize>],
    distances: &[Vec<f32>],
) -> Result<Vec<NeighborList>, KnnError> {
    if indices.len() != n_queries || distances.len() != n_queries {
        return Err(KnnError::Index(format!(
            "GPU query-vs-library shape mismatch: got {}/{} lists for n_queries={n_queries}",
            indices.len(),
            distances.len()
        )));
    }
    let mut out = Vec::with_capacity(n_queries);
    for i in 0..n_queries {
        out.push(NeighborList {
            indices: indices[i].iter().map(|&x| x as u32).collect(),
            distances: distances[i].clone(),
        });
    }
    Ok(out)
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
    try_shared_device().map_err(KnnError::GpuUnavailable)
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
    let (indices, distances_opt) = query_exhaustive_index_gpu_self(&index, k_fetch, true, false)
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

    let (indices, distances_opt) =
        query_ivf_index_gpu_self(&index, k_fetch, params.n_probes, None, true, false)
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

    let (indices, distances_opt) = query_nndescent_index_gpu_self(&mut index, k_fetch, None, true)
        .map_err(|e| KnnError::Index(e.to_string()))?;
    let distances = distances_opt
        .ok_or_else(|| KnnError::Index("NN-Descent GPU returned no distances".into()))?;
    to_neighbor_lists(n, k, &indices, &distances)
}

/// Held exhaustive GPU index for query-set ≠ database-set search.
pub(crate) struct GpuExactIndex {
    index: ExhaustiveIndexGpu<f32, WgpuRuntime>,
    d: usize,
}

impl fmt::Debug for GpuExactIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GpuExactIndex")
            .field("d", &self.d)
            .finish_non_exhaustive()
    }
}

impl GpuExactIndex {
    pub(crate) fn build(
        data: &[f32],
        n: usize,
        d: usize,
        metric: DistanceMetric,
    ) -> Result<Self, KnnError> {
        let dist = dist_metric_str(metric)?;
        let device = device_or_err()?.clone();
        let mat = row_major_to_gpu_mat(data, n, d);
        let index = build_exhaustive_index_gpu::<f32, WgpuRuntime>(mat.as_ref(), dist, device)
            .map_err(|e| KnnError::Index(e.to_string()))?;
        Ok(Self { index, d })
    }

    pub(crate) fn search(&self, query: &[f32], k: usize) -> Result<NeighborList, KnnError> {
        let mut lists = self.search_batch(query, 1, k)?;
        lists.pop().ok_or_else(|| {
            KnnError::Index("GPU exhaustive query returned no neighbour lists".into())
        })
    }

    pub(crate) fn search_batch(
        &self,
        queries: &[f32],
        n_queries: usize,
        k: usize,
    ) -> Result<Vec<NeighborList>, KnnError> {
        let mat = row_major_to_gpu_mat(queries, n_queries, self.d);
        let (indices, distances_opt) =
            query_exhaustive_index_gpu(mat.as_ref(), &self.index, k, true, false)
                .map_err(|e| KnnError::Index(e.to_string()))?;
        let distances = distances_opt.ok_or_else(|| {
            KnnError::Index("exhaustive GPU query-vs-library returned no distances".into())
        })?;
        gpu_query_to_neighbor_lists(n_queries, &indices, &distances)
    }
}

/// Held IVF GPU index for query-set ≠ database-set search.
pub(crate) struct GpuIvfIndex {
    index: IvfIndexGpu<f32, WgpuRuntime>,
    n_probes: Option<usize>,
    d: usize,
}

impl fmt::Debug for GpuIvfIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GpuIvfIndex")
            .field("d", &self.d)
            .field("n_probes", &self.n_probes)
            .finish_non_exhaustive()
    }
}

impl GpuIvfIndex {
    pub(crate) fn build(
        data: &[f32],
        n: usize,
        d: usize,
        params: &IvfGpuParams,
        metric: DistanceMetric,
    ) -> Result<Self, KnnError> {
        let dist = dist_metric_str(metric)?;
        let device = device_or_err()?.clone();
        let mat = row_major_to_gpu_mat(data, n, d);
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
        Ok(Self {
            index,
            n_probes: params.n_probes,
            d,
        })
    }

    pub(crate) fn search(&self, query: &[f32], k: usize) -> Result<NeighborList, KnnError> {
        let mut lists = self.search_batch(query, 1, k)?;
        lists
            .pop()
            .ok_or_else(|| KnnError::Index("GPU IVF query returned no neighbour lists".into()))
    }

    pub(crate) fn search_batch(
        &self,
        queries: &[f32],
        n_queries: usize,
        k: usize,
    ) -> Result<Vec<NeighborList>, KnnError> {
        let mat = row_major_to_gpu_mat(queries, n_queries, self.d);
        let (indices, distances_opt) = query_ivf_index_gpu(
            mat.as_ref(),
            &self.index,
            k,
            self.n_probes,
            None,
            true,
            false,
        )
        .map_err(|e| KnnError::Index(e.to_string()))?;
        let distances = distances_opt.ok_or_else(|| {
            KnnError::Index("IVF GPU query-vs-library returned no distances".into())
        })?;
        gpu_query_to_neighbor_lists(n_queries, &indices, &distances)
    }
}
