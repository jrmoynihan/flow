use crate::contour::calculate_contours;
use crate::PlotBytes;
use crate::density_calc::calculate_plot_pixels;
use crate::options::{DensityPlotOptions, PlotOptions};
use crate::plots::traits::Plot;
use crate::plots::PlotType;
use crate::render::RenderConfig;
use crate::render::plotters_backend::{render_contour, render_pixels};
use crate::scatter_data::ScatterPlotData;
use anyhow::Result;

/// Density plot implementation
///
/// Creates a 2D density plot from (x, y) coordinate pairs by binning
/// data points into pixels and coloring by density.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::plots::density::DensityPlot;
/// use flow_plots::plots::traits::Plot;
/// use flow_plots::options::{DensityPlotOptions, BasePlotOptions};
/// use flow_plots::render::RenderConfig;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let plot = DensityPlot::new();
/// let options = DensityPlotOptions::new()
///     .base(BasePlotOptions::new().width(800u32).height(600u32).build()?)
///     .build()?;
/// let data: Vec<(f32, f32)> = vec![(100.0, 200.0), (150.0, 250.0)];
/// let mut render_config = RenderConfig::default();
/// let bytes = plot.render(data.into(), &options, &mut render_config)?;
/// # Ok(())
/// # }
/// ```
pub struct DensityPlot;

impl DensityPlot {
    /// Create a new DensityPlot instance
    pub fn new() -> Self {
        Self
    }

    /// Render multiple density plots in batch
    ///
    /// High-level convenience method that handles both density calculation
    /// and rendering. For apps that want to orchestrate rendering themselves,
    /// use `calculate_density_per_pixel_batch` instead.
    ///
    /// # Arguments
    /// * `requests` - Vector of (data, options) tuples
    /// * `render_config` - Rendering configuration (shared across all plots)
    ///
    /// # Returns
    /// Vector of plot bytes, one per request
    pub fn render_batch(
        &self,
        requests: &[(ScatterPlotData, DensityPlotOptions)],
        render_config: &mut RenderConfig,
    ) -> Result<Vec<PlotBytes>> {
        let mut results = Vec::with_capacity(requests.len());
        for (data, options) in requests.iter() {
            let bytes = match options.plot_type.canonical() {
                PlotType::Contour | PlotType::ContourOverlay => {
                    let xy = data.xy();
                    let x_range = (*options.x_axis.range.start(), *options.x_axis.range.end());
                    let y_range = (*options.y_axis.range.start(), *options.y_axis.range.end());
                    let contour_data = calculate_contours(
                        xy,
                        options.contour_level_count,
                        options.contour_smoothing,
                        options.draw_outliers,
                        x_range,
                        y_range,
                    )?;
                    render_contour(contour_data, options, render_config)?
                }
                _ => {
                    let base = options.base();
                    let raw_pixels = calculate_plot_pixels(
                        data,
                        base.width as usize,
                        base.height as usize,
                        options,
                    );
                    render_pixels(raw_pixels, options, render_config)?
                }
            };
            results.push(bytes);
        }
        Ok(results)
    }
}

impl Plot for DensityPlot {
    type Options = DensityPlotOptions;
    type Data = ScatterPlotData;

    fn render(
        &self,
        data: Self::Data,
        options: &Self::Options,
        render_config: &mut RenderConfig,
    ) -> Result<PlotBytes> {
        let plot_start = std::time::Instant::now();

        match options.plot_type.canonical() {
            PlotType::Contour | PlotType::ContourOverlay => {
                let xy = data.xy();
                let x_range = (*options.x_axis.range.start(), *options.x_axis.range.end());
                let y_range = (*options.y_axis.range.start(), *options.y_axis.range.end());
                let contour_data = calculate_contours(
                    xy,
                    options.contour_level_count,
                    options.contour_smoothing,
                    options.draw_outliers,
                    x_range,
                    y_range,
                )?;
                eprintln!(
                    "  ├─ Contour calculation: {:?} ({} paths, {} outliers)",
                    plot_start.elapsed(),
                    contour_data.contours.len(),
                    contour_data.outliers.len()
                );
                let draw_start = std::time::Instant::now();
                let result = render_contour(contour_data, options, render_config);
                eprintln!("  └─ Draw + encode: {:?}", draw_start.elapsed());
                result
            }
            _ => {
                let base = options.base();
                let raw_pixels = calculate_plot_pixels(
                    &data,
                    base.width as usize,
                    base.height as usize,
                    options,
                );
                eprintln!(
                    "  ├─ Plot calculation: {:?} ({} pixels at {}x{})",
                    plot_start.elapsed(),
                    raw_pixels.len(),
                    base.width,
                    base.height
                );
                let draw_start = std::time::Instant::now();
                let result = render_pixels(raw_pixels, options, render_config);
                eprintln!("  └─ Draw + encode: {:?}", draw_start.elapsed());
                result
            }
        }
    }
}
