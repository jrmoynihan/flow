use crate::colormap::ColorMaps;
use crate::options::{AxisOptions, BasePlotOptions, PlotOptions};
use crate::plots::PlotType;
use derive_builder::Builder;

/// Options for density plots
///
/// Configuration for creating density plots, including base layout options,
/// axis configurations, and color map selection.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::options::{DensityPlotOptions, BasePlotOptions};
/// use flow_plots::colormap::ColorMaps;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let options = DensityPlotOptions::new()
///     .base(BasePlotOptions::new().width(800u32).height(600u32).build()?)
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

    /// Plot type (density, scatter, contour, etc.)
    #[builder(default)]
    pub plot_type: PlotType,

    /// Point size in pixels for scatter and density plots (0.5–4.0).
    /// For scatter: radius of each point. For density: radius of each point's contribution to the heatmap.
    #[builder(default = "1.0")]
    pub point_size: f32,

    /// Contour line thickness in pixels (when plot_type is Contour)
    #[builder(default = "1.0")]
    pub contour_line_thickness: f32,

    /// Number of contour levels (when plot_type is Contour)
    #[builder(default = "5")]
    pub contour_level_count: u32,

    /// KDE bandwidth adjustment factor (when plot_type is Contour).
    /// Higher values = more smoothing. Default 1.0. Typical range 0.5–2.0.
    #[builder(default = "1.0")]
    pub contour_smoothing: f64,

    /// Draw scatter points outside contour regions as outliers (when plot_type is Contour)
    #[builder(default = "false")]
    pub draw_outliers: bool,

    /// Colors for discrete gate overlay (ScatterOverlay, ContourOverlay).
    /// gate_ids in data index into this slice. Default palette used if empty.
    #[builder(default)]
    pub gate_colors: Vec<(u8, u8, u8)>,

    /// Z-axis range for continuous coloring (ScatterColoredContinuous).
    /// If None, min/max of z_values is used.
    #[builder(default)]
    pub z_range: Option<(f32, f32)>,
}

impl Default for DensityPlotOptions {
    fn default() -> Self {
        Self {
            base: BasePlotOptions::default(),
            x_axis: AxisOptions::default(),
            y_axis: AxisOptions::default(),
            colormap: ColorMaps::Viridis,
            plot_type: PlotType::Density,
            point_size: 1.0,
            contour_line_thickness: 1.0,
            contour_level_count: 5,
            contour_smoothing: 1.0,
            draw_outliers: false,
            gate_colors: Vec::new(),
            z_range: None,
        }
    }
}

/// Default palette for gate overlay (green, orange, blue, etc.)
pub fn default_gate_colors() -> Vec<(u8, u8, u8)> {
    vec![
        (34, 139, 34),   // forest green
        (255, 165, 0),   // orange
        (31, 119, 180),  // blue
        (214, 39, 40),   // red
        (148, 103, 189), // purple
        (140, 86, 75),   // brown
        (227, 119, 194), // pink
        (127, 127, 127), // gray
    ]
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
