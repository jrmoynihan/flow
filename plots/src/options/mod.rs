pub mod axis;
pub mod base;
pub mod density;

pub use axis::{AxisOptions, AxisOptionsBuilder};
pub use base::{BasePlotOptions, BasePlotOptionsBuilder};
pub use density::{DensityPlotOptions, DensityPlotOptionsBuilder};

/// Trait for plot options types
///
/// All plot-specific options structs should implement this trait to provide
/// access to the base options.
pub trait PlotOptions {
    /// Get a reference to the base plot options
    fn base(&self) -> &BasePlotOptions;
}
