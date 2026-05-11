//! faer-based linear algebra primitives for flow cytometry.

#[cfg(feature = "compensation")]
pub mod compensation;

#[cfg(feature = "compensation")]
pub use compensation::{apply_compensation_inv, compensate_channels, invert_spillover};
