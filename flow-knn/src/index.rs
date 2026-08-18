//! Reusable ANN / exact indices for query-set ≠ database-set search.
//!
//! [`compute_knn`](crate::compute_knn) builds a self-query neighbour graph and
//! discards the underlying index. Spectral library matching needs the opposite:
//! index reference spectra once, then search many stained events against them.

use crate::config::{DistanceMetric, KnnMethod};
use crate::error::KnnError;
use crate::exact;
use crate::graph::NeighborList;
use rayon::prelude::*;

#[cfg(feature = "ann-search")]
use crate::hnsw_ann;
#[cfg(feature = "hnsw")]
use crate::hnsw_usearch;

/// Held nearest-neighbour index over a fixed database of `n` points in `d` dims.
#[derive(Debug)]
pub struct AnnIndex {
    backend: AnnBackend,
    n: usize,
    d: usize,
    metric: DistanceMetric,
    provenance: String,
}

#[derive(Debug)]
enum AnnBackend {
    Exact {
        data: Vec<f32>,
    },
    #[cfg(feature = "hnsw")]
    Usearch(hnsw_usearch::UsearchIndex),
    #[cfg(feature = "ann-search")]
    AnnSearch(hnsw_ann::AnnSearchIndex),
}

impl AnnIndex {
    /// Number of database vectors.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Feature dimensionality.
    pub fn d(&self) -> usize {
        self.d
    }

