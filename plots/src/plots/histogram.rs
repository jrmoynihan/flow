//! Histogram plot implementation
//!
//! Creates 1D histograms from raw values or pre-binned data, with support for
//! filled/unfilled style and overlaid multiple series.

use crate::histogram_data::HistogramData;
use crate::options::HistogramPlotOptions;
use crate::plots::traits::Plot;
use crate::PlotBytes;
use crate::render::plotters_backend::render_histogram;
use crate::render::RenderConfig;
use anyhow::Result;

/// Histogram plot implementation
///
/// Creates 1D histograms with x-axis = values, y-axis = count/frequency.
/// Supports raw values (auto-binned), pre-binned data, and overlaid series.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::plots::histogram::HistogramPlot;
/// use flow_plots::plots::traits::Plot;
/// use flow_plots::options::{BasePlotOptions, HistogramPlotOptions};
/// use flow_plots::histogram_data::HistogramData;
/// use flow_plots::render::RenderConfig;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let plot = HistogramPlot::new();
/// let options = HistogramPlotOptions::new()
///     .base(BasePlotOptions::new().width(800u32).height(600u32).build()?)
///     .histogram_filled(true)
///     .build()?;
/// let data = HistogramData::from_values(vec![1.0, 2.0, 2.0, 3.0, 3.0, 3.0]);
/// let mut render_config = RenderConfig::default();
/// let bytes = plot.render(data, &options, &mut render_config)?;
/// # Ok(())
/// # }
/// ```
pub struct HistogramPlot;

impl HistogramPlot {
    /// Create a new HistogramPlot instance
    pub fn new() -> Self {
        Self
    }
}

impl Plot for HistogramPlot {
    type Options = HistogramPlotOptions;
    type Data = HistogramData;

    fn render(
        &self,
        data: Self::Data,
        options: &Self::Options,
        render_config: &mut RenderConfig,
    ) -> Result<PlotBytes> {
        render_histogram(data, options, render_config)
    }
}
