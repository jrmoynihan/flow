pub mod density;
pub mod histogram;
pub mod spectral;
pub mod traits;

pub use density::DensityPlot;
pub use histogram::HistogramPlot;
pub use spectral::SpectralSignaturePlot;
pub use traits::Plot;

/// Plot type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotType {
    /// Monocolor scatter points
    Scatter,
    /// Density heatmap (binned KDE)
    Density,
    /// Points colored by 3rd parameter value
    Intensity,
    /// Contour lines from KDE
    Contour,
    /// Histogram plot
    Histogram,
}

impl Default for PlotType {
    fn default() -> Self {
        PlotType::Density
    }
}

impl PlotType {
    pub fn canonical(self) -> Self {
        self
    }
}
