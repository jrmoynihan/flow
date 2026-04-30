use thiserror::Error;

/// Errors that can occur during TRU-OLS unmixing
#[derive(Error, Debug)]
pub enum TruOlsError {
    #[error("Matrix dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Invalid mixing matrix: {0}")]
    InvalidMixingMatrix(String),

    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    #[error("Linear algebra error: {0}")]
    LinearAlgebra(String),

    #[error("Invalid percentile: {0}. Must be between 0.0 and 1.0")]
    InvalidPercentile(f64),

    #[error("No autofluorescence endmember found")]
    NoAutofluorescenceEndmember,

    #[error("All endmembers removed for event {event_index}")]
    AllEndmembersRemoved { event_index: usize },

    /// More than [`crate::MAX_ENDMEMBERS_DEFAULT`] endmembers without the `large-panels` feature.
    #[error(
        "Endmember count {actual} exceeds the default limit of {max}; enable the `large-panels` Cargo feature on `flow-tru-ols` for larger panels"
    )]
    EndmemberCountExceedsDefaultLimit { max: usize, actual: usize },
}
