//! Configuration for k-NN search.

/// Approximate / exact nearest-neighbour method.
#[derive(Debug, Clone)]
pub enum KnnMethod {
    /// HNSW via usearch (C++ FFI, hardware SIMD) — default when the `hnsw` feature is on.
    #[cfg(feature = "hnsw")]
    Hnsw(HnswParams),

    /// Exact brute-force O(n · k · d) with a bounded heap per query.
    Exact,

    /// k-d tree via `kiddo` (currently falls back to exact).
    #[cfg(feature = "kdtree")]
    KdTree,

    /// HNSW via `ann-search-rs` (optional feature `ann-search`).
    #[cfg(feature = "ann-search")]
    AnnSearchHnsw(HnswParams),

    /// Exact exhaustive kNN on GPU (ann-search-rs + cubeCL / wgpu). Feature `gpu`.
    #[cfg(feature = "gpu")]
    GpuExact,

    /// IVF approximate kNN on GPU. Feature `gpu`.
    #[cfg(feature = "gpu")]
    GpuIvf(IvfGpuParams),

    /// NN-Descent / CAGRA-style approximate kNN on GPU. Feature `gpu`.
    #[cfg(feature = "gpu")]
    GpuNnDescent(NnDescentGpuParams),

    /// Reserved placeholder.
    Annoy,
}

/// IVF-GPU list / probe parameters (`None` → ann-search-rs defaults √n / √nlist).
#[derive(Debug, Clone, Default)]
#[cfg(feature = "gpu")]
pub struct IvfGpuParams {
    pub n_list: Option<usize>,
    pub n_probes: Option<usize>,
}

/// NN-Descent GPU graph parameters (`None` → library defaults).
#[derive(Debug, Clone)]
#[cfg(feature = "gpu")]
pub struct NnDescentGpuParams {
    /// Final graph degree after pruning (default: max(k, 30) at call site).
    pub k: Option<usize>,
    pub k_build: Option<usize>,
    pub n_trees: Option<usize>,
    pub delta: f32,
    pub rho: Option<f32>,
}

#[cfg(feature = "gpu")]
impl Default for NnDescentGpuParams {
    fn default() -> Self {
        Self {
            k: None,
            k_build: None,
            n_trees: None,
            delta: 0.001,
            rho: None,
        }
    }
}

impl Default for KnnMethod {
    fn default() -> Self {
        // Prefer ann-search-rs when that feature is enabled (matches manifolds-rs HNSW).
        #[cfg(feature = "ann-search")]
        return Self::AnnSearchHnsw(HnswParams::default());
        #[cfg(all(not(feature = "ann-search"), feature = "hnsw"))]
        return Self::Hnsw(HnswParams::default());
        #[cfg(all(not(feature = "ann-search"), not(feature = "hnsw")))]
        return Self::Exact;
    }
}

/// Quality / memory trade-off for HNSW indices (usearch or ann-search-rs).
#[derive(Debug, Clone)]
pub struct HnswParams {
    /// Graph connectivity (M). Default 16.
    pub m: usize,
    /// Build-time candidate set size. Default 200.
    pub ef_construction: usize,
    /// Query-time candidate set size. Default 50.
    pub ef_search: usize,
    /// Storage quantization (usearch only; ignored by ann-search-rs).
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

/// Vector quantization for usearch HNSW storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quantization {
    #[default]
    F32,
    F16,
    I8,
}

/// Distance metric for the KNN graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistanceMetric {
    #[default]
    Euclidean,
    EuclideanSq,
    Cosine,
    Manhattan,
}
