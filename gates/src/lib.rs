//! # flow-gates
//!
//! A comprehensive Rust library for working with gates in flow cytometry data analysis.
//!
//! This library provides tools for creating, managing, and applying gates to flow cytometry
//! data, supporting the GatingML 2.0 standard for gate definitions and hierarchies.
//!
//! ## Overview
//!
//! Flow cytometry gates define regions of interest in multi-dimensional data space,
//! allowing researchers to identify and analyze specific cell populations. This library
//! provides:
//!
//! - **Gate Types**: Polygon, Rectangle, and Ellipse geometries
//! - **Gate Hierarchies**: Parent-child relationships for sequential gating
//! - **Event Filtering**: Efficient spatial indexing and filtering of cytometry events
//! - **Statistics**: Comprehensive statistics for gated populations
//! - **GatingML Support**: Import/export gates in GatingML 2.0 XML format
//! - **Thread-Safe Storage**: Concurrent gate management
//!
//! ## Quick Start
//!
//! ```rust
//! use flow_gates::*;
//! use flow_gates::geometry::*;
//!
//! // Create a polygon gate
//! let coords = vec![
//!     (100.0, 200.0),
//!     (300.0, 200.0),
//!     (300.0, 400.0),
//!     (100.0, 400.0),
//! ];
//! let geometry = create_polygon_geometry(coords, "FSC-A", "SSC-A")?;
//!
//! let gate = Gate::new(
//!     "my-gate",
//!     "My Gate",
//!     geometry,
//!     "FSC-A",
//!     "SSC-A",
//! );
//!
//! // Apply gate to FCS data
//! let event_indices = filter_events_by_gate(&fcs_file, &gate, None)?;
//! ```
//!
//! ## Core Concepts
//!
//! ### Gates
//!
//! A [`Gate`] represents a region of interest defined by:
//! - **Geometry**: The shape (polygon, rectangle, or ellipse)
//! - **Parameters**: The two channels (x and y) the gate operates on
//! - **Mode**: Whether the gate applies globally or to specific files
//!
//! ### Gate Hierarchies
//!
//! Gates can be organized in parent-child relationships, where child gates
//! are applied to events that pass through their parent gates. Use [`GateHierarchy`]
//! to manage these relationships.
//!
//! ### Event Filtering
//!
//! The library provides efficient event filtering using spatial indexing (R*-tree)
//! for fast point-in-gate queries. Use [`EventIndex`] for repeated filtering operations
//! on the same dataset.
//!
//! ## Examples
//!
//! See the [README](https://github.com/jrmoynihan/flow-gates) for detailed examples
//! including:
//! - Creating different gate types
//! - Building gate hierarchies
//! - Filtering events
//! - Calculating statistics
//! - Application integration patterns
//!
//! ## Error Handling
//!
//! The library uses [`GateError`] for all error conditions. Most operations return
//! [`Result<T, GateError>`](GateResult).

pub mod ellipse;
pub mod error;
pub mod filtering;
pub mod gatingml;
pub mod geometry;
pub mod hierarchy;
pub mod polygon;
pub mod rectangle;
pub mod scope;
pub mod statistics;
pub mod traits;
pub mod traits_tests;
pub mod transforms;
pub mod types;

#[cfg(test)]
mod error_tests;

/// Error types for gate operations
pub use error::{GateError, Result as GateResult};

/// Event filtering and spatial indexing
pub use filtering::{
    EventIndex, FilterCache, FilterCacheKey, filter_events_by_gate, filter_events_by_hierarchy,
};

/// Geometry construction helpers
pub use geometry::{create_ellipse_geometry, create_polygon_geometry, create_rectangle_geometry};

/// Gate hierarchy management
pub use hierarchy::GateHierarchy;

/// Statistics for gated populations
pub use statistics::GateStatistics;

/// Core gate types and structures
pub use types::{Gate, GateGeometry, GateMode, GateNode};

// Note: FilterCache and GateStorage are application-specific and should be
// implemented in the application crate, not in this library crate.
