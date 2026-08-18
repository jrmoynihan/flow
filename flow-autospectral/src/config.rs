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

/// Returns true when `FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL` disables Rayon.
pub fn force_sequential() -> bool {
    match std::env::var("FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL") {
        Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    }
}
