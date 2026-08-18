//! Clustering algorithms module
//!
//! Provides K-means, DBSCAN, Gaussian Mixture Model clustering, silhouette scoring,
//! and optional PARC (feature `parc`).

mod dbscan;
mod gmm;
mod kmeans;
#[cfg(feature = "parc")]
pub mod parc;
pub mod silhouette;

pub use dbscan::{Dbscan, DbscanConfig, DbscanResult};
pub use gmm::{Gmm, GmmConfig, GmmResult};
pub use kmeans::{KMeans, KMeansConfig, KMeansResult};
#[cfg(feature = "parc")]
pub use parc::{
    JacStdGlobal, KeepLocalDist, Parc, ParcConfig, ParcDistance, ParcPartition, ParcResult,
};
pub use silhouette::{SilhouetteResult, silhouette_scores, silhouette_scores_sampled};

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
}

pub type ClusteringResult<T> = Result<T, ClusteringError>;
