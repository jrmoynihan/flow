//! Configuration for AF library discovery and event matching.

use flow_knn::{DistanceMetric, HnswParams, KnnMethod};

/// How to build an [`crate::AfLibrary`] from unstained (or cleaned) events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiscoveryBackend {
    /// Gaussian mixture centroids (default Phase 1 path).
    #[default]
    Gmm,
    /// K-means centroids.
    KMeans,
    /// K-means assignments, then the HNSW neighbour of each centroid as the signature.
    HnswMedoid,
    /// Batch SOM codebook, then k-means metaclusters (FlowSOM).
    FlowSom,
}

/// How to assign stained events to AF library columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchStrategy {
    /// Try every AF column (or ANN shortlist) and keep lowest OLS residual.
    #[default]
    ResidualOls,
    /// Cosine / Euclidean nearest neighbour only (fast quality floor).
    NearestNeighbor,
}

/// Optional scatter-match of unstained events against stained scatter.
#[derive(Debug, Clone)]
pub struct ScatterCleanConfig {
    /// Keep unstained events whose NN distance to stained scatter is at or below
    /// this percentile of those distances (0–1).
    pub keep_percentile: f64,
    pub knn_method: KnnMethod,
    pub metric: DistanceMetric,
}

impl Default for ScatterCleanConfig {
    fn default() -> Self {
        Self {
            keep_percentile: 0.95,
            knn_method: KnnMethod::default(),
            metric: DistanceMetric::Euclidean,
        }
    }
}

/// Drop fluorescence outliers in PCA space (intrusive / debris-like events).
#[derive(Debug, Clone)]
pub struct PcaCleanConfig {
    pub n_components: usize,
    /// Keep events whose PC-space radius is at or below this percentile (0–1).
    pub keep_percentile: f64,
}

impl Default for PcaCleanConfig {
    fn default() -> Self {
        Self {
            n_components: 3,
            keep_percentile: 0.99,
        }
    }
}

/// Optional pre-discovery cleaning. Default is a no-op.
#[derive(Debug, Clone, Default)]
pub struct CleanConfig {
    pub scatter: Option<ScatterCleanConfig>,
    pub pca: Option<PcaCleanConfig>,
}

/// FlowSOM grid used when [`DiscoveryBackend::FlowSom`] is selected.
#[derive(Debug, Clone)]
pub struct SomDiscoverConfig {
    pub width: usize,
    pub height: usize,
    pub n_epochs: usize,
    pub radius: Option<f64>,
}

