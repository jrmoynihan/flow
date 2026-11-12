//! Clustering algorithms for flow cytometry.
pub mod clustering;

pub use clustering::{
    ClusteringError, ClusteringResult, Dbscan, DbscanConfig, DbscanResult, Gmm, GmmConfig,
    GmmResult, SilhouetteResult, silhouette_scores, silhouette_scores_sampled,
};

#[cfg(feature = "kmeans")]
pub use clustering::{KMeans, KMeansConfig, KMeansResult};

#[cfg(feature = "som")]
pub use clustering::{FlowSom, FlowSomConfig, FlowSomResult, Som, SomConfig, SomResult};

#[cfg(feature = "agglomerative")]
pub use clustering::{
    Agglomerative, AgglomerativeConfig, AgglomerativeResult, Dendrogram, DendrogramMerge,
    LinkageMethod, MetaclusterResult, Metric, metacluster,
};

#[cfg(feature = "emeans")]
pub use clustering::{
    AssignBackend, EMeans, EMeansConfig, EMeansProgress, EMeansResult, EMeansStage, Finalization,
    InitMethod, Lloyd, LloydConfig, LloydResult, REVIVAL_DELTA_BF16, prefer_emeans,
};

#[cfg(feature = "parc")]
pub use clustering::{
    JacStdGlobal, KeepLocalDist, Parc, ParcConfig, ParcDistance, ParcPartition, ParcProgress,
    ParcResult, ParcStage,
};
