use crate::options::AxisOptionsBuilder;
use crate::options::DensityPlotOptionsBuilder;
use anyhow::Result;
use flow_fcs::{Fcs, Parameter, TransformType, Transformable};

/// Create a DensityPlotOptions builder with sensible defaults based on FCS file data
///
/// This helper function analyzes the FCS file and parameters to determine
/// appropriate plot ranges and transforms, similar to the old `PlotOptions::new()`
/// method. It returns a builder that can be further customized.
///
/// # Arguments
///
/// * `fcs` - The FCS file to analyze
/// * `x_parameter` - The parameter to use for the x-axis
/// * `y_parameter` - The parameter to use for the y-axis
///
/// # Returns
///
/// A `DensityPlotOptionsBuilder` with pre-configured ranges and transforms
///
/// # Example
///
/// ```rust,no_run
/// use flow_plots::helpers::density_options_from_fcs;
/// use flow_fcs::{Fcs, Parameter};
///
/// let mut builder = density_options_from_fcs(fcs, x_param, y_param)?;
/// let options = builder
///     .width(800)
///     .height(600)
///     .build()?;
/// ```
pub fn density_options_from_fcs(
    fcs: &Fcs,
    x_parameter: &Parameter,
    y_parameter: &Parameter,
) -> Result<DensityPlotOptionsBuilder> {
    let default_range = 0f32..=200_000f32;

    // Determine plot ranges (avoid panics so backend failures remain enumerable)
    // Note: We use the transform from the parameter directly since TransformType implements Transformable
    let plot_range_x = match x_parameter.channel_name.as_ref() {
        name if name.contains("FSC") || name.contains("SSC") => default_range.clone(),
        name if name.contains("Time") => {
            let time_values = fcs.get_parameter_events_slice(&x_parameter.channel_name)?;
            let time_max = time_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            0f32..=time_max
        }
        _ => {
            let raw_view = fcs.get_parameter_events_slice(&x_parameter.channel_name)?;
            let transformed = raw_view
                .iter()
                .map(|&v| x_parameter.transform.transform(&v))
                .collect::<Vec<_>>();
            crate::get_percentile_bounds(&transformed, 0.01, 0.99)
        }
    };

    let plot_range_y = match y_parameter.channel_name.as_ref() {
        name if name.contains("FSC") || name.contains("SSC") => default_range.clone(),
        name if name.contains("Time") => {
            let time_values = fcs.get_parameter_events_slice(&y_parameter.channel_name)?;
            let time_max = time_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            0f32..=time_max
        }
        _ => {
            let raw_view = fcs.get_parameter_events_slice(&y_parameter.channel_name)?;
            let transformed = raw_view
                .iter()
                .map(|&v| y_parameter.transform.transform(&v))
                .collect::<Vec<_>>();
            crate::get_percentile_bounds(&transformed, 0.01, 0.99)
        }
    };

    let x_label_transform = match x_parameter.channel_name.as_ref() {
        name if name.contains("FSC") || name.contains("SSC") => TransformType::Linear,
        _ => TransformType::default(),
    };

    let y_label_transform = match y_parameter.channel_name.as_ref() {
        name if name.contains("FSC") || name.contains("SSC") => TransformType::Linear,
        _ => TransformType::default(),
    };

    let title = fcs.get_fil_keyword()?.to_string();

    let x_axis = AxisOptionsBuilder::default()
        .range(plot_range_x)
        .transform(x_label_transform)
        .build()?;

    let y_axis = AxisOptionsBuilder::default()
        .range(plot_range_y)
        .transform(y_label_transform)
        .build()?;

    let base = crate::options::BasePlotOptionsBuilder::default()
        .title(title)
        .build()?;

    let mut builder = DensityPlotOptionsBuilder::default();
    builder.base(base);
    builder.x_axis(x_axis);
    builder.y_axis(y_axis);

    Ok(builder)
}
