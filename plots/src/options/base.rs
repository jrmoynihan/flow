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

/// Generates passthrough setters for the given [`BasePlotOptions`] fields directly on a
/// plot-options builder that embeds `base: BasePlotOptions`, so callers don't have to
/// build a `BasePlotOptions` separately just to set width/height/title/etc.
///
/// `.base(...)` keeps working unchanged for building a `BasePlotOptions` once and
/// reusing/sharing it across multiple option sets.
///
/// Must be invoked from the same module as the target `*Builder` struct, since it
/// accesses the (private) generated `base` field directly. Use the `mut` mode for
/// builders using derive_builder's default "mutable" pattern (`&mut self -> &mut Self`),
/// and the `owned` mode for builders using `#[builder(pattern = "owned")]` (`self -> Self`).
///
/// Takes an explicit field list so a caller whose own struct already has a
/// same-named top-level field (e.g. `SpectralSignaturePlotOptions::show_grid`) can
/// omit that field to avoid a duplicate-setter collision with derive_builder's
/// own generated setter.
macro_rules! impl_base_options_passthrough {
    ($mode:ident $builder:ty : $($field:ident),+ $(,)?) => {
        impl $builder {
            $(
                $crate::options::base::base_option_passthrough_field!($mode, $field);
            )+
        }
    };
}

/// One passthrough setter per `(mode, field)` pair. See [`impl_base_options_passthrough`].
macro_rules! base_option_passthrough_field {
    (mut, width) => {
        /// Plot width in pixels. Promotes directly onto the nested `base` options.
        pub fn width(&mut self, width: u32) -> &mut Self {
            self.base.get_or_insert_with(BasePlotOptions::default).width = width;
            self
        }
    };
    (mut, height) => {
        /// Plot height in pixels. Promotes directly onto the nested `base` options.
        pub fn height(&mut self, height: u32) -> &mut Self {
            self.base.get_or_insert_with(BasePlotOptions::default).height = height;
            self
        }
    };
    (mut, margin) => {
        /// Margin around the plot area in pixels. Promotes directly onto the nested `base` options.
        pub fn margin(&mut self, margin: u32) -> &mut Self {
            self.base.get_or_insert_with(BasePlotOptions::default).margin = margin;
            self
        }
    };
    (mut, x_label_area_size) => {
        /// Size of the x-axis label area in pixels. Promotes directly onto the nested `base` options.
        pub fn x_label_area_size(&mut self, size: u32) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .x_label_area_size = size;
            self
        }
    };
    (mut, y_label_area_size) => {
        /// Size of the y-axis label area in pixels. Promotes directly onto the nested `base` options.
        pub fn y_label_area_size(&mut self, size: u32) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .y_label_area_size = size;
            self
        }
    };
    (mut, title) => {
        /// Plot title. Promotes directly onto the nested `base` options.
        pub fn title(&mut self, title: impl Into<String>) -> &mut Self {
            self.base.get_or_insert_with(BasePlotOptions::default).title = title.into();
            self
        }
    };
    (mut, show_title) => {
        /// Whether to show the chart title. Promotes directly onto the nested `base` options.
        pub fn show_title(&mut self, show: bool) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .show_title = show;
            self
        }
    };
    (mut, show_colorbar) => {
        /// Whether to show the colorbar/legend. Promotes directly onto the nested `base` options.
        pub fn show_colorbar(&mut self, show: bool) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .show_colorbar = show;
            self
        }
    };
    (mut, font_family) => {
        /// Font family for title, axis labels, and ticks. Promotes directly onto the nested `base` options.
        pub fn font_family(&mut self, font: impl Into<String>) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .font_family = Some(font.into());
            self
        }
    };
    (mut, title_size) => {
        /// Title font size in points. Promotes directly onto the nested `base` options.
        pub fn title_size(&mut self, size: u32) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .title_size = Some(size);
            self
        }
    };
    (mut, label_size) => {
        /// Axis label font size in points. Promotes directly onto the nested `base` options.
        pub fn label_size(&mut self, size: u32) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .label_size = Some(size);
            self
        }
    };
    (mut, tick_size) => {
        /// Tick label font size in points. Promotes directly onto the nested `base` options.
        pub fn tick_size(&mut self, size: u32) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .tick_size = Some(size);
            self
        }
    };
    (mut, show_grid) => {
        /// Whether to show grid lines. Promotes directly onto the nested `base` options.
        pub fn show_grid(&mut self, show: bool) -> &mut Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .show_grid = show;
            self
        }
    };
    (owned, width) => {
        /// Plot width in pixels. Promotes directly onto the nested `base` options.
        pub fn width(mut self, width: u32) -> Self {
            self.base.get_or_insert_with(BasePlotOptions::default).width = width;
            self
        }
    };
    (owned, height) => {
        /// Plot height in pixels. Promotes directly onto the nested `base` options.
        pub fn height(mut self, height: u32) -> Self {
            self.base.get_or_insert_with(BasePlotOptions::default).height = height;
            self
        }
    };
    (owned, margin) => {
        /// Margin around the plot area in pixels. Promotes directly onto the nested `base` options.
        pub fn margin(mut self, margin: u32) -> Self {
            self.base.get_or_insert_with(BasePlotOptions::default).margin = margin;
            self
        }
    };
    (owned, x_label_area_size) => {
        /// Size of the x-axis label area in pixels. Promotes directly onto the nested `base` options.
        pub fn x_label_area_size(mut self, size: u32) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .x_label_area_size = size;
            self
        }
    };
    (owned, y_label_area_size) => {
        /// Size of the y-axis label area in pixels. Promotes directly onto the nested `base` options.
        pub fn y_label_area_size(mut self, size: u32) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .y_label_area_size = size;
            self
        }
    };
    (owned, title) => {
        /// Plot title. Promotes directly onto the nested `base` options.
        pub fn title(mut self, title: impl Into<String>) -> Self {
            self.base.get_or_insert_with(BasePlotOptions::default).title = title.into();
            self
        }
    };
    (owned, show_title) => {
        /// Whether to show the chart title. Promotes directly onto the nested `base` options.
        pub fn show_title(mut self, show: bool) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .show_title = show;
            self
        }
    };
    (owned, show_colorbar) => {
        /// Whether to show the colorbar/legend. Promotes directly onto the nested `base` options.
        pub fn show_colorbar(mut self, show: bool) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .show_colorbar = show;
            self
        }
    };
    (owned, font_family) => {
        /// Font family for title, axis labels, and ticks. Promotes directly onto the nested `base` options.
        pub fn font_family(mut self, font: impl Into<String>) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .font_family = Some(font.into());
            self
        }
    };
    (owned, title_size) => {
        /// Title font size in points. Promotes directly onto the nested `base` options.
        pub fn title_size(mut self, size: u32) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .title_size = Some(size);
            self
        }
    };
    (owned, label_size) => {
        /// Axis label font size in points. Promotes directly onto the nested `base` options.
        pub fn label_size(mut self, size: u32) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .label_size = Some(size);
            self
        }
    };
    (owned, tick_size) => {
        /// Tick label font size in points. Promotes directly onto the nested `base` options.
        pub fn tick_size(mut self, size: u32) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .tick_size = Some(size);
            self
        }
    };
    (owned, show_grid) => {
        /// Whether to show grid lines. Promotes directly onto the nested `base` options.
        pub fn show_grid(mut self, show: bool) -> Self {
            self.base
                .get_or_insert_with(BasePlotOptions::default)
                .show_grid = show;
            self
        }
    };
}
pub(crate) use base_option_passthrough_field;
pub(crate) use impl_base_options_passthrough;
