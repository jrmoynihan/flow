//! HNSW via ann-search-rs (manifolds-rs peer stack).

use crate::config::{DistanceMetric, HnswParams};
use crate::error::KnnError;
use crate::exact::exact_knn;
use crate::graph::NeighborList;
use ann_search_rs::{build_hnsw_index, query_hnsw_self};
use faer::Mat;

pub fn ann_search_hnsw_knn(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    params: &HnswParams,
    metric: DistanceMetric,
) -> Result<Vec<NeighborList>, KnnError> {
    if metric == DistanceMetric::Manhattan {
        return exact_knn(data, n, d, k, metric);
    }

    let dist_metric = match metric {
        DistanceMetric::Euclidean | DistanceMetric::EuclideanSq => "euclidean",
        DistanceMetric::Cosine => "cosine",
        DistanceMetric::Manhattan => unreachable!(),
    };

    let mat = Mat::from_fn(n, d, |i, j| data[i * d + j]);
    // Request k+1 so we can drop self-matches if present.
    let k_fetch = k + 1;
    let index = build_hnsw_index(
        mat.as_ref(),
        params.m,
        params.ef_construction,
        dist_metric,
        42,
        false,
    );

    let (indices, distances_opt) = query_hnsw_self(&index, k_fetch, params.ef_search, true, false)
        .map_err(|e| KnnError::Index(e.to_string()))?;

    let distances = distances_opt.ok_or_else(|| {
        KnnError::Index("ann-search-rs returned no distances".to_string())
    })?;

    if indices.len() != n || distances.len() != n {
        return Err(KnnError::Index(format!(
            "ann-search-rs self-query shape mismatch: got {}/{} lists for n={n}",
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
