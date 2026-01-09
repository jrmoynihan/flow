use crate::colormap::ColorMaps;
use crate::options::{AxisOptions, BasePlotOptions, PlotOptions};
use derive_builder::Builder;

/// Options for density plots
///
/// Configuration for creating density plots, including base layout options,
/// axis configurations, and color map selection.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::options::DensityPlotOptions;
/// use flow_plots::colormap::ColorMaps;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let options = DensityPlotOptions::new()
///     .width(800)
///     .height(600)
///     .colormap(ColorMaps::Viridis)
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// @deprecated The old PlotOptions struct has been removed. Use DensityPlotOptions with builder pattern instead.
#[derive(Builder, Clone, Debug)]
#[builder(setter(into, strip_option), default)]
pub struct DensityPlotOptions {
    /// Base plot options (layout, dimensions, etc.)
    #[builder(default)]
    pub base: BasePlotOptions,

    /// X-axis configuration
    #[builder(default)]
    pub x_axis: AxisOptions,

    /// Y-axis configuration
    #[builder(default)]
    pub y_axis: AxisOptions,

    /// Color map to use for density visualization
    #[builder(default = "ColorMaps::Viridis")]
    pub colormap: ColorMaps,
}

impl Default for DensityPlotOptions {
    fn default() -> Self {
        Self {
            base: BasePlotOptions::default(),
            x_axis: AxisOptions::default(),
            y_axis: AxisOptions::default(),
            colormap: ColorMaps::Viridis,
        }
    }
}

impl PlotOptions for DensityPlotOptions {
    fn base(&self) -> &BasePlotOptions {
        &self.base
    }
}

impl DensityPlotOptions {
    /// Create a new builder for DensityPlotOptions
    pub fn new() -> DensityPlotOptionsBuilder {
        DensityPlotOptionsBuilder::default()
    }
}
