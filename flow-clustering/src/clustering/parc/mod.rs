//! PARC: phenotyping by accelerated refined community-partitioning.
//!
//! Pipeline: k-NN graph ([`flow_knn`]) → local distance prune → Jaccard global
//! prune → Leiden community detection → optional too-big recursive split →
//! small-population reassignment.
//!
//! Reference: Stassen et al., *Bioinformatics* 36(9):2778–2786 (2020),
//! doi:10.1093/bioinformatics/btaa042. Defaults follow the Python reference
//! implementation (`parc/_parc.py`), not the README where they disagree
//! (e.g. `dist_std_local = 3`).

mod leiden;
mod prune;
mod refine;

use crate::clustering::{ClusteringError, ClusteringResult};
use flow_knn::{
    compute_knn, DistanceMetric, HnswParams, KnnGraph, KnnMethod, NeighborList,
};
use ndarray::Array2;
use prune::{global_jaccard_prune, local_distance_prune};
use refine::{
    reassign_small_populations, renumber_labels, split_too_big_clusters,
};

pub use leiden::ParcPartition;
pub use prune::{JacStdGlobal, KeepLocalDist};

/// Distance space for k-NN construction (mapped onto [`DistanceMetric`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParcDistance {
    /// Euclidean L2 (default). Matches PARC `distance='l2'` intent with
    /// true Euclidean distances from `flow-knn` (hnswlib stores squared L2).
    #[default]
    L2,
    /// Cosine distance.
    Cosine,
}

impl ParcDistance {
    fn to_metric(self) -> DistanceMetric {
        match self {
            Self::L2 => DistanceMetric::Euclidean,
            Self::Cosine => DistanceMetric::Cosine,
        }
    }
}

/// Configuration for PARC clustering.
#[derive(Debug, Clone)]
pub struct ParcConfig {
    /// Neighbours per node in the k-NN graph (default 30).
    pub knn: usize,
    /// Local prune: keep edges with dist < μ + `dist_std_local` · σ (default 3.0).
    pub dist_std_local: f64,
    /// Global Jaccard prune threshold (default median).
    pub jac_std_global: JacStdGlobal,
    /// Whether to skip local distance pruning (default auto: skip when n > 300_000).
    pub keep_all_local_dist: KeepLocalDist,
    /// Pass Jaccard weights into Leiden (default true).
    pub jac_weighted_edges: bool,
    /// Minimum community size before reassignment (default 10).
    pub small_pop: usize,
    /// Max fraction of n for a single community before recursive recluster (default 0.4).
    pub too_big_factor: f64,
    /// Leiden iteration budget (default 5; mapped to `leiden-rs` max_iterations).
    pub n_iter_leiden: usize,
    /// RNG seed for Leiden (default 42).
    pub random_seed: u64,
    /// Resolution for RBConfiguration when ≠ 1 (default 1.0 → Modularity).
    pub resolution_parameter: f64,
    /// Leiden quality function (default Modularity; forced to RB when resolution ≠ 1).
    pub partition: ParcPartition,
    /// Distance for k-NN.
    pub distance: ParcDistance,
    /// HNSW construction / search parameters (defaults tuned toward PARC).
    pub hnsw: HnswParams,
    /// Optional override of [`KnnMethod`]; `None` uses HNSW with [`Self::hnsw`].
    pub knn_method: Option<KnnMethod>,
    /// Max passes when cleaning small populations (deterministic; replaces wall-clock).
    pub small_pop_max_iters: usize,
    /// Global Jaccard σ for too-big sub-PARC (default 0.3).
    pub jac_std_toobig: f64,
}

impl Default for ParcConfig {
    fn default() -> Self {
        Self {
            knn: 30,
            dist_std_local: 3.0,
            jac_std_global: JacStdGlobal::Median,
            keep_all_local_dist: KeepLocalDist::Auto,
            jac_weighted_edges: true,
            small_pop: 10,
            too_big_factor: 0.4,
            n_iter_leiden: 5,
            random_seed: 42,
            resolution_parameter: 1.0,
            partition: ParcPartition::Modularity,
            distance: ParcDistance::L2,
            hnsw: HnswParams {
                m: 24,
                ef_construction: 150,
                ef_search: 100,
                quantization: flow_knn::Quantization::F32,
            },
            knn_method: None,
            small_pop_max_iters: 50,
            jac_std_toobig: 0.3,
        }
    }
}

impl ParcConfig {
    fn effective_partition(&self) -> ParcPartition {
        if (self.resolution_parameter - 1.0).abs() > f64::EPSILON {
            ParcPartition::RbConfiguration
        } else {
            self.partition
        }
    }

    fn skip_local_prune(&self, n: usize) -> bool {
        match self.keep_all_local_dist {
            KeepLocalDist::Always => true,
            KeepLocalDist::Never => false,
            KeepLocalDist::Auto => n > 300_000,
        }
    }

