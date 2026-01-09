use crate::PlotBytes;
use crate::density_calc::calculate_density_per_pixel;
use crate::options::{DensityPlotOptions, PlotOptions};
use crate::plots::traits::Plot;
use crate::render::RenderConfig;
use crate::render::plotters_backend::render_pixels;
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
/// use flow_plots::options::DensityPlotOptions;
/// use flow_plots::render::RenderConfig;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let plot = DensityPlot::new();
/// let options = DensityPlotOptions::new()
///     .width(800)
///     .height(600)
///     .build()?;
/// let data: Vec<(f32, f32)> = vec![(100.0, 200.0), (150.0, 250.0)];
/// let mut render_config = RenderConfig::default();
/// let bytes = plot.render(data, &options, &mut render_config)?;
/// # Ok(())
/// # }
/// ```
pub struct DensityPlot;

impl DensityPlot {
    /// Create a new DensityPlot instance
    pub fn new() -> Self {
        Self
    }
}

impl Plot for DensityPlot {
    type Options = DensityPlotOptions;
    type Data = Vec<(f32, f32)>;

    fn render(
        &self,
        data: Self::Data,
        options: &Self::Options,
        render_config: &mut RenderConfig,
    ) -> Result<PlotBytes> {
        let density_start = std::time::Instant::now();

        // Calculate density per pixel
        let base = options.base();
        let raw_pixels = calculate_density_per_pixel(
            &data[..],
            base.width as usize,
            base.height as usize,
            options,
        );

        eprintln!(
            "  ├─ Density calculation: {:?} ({} pixels at {}x{})",
            density_start.elapsed(),
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
