//! Geometry construction utilities for creating gate geometries from raw coordinates.
//!
//! This module provides helper functions to create `GateGeometry` variants from
//! simple coordinate tuples, making it easier to construct gates programmatically
//! or from user input.

pub mod construction;

pub use construction::{
    create_ellipse_geometry, create_polygon_geometry, create_rectangle_geometry,
};

