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

    /// Hierarchy cycle detection
    #[error(
        "Hierarchy cycle detected: adding '{gate_id}' as parent of '{would_create_cycle_to}' would create a cycle"
    )]
    HierarchyCycle {
        gate_id: String,
        would_create_cycle_to: String,
    },

    /// Invalid boolean operation configuration
    #[error(
        "Invalid boolean operation '{operation}': expected {expected_count} operand(s), got {operand_count}"
    )]
    InvalidBooleanOperation {
        operation: String,
        operand_count: usize,
        expected_count: usize,
    },

    /// Referenced gate not found
    #[error("Gate '{gate_id}' not found: {context}")]
    GateNotFound { gate_id: String, context: String },

    /// Invalid gate link operation
    #[error("Invalid link from '{linking_gate_id}' to '{target_gate_id}': {reason}")]
    InvalidLink {
        target_gate_id: String,
        linking_gate_id: String,
        reason: String,
    },

    /// Cannot reparent gate
    #[error("Cannot reparent gate '{gate_id}' to '{new_parent_id}': {reason}")]
    CannotReparent {
        gate_id: String,
        new_parent_id: String,
        reason: String,
    },

    /// Invalid subtree operation
    #[error("Invalid subtree operation '{operation}' on gate '{gate_id}': {reason}")]
    InvalidSubtreeOperation {
        gate_id: String,
        operation: String,
        reason: String,
    },

    /// Boolean operation with no operands
    #[error("Boolean operation '{operation}' requires at least one operand")]
    EmptyOperands { operation: String },

    /// Builder in invalid state
    #[error("Builder field '{field}' is invalid: {reason}")]
    InvalidBuilderState { field: String, reason: String },

    /// Duplicate gate ID
    #[error("Gate ID '{gate_id}' already exists")]
    DuplicateGateId { gate_id: String },

    /// Gate coordinate space doesn't match the event data's space.
    ///
    /// Filters compare gate node coordinates against event values; a mismatch
    /// means the gate would silently produce wrong results. Callers must fetch
    /// event data in the gate's declared coordinate space (raw, compensated, or
    /// unmixed) before calling `filter_events_by_gate`.
    #[error(
        "Coordinate space mismatch: gate is in {gate_space} but event data is in {data_space}"
    )]
    SpaceMismatch {
        gate_space: String,
        data_space: String,
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

    /// Create a HierarchyCycle error
    pub fn hierarchy_cycle(
        gate_id: impl Into<String>,
        would_create_cycle_to: impl Into<String>,
    ) -> Self {
        Self::HierarchyCycle {
            gate_id: gate_id.into(),
            would_create_cycle_to: would_create_cycle_to.into(),
        }
    }

    /// Create an InvalidBooleanOperation error
    pub fn invalid_boolean_operation(
        operation: impl Into<String>,
        operand_count: usize,
        expected_count: usize,
    ) -> Self {
        Self::InvalidBooleanOperation {
            operation: operation.into(),
            operand_count,
            expected_count,
        }
    }

    /// Create a GateNotFound error
    pub fn gate_not_found(gate_id: impl Into<String>, context: impl Into<String>) -> Self {
        Self::GateNotFound {
            gate_id: gate_id.into(),
            context: context.into(),
        }
    }

    /// Create an InvalidLink error
    pub fn invalid_link(
        target_gate_id: impl Into<String>,
        linking_gate_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidLink {
            target_gate_id: target_gate_id.into(),
            linking_gate_id: linking_gate_id.into(),
            reason: reason.into(),
        }
    }

    /// Create a CannotReparent error
    pub fn cannot_reparent(
        gate_id: impl Into<String>,
        new_parent_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::CannotReparent {
            gate_id: gate_id.into(),
            new_parent_id: new_parent_id.into(),
            reason: reason.into(),
        }
    }

    /// Create an InvalidSubtreeOperation error
    pub fn invalid_subtree_operation(
        gate_id: impl Into<String>,
        operation: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidSubtreeOperation {
            gate_id: gate_id.into(),
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Create an EmptyOperands error
    pub fn empty_operands(operation: impl Into<String>) -> Self {
        Self::EmptyOperands {
            operation: operation.into(),
        }
    }

    /// Create an InvalidBuilderState error
    pub fn invalid_builder_state(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidBuilderState {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Create a DuplicateGateId error
    pub fn duplicate_gate_id(gate_id: impl Into<String>) -> Self {
        Self::DuplicateGateId {
            gate_id: gate_id.into(),
        }
    }

    /// Create a SpaceMismatch error from any types that can be displayed.
    pub fn space_mismatch(
        gate_space: impl std::fmt::Debug,
        data_space: impl std::fmt::Debug,
    ) -> Self {
        Self::SpaceMismatch {
            gate_space: format!("{:?}", gate_space),
            data_space: format!("{:?}", data_space),
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
            Self::HierarchyCycle {
                gate_id,
                would_create_cycle_to,
            } => Self::HierarchyCycle {
                gate_id,
                would_create_cycle_to,
            },
            Self::InvalidBooleanOperation {
                operation,
                operand_count,
                expected_count,
            } => Self::InvalidBooleanOperation {
                operation,
                operand_count,
                expected_count,
            },
            Self::GateNotFound {
                gate_id,
                context: ctx,
            } => Self::GateNotFound {
                gate_id,
                context: format!("{}: {}", context.into(), ctx),
            },
            Self::InvalidLink {
                target_gate_id,
                linking_gate_id,
                reason,
            } => Self::InvalidLink {
                target_gate_id,
                linking_gate_id,
                reason: format!("{}: {}", context.into(), reason),
            },
            Self::CannotReparent {
                gate_id,
                new_parent_id,
                reason,
            } => Self::CannotReparent {
                gate_id,
                new_parent_id,
                reason: format!("{}: {}", context.into(), reason),
            },
            Self::InvalidSubtreeOperation {
                gate_id,
                operation,
                reason,
            } => Self::InvalidSubtreeOperation {
                gate_id,
                operation,
                reason: format!("{}: {}", context.into(), reason),
            },
            Self::EmptyOperands { operation } => Self::EmptyOperands { operation },
            Self::InvalidBuilderState { field, reason } => Self::InvalidBuilderState {
                field,
                reason: format!("{}: {}", context.into(), reason),
            },
            Self::DuplicateGateId { gate_id } => Self::DuplicateGateId { gate_id },
            Self::SpaceMismatch {
                gate_space,
                data_space,
            } => Self::SpaceMismatch {
                gate_space,
                data_space,
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

// Conversion from quick_xml errors for GatingML parsing
impl From<quick_xml::Error> for GateError {
    fn from(err: quick_xml::Error) -> Self {
        Self::Other {
            message: format!("XML parsing error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

// Conversion from std::io::Error for GatingML writing
impl From<std::io::Error> for GateError {
    fn from(err: std::io::Error) -> Self {
        Self::Other {
            message: format!("IO error: {}", err),
            source: Some(Box::new(err)),
        }
    }
}

// Type alias for Result using GateError
pub type Result<T> = std::result::Result<T, GateError>;
