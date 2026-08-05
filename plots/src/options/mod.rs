pub mod axis;
pub mod base;
pub mod density;
pub mod histogram;
pub mod spectral;

pub use axis::{AxisOptions, AxisOptionsBuilder};
pub use base::{BasePlotOptions, BasePlotOptionsBuilder};
pub(crate) use base::impl_base_options_passthrough;
pub use density::{DensityPlotOptions, DensityPlotOptionsBuilder};
pub use histogram::{HistogramPlotOptions, HistogramPlotOptionsBuilder};
pub use spectral::{SpectralSignaturePlotOptions, SpectralSignaturePlotOptionsBuilder};

/// Trait for plot options types
///
/// All plot-specific options structs should implement this trait to provide
/// access to the base options.
pub trait PlotOptions {
    /// Get a reference to the base plot options
    fn base(&self) -> &BasePlotOptions;
}
