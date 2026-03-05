//! Options for histogram plots

use crate::options::density::default_gate_colors;
use crate::options::{AxisOptions, BasePlotOptions, PlotOptions};
use derive_builder::Builder;

/// Options for histogram plots
///
/// Supports single and overlaid histograms with filled or line-only style.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::options::{BasePlotOptions, HistogramPlotOptions};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let options = HistogramPlotOptions::new()
///     .base(BasePlotOptions::new().width(800u32).height(600u32).build()?)
///     .histogram_filled(true)
///     .num_bins(50usize)
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Builder, Clone, Debug)]
#[builder(setter(into, strip_option), default)]
pub struct HistogramPlotOptions {
    /// Base plot options (layout, dimensions, etc.)
    #[builder(default)]
    pub base: BasePlotOptions,

    /// X-axis configuration (data range, transform, label)
    #[builder(default)]
    pub x_axis: AxisOptions,

    /// Draw filled histogram (AreaSeries) vs line-only (LineSeries)
    #[builder(default = "true")]
    pub histogram_filled: bool,

    /// Number of bins when binning raw values. Ignored for pre-binned data.
    #[builder(default = "50")]
    pub num_bins: usize,

    /// Colors for each gate/series in overlaid histograms. Indexed by gate_id.
    #[builder(default)]
    pub gate_colors: Vec<(u8, u8, u8)>,

    /// Y-offset between baselines for overlaid histograms. 0 = no separation (overlap).
    #[builder(default = "0.0")]
    pub baseline_separation: f32,

    /// Scale each overlaid series to its modal peak (max count = 1.0) for visual comparison
    #[builder(default = "false")]
    pub scale_to_peak: bool,

    /// Line width for unfilled histogram (when histogram_filled is false)
    #[builder(default = "2.0")]
    pub line_width: f64,
}

impl Default for HistogramPlotOptions {
    fn default() -> Self {
        Self {
            base: BasePlotOptions::default(),
            x_axis: AxisOptions::default(),
            histogram_filled: true,
            num_bins: 50,
            gate_colors: Vec::new(),
            baseline_separation: 0.0,
            scale_to_peak: false,
            line_width: 2.0,
        }
    }
}

impl PlotOptions for HistogramPlotOptions {
    fn base(&self) -> &BasePlotOptions {
        &self.base
    }
}

impl HistogramPlotOptions {
    /// Create a new builder for HistogramPlotOptions
    pub fn new() -> HistogramPlotOptionsBuilder {
        HistogramPlotOptionsBuilder::default()
    }

    /// Get gate color for the given gate_id
    pub fn gate_color(&self, gate_id: u32) -> (u8, u8, u8) {
        let colors = if self.gate_colors.is_empty() {
            default_gate_colors()
        } else {
            self.gate_colors.clone()
        };
        colors
            .get(gate_id as usize)
            .copied()
            .unwrap_or((60, 60, 60))
    }
}