    fn knn_method_for(&self, n: usize, d: usize) -> KnnMethod {
        if let Some(ref method) = self.knn_method {
            return method.clone();
        }
        let mut params = self.hnsw.clone();
        // PARC heuristics: higher M for high-d modest-n; higher ef for small n.
        if d > 30 && n <= 50_000 {
            params.m = params.m.max(48);
        }
        if n < 10_000 {
            let ef = (n.saturating_sub(10)).min(500).max(params.ef_search);
            params.ef_search = ef;
            params.ef_construction = ef.max(params.ef_construction);
        } else {
            params.ef_search = params.ef_search.max(self.knn.saturating_add(1).max(100));
        }
        // `parc` enables `flow-knn` with default features (includes `hnsw`).
        KnnMethod::Hnsw(params)
    }
}

/// Result of PARC clustering.
#[derive(Debug, Clone)]
pub struct ParcResult {
    /// Contiguous community labels in `0..n_clusters`.
    pub assignments: Vec<usize>,
    /// Number of communities after refinement.
    pub n_clusters: usize,
}

/// PARC clustering entry point.
pub struct Parc;

impl Parc {
    /// Run PARC on row-major `data` (`n × d`), computing a k-NN graph internally.
    pub fn fit(data: &Array2<f64>, config: &ParcConfig) -> ClusteringResult<ParcResult> {
        Self::fit_with_knn(data, config, None)
    }

