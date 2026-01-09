pub mod density;
pub mod traits;

pub use density::DensityPlot;
pub use traits::Plot;

/// Plot type enumeration
///
/// This enum can be used to dispatch to different plot implementations.
/// However, for better type safety and extensibility, prefer using the
/// `Plot` trait directly with specific plot types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotType {
    /// Dot plot (scatter plot)
    Dot,
    /// Density plot (2D histogram)
    Density,
    /// Contour plot
    Contour,
    /// Zebra plot
    Zebra,
    /// Histogram plot
    Histogram,
}
