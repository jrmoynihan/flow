//! Configuration types for a PaCMAP embedding run.

pub use flow_knn::{DistanceMetric, HnswParams, KnnMethod, Quantization};

/// Initialisation strategy for the 2-D embedding.
#[derive(Debug, Clone)]
pub enum Init {
    /// PCA projection onto the top-2 principal components (default).
    Pca,
    /// Draw from N(0, 10⁻⁴ · I) with an optional seed.
    Random(Option<u64>),
}

/// Backend for the Adam + pair-gradient optimize loop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OptimizeBackend {
    /// Rayon CPU path (default).
    #[default]
    Cpu,
    /// cubeCL CSR gradients + Burn Adam via wgpu (requires the `cubecl` feature and a WGPU adapter).
    #[cfg(feature = "cubecl")]
    Gpu,
}

/// Full configuration for a `fit_transform` call.
#[derive(Debug, Clone)]
pub struct PaCMAPConfig {
    /// Number of near-neighbour pairs per point. Default 10.
    pub n_neighbors: usize,
    /// Ratio nMN / n_neighbors. Default 0.5.
    pub mn_ratio: f32,
    /// Ratio nFP / n_neighbors. Default 2.0.
    pub fp_ratio: f32,
    /// Iteration counts for phases [1, 2, 3]. Sum = total iterations. Default [100, 100, 250].
    pub phase_iters: [usize; 3],
    /// Adam learning rate. Default 1.0.
    pub learning_rate: f32,
    /// Embedding initialisation. Default `Init::Pca`.
    pub init: Init,
    /// Random seed for reproducible runs.
    pub seed: Option<u64>,
    /// KNN method. Default HNSW when the `hnsw` feature is enabled.
    pub knn_method: KnnMethod,
    /// Distance metric. Default Euclidean.
    pub distance_metric: DistanceMetric,
    /// Optimize backend. Default [`OptimizeBackend::Cpu`].
    pub optimize_backend: OptimizeBackend,
}

impl Default for PaCMAPConfig {
    fn default() -> Self {
        Self {
            n_neighbors: 10,
            mn_ratio: 0.5,
            fp_ratio: 2.0,
            phase_iters: [100, 100, 250],
            learning_rate: 1.0,
            init: Init::Pca,
            seed: None,
            knn_method: KnnMethod::default(),
            distance_metric: DistanceMetric::default(),
            optimize_backend: OptimizeBackend::default(),
        }
    }
}

impl PaCMAPConfig {
    /// Derived: number of mid-near pairs per point = floor(n_neighbors * mn_ratio).
    pub fn n_mn(&self) -> usize {
        (self.n_neighbors as f32 * self.mn_ratio).floor() as usize
    }
    /// Derived: number of further pairs per point = floor(n_neighbors * fp_ratio).
    pub fn n_fp(&self) -> usize {
        (self.n_neighbors as f32 * self.fp_ratio).floor() as usize
    }
    /// Total optimization iterations across all phases.
    pub fn total_iters(&self) -> usize {
        self.phase_iters.iter().sum()
    }
}
