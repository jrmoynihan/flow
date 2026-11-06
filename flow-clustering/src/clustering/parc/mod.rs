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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

pub use leiden::ParcPartition;
pub use prune::{JacStdGlobal, KeepLocalDist};

/// Coarse PARC pipeline stage for progress events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParcStage {
    /// Building or validating the k-NN graph.
    Knn,
    /// Local distance prune.
    LocalPrune,
    /// Global Jaccard prune.
    JaccardPrune,
    /// Leiden community detection.
    Leiden,
    /// Recursive split of oversized communities.
    SplitTooBig,
    /// Reassign points in undersized communities.
    ReassignSmall,
    /// Pipeline finished.
    Done,
}

impl ParcStage {
    fn label(self) -> &'static str {
        match self {
            Self::Knn => "knn",
            Self::LocalPrune => "local_prune",
            Self::JaccardPrune => "jaccard_prune",
            Self::Leiden => "leiden",
            Self::SplitTooBig => "split_too_big",
            Self::ReassignSmall => "reassign_small",
            Self::Done => "done",
        }
    }
}

/// Progress event emitted at coarse PARC stage boundaries.
#[derive(Debug, Clone)]
pub struct ParcProgress {
    /// Current pipeline stage.
    pub stage: ParcStage,
    /// Zero-based index among [`Self::stage_count`] coarse stages.
    pub stage_index: usize,
    /// Total coarse stages for this run (includes [`ParcStage::Done`]).
    pub stage_count: usize,
    /// Coarse overall fraction in `0.0..=1.0`.
    pub fraction: f32,
    /// Optional detail (e.g. too-big subpass size).
    pub detail: Option<String>,
}

/// Optional progress channel and cancellation token (PaCMAP-shaped).
#[derive(Clone, Copy)]
pub(crate) struct ParcControl<'a> {
    progress: Option<&'a mpsc::Sender<ParcProgress>>,
    cancel: Option<&'a AtomicBool>,
    stage_count: usize,
}

