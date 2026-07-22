//! Configuration types for a PaCMAP embedding run.

/// Initialisation strategy for the 2-D embedding.
#[derive(Debug, Clone)]
pub enum Init {
    /// PCA projection onto the top-2 principal components (default).
    /// Faster convergence; more stable global structure.
    Pca,
    /// Draw from N(0, 10⁻⁴ · I) with an optional seed.
    Random(Option<u64>),
}

/// Approximate nearest-neighbour method used during graph construction.
#[derive(Debug, Clone)]
pub enum KnnMethod {
    /// HNSW via usearch v2.25 (C++ FFI, hardware SIMD) — **default**.
    /// Sub-linear query time; ~40–80 bytes/node overhead; optional f16 quantization.
    #[cfg(feature = "hnsw")]
    Hnsw(HnswParams),

    /// Exact brute-force O(n·(n+50)·d).
    /// Correctness baseline; practical only for n < ~50 K.
    Exact,

    /// k-d tree via `kiddo` v5 (pure Rust, exact).
    /// Best for d < 10, n < 1M; degrades for high-dimensional flow data.
    #[cfg(feature = "kdtree")]
    KdTree,

    /// Annoy — reserved placeholder; returns `PaCMAPError::MethodNotImplemented`.
    Annoy,
}

impl Default for KnnMethod {
    fn default() -> Self {
        #[cfg(feature = "hnsw")]
        return Self::Hnsw(HnswParams::default());
        #[cfg(not(feature = "hnsw"))]
        return Self::Exact;
    }
}

/// Quality/memory trade-off parameters for the HNSW index.
#[derive(Debug, Clone)]
pub struct HnswParams {
    /// Graph connectivity (M). Higher = better recall, more memory.
    /// Default 16; range 8–64.
    pub m: usize,
    /// Build-time candidate set size. Higher = better index quality, slower build.
    /// Default 200.
    pub ef_construction: usize,
    /// Query-time candidate set size. Higher = better recall, slower query.
    /// Default 50.
    pub ef_search: usize,
    /// Index vector quantization. `F16` halves memory at ~1% recall cost.
    pub quantization: Quantization,
}

impl Default for HnswParams {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            quantization: Quantization::F32,
        }
    }
}

/// Vector quantization for the HNSW index storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quantization {
    /// 32-bit float — lossless, full precision (default).
    #[default]
    F32,
    /// 16-bit float — ~50% memory reduction, ~1% recall loss.
    F16,
    /// 8-bit integer — ~75% memory reduction, ~3–5% recall loss.
    I8,
}

/// Distance metric for the KNN graph and sigma normalisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistanceMetric {
    /// Euclidean (L2) — default; sigma normalisation is in L2 space.
    #[default]
    Euclidean,
    /// Squared Euclidean (L2²) — avoids sqrt; identical ranking to L2.
    EuclideanSq,
    /// Cosine similarity distance — for normalised or spectral data.
    Cosine,
    /// Manhattan (L1).
    Manhattan,
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
    /// Random seed for reproducible runs. Applies to Init::Random, mid-near/further sampling.
    pub seed: Option<u64>,
    /// KNN method. Default `KnnMethod::Hnsw(HnswParams::default())`.
    pub knn_method: KnnMethod,
    /// Distance metric. Default `DistanceMetric::Euclidean`.
    pub distance_metric: DistanceMetric,
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
