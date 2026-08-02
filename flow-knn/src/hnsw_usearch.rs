//! HNSW via usearch.

use crate::config::{DistanceMetric, HnswParams, Quantization};
use crate::error::KnnError;
use crate::exact::exact_knn;
use crate::graph::NeighborList;
use rayon::prelude::*;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

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

    let metric_kind = match metric {
        DistanceMetric::Euclidean | DistanceMetric::EuclideanSq => MetricKind::L2sq,
        DistanceMetric::Cosine => MetricKind::Cos,
        DistanceMetric::Manhattan => unreachable!(),
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

    let k_fetch = k + 1;
    let result: Vec<NeighborList> = (0..n)
        .into_par_iter()
        .map(|i| {
            let row = &data[i * d..(i + 1) * d];
            let matches = index.search(row, k_fetch).unwrap_or_else(|_| usearch::ffi::Matches {
                keys: vec![],
                distances: vec![],
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

    drop(index);
    Ok(result)
}
