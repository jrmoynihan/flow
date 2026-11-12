//! Full-batch Lloyd k-means on the shared [`Assign`] trait.
//!
//! Exposed as an experimental baseline so e-means is compared against the same
//! assignment kernel rather than the linfa wrapper in `kmeans.rs`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::index::sample;

use crate::clustering::{ClusteringError, ClusteringResult};

use super::assign::{self, make_assigner};
use super::config::{
    AssignBackend, EMeansControl, EMeansProgress, EMeansStage, InitMethod,
};
use super::{init_centroids, validate_input};

/// Configuration for [`Lloyd::fit`].
#[derive(Debug, Clone)]
pub struct LloydConfig {
    pub n_clusters: usize,
    /// Stop after this many iterations (ignored when `until_convergence` is set
    /// and convergence happens first).
    pub max_iterations: usize,
    /// Relative inertia change below which we stop.
    pub tolerance: f64,
    /// When true, keep iterating until tolerance or `max_iterations`.
    pub until_convergence: bool,
    pub init: InitMethod,
    pub seed: Option<u64>,
    pub backend: AssignBackend,
}

impl Default for LloydConfig {
    fn default() -> Self {
        Self {
            n_clusters: 32,
            max_iterations: 25,
            tolerance: 1e-4,
            until_convergence: false,
            init: InitMethod::Random,
            seed: Some(42),
            backend: AssignBackend::default(),
        }
    }
}

/// Result of a Lloyd run.
#[derive(Debug, Clone)]
pub struct LloydResult {
    pub centroids: Vec<f32>,
    pub n_clusters: usize,
    pub n_features: usize,
    pub assignments: Vec<u32>,
    pub counts: Vec<usize>,
    pub inertia: f64,
    pub iterations: usize,
}

/// Full-batch Lloyd k-means.
pub struct Lloyd;

impl Lloyd {
    /// Fit Lloyd on row-major `f32` data of shape `n × d`.
    pub fn fit(data: &[f32], n: usize, d: usize, config: &LloydConfig) -> ClusteringResult<LloydResult> {
        Self::fit_with_control(data, n, d, config, None, None)
    }

    /// Fit with optional progress and cooperative cancel.
    pub fn fit_with_control(
        data: &[f32],
        n: usize,
        d: usize,
        config: &LloydConfig,
        progress: Option<mpsc::Sender<EMeansProgress>>,
        cancel: Option<Arc<AtomicBool>>,
    ) -> ClusteringResult<LloydResult> {
        validate_input(data, n, d, config.n_clusters)?;
        let k = config.n_clusters;
        let seed = config.seed.unwrap_or_else(rand::random);
        let mut rng = StdRng::seed_from_u64(seed);
        let assigner = make_assigner(config.backend)?;

        let stage_count = 3; // init, iterate, done
        let control = EMeansControl::new(progress.as_ref(), cancel.as_deref(), stage_count);
        control.emit(EMeansStage::Init, 0, None)?;

        let mut centroids = init_centroids(data, n, d, k, config.init, &mut rng)?;
        let mut assignments = vec![0u32; n];
        let mut prev_inertia = f64::INFINITY;
        let mut iterations = 0usize;

        let max_iter = config.max_iterations.max(1);
        for iter in 0..max_iter {
            control.emit(
                EMeansStage::Epoch,
                1,
                Some(format!("lloyd_iter={iter}")),
            )?;
            assigner.assign(data, n, d, &centroids, k, &mut assignments)?;

            let mut counts = vec![0.0f64; k];
            let mut sums = vec![0.0f64; k * d];
            assign::accumulate_batch(data, n, d, &assignments, k, &mut counts, &mut sums);

            // Recompute centroids; empty clusters keep their previous location.
            for j in 0..k {
                if counts[j] > 0.0 {
                    let inv = 1.0 / counts[j];
                    for t in 0..d {
                        centroids[j * d + t] = (sums[j * d + t] * inv) as f32;
                    }
                }
            }

            iterations = iter + 1;
            let cur = assign::inertia(data, n, d, &centroids, &assignments);
            let rel = if prev_inertia > 0.0 {
                ((prev_inertia - cur) / prev_inertia).abs()
            } else {
                0.0
            };
            prev_inertia = cur;
            if config.until_convergence && rel < config.tolerance {
                break;
            }
            if !config.until_convergence && iterations >= config.max_iterations {
                break;
            }
        }

        assigner.assign(data, n, d, &centroids, k, &mut assignments)?;
        let inertia = assign::inertia(data, n, d, &centroids, &assignments);
        let mut counts_u = vec![0usize; k];
        for &a in &assignments {
            if (a as usize) < k {
                counts_u[a as usize] += 1;
            }
        }
        control.emit(EMeansStage::Done, 2, None)?;

        Ok(LloydResult {
            centroids,
            n_clusters: k,
            n_features: d,
            assignments,
            counts: counts_u,
            inertia,
            iterations,
        })
    }
}

/// Sample `k` distinct row indices without replacement.
pub(crate) fn sample_distinct_rows(
    n: usize,
    k: usize,
    rng: &mut impl rand::Rng,
) -> ClusteringResult<Vec<usize>> {
    if k > n {
        return Err(ClusteringError::InsufficientData {
            min: k,
            actual: n,
        });
    }
    Ok(sample(rng, n, k).into_vec())
}
