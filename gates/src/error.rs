//! Error types for gate operations.
//!
//! This module defines `GateError`, a comprehensive error type for all gate-related
//! operations. It uses `thiserror` for convenient error construction and implements
//! standard error traits for integration with error handling libraries.

use std::error::Error as StdError;
use thiserror::Error;

/// Custom error type for gate operations.
///
/// All gate operations return `Result<T, GateError>`. The error type provides
/// detailed context about what went wrong, making debugging easier.
#[derive(Debug, Error)]
pub enum GateError {
    /// Geometry validation failures
    #[error("Invalid geometry: {message}")]
    InvalidGeometry { message: String },

    /// Missing required parameter/channel
    #[error("Missing parameter '{parameter}' in context: {context}")]
    MissingParameter { parameter: String, context: String },

    /// Invalid coordinate values
    #[error("Invalid coordinate '{coordinate}': value {value} is not finite or out of range")]
    InvalidCoordinate { coordinate: String, value: f32 },

    /// Event filtering failures
    #[error("Filtering error: {message}")]
    FilteringError { message: String },

    /// Hierarchy operation failures
    #[error("Hierarchy error: {message}")]
    HierarchyError { message: String },

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// EventIndex build/query errors
    #[error("Index error: {message}")]
    IndexError { message: String },

    /// Generic error with context (for wrapping other errors)
    #[error("{message}")]
    Other {
        message: String,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
}

impl GateError {
    /// Create an InvalidGeometry error with a message
    pub fn invalid_geometry(message: impl Into<String>) -> Self {
        Self::InvalidGeometry {
            message: message.into(),
        }
    }

    /// Create a MissingParameter error
    pub fn missing_parameter(parameter: impl Into<String>, context: impl Into<String>) -> Self {
        Self::MissingParameter {
            parameter: parameter.into(),
            context: context.into(),
        }
    }

    /// Create an InvalidCoordinate error
    pub fn invalid_coordinate(coordinate: impl Into<String>, value: f32) -> Self {
        Self::InvalidCoordinate {
            coordinate: coordinate.into(),
            value,
        }
    }

    /// Create a FilteringError with a message
    pub fn filtering_error(message: impl Into<String>) -> Self {
        Self::FilteringError {
            message: message.into(),
        }
    }

    /// Create a HierarchyError with a message
    pub fn hierarchy_error(message: impl Into<String>) -> Self {
        Self::HierarchyError {
            message: message.into(),
        }
    }

    /// Create an IndexError with a message
    pub fn index_error(message: impl Into<String>) -> Self {
        Self::IndexError {
            message: message.into(),
        }
    }

    /// Add context to an error
    pub fn with_context(self, context: impl Into<String>) -> Self {
        match self {
            Self::InvalidGeometry { message } => Self::InvalidGeometry {
                message: format!("{}: {}", context.into(), message),
            },
            Self::MissingParameter {
                parameter,
                context: ctx,
            } => Self::MissingParameter {
                parameter,
                context: format!("{}: {}", context.into(), ctx),
            },
            Self::InvalidCoordinate { coordinate, value } => {
                Self::InvalidCoordinate { coordinate, value }
            }
            Self::FilteringError { message } => Self::FilteringError {
                message: format!("{}: {}", context.into(), message),
            },
            Self::HierarchyError { message } => Self::HierarchyError {
                message: format!("{}: {}", context.into(), message),
            },
            Self::SerializationError(e) => Self::Other {
                message: format!("{}: {}", context.into(), e),
                source: Some(Box::new(e)),
            },
            Self::IndexError { message } => Self::IndexError {
                message: format!("{}: {}", context.into(), message),
            },
            Self::Other { message, source } => Self::Other {
                message: format!("{}: {}", context.into(), message),
                source,
            },
        }
    }
}

// Conversion from anyhow::Error for convenience
impl From<anyhow::Error> for GateError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other {
            message: err.to_string(),
            source: None, // anyhow::Error already contains the full context
        }
    }
}

// Type alias for Result using GateError
pub type Result<T> = std::result::Result<T, GateError>;
