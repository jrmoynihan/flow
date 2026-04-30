//! Clustering algorithms module
//!
//! Provides K-means, DBSCAN, Gaussian Mixture Model clustering, and silhouette scoring.

mod dbscan;
mod gmm;
mod kmeans;
pub mod silhouette;

pub use dbscan::{Dbscan, DbscanConfig, DbscanResult};
pub use gmm::{Gmm, GmmConfig, GmmResult};
pub use kmeans::{KMeans, KMeansConfig, KMeansResult};
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