impl Default for SomDiscoverConfig {
    fn default() -> Self {
        Self {
            width: 6,
            height: 6,
            n_epochs: 12,
            radius: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoverConfig {
    pub backend: DiscoveryBackend,
    /// Inclusive range explored when choosing component count (GMM / k-means).
    pub k_min: usize,
    pub k_max: usize,
    /// Fixed `k` overrides the grid when `Some`.
    pub fixed_k: Option<usize>,
    pub max_iterations: usize,
    pub seed: Option<u64>,
    /// Merge library columns whose cosine similarity exceeds this threshold.
    pub merge_cosine: f64,
    pub clean: CleanConfig,
    pub som: SomDiscoverConfig,
    /// ANN backend for [`DiscoveryBackend::HnswMedoid`] (and scatter-clean).
    pub knn_method: KnnMethod,
    pub metric: DistanceMetric,
}

impl Default for DiscoverConfig {
    fn default() -> Self {
        Self {
            backend: DiscoveryBackend::Gmm,
            k_min: 2,
            k_max: 8,
            fixed_k: None,
            max_iterations: 100,
            seed: Some(42),
            merge_cosine: 0.995,
            clean: CleanConfig::default(),
            som: SomDiscoverConfig::default(),
            knn_method: KnnMethod::default(),
            metric: DistanceMetric::Cosine,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MatchConfig {
    pub strategy: MatchStrategy,
    pub metric: DistanceMetric,
    /// When library size exceeds this, ANN shortlists `ann_candidates` first.
    pub exhaustive_residual_max_k: usize,
    pub ann_candidates: usize,
    pub knn_method: KnnMethod,
    /// Parallel residual matching above this event count (unless force sequential).
    pub parallel_event_threshold: usize,
    /// Reuse per-AF mixing matrices / Gram factors across events (default true).
    /// Set `false` for Criterion A/B against rebuild-M-and-QR-per-candidate.
    pub reuse_af_factors: bool,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            strategy: MatchStrategy::ResidualOls,
            metric: DistanceMetric::Cosine,
            exhaustive_residual_max_k: 32,
            ann_candidates: 8,
            knn_method: KnnMethod::default(),
            parallel_event_threshold: 256,
            reuse_af_factors: true,
        }
    }
}

impl MatchConfig {
    pub fn with_hnsw_params(mut self, params: HnswParams) -> Self {
        #[cfg(feature = "hnsw")]
        {
            self.knn_method = KnnMethod::Hnsw(params);
        }
        #[cfg(not(feature = "hnsw"))]
        {
            let _ = params;
            self.knn_method = KnnMethod::Exact;
        }
        self
    }
}

/// Batch OLS unmix: factor once and optionally Rayon over events.
#[derive(Debug, Clone)]
pub struct OlsUnmixConfig {
    /// Parallel unmix above this event count (unless force sequential).
    pub parallel_event_threshold: usize,
    /// Factor \(M^\top M\) once (Gram Cholesky, QR fallback) then per-event solve.
    /// Set `false` for Criterion A/B against per-event QR.
    pub reuse_factor: bool,
}

impl Default for OlsUnmixConfig {
    fn default() -> Self {
        Self {
            parallel_event_threshold: 256,
            reuse_factor: true,
        }
    }
}

/// SOM + cosine QC for [`crate::discover_spectral_variants`].
///
/// Defaults match AutoSpectral `get.fluor.variants`: cap `n_cells = 10_000`,
/// square SOM `10 × 10`, cosine ≥ `0.985`, scatter `k.neighbors = 3`.
#[derive(Debug, Clone)]
pub struct VariantDiscoverConfig {
    /// Maximum positive events per fluorophore sent to the SOM.
    pub n_cells: usize,
    pub som_width: usize,
    pub som_height: usize,
    pub som_n_epochs: usize,
    pub som_radius: Option<f64>,
    /// Drop SOM nodes whose cosine to the master spectrum is below this.
    pub sim_threshold: f64,
    /// Scatter-space neighbours in the unstained pool for background subtraction.
    pub k_neighbors: usize,
    /// Quantile of unstained (raw peak detector and unmixed channel) for positivity.
    pub positivity_quantile: f64,
    /// Blend off-peak channels toward the master (`0.5` in AutoSpectral). `None` skips.
    pub off_peak_blend: Option<f64>,
    /// Master-spectrum entries above this are treated as on-peak for the blend.
    pub off_peak_master_min: f64,
    pub seed: Option<u64>,
    pub knn_method: KnnMethod,
    pub metric: DistanceMetric,
}

impl Default for VariantDiscoverConfig {
    fn default() -> Self {
        Self {
            n_cells: 10_000,
            som_width: 10,
            som_height: 10,
            som_n_epochs: 10,
            som_radius: None,
            sim_threshold: 0.985,
            k_neighbors: 3,
            positivity_quantile: 0.995,
            off_peak_blend: Some(0.5),
            off_peak_master_min: 0.05,
            seed: Some(42),
            knn_method: KnnMethod::default(),
            metric: DistanceMetric::Euclidean,
        }
    }
}

/// Numeric width for the joint-unmix GEMV / Cholesky path.
///
/// Default [`Self::F64`] matches R AutoSpectral / AutoSpectralRcpp (`double`).
/// [`Self::F32`] runs the same algorithm in `faer::Mat<f32>` (inputs still `&[f64]`;
/// abundances are promoted back to `f64`). Quality versus the `f64` path must be
/// checked before treating `F32` as a keep (`flow-crates-0ap.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JointUnmixPrecision {
    /// IEEE-754 binary64 throughout precompute and the per-event loop.
    #[default]
    F64,
    /// Cast panel + events to binary32 for arithmetic; return `f64` abundances.
    F32,
}

/// Joint per-cell AF + fluorophore-variant unmix (AutoSpectral v1.6 `pipeline = "joint"`).
///
/// Defaults follow R `unmix.fcs` (`n_passes = 1`), not the C++ signature default of 2.
#[derive(Debug, Clone)]
pub struct JointUnmixConfig {
    /// Coordinate-descent passes over fluorophore variants.
    pub n_passes: usize,
    /// AF matching-pursuit passes (first pass always runs; extras refine high-AF cells).
    pub n_af_passes: usize,
    /// Fraction of high-AF cells revisited when `n_af_passes > 1` (Hyndman–Fan type 7).
    pub refine_af_quantile: f64,
    /// Per-detector weights `1 / max(mean, noise_floor)` (and per-cell `y_hat` weights).
    pub cell_weight: bool,
    /// Scalar dark-channel floor when [`Self::noise_floor_per_detector`] is `None`.
    pub noise_floor: f64,
    /// Optional length-`D` floor; a one-element vector is treated as a scalar.
    pub noise_floor_per_detector: Option<Vec<f64>>,
    /// Score = `resid_ratio^α × leakage_ratio^(1−α)`.
    pub alpha: f64,
    /// Cosine of unmixing rows `P_i`, `P_j` above which a pair is collinear.
    pub collinear_threshold: f64,
    /// Combinatorial retry for collinear pairs after the partner commits.
    pub joint_pair_resolution: bool,
    /// Parallel event loop above this count (unless [`force_sequential`]).
    pub parallel_event_threshold: usize,
    /// `f64` (default, vs-R) or internal `f32` faer (`flow-crates-0ap.1`).
    pub precision: JointUnmixPrecision,
}

impl Default for JointUnmixConfig {
    fn default() -> Self {
        Self {
            n_passes: 1,
            n_af_passes: 1,
            refine_af_quantile: 0.5,
            cell_weight: false,
            noise_floor: 125.0,
            noise_floor_per_detector: None,
            alpha: 0.5,
            collinear_threshold: 0.5,
            joint_pair_resolution: true,
            parallel_event_threshold: 256,
            precision: JointUnmixPrecision::F64,
        }
    }
}

/// Returns true when `FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL` disables Rayon.
pub fn force_sequential() -> bool {
    match std::env::var("FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL") {
        Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    }
}

/// Hyndman–Fan type-7 quantile (R `quantile(..., type = 7)` / AutoSpectral C++).
pub(crate) fn quantile_type7(values: &[f64], p: f64) -> f64 {
    let n = values.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return values[0];
    }
    let p = p.clamp(0.0, 1.0);
    let mut x = values.to_vec();
    x.sort_by(|a, b| a.total_cmp(b));
    let h = (n - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    x[lo] + (h - lo as f64) * (x[hi] - x[lo])
}