impl<'a> ParcControl<'a> {
    pub(crate) fn check_cancel(&self) -> ClusteringResult<()> {
        if let Some(flag) = self.cancel
            && flag.load(Ordering::Relaxed)
        {
            return Err(ClusteringError::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn emit(
        &self,
        stage: ParcStage,
        stage_index: usize,
        detail: Option<String>,
    ) -> ClusteringResult<()> {
        self.check_cancel()?;
        if let Some(tx) = self.progress {
            let denom = self.stage_count.saturating_sub(1).max(1) as f32;
            let fraction = (stage_index as f32 / denom).clamp(0.0, 1.0);
            let _ = tx.send(ParcProgress {
                stage,
                stage_index,
                stage_count: self.stage_count,
                fraction,
                detail,
            });
        }
        Ok(())
    }
}

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
    /// Use Rayon for local and Jaccard prune stages (default true).
    /// Leiden may still use Rayon internally unless the process thread pool is capped.
    pub parallel_prune: bool,
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
            parallel_prune: true,
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

/// Top-level stage count (Knn … Done).
const TOP_STAGE_COUNT: usize = 7;
/// Sub-PARC omits SplitTooBig and outer Done bookkeeping uses fewer emits.
const SUB_STAGE_COUNT: usize = 6;

/// PARC clustering entry point.
pub struct Parc;

impl Parc {
    /// Run PARC on row-major `data` (`n × d`), computing a k-NN graph internally.
    pub fn fit(data: &Array2<f64>, config: &ParcConfig) -> ClusteringResult<ParcResult> {
        Self::fit_with_knn(data, config, None, None, None)
    }

    /// Run PARC, optionally reusing a precomputed [`KnnGraph`].
    ///
    /// When `knn` is `Some`, it must have `graph.n == data.nrows()` and
    /// `graph.k >= config.knn` (extra neighbours are truncated). Metric should
    /// match [`ParcConfig::distance`].
    ///
    /// - `progress`: optional channel for coarse stage events (PaCMAP-shaped)
    /// - `cancel`: optional flag; checked at stage boundaries (`true` →
    ///   [`ClusteringError::Cancelled`])
    pub fn fit_with_knn(
        data: &Array2<f64>,
        config: &ParcConfig,
        knn: Option<&KnnGraph>,
        progress: Option<mpsc::Sender<ParcProgress>>,
        cancel: Option<Arc<AtomicBool>>,
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

        let control = ParcControl {
            progress: progress.as_ref(),
            cancel: cancel.as_deref(),
            stage_count: TOP_STAGE_COUNT,
        };

        control.emit(ParcStage::Knn, 0, None)?;

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
        control.check_cancel()?;

        run_parc_on_graph(&flat_f32, n, d, &graph, config, false, control)
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
    control: ParcControl<'_>,
) -> ClusteringResult<ParcResult> {
    // Stage indices: 0=Knn (already emitted for top-level), then 1.. 
    let local_idx = if is_toobig_sub { 1 } else { 1 };
    let jac_idx = local_idx + 1;
    let leiden_idx = jac_idx + 1;
    let split_idx = leiden_idx + 1;
    let reassign_idx = if is_toobig_sub {
        leiden_idx + 1
    } else {
        split_idx + 1
    };
    let done_idx = reassign_idx + 1;

    control.emit(ParcStage::LocalPrune, local_idx, None)?;
    let skip_local = config.skip_local_prune(n);
    let distances_are_squared = matches!(graph.metric, DistanceMetric::EuclideanSq);

    let local_edges = local_distance_prune(
        &graph.neighbors,
        config.dist_std_local,
        skip_local,
        distances_are_squared,
        config.parallel_prune,
    );

    control.emit(ParcStage::JaccardPrune, jac_idx, None)?;
    let jac_std = if is_toobig_sub {
        JacStdGlobal::Sigma(config.jac_std_toobig)
    } else {
        config.jac_std_global
    };

    let pruned = global_jaccard_prune(
        n,
        &local_edges,
        jac_std,
        config.jac_weighted_edges,
        config.parallel_prune,
    )?;

    control.emit(ParcStage::Leiden, leiden_idx, None)?;
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
        control.emit(ParcStage::SplitTooBig, split_idx, None)?;
        labels = split_too_big_clusters(
            data_f32,
            n,
            d,
            labels,
            &neighbor_indices,
            config,
            control,
        )?;
    }

    control.emit(ParcStage::ReassignSmall, reassign_idx, None)?;
    labels = reassign_small_populations(
        labels,
        &neighbor_indices,
        config.small_pop,
        config.small_pop_max_iters,
    );
    labels = renumber_labels(labels);

    control.emit(
        ParcStage::Done,
        done_idx.min(control.stage_count.saturating_sub(1)),
        Some(ParcStage::Done.label().to_string()),
    )?;

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
    control: ParcControl<'_>,
) -> ClusteringResult<Vec<usize>> {
    control.emit(
        ParcStage::Knn,
        0,
        Some(format!("subparc n={n}")),
    )?;

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
    control.check_cancel()?;

    let sub_control = ParcControl {
        progress: control.progress,
        cancel: control.cancel,
        stage_count: SUB_STAGE_COUNT,
    };
    let result = run_parc_on_graph(data_f32, n, d, &graph, config, true, sub_control)?;
    Ok(result.assignments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;
    use std::sync::mpsc;
    use std::time::Duration;

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

    fn test_config() -> ParcConfig {
        ParcConfig {
            knn: 15,
            knn_method: Some(KnnMethod::Exact),
            keep_all_local_dist: KeepLocalDist::Always,
            jac_std_global: JacStdGlobal::Sigma(1.0),
            small_pop: 5,
            too_big_factor: 0.9,
            ..ParcConfig::default()
        }
    }

    #[test]
    fn parc_separates_two_blobs() {
        let data = two_blobs(50, 3);
        let config = test_config();
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

    #[test]
    fn parc_cancel_before_run() {
        let data = two_blobs(40, 3);
        let cancel = Arc::new(AtomicBool::new(true));
        let err = Parc::fit_with_knn(&data, &test_config(), None, None, Some(cancel))
            .unwrap_err();
        assert!(matches!(err, ClusteringError::Cancelled));
    }

    #[test]
    fn parc_progress_emits_ordered_stages() {
        let data = two_blobs(40, 3);
        let (tx, rx) = mpsc::channel();
        let result = Parc::fit_with_knn(&data, &test_config(), None, Some(tx), None)
            .expect("parc");
        assert_eq!(result.assignments.len(), 80);

        let mut stages = Vec::new();
        while let Ok(ev) = rx.recv_timeout(Duration::from_millis(50)) {
            stages.push(ev.stage);
        }
        assert!(
            stages.first() == Some(&ParcStage::Knn),
            "first stage={:?}",
            stages.first()
        );
        assert!(
            stages.last() == Some(&ParcStage::Done),
            "last stage={:?}",
            stages.last()
        );
        assert!(stages.contains(&ParcStage::LocalPrune));
        assert!(stages.contains(&ParcStage::JaccardPrune));
        assert!(stages.contains(&ParcStage::Leiden));
        assert!(stages.contains(&ParcStage::ReassignSmall));
        // Monotonic stage_index among received events.
        let mut prev = 0usize;
        let (tx2, rx2) = mpsc::channel();
        let _ = Parc::fit_with_knn(&data, &test_config(), None, Some(tx2), None);
        while let Ok(ev) = rx2.recv_timeout(Duration::from_millis(50)) {
            assert!(ev.stage_index >= prev);
            prev = ev.stage_index;
            assert!(ev.fraction >= 0.0 && ev.fraction <= 1.0);
        }
    }
}
