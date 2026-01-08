use crate::PlotRange;
use derive_builder::Builder;
use flow_fcs::TransformType;

/// Options for configuring a plot axis
///
/// Controls the range, transformation, and label for a single axis.
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::options::AxisOptions;
/// use flow_fcs::TransformType;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let axis = AxisOptions::new()
///     .range(0.0..=200_000.0)
///     .transform(TransformType::Arcsinh { cofactor: 150.0 })
///     .label("FSC-A")
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Builder, Clone, Debug)]
#[builder(setter(into, strip_option), default)]
pub struct AxisOptions {
    /// Data range for this axis
    #[builder(default = "0f32..=200_000f32")]
    pub range: PlotRange,

    /// Transform to apply to axis labels
    #[builder(default = "TransformType::default()")]
    pub transform: TransformType,

    /// Optional axis label
    pub label: Option<String>,
}

impl Default for AxisOptions {
    fn default() -> Self {
        Self {
            range: 0f32..=200_000f32,
            transform: TransformType::default(),
            label: None,
        }
    }
}

impl AxisOptions {
    /// Create a new builder for AxisOptions
    pub fn new() -> AxisOptionsBuilder {
        AxisOptionsBuilder::default()
    }
}
