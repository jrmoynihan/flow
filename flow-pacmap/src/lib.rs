//! # flow-pacmap
//!
//! PaCMAP (Pairwise Controlled Manifold Approximation Projection) dimensionality
//! reduction: three-phase pair-weighted embedding with PCA initialization.
//!
//! Designed for large-n flow cytometry data:
//! - No ndarray version conflicts — pure `&[f32]` / `Vec<[f32;2]>` API
//! - PCA via faer SVD on the d×d covariance matrix (O(n·d²), no large intermediates)
//! - HNSW KNN via usearch (C++ FFI, hardware SIMD, optional f16 quantization)
//! - All pair counts use `checked_mul`; no debug-mode overflow panics
//! - Progress reporting via `mpsc::Sender<PaCMAPProgress>` (per phase + every 10 iters)
//! - Cancellation via `Arc<AtomicBool>`

pub mod adam;
pub mod config;
pub mod error;
pub mod gradient;
pub mod knn;
pub mod pairs;
pub mod pca;
pub mod weights;

pub use config::{
    DistanceMetric, HnswParams, Init, KnnMethod, PaCMAPConfig, Quantization,
};
pub use error::PaCMAPError;

use adam::{AdamState, adam_step};
use gradient::compute_gradient;
use knn::compute_knn;
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
/// - `progress`: optional channel for per-iteration progress events
/// - `cancel`: optional cancellation token; checked once per iteration
///
/// # Returns
/// `n` `[f32; 2]` pairs aligned with the input rows, or a `PaCMAPError`.
pub fn fit_transform(
    data: &[f32],
    n: usize,
    d: usize,
    config: PaCMAPConfig,
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

    // ── KNN construction ──────────────────────────────────────────────────
    // Request min(n_nb + 50, n-1) candidates; rerank by scaled distance inside build_pairs.
    let k_candidates = (n_nb + 50).min(n - 1);
    let knn = compute_knn(data, n, d, k_candidates, &config.knn_method, config.distance_metric)?;

    // ── Pair sampling ─────────────────────────────────────────────────────
    let pairs = build_pairs(&knn, data, n, d, n_nb, n_mn, n_fp, config.seed)?;
    drop(knn); // free sigma + distance buffers before embedding allocation

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
    let mut adam = AdamState::new(n);
    let mut global_iter = 0usize;

    for (phase_idx, &phase_len) in config.phase_iters.iter().enumerate() {
        let phase = (phase_idx + 1) as u8;

        for local_iter in 0..phase_len {
            // Cancellation check
            if let Some(ref cancel) = cancel {
                if cancel.load(Ordering::Relaxed) {
                    return Err(PaCMAPError::Cancelled);
                }
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

            adam_step(&mut embedding, &grad, &mut adam, global_iter, config.learning_rate);

            // Emit progress every 10 iterations and at phase boundaries
            if let Some(ref tx) = progress {
                if local_iter == 0 || local_iter == phase_len - 1 || global_iter % 10 == 0 {
                    let _ = tx.send(PaCMAPProgress {
                        phase,
                        iter: global_iter,
                        total_iters,
                        loss,
                    });
                }
            }
        }
    }

    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: two well-separated Gaussian clusters should remain separated
    /// after embedding.
    #[test]
    fn two_cluster_separation() {
        use rand::{RngExt, SeedableRng, rngs::SmallRng};
        let mut rng = SmallRng::seed_from_u64(0);
        let n_per_cluster = 100;
        let n = n_per_cluster * 2;
        let d = 10;

        let mut data = Vec::with_capacity(n * d);
        // Cluster A centred at [0, 0, ..., 0]
        for _ in 0..n_per_cluster {
            for _ in 0..d {
                data.push(rng.random::<f32>() * 0.5);
            }
        }
        // Cluster B centred at [5, 5, ..., 5]
        for _ in 0..n_per_cluster {
            for _ in 0..d {
                data.push(5.0 + rng.random::<f32>() * 0.5);
            }
        }

        let config = PaCMAPConfig {
            n_neighbors: 5,
            phase_iters: [50, 50, 100],
            knn_method: KnnMethod::Exact,
            init: Init::Random(Some(42)),
            ..Default::default()
        };

        let emb = fit_transform(&data, n, d, config, None, None).unwrap();
        assert_eq!(emb.len(), n);

        // Centroids of the two clusters in the embedding should be far apart
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
}
