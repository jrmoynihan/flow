use crate::PlotBytes;
use crate::options::PlotOptions;
use crate::render::RenderConfig;
use anyhow::Result;

/// Trait for plot types
///
/// This trait defines the interface that all plot types must implement.
/// Each plot type specifies its own options type and data type.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::plots::traits::Plot;
/// use flow_plots::options::{PlotOptions, BasePlotOptions};
/// use flow_plots::render::RenderConfig;
/// use flow_plots::PlotBytes;
/// use anyhow::Result;
///
/// struct MyPlotOptions {
///     base: BasePlotOptions,
///     // ... your options
/// }
///
/// impl PlotOptions for MyPlotOptions {
///     fn base(&self) -> &BasePlotOptions { &self.base }
/// }
///
/// struct MyPlot;
///
/// impl Plot for MyPlot {
///     type Options = MyPlotOptions;
///     type Data = Vec<(f32, f32)>;
///
///     fn render(
///         &self,
///         data: Self::Data,
///         options: &Self::Options,
///         render_config: &mut RenderConfig,
///     ) -> Result<PlotBytes> {
///         // ... your rendering logic
///         Ok(vec![])
///     }
/// }
/// ```
pub trait Plot {
    /// The options type for this plot
    type Options: PlotOptions;

    /// The data type this plot accepts
    type Data;

    /// Render the plot with the given data and options
    ///
    /// # Arguments
    ///
    /// * `data` - The data to plot
    /// * `options` - Plot-specific options
    /// * `render_config` - Rendering configuration (progress callbacks, etc.)
    ///
    /// # Returns
    ///
    /// JPEG-encoded plot image bytes
    fn render(
        &self,
        data: Self::Data,
        options: &Self::Options,
        render_config: &mut RenderConfig,
    ) -> Result<PlotBytes>;
}
