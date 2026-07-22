//! faer-based linear algebra primitives for flow cytometry.

#[cfg(feature = "compensation")]
pub mod compensation;

#[cfg(feature = "compensation")]
pub use compensation::{
    SingleStainControl, apply_compensation_inv, compensate_channels, estimate_spillover,
    invert_spillover, median,
};
