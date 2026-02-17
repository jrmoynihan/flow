//! Spectral signature plot implementation
//!
//! Creates line plots showing normalized spectral signatures (1.0 to 0.0 vs channels)
//! for flow cytometry fluorophores.

use crate::PlotBytes;
// Spectral plot implementation
use crate::plots::traits::Plot;
use crate::render::RenderConfig;
use crate::render::plotters_backend::render_spectral_signature;
use anyhow::Result;

/// Spectral signature plot implementation
///
/// Creates a line plot showing normalized spectral signatures across detector channels.
/// The y-axis represents normalized intensity (0.0 to 1.0), and the x-axis represents
/// detector channels.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::plots::spectral::SpectralSignaturePlot;
/// use flow_plots::plots::traits::Plot;
/// use flow_plots::options::spectral::SpectralSignaturePlotOptions;
/// use flow_plots::options::BasePlotOptions;
/// use flow_plots::render::RenderConfig;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let plot = SpectralSignaturePlot::new();
/// let options = SpectralSignaturePlotOptions::new()
///     .base(BasePlotOptions::new().width(1200u32).height(600u32).build()?)
///     .build()?;
/// let data: Vec<(usize, f64)> = vec![(0, 0.1), (1, 0.5), (2, 1.0), (3, 0.8)];
/// let channel_names: Vec<String> = vec!["UV1-A".into(), "UV2-A".into(), "UV3-A".into(), "UV4-A".into()];
/// let mut render_config = RenderConfig::default();
/// let bytes = plot.render((data, channel_names), &options, &mut render_config)?;
/// # Ok(())
/// # }
/// ```
pub struct SpectralSignaturePlot;

impl SpectralSignaturePlot {
    /// Create a new SpectralSignaturePlot instance
    pub fn new() -> Self {
        Self
    }
}

impl Plot for SpectralSignaturePlot {
    type Options = crate::options::spectral::SpectralSignaturePlotOptions;
    type Data = (Vec<(usize, f64)>, Vec<String>); // (channel_index, normalized_intensity), channel_names

    fn render(
        &self,
        data: Self::Data,
        options: &Self::Options,
        render_config: &mut RenderConfig,
    ) -> Result<PlotBytes> {
        render_spectral_signature(data, options, render_config)
    }
}
