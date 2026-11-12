//! Configuration, progress, and result types for e-means / Lloyd baselines.

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use super::schedule::SCHEDULE_FLOOR;

/// BF16 machine epsilon (2⁻⁷). Used as the donor-split perturbation δ
/// (Bessa et al., Appendix H.1).
pub const REVIVAL_DELTA_BF16: f32 = 0.007_812_5;

/// How to place the initial centroids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InitMethod {
    /// Uniform sample of `k` distinct rows (paper default).
    #[default]
    Random,
    /// k-means++ seeding (Arthur & Vassilvitskii).
    KMeansPlusPlus,
}

/// How to turn running EMA state into the returned centroids.
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum Finalization {
    /// Average of points assigned during the last epoch (paper default).
    #[default]
    LastEpochMean,
    /// Extra slow EMA of assigned points; constant step = `multiplier · ε · α₀`.
    ///
    /// The multiplier defaults to 1.0. Appendix I describes this as a multiplier
    /// on the cosine floor ε·α₀; the paper's half-life `h` is exposed separately
    /// when callers prefer that parametrisation via [`EMeansConfig::slow_ema_from_half_life`].
    SlowEma {
        /// Multiplier on `schedule_floor · alpha0` used as the constant EMA step.
        multiplier: f32,
    },
    /// Return the EMA ratio Sⱼ/Nⱼ with no extra pass.
    None,
}


/// Assignment backend for e-means / Lloyd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssignBackend {
    /// Rayon over points, hand-written distance loop (default until faer A/B wins).
    #[default]
    CpuLoop,
    /// Rayon over point chunks with a faer matrix product for the cross term.
    #[cfg(feature = "emeans-faer")]
    CpuFaer,
    /// cubeCL nearest-centroid kernel (requires `emeans-gpu`).
    #[cfg(feature = "emeans-gpu")]
    Gpu,
}

/// Coarse stages for [`EMeansProgress`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EMeansStage {
    Init,
    Epoch,
    Finalize,
    Done,
}

/// Progress event at coarse stage boundaries.
#[derive(Debug, Clone)]
pub struct EMeansProgress {
    pub stage: EMeansStage,
    pub stage_index: usize,
    pub stage_count: usize,
    pub fraction: f32,
    pub detail: Option<String>,
}

/// Configuration for [`super::EMeans::fit`].
#[derive(Debug, Clone)]
pub struct EMeansConfig {
    /// Number of clusters k.
    pub n_clusters: usize,
    /// Number of epochs T (full passes over the data).
    pub epochs: usize,
    /// Override batch size B. `None` uses the published scaling law.
    pub batch_size: Option<usize>,
    /// Override peak step α₀. `None` uses the published scaling law.
    pub alpha0: Option<f32>,
    /// Floor fraction ε of the cosine schedule (default 0.1).
    pub schedule_floor: f32,
    /// Check for dead clusters every this many batches (default 5).
    pub revival_interval: usize,
    /// Revive when Nⱼ < τ · mean(N) (default 0.1).
    pub revival_threshold: f32,
    /// Perturbation δ for donor split (default BF16 ε).
    pub revival_delta: f32,
    /// Centroid initialisation.
    pub init: InitMethod,
    /// Finalisation strategy.
    pub finalization: Finalization,
    /// RNG seed. Required for reproducibility; when `None` a seed is drawn from
    /// the system RNG once at the start of the fit.
    pub seed: Option<u64>,
    /// Assignment backend.
    pub backend: AssignBackend,
}

impl Default for EMeansConfig {
    fn default() -> Self {
        Self {
            n_clusters: 32,
            epochs: 3,
            batch_size: None,
            alpha0: None,
            schedule_floor: SCHEDULE_FLOOR,
            revival_interval: 5,
            revival_threshold: 0.1,
            revival_delta: REVIVAL_DELTA_BF16,
            init: InitMethod::Random,
            finalization: Finalization::default(),
            seed: Some(42),
            backend: AssignBackend::default(),
        }
    }
}

impl EMeansConfig {
    /// Set [`Finalization::SlowEma`] so the constant step equals
    /// `1 − 0.5^(1/half_life_batches)`, then store it as a multiplier on
    /// `schedule_floor · alpha0` once α₀ is known at fit time.
    ///
    /// Callers who already know the desired multiplier should set
    /// `finalization = Finalization::SlowEma { multiplier }` directly.
    pub fn with_slow_ema_half_life(mut self, half_life_batches: f32) -> Self {
        // Placeholder multiplier; resolved in fit once alpha0 is known.
        // We stash the half-life via a sentinel negative multiplier and
        // interpret it in the runner — see `resolve_slow_ema_step`.
        self.finalization = Finalization::SlowEma {
            multiplier: -half_life_batches,
        };
        self
    }
}

/// Resolve the constant slow-EMA step from config once `alpha0` is known.
pub(crate) fn resolve_slow_ema_step(finalization: Finalization, floor: f32, alpha0: f32) -> Option<f32> {
    match finalization {
        Finalization::SlowEma { multiplier } if multiplier < 0.0 => {
            let half_life = (-multiplier).max(1.0);
            let alpha = 1.0 - 0.5_f32.powf(1.0 / half_life);
            Some(alpha.clamp(1e-8, 1.0))
        }
        Finalization::SlowEma { multiplier } => {
            Some((multiplier * floor * alpha0).clamp(1e-8, 1.0))
        }
        _ => None,
    }
}

/// Output of a successful e-means fit.
#[derive(Debug, Clone)]
pub struct EMeansResult {
    /// Centroids, row-major `k × d`.
    pub centroids: Vec<f32>,
    /// Number of clusters.
    pub n_clusters: usize,
    /// Feature dimension.
    pub n_features: usize,
    /// Hard assignment of every training point (length `n`).
    pub assignments: Vec<u32>,
    /// Number of points per cluster (length `k`).
    pub counts: Vec<usize>,
    /// Mean squared Euclidean distance to assigned centroid.
    pub inertia: f64,
    /// Epochs completed.
    pub epochs_run: usize,
    /// Batch size actually used.
    pub batch_size: usize,
    /// Peak step actually used.
    pub alpha0: f32,
    /// Number of donor-split revivals performed.
    pub n_revivals: usize,
}

/// Optional progress + cancel handles (PARC-shaped).
pub(crate) struct EMeansControl<'a> {
    progress: Option<&'a mpsc::Sender<EMeansProgress>>,
    cancel: Option<&'a AtomicBool>,
    stage_count: usize,
}

impl<'a> EMeansControl<'a> {
    pub(crate) fn new(
        progress: Option<&'a mpsc::Sender<EMeansProgress>>,
        cancel: Option<&'a AtomicBool>,
        stage_count: usize,
    ) -> Self {
        Self {
            progress,
            cancel,
            stage_count,
        }
    }

    pub(crate) fn check_cancel(&self) -> crate::clustering::ClusteringResult<()> {
        use std::sync::atomic::Ordering;
        if let Some(flag) = self.cancel
            && flag.load(Ordering::Relaxed)
        {
            return Err(crate::clustering::ClusteringError::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn emit(
        &self,
        stage: EMeansStage,
        stage_index: usize,
        detail: Option<String>,
    ) -> crate::clustering::ClusteringResult<()> {
        self.check_cancel()?;
        if let Some(tx) = self.progress {
            let denom = self.stage_count.saturating_sub(1).max(1) as f32;
            let fraction = (stage_index as f32 / denom).clamp(0.0, 1.0);
            let _ = tx.send(EMeansProgress {
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
