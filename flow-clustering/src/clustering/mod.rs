//! Clustering algorithms module
//!
//! Provides K-means, DBSCAN, GMM, SOM / FlowSOM, silhouette scoring,
//! optional PARC (feature `parc`), e-means (feature `emeans`, default),
//! and agglomerative linkage (feature `agglomerative`).

#[cfg(feature = "agglomerative")]
pub mod agglomerative;
mod dbscan;
#[cfg(feature = "emeans")]
pub mod emeans;
mod gmm;
mod kmeans;
#[cfg(feature = "parc")]
pub mod parc;
pub mod silhouette;
mod som;

#[cfg(feature = "agglomerative")]
pub use agglomerative::{
    Agglomerative, AgglomerativeConfig, AgglomerativeResult, Dendrogram, DendrogramMerge,
    LinkageMethod, MetaclusterResult, Metric, metacluster,
};
pub use dbscan::{Dbscan, DbscanConfig, DbscanResult};
#[cfg(feature = "emeans")]
pub use emeans::{
    AssignBackend, EMeans, EMeansConfig, EMeansProgress, EMeansResult, EMeansStage, Finalization,
    InitMethod, Lloyd, LloydConfig, LloydResult, REVIVAL_DELTA_BF16, prefer_emeans,
};
pub use gmm::{Gmm, GmmConfig, GmmResult};
pub use kmeans::{KMeans, KMeansConfig, KMeansResult};
#[cfg(feature = "parc")]
pub use parc::{
    JacStdGlobal, KeepLocalDist, Parc, ParcConfig, ParcDistance, ParcPartition, ParcProgress,
    ParcResult, ParcStage,
};
pub use silhouette::{SilhouetteResult, silhouette_scores, silhouette_scores_sampled};
pub use som::{FlowSom, FlowSomConfig, FlowSomResult, Som, SomConfig, SomResult};

use thiserror::Error;

/// Error type for clustering operations
#[derive(Error, Debug)]
pub enum ClusteringError {
    #[error("Empty data")]
    EmptyData,
    #[error("Insufficient data: need at least {min} points, got {actual}")]
    InsufficientData { min: usize, actual: usize },
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Parameter validation failed: {0}")]
    ValidationFailed(String),
    #[error("Clustering failed: {0}")]
    ClusteringFailed(String),
    /// Cooperative cancellation via an optional `Arc<AtomicBool>` (PARC / e-means).
    #[error("Run cancelled by caller")]
    Cancelled,
    /// GPU backend requested but no usable WGPU adapter / cubeCL context.
    #[error("GPU unavailable: {0}")]
    GpuUnavailable(String),
}

pub type ClusteringResult<T> = Result<T, ClusteringError>;
