//! Clustering algorithms for flow cytometry.
pub mod clustering;

pub use clustering::{
    ClusteringError, ClusteringResult, Dbscan, DbscanConfig, DbscanResult, FlowSom, FlowSomConfig,
    FlowSomResult, Gmm, GmmConfig, GmmResult, KMeans, KMeansConfig, KMeansResult, SilhouetteResult,
    Som, SomConfig, SomResult, silhouette_scores, silhouette_scores_sampled,
};

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