    /// Run PARC, optionally reusing a precomputed [`KnnGraph`].
    ///
    /// When `knn` is `Some`, it must have `graph.n == data.nrows()` and
    /// `graph.k >= config.knn` (extra neighbours are truncated). Metric should
    /// match [`ParcConfig::distance`].
    pub fn fit_with_knn(
        data: &Array2<f64>,
        config: &ParcConfig,
        knn: Option<&KnnGraph>,
    ) -> ClusteringResult<ParcResult> {
        let n = data.nrows();
        let d = data.ncols();
        if n == 0 {
            return Err(ClusteringError::EmptyData);
        }
        if n < 2 {
            return Err(ClusteringError::InsufficientData { min: 2, actual: n });
        }
        if config.knn == 0 {
            return Err(ClusteringError::InvalidConfig(
                "knn must be >= 1".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&config.too_big_factor) {
            return Err(ClusteringError::InvalidConfig(
                "too_big_factor must be in [0, 1]".to_string(),
            ));
        }

        let flat_f32: Vec<f32> = data.iter().map(|&x| x as f32).collect();
        let metric = config.distance.to_metric();
        let graph = match knn {
            Some(g) => {
                validate_injected_knn(g, n, config.knn, metric)?;
                truncate_knn(g, config.knn.min(n - 1))
            }
            None => {
                let method = config.knn_method_for(n, d);
                let k = config.knn.min(n - 1);
                compute_knn(&flat_f32, n, d, k, &method, metric).map_err(|e| {
                    ClusteringError::ClusteringFailed(format!("k-NN failed: {e}"))
                })?
            }
        };

        run_parc_on_graph(&flat_f32, n, d, &graph, config, false)
    }
}

fn validate_injected_knn(
    graph: &KnnGraph,
    n: usize,
    required_k: usize,
    metric: DistanceMetric,
) -> ClusteringResult<()> {
    graph
        .validate(n, required_k.min(n.saturating_sub(1)), metric)
        .map_err(|e| ClusteringError::ValidationFailed(e.to_string()))
}

fn truncate_knn(graph: &KnnGraph, k: usize) -> KnnGraph {
    if graph.k <= k {
        return graph.clone();
    }
    let neighbors = graph
        .neighbors
        .iter()
        .map(|nl| NeighborList {
            indices: nl.indices.iter().take(k).copied().collect(),
            distances: nl.distances.iter().take(k).copied().collect(),
        })
        .collect();
    KnnGraph {
        neighbors,
        n: graph.n,
        k,
        metric: graph.metric,
        provenance: graph.provenance.clone(),
    }
}

/// Core PARC after a k-NN graph is available.
fn run_parc_on_graph(
    data_f32: &[f32],
    n: usize,
    d: usize,
    graph: &KnnGraph,
    config: &ParcConfig,
    is_toobig_sub: bool,
) -> ClusteringResult<ParcResult> {
    let skip_local = config.skip_local_prune(n);
    let distances_are_squared = matches!(graph.metric, DistanceMetric::EuclideanSq);

    let local_edges = local_distance_prune(
        &graph.neighbors,
        config.dist_std_local,
        skip_local,
        distances_are_squared,
    );

    let jac_std = if is_toobig_sub {
        JacStdGlobal::Sigma(config.jac_std_toobig)
    } else {
        config.jac_std_global
    };

    let pruned = global_jaccard_prune(n, &local_edges, jac_std, config.jac_weighted_edges)?;

    let partition = config.effective_partition();
    let mut labels = leiden::run_leiden(
        n,
        &pruned,
        config.jac_weighted_edges,
        partition,
        config.resolution_parameter,
        config.n_iter_leiden,
        config.random_seed,
    )?;

    // Original HNSW neighbour indices (for small-pop reassignment).
    let neighbor_indices: Vec<Vec<u32>> = graph
        .neighbors
        .iter()
        .map(|nl| nl.indices.clone())
        .collect();

    if !is_toobig_sub {
        labels = split_too_big_clusters(
            data_f32,
            n,
            d,
            labels,
            &neighbor_indices,
            config,
        )?;
    }

    labels = reassign_small_populations(
        labels,
        &neighbor_indices,
        config.small_pop,
        config.small_pop_max_iters,
    );
    labels = renumber_labels(labels);

    let n_clusters = labels.iter().copied().max().map(|m| m + 1).unwrap_or(0);
    Ok(ParcResult {
        assignments: labels,
        n_clusters,
    })
}

/// Build k-NN for a subset (too-big path) and run PARC on that subset.
pub(crate) fn run_subparc_on_subset(
    data_f32: &[f32],
    n: usize,
    d: usize,
    config: &ParcConfig,
) -> ClusteringResult<Vec<usize>> {
    let knn_big = if n > config.knn {
        config.knn
    } else {
        ((0.2 * n as f64) as usize).max(5).min(n.saturating_sub(1))
    };
    let mut params = config.hnsw.clone();
    params.ef_construction = 200;
    params.m = 30;
    params.ef_search = params.ef_search.max(knn_big + 1).max(100);

    let method = KnnMethod::Hnsw(params);

    let graph = compute_knn(
        data_f32,
        n,
        d,
        knn_big,
        &method,
        DistanceMetric::Euclidean,
    )
    .map_err(|e| ClusteringError::ClusteringFailed(format!("sub-PARC k-NN failed: {e}")))?;

    let result = run_parc_on_graph(data_f32, n, d, &graph, config, true)?;
    Ok(result.assignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    /// Two isotropic clouds centred far apart (not a 1D chain — modularity splits chains).
    fn two_blobs(n_per: usize, d: usize) -> Array2<f64> {
        assert!(d >= 2);
        let mut rows = Vec::with_capacity(n_per * 2 * d);
        let mut push_cloud = |center0: f64, seed: u64| {
            for i in 0..n_per {
                for j in 0..d {
                    // Mix both i and j into high bits (adding j alone does not).
                    let s = seed
                        .wrapping_mul(0x9E3779B97F4A7C15)
                        .wrapping_add((i as u64).wrapping_mul(0xBF58476D1CE4E5B9))
                        .wrapping_add((j as u64).wrapping_mul(0x94D049BB133111EB))
                        .wrapping_mul(6364136223846793005);
                    let u = ((s >> 33) as f64) / (u32::MAX as f64) - 0.5;
                    let v = if j == 0 {
                        center0 + u * 0.5
                    } else {
                        u * 0.5
                    };
                    rows.push(v);
                }
            }
        };
        push_cloud(0.0, 0xC0FFEE);
        push_cloud(40.0, 0xBADC0DE);
        Array2::from_shape_vec((n_per * 2, d), rows).expect("shape")
    }

    #[test]
    fn parc_separates_two_blobs() {
        let data = two_blobs(50, 3);
        let config = ParcConfig {
            knn: 15,
            knn_method: Some(KnnMethod::Exact),
            keep_all_local_dist: KeepLocalDist::Always,
            jac_std_global: JacStdGlobal::Sigma(1.0),
            small_pop: 5,
            too_big_factor: 0.9,
            ..ParcConfig::default()
        };
        let result = Parc::fit(&data, &config).expect("parc");
        assert_eq!(result.assignments.len(), 100);
        assert!(
            result.n_clusters >= 2,
            "expected >= 2 clusters, got {}",
            result.n_clusters
        );

        // Well-separated clouds must not share community labels (Leiden may still
        // find substructure *within* a cloud).
        let set_a: std::collections::HashSet<_> =
            result.assignments[..50].iter().copied().collect();
        let set_b: std::collections::HashSet<_> =
            result.assignments[50..].iter().copied().collect();
        assert!(
            set_a.is_disjoint(&set_b),
            "halves share labels: {set_a:?} vs {set_b:?} (n_clusters={})",
            result.n_clusters
        );
    }

    #[test]
    fn parc_rejects_empty() {
        let data = Array2::<f64>::zeros((0, 2));
        let err = Parc::fit(&data, &ParcConfig::default()).unwrap_err();
        assert!(matches!(err, ClusteringError::EmptyData));
    }
}

