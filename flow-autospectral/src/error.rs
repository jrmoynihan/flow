//! Errors for AutoSpectral-style AF library discovery and matching.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutospectralError {
    #[error("empty event matrix: need at least one event")]
    EmptyEvents,

    #[error("dimension mismatch: expected {expected} detectors, got {got}")]
    DetectorMismatch { expected: usize, got: usize },

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("clustering failed: {0}")]
    Clustering(String),

    #[error("KNN / ANN index error: {0}")]
    Knn(String),

    #[error("linear algebra failure: {0}")]
    Linalg(String),

    #[error("AF library is empty")]
    EmptyLibrary,

    #[error("AF index {index} out of range for library of size {n}")]
    AfIndexOutOfRange { index: usize, n: usize },
}

pub type Result<T> = std::result::Result<T, AutospectralError>;
