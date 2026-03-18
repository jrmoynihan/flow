use derive_builder::Builder;

/// Base plot options containing layout and display settings
///
/// These options are common to all plot types and control the overall
/// appearance and layout of the plot.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::options::BasePlotOptions;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let base = BasePlotOptions::new()
///     .width(800u32)
///     .height(600u32)
///     .title("My Plot")
///     .build()?;
/// # Ok(())
/// # }
/// ```
///
/// @deprecated The old PlotOptions struct has been removed. Use DensityPlotOptions with builder pattern instead.
#[derive(Builder, Clone, Debug)]
#[builder(setter(into, strip_option), default)]
pub struct BasePlotOptions {
    /// Plot width in pixels
    #[builder(default = "400")]
    pub width: u32,

    /// Plot height in pixels
    #[builder(default = "400")]
    pub height: u32,

    /// Margin around the plot area in pixels
    #[builder(default = "10")]
    pub margin: u32,

    /// Size of the x-axis label area in pixels
    #[builder(default = "50")]
    pub x_label_area_size: u32,

    /// Size of the y-axis label area in pixels
    #[builder(default = "50")]
    pub y_label_area_size: u32,

    /// Plot title
    #[builder(default = "\"Density Plot\".to_string()")]
    pub title: String,

    /// Whether to show the chart title (default true).
    #[builder(default = "true")]
    pub show_title: bool,

    /// Whether to show the colorbar/legend (default true).
    #[builder(default = "true")]
    pub show_colorbar: bool,

    /// Font family for title, axis labels, and ticks (e.g. "sans-serif"). None = backend default.
    pub font_family: Option<String>,

    /// Title font size in points. None = backend default.
    pub title_size: Option<u32>,

    /// Axis label font size in points. None = backend default.
    pub label_size: Option<u32>,

    /// Tick label font size in points. None = backend default.
    pub tick_size: Option<u32>,

    /// Whether to show grid lines (default true).
    #[builder(default = "true")]
    pub show_grid: bool,
}

impl Default for BasePlotOptions {
    fn default() -> Self {
        Self {
            width: 400,
            height: 400,
            margin: 10,
            x_label_area_size: 50,
            y_label_area_size: 50,
            title: "Density Plot".to_string(),
            show_title: true,
            show_colorbar: true,
            font_family: None,
            title_size: None,
            label_size: None,
            tick_size: None,
            show_grid: true,
        }
    }
}

impl BasePlotOptions {
    /// Create a new builder for BasePlotOptions
    pub fn new() -> BasePlotOptionsBuilder {
        BasePlotOptionsBuilder::default()
    }
}
