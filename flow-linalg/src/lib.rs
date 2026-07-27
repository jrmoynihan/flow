//! faer-based linear algebra primitives for flow cytometry.

pub mod condition;
pub mod hotspot;

#[cfg(feature = "compensation")]
pub mod compensation;

pub use condition::{ConditionMetrics, condition_metrics, condition_metrics_f32, condition_number_2};
pub use hotspot::{HotspotMatrix, hotspot_from_mixing_matrix, hotspot_from_similarity};

#[cfg(feature = "compensation")]
pub use compensation::{
    SingleStainControl, apply_compensation_inv, compensate_channels, estimate_spillover,
    invert_spillover, median,
};
