//! HNSW via ann-search-rs (manifolds-rs peer stack).

use crate::config::{DistanceMetric, HnswParams};
use crate::error::KnnError;
use crate::exact::exact_knn;
use crate::graph::NeighborList;
use ann_search_rs::{build_hnsw_index, query_hnsw_index, query_hnsw_self};
use faer::Mat;

/// Held ann-search-rs HNSW index for external queries.
pub struct AnnSearchIndex {
    // ann-search-rs does not expose a stable public type name for the index;
    // keep the built index behind the query helpers by storing the matrix + params.
    // We store the opaque index as returned by build_hnsw_index.
    index: ann_search_rs::cpu::hnsw::HnswIndex<f32>,
    d: usize,
    ef_search: usize,
}

impl std::fmt::Debug for AnnSearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnnSearchIndex")
            .field("d", &self.d)
            .field("ef_search", &self.ef_search)
            .finish_non_exhaustive()
    }
}

impl AnnSearchIndex {
    pub fn build(
        data: &[f32],
        n: usize,
        d: usize,
        params: &HnswParams,
        metric: DistanceMetric,
    ) -> Result<Self, KnnError> {
        let dist_metric = match metric {
            DistanceMetric::Euclidean | DistanceMetric::EuclideanSq => "euclidean",
            DistanceMetric::Cosine => "cosine",
            DistanceMetric::Manhattan => {
                return Err(KnnError::Index(
                    "ann-search HNSW does not support Manhattan; use Exact".into(),
                ));
            }
        };
        let mat = Mat::from_fn(n, d, |i, j| data[i * d + j]);
        let index = build_hnsw_index(
            mat.as_ref(),
            params.m,
            params.ef_construction,
            dist_metric,
            42,
            false,
        );
        Ok(Self {
            index,
            d,
            ef_search: params.ef_search,
        })
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<NeighborList, KnnError> {
        if query.len() != self.d {
            return Err(KnnError::QueryDimensionMismatch {
                query_d: query.len(),
                index_d: self.d,
            });
        }
        let mat = Mat::from_fn(1, self.d, |_, j| query[j]);
        let (indices, distances_opt) = query_hnsw_index(
            mat.as_ref(),
            &self.index,
            k,
            self.ef_search,
            true,
            false,
        )
        .map_err(|e| KnnError::Index(e.to_string()))?;
        let distances = distances_opt.ok_or_else(|| {
            KnnError::Index("ann-search-rs returned no distances".to_string())
        })?;
        if indices.is_empty() {
            return Ok(NeighborList {
                indices: Vec::new(),
                distances: Vec::new(),
            });
        }
        Ok(NeighborList {
            indices: indices[0].iter().map(|&x| x as u32).collect(),
            distances: distances[0].clone(),
        })
    }

    pub fn search_batch(
        &self,
        queries: &[f32],
        n_queries: usize,
        k: usize,
    ) -> Result<Vec<NeighborList>, KnnError> {
        let mat = Mat::from_fn(n_queries, self.d, |i, j| queries[i * self.d + j]);
        let (indices, distances_opt) = query_hnsw_index(
            mat.as_ref(),
            &self.index,
            k,
            self.ef_search,
            true,
            false,
        )
        .map_err(|e| KnnError::Index(e.to_string()))?;
        let distances = distances_opt.ok_or_else(|| {
            KnnError::Index("ann-search-rs returned no distances".to_string())
        })?;
        if indices.len() != n_queries {
            return Err(KnnError::Index(format!(
                "ann-search-rs batch shape mismatch: got {} lists for n_queries={n_queries}",
                indices.len()
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
}

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
