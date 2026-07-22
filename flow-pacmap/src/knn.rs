//! K-nearest-neighbour search for PaCMAP graph construction.
//!
//! Returns, for each point i, its k nearest neighbours by the chosen metric,
//! along with their distances — both used in the scaled-distance reranking step
//! and for sigma computation (avg distance to 4th–6th neighbours).

use crate::config::{DistanceMetric, KnnMethod};
use crate::error::PaCMAPError;
use rayon::prelude::*;

/// KNN result for a single query point.
pub struct NeighborList {
    /// Indices of k nearest neighbours (excluding self), ascending distance order.
    pub indices: Vec<u32>,
    /// Distances corresponding to each index.
    pub distances: Vec<f32>,
}

/// Compute k nearest neighbours for all n points in `data` (n×d row-major).
///
/// Returns a `Vec<NeighborList>` of length n.
/// The HNSW index is built, queried in one parallel pass, then dropped —
/// it never coexists in memory with the pair matrices.
pub fn compute_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    method: &KnnMethod,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, PaCMAPError> {
    match method {
        #[cfg(feature = "hnsw")]
        KnnMethod::Hnsw(params) => hnsw_knn(data, n, d, k, params, metric),
        KnnMethod::Exact => exact_knn(data, n, d, k, metric),
        #[cfg(feature = "kdtree")]
        KnnMethod::KdTree => kdtree_knn(data, n, d, k, metric),
        KnnMethod::Annoy => Err(PaCMAPError::MethodNotImplemented {
            method: "Annoy".to_string(),
        }),
        // When a feature is disabled, the variant does not exist; unreachable patterns
        // are excluded by cfg. The catch-all handles any edge cases.
        #[allow(unreachable_patterns)]
        _ => Err(PaCMAPError::MethodNotImplemented {
            method: "unknown (feature disabled)".to_string(),
        }),
    }
}

// ── Distance helpers ──────────────────────────────────────────────────────────

#[inline(always)]
fn l2_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

#[inline(always)]
fn dist(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Euclidean => l2_sq(a, b).sqrt(),
        DistanceMetric::EuclideanSq => l2_sq(a, b),
        DistanceMetric::Cosine => {
            let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
            1.0 - dot / (na * nb + f32::EPSILON)
        }
        DistanceMetric::Manhattan => a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum(),
    }
}

// ── Exact brute-force ─────────────────────────────────────────────────────────

pub fn exact_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, PaCMAPError> {
    let k_capped = k.min(n - 1);
    let result: Vec<NeighborList> = (0..n)
        .into_par_iter()
        .map(|i| {
            let row_i = &data[i * d..(i + 1) * d];
            // Bounded max-heap: keep the k closest seen so far.
            // Using a Vec sorted by descending distance (max at front).
            let mut heap: Vec<(f32, u32)> = Vec::with_capacity(k_capped + 1);

            for j in 0..n {
                if j == i {
                    continue;
                }
                let row_j = &data[j * d..(j + 1) * d];
                let d_ij = dist(row_i, row_j, metric);

                if heap.len() < k_capped {
                    heap.push((d_ij, j as u32));
                    // Keep sorted descending by distance (max at index 0)
                    heap.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                } else if d_ij < heap[0].0 {
                    heap[0] = (d_ij, j as u32);
                    heap.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
                }
            }

            // Sort ascending by distance for output
            heap.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            NeighborList {
                indices: heap.iter().map(|(_, idx)| *idx).collect(),
                distances: heap.iter().map(|(d, _)| *d).collect(),
            }
        })
        .collect();

    Ok(result)
}

// ── HNSW via usearch ──────────────────────────────────────────────────────────

#[cfg(feature = "hnsw")]
fn hnsw_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    params: &crate::config::HnswParams,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, PaCMAPError> {
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

    // usearch does not support L1 natively; fall back to brute-force for Manhattan
    if metric == DistanceMetric::Manhattan {
        return exact_knn(data, n, d, k, metric);
    }

    let metric_kind = match metric {
        DistanceMetric::Euclidean | DistanceMetric::EuclideanSq => MetricKind::L2sq,
        DistanceMetric::Cosine => MetricKind::Cos,
        DistanceMetric::Manhattan => unreachable!(),
    };

    let scalar_kind = match params.quantization {
        crate::config::Quantization::F32 => ScalarKind::F32,
        crate::config::Quantization::F16 => ScalarKind::F16,
        crate::config::Quantization::I8 => ScalarKind::I8,
    };

    let options = IndexOptions {
        dimensions: d,
        metric: metric_kind,
        quantization: scalar_kind,
        connectivity: params.m,
        expansion_add: params.ef_construction,
        expansion_search: params.ef_search,
        ..Default::default()
    };

    let index = Index::new(&options)
        .map_err(|e| PaCMAPError::KnnIndex(e.to_string()))?;
    index.reserve(n)
        .map_err(|e| PaCMAPError::KnnIndex(e.to_string()))?;

    // Parallel add — usearch Index is Send + Sync
    (0..n).into_par_iter().try_for_each(|i| {
        let row = &data[i * d..(i + 1) * d];
        index.add(i as u64, row)
            .map_err(|e| PaCMAPError::KnnIndex(e.to_string()))
    })?;

    // Parallel query all n points; exclude self-match
    let k_fetch = k + 1; // fetch one extra to exclude self
    let result: Vec<NeighborList> = (0..n)
        .into_par_iter()
        .map(|i| {
            let row = &data[i * d..(i + 1) * d];
            let matches = index.search(row, k_fetch).unwrap_or_else(|_| {
                usearch::ffi::Matches {
                    keys: vec![],
                    distances: vec![],
                }
            });

            let mut indices = Vec::with_capacity(k);
            let mut distances = Vec::with_capacity(k);
            for (&key, &dist) in matches.keys.iter().zip(matches.distances.iter()) {
                if key == i as u64 {
                    continue;
                }
                if indices.len() >= k {
                    break;
                }
                indices.push(key as u32);
                distances.push(dist);
            }
            NeighborList { indices, distances }
        })
        .collect();

    // Index dropped here — HNSW memory released before pair allocation
    drop(index);

    Ok(result)
}

// ── k-d tree via kiddo ────────────────────────────────────────────────────────

#[cfg(feature = "kdtree")]
fn kdtree_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    _metric: DistanceMetric,
) -> Result<Vec<NeighborList>, PaCMAPError> {
    // kiddo's API is generic over dimension; we use a dynamic approach via ImmutableKdTree
    // which supports arbitrary compile-time dimensions via const generics. Since d varies,
    // we fall back to the exact brute-force for now and note that a macro-dispatch over
    // common flow cytometry dimensions (5, 10, 20, 30, 40, 50) can be added later.
    exact_knn(data, n, d, k, _metric)
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
        // Point 5 (at x=5) should have point 4 and 6 as nearest neighbours
        let nbrs = &knn[5].indices;
        assert!(nbrs.contains(&4u32) || nbrs.contains(&6u32));
    }
}
