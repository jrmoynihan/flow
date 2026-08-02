//! # flow-pacmap
//!
//! First-party implementation of PaCMAP (Pairwise Controlled Manifold
//! Approximation Projection) from Wang et al. 2021 (JMLR 22, Algorithm 1).
//!
//! Designed for large-n flow cytometry data:
//! - No ndarray version conflicts — pure `&[f32]` / `Vec<[f32;2]>` API
//! - PCA via faer SVD on the d×d covariance matrix (O(n·d²), no large intermediates)
//! - HNSW KNN via usearch (C++ FFI, hardware SIMD, optional f16 quantization)
//! - All pair counts use `checked_mul`; no debug-mode overflow panics
//! - Progress reporting via `mpsc::Sender<PaCMAPProgress>` (per phase + every 10 iters)
//! - Cancellation via `Arc<AtomicBool>`
//! - Staged KNN: [`compute_knn`] → [`KnnGraph`] → optional input to [`fit_transform`]

pub mod adam;
pub mod config;
pub mod error;
pub mod gradient;
#[cfg(feature = "cubecl")]
pub mod gpu;
pub mod knn;
pub mod pairs;
pub mod pca;
pub mod weights;

pub use config::{
    DistanceMetric, HnswParams, Init, KnnMethod, OptimizeBackend, PaCMAPConfig, Quantization,
};
pub use error::PaCMAPError;
pub use knn::{KnnGraph, NeighborList, compute_knn, read_knn_graph, validate_knn_for_pacmap, write_knn_graph};

use adam::{AdamState, adam_step};
use gradient::compute_gradient;
use pairs::build_pairs;
use pca::pca_init;
use weights::weights_at;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

/// Progress event emitted during optimization.
#[derive(Debug, Clone)]
pub struct PaCMAPProgress {
    /// Current optimization phase (1, 2, or 3).
    pub phase: u8,
    /// Current iteration number (1-indexed).
    pub iter: usize,
    /// Total iterations across all phases.
    pub total_iters: usize,
    /// Loss value at this iteration.
    pub loss: f32,
}

