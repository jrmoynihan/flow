//! Options for spectral signature plots

use crate::options::{AxisOptions, BasePlotOptions, PlotOptions, impl_base_options_passthrough};
use derive_builder::Builder;

/// Options for spectral signature plots
#[derive(Builder, Debug, Clone)]
#[builder(pattern = "owned")]
pub struct SpectralSignaturePlotOptions {
    /// Base plot options (layout, dimensions, etc.)
    #[builder(default)]
    pub base: BasePlotOptions,

    /// X-axis configuration (detector channels)
    #[builder(default)]
    pub x_axis: Option<AxisOptions>,

    /// Y-axis configuration (normalized intensity 0.0-1.0)
    #[builder(default)]
    pub y_axis: Option<AxisOptions>,

    /// Line color (default: blue)
    #[builder(default = "String::from(\"#1f77b4\")")]
    pub line_color: String,

    /// Line width (default: 2.0)
    #[builder(default = "2.0")]
    pub line_width: f64,

    /// Show grid (default: true)
    #[builder(default = "true")]
    pub show_grid: bool,
}

impl PlotOptions for SpectralSignaturePlotOptions {
    fn base(&self) -> &BasePlotOptions {
        &self.base
    }
}

impl SpectralSignaturePlotOptions {
    /// Create a new builder for SpectralSignaturePlotOptions
    pub fn new() -> SpectralSignaturePlotOptionsBuilder {
        SpectralSignaturePlotOptionsBuilder::default()
    }
}

// show_grid is intentionally omitted: SpectralSignaturePlotOptions already has its
// own top-level `show_grid` field with a derive_builder-generated setter of the
// same name, so promoting `base.show_grid` too would collide.
impl_base_options_passthrough!(owned SpectralSignaturePlotOptionsBuilder:
    width, height, margin, x_label_area_size, y_label_area_size,
    title, show_title, show_colorbar, font_family, title_size, label_size, tick_size);
