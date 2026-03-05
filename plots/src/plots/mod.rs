pub mod density;
pub mod histogram;
pub mod spectral;
pub mod traits;

pub use density::DensityPlot;
pub use histogram::HistogramPlot;
pub use spectral::SpectralSignaturePlot;
pub use traits::Plot;

/// Plot type enumeration
///
/// Maps to UI labels: Scatterplot (Solid), Scatterplot (Density), etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotType {
    /// Monocolor scatter (Scatterplot Solid)
    ScatterSolid,
    /// Density heatmap (Scatterplot Density)
    Density,
    /// Z-axis colored scatter (Scatterplot Colored-continuous)
    ScatterColoredContinuous,
    /// Metadata-group colored (Scatterplot Overlay)
    ScatterOverlay,
    /// Contour lines from KDE
    Contour,
    /// Contour with metadata groups
    ContourOverlay,
    /// Legacy alias for ScatterSolid
    #[serde(alias = "dot")]
    Dot,
    /// Legacy alias for Contour
    Zebra,
    /// Histogram plot
    Histogram,
}

impl Default for PlotType {
    fn default() -> Self {
        PlotType::Density
    }
}

impl PlotType {
    /// Normalize legacy variants to canonical form for dispatch
    pub fn canonical(self) -> Self {
        match self {
            PlotType::Dot => PlotType::ScatterSolid,
            other => other,
        }
    }
}