/// Embed `n × d` row-major f32 data into 2 dimensions.
///
/// # Arguments
/// - `data`: flat row-major slice, `len = n * d`
/// - `n`: number of points; must be ≥ 2 and ≤ `u32::MAX`
/// - `d`: number of dimensions per point; must be ≥ 1
/// - `config`: algorithm configuration
/// - `knn`: optional precomputed [`KnnGraph`]. When `None`, KNN is computed
///   internally (`n_neighbors + 50` candidates, capped by `n − 1`). When
///   `Some`, search is skipped after validating `n` / `k` / metric.
/// - `progress`: optional channel for per-iteration progress events
/// - `cancel`: optional cancellation token; checked once per iteration
///
/// # Returns
/// `n` `[f32; 2]` pairs aligned with the input rows, or a `PaCMAPError`.
///
/// # Breaking change
/// As of 0.1.2 the signature inserts `knn: Option<&KnnGraph>` after `config`.
/// One-shot callers pass `None`; staged callers pass `Some(&graph)` from
/// [`compute_knn`].
pub fn fit_transform(
    data: &[f32],
    n: usize,
    d: usize,
    config: PaCMAPConfig,
    knn: Option<&KnnGraph>,
    progress: Option<mpsc::Sender<PaCMAPProgress>>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<[f32; 2]>, PaCMAPError> {
    // ── Input validation ──────────────────────────────────────────────────
    if n < 2 {
        return Err(PaCMAPError::DatasetTooSmall { n });
    }
    if data.len() != n * d {
        return Err(PaCMAPError::DimensionMismatch { len: data.len(), d });
    }
    if n > u32::MAX as usize {
        return Err(PaCMAPError::DatasetTooLarge { n });
    }

    let n_nb = config.n_neighbors.min(n - 1);
    let n_mn = config.n_mn();
    let n_fp = config.n_fp();
    let total_iters = config.total_iters();

    // ── KNN construction (or reuse) + pair sampling ───────────────────────
    // Request min(n_nb + 50, n-1) candidates; rerank by scaled distance inside build_pairs.
    // Owned graphs are dropped when this block ends (before embedding allocation).
    let pairs = {
        let k_candidates = KnnGraph::required_k_for_pacmap(n, config.n_neighbors);
        let owned_knn;
        let knn_graph: &KnnGraph = if let Some(graph) = knn {
            validate_knn_for_pacmap(graph, n, config.n_neighbors, config.distance_metric)?;
            graph
        } else {
            owned_knn = compute_knn(
                data,
                n,
                d,
                k_candidates,
                &config.knn_method,
                config.distance_metric,
            )?;
            &owned_knn
        };
        build_pairs(
            &knn_graph.neighbors,
            data,
            n,
            d,
            n_nb,
            n_mn,
            n_fp,
            config.seed,
        )?
    };

    // ── Initialisation ────────────────────────────────────────────────────
    let mut embedding: Vec<[f32; 2]> = match &config.init {
        Init::Pca => pca_init(data, n, d)?,
        Init::Random(seed) => {
            use rand::{RngExt, SeedableRng, rngs::SmallRng};
            let mut rng = match seed {
                Some(s) => SmallRng::seed_from_u64(*s),
                None => rand::make_rng::<SmallRng>(),
            };
            let scale = (1e-4_f32).sqrt();
            (0..n)
                .map(|_| [rng.random::<f32>() * scale, rng.random::<f32>() * scale])
                .collect()
        }
    };

    // ── Optimization (Adam, 3-phase weight schedule) ──────────────────────
    #[cfg(feature = "cubecl")]
    if matches!(config.optimize_backend, OptimizeBackend::Gpu) {
        crate::gpu::optimize_embedding_gpu(
            &mut embedding,
            &pairs.near,
            &pairs.mid_near,
            &pairs.further,
            &config.phase_iters,
            config.learning_rate,
            cancel,
        )?;
        return Ok(embedding);
    }

    let mut adam = AdamState::new(n);
    let mut global_iter = 0usize;

    for (phase_idx, &phase_len) in config.phase_iters.iter().enumerate() {
        let phase = (phase_idx + 1) as u8;

        for local_iter in 0..phase_len {
            // Cancellation check
            if let Some(ref cancel) = cancel
                && cancel.load(Ordering::Relaxed)
            {
                return Err(PaCMAPError::Cancelled);
            }

            global_iter += 1;
            let w = weights_at(global_iter, &config.phase_iters);

            let (grad, loss) = compute_gradient(
                &embedding,
                &pairs.near,
                &pairs.mid_near,
                &pairs.further,
                &w,
                n,
            );

            adam_step(
                &mut embedding,
                &grad,
                &mut adam,
                global_iter,
                config.learning_rate,
            );

            // Emit progress every 10 iterations and at phase boundaries
            if let Some(ref tx) = progress
                && (local_iter == 0
                    || local_iter == phase_len - 1
                    || global_iter.is_multiple_of(10))
            {
                let _ = tx.send(PaCMAPProgress {
                    phase,
                    iter: global_iter,
                    total_iters,
                    loss,
                });
            }
        }
    }

    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_cluster_data(n_per_cluster: usize, d: usize) -> (Vec<f32>, usize) {
        use rand::{RngExt, SeedableRng, rngs::SmallRng};
        let mut rng = SmallRng::seed_from_u64(0);
        let n = n_per_cluster * 2;
        let mut data = Vec::with_capacity(n * d);
        for _ in 0..n_per_cluster {
            for _ in 0..d {
                data.push(rng.random::<f32>() * 0.5);
            }
        }
        for _ in 0..n_per_cluster {
            for _ in 0..d {
                data.push(5.0 + rng.random::<f32>() * 0.5);
            }
        }
        (data, n)
    }

    fn base_config() -> PaCMAPConfig {
        PaCMAPConfig {
            n_neighbors: 5,
            phase_iters: [50, 50, 100],
            knn_method: KnnMethod::Exact,
            init: Init::Random(Some(42)),
            seed: Some(7),
            ..Default::default()
        }
    }

    /// Smoke test: two well-separated Gaussian clusters should remain separated
    /// after embedding.
    #[test]
    fn two_cluster_separation() {
        let n_per_cluster = 100;
        let d = 10;
        let (data, n) = two_cluster_data(n_per_cluster, d);
        let config = base_config();

        let emb = fit_transform(&data, n, d, config, None, None, None).unwrap();
        assert_eq!(emb.len(), n);

        let c_a: [f32; 2] = {
            let sx: f32 = emb[..n_per_cluster].iter().map(|p| p[0]).sum();
            let sy: f32 = emb[..n_per_cluster].iter().map(|p| p[1]).sum();
            [sx / n_per_cluster as f32, sy / n_per_cluster as f32]
        };
        let c_b: [f32; 2] = {
            let sx: f32 = emb[n_per_cluster..].iter().map(|p| p[0]).sum();
            let sy: f32 = emb[n_per_cluster..].iter().map(|p| p[1]).sum();
            [sx / n_per_cluster as f32, sy / n_per_cluster as f32]
        };
        let sep = ((c_a[0] - c_b[0]).powi(2) + (c_a[1] - c_b[1]).powi(2)).sqrt();
        assert!(
            sep > 0.5,
            "cluster centroids should be separated in embedding (got {sep:.3})"
        );
    }

    #[test]
    fn precomputed_knn_matches_internal() {
        let (data, n) = two_cluster_data(40, 8);
        let mut config = base_config();
        config.knn_method = KnnMethod::Exact;
        let k = KnnGraph::required_k_for_pacmap(n, config.n_neighbors);
        let graph = compute_knn(
            &data,
            n,
            8,
            k,
            &KnnMethod::Exact,
            Default::default(),
        )
        .unwrap();
        let a = fit_transform(&data, n, 8, config.clone(), None, None, None).unwrap();
        let b = fit_transform(&data, n, 8, config, Some(&graph), None, None).unwrap();
        for (p, q) in a.iter().zip(b.iter()) {
            assert!((p[0] - q[0]).abs() < 1e-4 && (p[1] - q[1]).abs() < 1e-4);
        }
    }

    #[cfg(feature = "cubecl")]
    #[test]
    fn gpu_optimize_runs_when_adapter_available() {
        if !crate::gpu::gpu_context_available() {
            eprintln!("skipping gpu_optimize_runs_when_adapter_available: no WGPU adapter");
            return;
        }
        let (data, n) = two_cluster_data(80, 6);
        let mut config = base_config();
        config.phase_iters = [8, 8, 16];
        config.optimize_backend = OptimizeBackend::Gpu;
        let emb = fit_transform(&data, n, 6, config, None, None, None).unwrap();
        assert_eq!(emb.len(), n);
        assert!(emb.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
    }

    #[cfg(feature = "cubecl")]
    #[test]
    fn gpu_and_cpu_embeddings_correlate() {
        if !crate::gpu::gpu_context_available() {
            eprintln!("skipping gpu_and_cpu_embeddings_correlate: no WGPU adapter");
            return;
        }
        let (data, n) = two_cluster_data(60, 5);
        let mut cpu_cfg = base_config();
        cpu_cfg.phase_iters = [20, 20, 40];
        cpu_cfg.optimize_backend = OptimizeBackend::Cpu;
        let mut gpu_cfg = cpu_cfg.clone();
        gpu_cfg.optimize_backend = OptimizeBackend::Gpu;

        let cpu = fit_transform(&data, n, 5, cpu_cfg, None, None, None).unwrap();
        let gpu = fit_transform(&data, n, 5, gpu_cfg, None, None, None).unwrap();

        // Same init + pairs → embeddings should be close (FP / schedule differences allowed).
        let mut max_delta = 0.0f32;
        for (a, b) in cpu.iter().zip(gpu.iter()) {
            max_delta = max_delta.max((a[0] - b[0]).abs()).max((a[1] - b[1]).abs());
        }
        assert!(
            max_delta < 2.0,
            "cpu vs gpu embedding max |Δ|={max_delta} (expected loose agreement)"
        );
    }

    #[test]
    fn precomputed_knn_rejects_bad_n_and_k() {
        let (data, n) = two_cluster_data(30, 4);
        let d = 4;
        let config = base_config();
        let k = KnnGraph::required_k_for_pacmap(n, config.n_neighbors);
        let graph = compute_knn(
            &data,
            n,
            d,
            k,
            &KnnMethod::Exact,
            DistanceMetric::Euclidean,
        )
        .unwrap();

        let mut bad_n = graph.clone();
        bad_n.n = n + 1;
        assert!(matches!(
            fit_transform(&data, n, d, config.clone(), Some(&bad_n), None, None),
            Err(PaCMAPError::KnnGraphSizeMismatch { .. })
        ));

        let mut bad_k = graph.clone();
        bad_k.k = 1;
        assert!(matches!(
            fit_transform(&data, n, d, config.clone(), Some(&bad_k), None, None),
            Err(PaCMAPError::KnnGraphInsufficientK { .. })
        ));

        let mut bad_metric = graph;
        bad_metric.metric = DistanceMetric::Cosine;
        assert!(matches!(
            fit_transform(&data, n, d, config, Some(&bad_metric), None, None),
            Err(PaCMAPError::KnnGraphMetricMismatch { .. })
        ));
    }

    #[test]
    fn one_knn_graph_reused_across_configs() {
        let (data, n) = two_cluster_data(30, 4);
        let d = 4;
        let n_neighbors = 5;
        let k = KnnGraph::required_k_for_pacmap(n, n_neighbors);
        let graph = compute_knn(
            &data,
            n,
            d,
            k,
            &KnnMethod::Exact,
            DistanceMetric::Euclidean,
        )
        .unwrap();

        let config_a = PaCMAPConfig {
            n_neighbors,
            phase_iters: [20, 20, 40],
            knn_method: KnnMethod::Exact,
            init: Init::Random(Some(1)),
            seed: Some(11),
            learning_rate: 1.0,
            ..Default::default()
        };
        let config_b = PaCMAPConfig {
            n_neighbors,
            phase_iters: [10, 10, 20],
            knn_method: KnnMethod::Exact,
            init: Init::Random(Some(2)),
            seed: Some(22),
            learning_rate: 0.5,
            ..Default::default()
        };

        let emb_a = fit_transform(&data, n, d, config_a, Some(&graph), None, None).unwrap();
        let emb_b = fit_transform(&data, n, d, config_b, Some(&graph), None, None).unwrap();
        assert_eq!(emb_a.len(), n);
        assert_eq!(emb_b.len(), n);
        // Different configs should generally produce different embeddings.
        let differs = emb_a
            .iter()
            .zip(emb_b.iter())
            .any(|(a, b)| (a[0] - b[0]).abs() > 1e-3 || (a[1] - b[1]).abs() > 1e-3);
        assert!(differs, "distinct configs should not yield identical embeddings");
    }
}
