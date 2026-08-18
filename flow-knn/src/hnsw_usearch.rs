//! HNSW via usearch.

use crate::config::{DistanceMetric, HnswParams, Quantization};
use crate::error::KnnError;
use crate::exact::exact_knn;
use crate::graph::NeighborList;
use rayon::prelude::*;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// Held usearch HNSW index for external (non-self) queries.
pub struct UsearchIndex {
    index: Index,
    d: usize,
}

impl std::fmt::Debug for UsearchIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsearchIndex")
            .field("d", &self.d)
            .finish_non_exhaustive()
    }
}

impl UsearchIndex {
    pub fn build(
        data: &[f32],
        n: usize,
        d: usize,
        params: &HnswParams,
        metric: DistanceMetric,
    ) -> Result<Self, KnnError> {
        let metric_kind = match metric {
            DistanceMetric::Euclidean | DistanceMetric::EuclideanSq => MetricKind::L2sq,
            DistanceMetric::Cosine => MetricKind::Cos,
            DistanceMetric::Manhattan => {
                return Err(KnnError::Index(
                    "usearch does not support Manhattan; use Exact".into(),
                ));
            }
        };

        let scalar_kind = match params.quantization {
            Quantization::F32 => ScalarKind::F32,
            Quantization::F16 => ScalarKind::F16,
            Quantization::I8 => ScalarKind::I8,
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

        let index = Index::new(&options).map_err(|e| KnnError::Index(e.to_string()))?;
        index
            .reserve(n)
            .map_err(|e| KnnError::Index(e.to_string()))?;

        (0..n).into_par_iter().try_for_each(|i| {
            let row = &data[i * d..(i + 1) * d];
            index
                .add(i as u64, row)
                .map_err(|e| KnnError::Index(e.to_string()))
        })?;

        Ok(Self { index, d })
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<NeighborList, KnnError> {
        if query.len() != self.d {
            return Err(KnnError::QueryDimensionMismatch {
                query_d: query.len(),
                index_d: self.d,
            });
        }
        let matches = self
            .index
            .search(query, k)
            .map_err(|e| KnnError::Index(e.to_string()))?;
        let mut indices = Vec::with_capacity(matches.keys.len());
        let mut distances = Vec::with_capacity(matches.distances.len());
        for (&key, &dist) in matches.keys.iter().zip(matches.distances.iter()) {
            indices.push(key as u32);
            distances.push(dist);
        }
        Ok(NeighborList { indices, distances })
    }
}

pub fn hnsw_knn(
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

    let held = UsearchIndex::build(data, n, d, params, metric)?;
    let k_fetch = k + 1;
    let result: Vec<NeighborList> = (0..n)
        .into_par_iter()
        .map(|i| {
            let row = &data[i * d..(i + 1) * d];
            let matches = held.index.search(row, k_fetch).unwrap_or_else(|_| {
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

    Ok(result)
}