    pub fn metric(&self) -> DistanceMetric {
        self.metric
    }

    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Build an index over row-major `data` (`n × d`).
    ///
    /// Empty databases (`n == 0`) are rejected. A single-point database is
    /// allowed (useful for tiny AF libraries).
    pub fn build(
        data: &[f32],
        n: usize,
        d: usize,
        method: &KnnMethod,
        metric: DistanceMetric,
    ) -> Result<Self, KnnError> {
        if n == 0 {
            return Err(KnnError::DatasetTooSmall { n });
        }
        if d == 0 || data.len() != n * d {
            return Err(KnnError::DimensionMismatch {
                len: data.len(),
                d,
            });
        }

        let (backend, provenance) = match method {
            KnnMethod::Exact => (
                AnnBackend::Exact {
                    data: data.to_vec(),
                },
                "Exact".to_string(),
            ),
            #[cfg(feature = "kdtree")]
            KnnMethod::KdTree => (
                AnnBackend::Exact {
                    data: data.to_vec(),
                },
                "Exact".to_string(),
            ),
            #[cfg(feature = "hnsw")]
            KnnMethod::Hnsw(params) => {
                if metric == DistanceMetric::Manhattan {
                    (
                        AnnBackend::Exact {
                            data: data.to_vec(),
                        },
                        "Exact".to_string(),
                    )
                } else {
                    let index = hnsw_usearch::UsearchIndex::build(data, n, d, params, metric)?;
                    (AnnBackend::Usearch(index), "Hnsw".to_string())
                }
            }
            #[cfg(feature = "ann-search")]
            KnnMethod::AnnSearchHnsw(params) => {
                if metric == DistanceMetric::Manhattan {
                    (
                        AnnBackend::Exact {
                            data: data.to_vec(),
                        },
                        "Exact".to_string(),
                    )
                } else {
                    let index = hnsw_ann::AnnSearchIndex::build(data, n, d, params, metric)?;
                    (AnnBackend::AnnSearch(index), "AnnSearchHnsw".to_string())
                }
            }
            #[cfg(feature = "gpu")]
            KnnMethod::GpuExact | KnnMethod::GpuIvf(_) | KnnMethod::GpuNnDescent(_) => {
                return Err(KnnError::MethodNotImplemented {
                    method: "GPU AnnIndex (use Exact or Hnsw for library search)".to_string(),
                });
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

        Ok(Self {
            backend,
            n,
            d,
            metric,
            provenance,
        })
    }

    /// Search one query vector (`len == d`) for up to `k` neighbours.
    pub fn search(&self, query: &[f32], k: usize) -> Result<NeighborList, KnnError> {
        if query.len() != self.d {
            return Err(KnnError::QueryDimensionMismatch {
                query_d: query.len(),
                index_d: self.d,
            });
        }
        let k = k.min(self.n).max(0);
        if k == 0 {
            return Ok(NeighborList {
                indices: Vec::new(),
                distances: Vec::new(),
            });
        }
        match &self.backend {
            AnnBackend::Exact { data } => Ok(exact::exact_search_one(
                data,
                self.n,
                self.d,
                query,
                k,
                self.metric,
            )),
            #[cfg(feature = "hnsw")]
            AnnBackend::Usearch(index) => index.search(query, k),
            #[cfg(feature = "ann-search")]
            AnnBackend::AnnSearch(index) => index.search(query, k),
        }
    }

    /// Search many queries (row-major `n_queries × d`) in parallel.
    pub fn search_batch(
        &self,
        queries: &[f32],
        n_queries: usize,
        k: usize,
    ) -> Result<Vec<NeighborList>, KnnError> {
        if n_queries == 0 {
            return Ok(Vec::new());
        }
        if queries.len() != n_queries * self.d {
            return Err(KnnError::DimensionMismatch {
                len: queries.len(),
                d: self.d,
            });
        }
        let k = k.min(self.n);
        if k == 0 {
            return Ok(vec![
                NeighborList {
                    indices: Vec::new(),
                    distances: Vec::new(),
                };
                n_queries
            ]);
        }

        match &self.backend {
            AnnBackend::Exact { data } => {
                exact::exact_search_batch(data, self.n, self.d, queries, n_queries, k, self.metric)
            }
            #[cfg(feature = "hnsw")]
            AnnBackend::Usearch(index) => {
                let out: Result<Vec<_>, _> = (0..n_queries)
                    .into_par_iter()
                    .map(|i| {
                        let q = &queries[i * self.d..(i + 1) * self.d];
                        index.search(q, k)
                    })
                    .collect();
                out
            }
            #[cfg(feature = "ann-search")]
            AnnBackend::AnnSearch(index) => index.search_batch(queries, n_queries, k),
        }
    }
}

/// Convenience: build with default HNSW (or Exact when `hnsw` is off).
pub fn build_ann_index(
    data: &[f32],
    n: usize,
    d: usize,
    metric: DistanceMetric,
) -> Result<AnnIndex, KnnError> {
    AnnIndex::build(data, n, d, &KnnMethod::default(), metric)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Vec<f32> {
        // Three 2-d prototypes on the unit circle-ish.
        vec![1.0, 0.0, 0.0, 1.0, -1.0, 0.0]
    }

    #[test]
    fn exact_index_query_not_in_database() {
        let data = library();
        let index = AnnIndex::build(&data, 3, 2, &KnnMethod::Exact, DistanceMetric::Euclidean)
            .expect("build");
        let nbrs = index.search(&[0.9, 0.1], 1).expect("search");
        assert_eq!(nbrs.indices, vec![0]);
    }

    #[test]
    fn search_batch_matches_single() {
        let data = library();
        let index =
            AnnIndex::build(&data, 3, 2, &KnnMethod::Exact, DistanceMetric::Cosine).expect("build");
        let queries = vec![0.95f32, 0.05, 0.05, 0.95];
        let batch = index.search_batch(&queries, 2, 1).expect("batch");
        let a = index.search(&queries[0..2], 1).unwrap();
        let b = index.search(&queries[2..4], 1).unwrap();
        assert_eq!(batch[0].indices, a.indices);
        assert_eq!(batch[1].indices, b.indices);
    }

    #[cfg(feature = "hnsw")]
    #[test]
    fn usearch_index_finds_nearest_prototype() {
        use crate::config::HnswParams;
        let data = library();
        let index = AnnIndex::build(
            &data,
            3,
            2,
            &KnnMethod::Hnsw(HnswParams::default()),
            DistanceMetric::Euclidean,
        )
        .expect("build");
        assert_eq!(index.provenance(), "Hnsw");
        let nbrs = index.search(&[0.8, 0.2], 1).expect("search");
        assert_eq!(nbrs.indices[0], 0);
    }

    #[test]
    fn rejects_query_dim_mismatch() {
        let data = library();
        let index = AnnIndex::build(&data, 3, 2, &KnnMethod::Exact, DistanceMetric::Euclidean)
            .unwrap();
        let err = index.search(&[1.0, 2.0, 3.0], 1).unwrap_err();
        assert!(matches!(err, KnnError::QueryDimensionMismatch { .. }));
    }
}
