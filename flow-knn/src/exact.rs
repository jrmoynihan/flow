//! Exact (brute-force) k-NN.

use crate::config::DistanceMetric;
use crate::error::KnnError;
use crate::graph::NeighborList;
use rayon::prelude::*;

#[inline(always)]
fn l2_sq(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

#[inline(always)]
pub(crate) fn dist(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
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

/// Exact k-NN for all `n` points in row-major `data`.
///
/// Note: `get_unchecked` row slices were A/B'd (see `docs/PERF_MATRIX.md`) and
/// did not meet the ≥5% keep rule at 10k×20; safe indexing kept.
pub fn exact_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, KnnError> {
    let k_capped = k.min(n - 1);
    let result: Vec<NeighborList> = (0..n)
        .into_par_iter()
        .map(|i| {
            let row_i = &data[i * d..(i + 1) * d];
            let mut heap: Vec<(f32, u32)> = Vec::with_capacity(k_capped + 1);

            for j in 0..n {
                if j == i {
                    continue;
                }
                let row_j = &data[j * d..(j + 1) * d];
                let d_ij = dist(row_i, row_j, metric);

                if heap.len() < k_capped {
                    heap.push((d_ij, j as u32));
                    heap.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                } else if d_ij < heap[0].0 {
                    heap[0] = (d_ij, j as u32);
                    heap.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                }
            }

            heap.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            NeighborList {
                indices: heap.iter().map(|(_, idx)| *idx).collect(),
                distances: heap.iter().map(|(d, _)| *d).collect(),
            }
        })
        .collect();

    Ok(result)
}

/// Exact k-NN for one query against a database (query may be outside the DB).
pub(crate) fn exact_search_one(
    data: &[f32],
    n: usize,
    d: usize,
    query: &[f32],
    k: usize,
    metric: DistanceMetric,
) -> NeighborList {
    let k = k.min(n);
    let mut heap: Vec<(f32, u32)> = Vec::with_capacity(k + 1);
    for j in 0..n {
        let row_j = &data[j * d..(j + 1) * d];
        let d_ij = dist(query, row_j, metric);
        if heap.len() < k {
            heap.push((d_ij, j as u32));
            heap.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        } else if d_ij < heap[0].0 {
            heap[0] = (d_ij, j as u32);
            heap.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }
    }
    heap.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    NeighborList {
        indices: heap.iter().map(|(_, idx)| *idx).collect(),
        distances: heap.iter().map(|(d, _)| *d).collect(),
    }
}

/// Exact k-NN for many queries (row-major) against a fixed database.
pub(crate) fn exact_search_batch(
    data: &[f32],
    n: usize,
    d: usize,
    queries: &[f32],
    n_queries: usize,
    k: usize,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, KnnError> {
    let out: Vec<NeighborList> = (0..n_queries)
        .into_par_iter()
        .map(|i| {
            let q = &queries[i * d..(i + 1) * d];
            exact_search_one(data, n, d, q, k, metric)
        })
        .collect();
    Ok(out)
}
