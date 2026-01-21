//! # flow-plots
//!
//! A library for creating visualizations of flow cytometry data.
//!
//! ## Overview
//!
//! This library provides a flexible, extensible API for creating different types of plots
//! from flow cytometry data. The architecture is designed to be easily extended with new
//! plot types.
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use flow_plots::{DensityPlot, DensityPlotOptions};
//! use flow_plots::render::RenderConfig;
//!
//! let plot = DensityPlot::new();
//! let options = DensityPlotOptions::new()
//!     .width(800)
//!     .height(600)
//!     .build()?;
//! let data: Vec<(f32, f32)> = vec![(100.0, 200.0)];
//! let mut render_config = RenderConfig::default();
//! let bytes = plot.render(data, &options, &mut render_config)?;
//! ```
//!
//! ## Architecture
//!
//! The library is organized into several modules:
//!
//! - `options`: Plot configuration types using the builder pattern
//! - `plots`: Plot implementations (currently `DensityPlot`)
//! - `render`: Rendering infrastructure and progress reporting
//! - `density`: Density calculation algorithms
//! - `colormap`: Color map implementations
//! - `helpers`: Helper functions for common initialization patterns

pub mod colormap;
pub mod density_calc;
pub mod helpers;
pub mod options;
pub mod plots;
pub mod render;

// Re-export commonly used types
pub use colormap::ColorMaps;
pub use options::{AxisOptions, BasePlotOptions, DensityPlotOptions, PlotOptions};
pub use plots::{DensityPlot, Plot, PlotType};
pub use render::{ProgressCallback, ProgressInfo, RenderConfig};

// Type aliases
pub type PlotBytes = Vec<u8>;
pub type PlotRange = std::ops::RangeInclusive<f32>;

use flow_fcs::TransformType;
use std::ops::Range;

/// @deprecated The old PlotOptions struct has been removed. Use DensityPlotOptions with builder pattern instead.
///
/// The old `PlotOptions` struct mixed concerns and was difficult to extend.
/// It has been replaced with a hierarchy of option types:
/// - `BasePlotOptions`: Layout and display settings
/// - `AxisOptions`: Axis configuration
/// - `DensityPlotOptions`: Complete density plot configuration
///
/// See the module documentation for examples of the new API.

/// Create appropriate axis specifications with nice bounds and labels
///
/// This function creates axis ranges that work well with the specified transforms,
/// using "nice" number bounds for linear scales.
pub fn create_axis_specs(
    plot_range_x: &PlotRange,
    plot_range_y: &PlotRange,
    x_transform: &TransformType,
    y_transform: &TransformType,
) -> anyhow::Result<(Range<f32>, Range<f32>)> {
    // For linear scales, use nice number bounds
    // For arcsinh and biexponential, ensure we use proper transformed bounds
    let x_spec = match x_transform {
        TransformType::Linear => {
            let min = plot_range_x.start();
            let max = plot_range_x.end();
            let (nice_min, nice_max) = nice_bounds(*min, *max);
            nice_min..nice_max
        }
        TransformType::Arcsinh { cofactor: _ } | TransformType::Biexponential { .. } => {
            // Keep the transformed range but we'll format nicely in the formatter
            *plot_range_x.start()..*plot_range_x.end()
        }
    };

    let y_spec = match y_transform {
        TransformType::Linear => {
            let min = plot_range_y.start();
            let max = plot_range_y.end();
            let (nice_min, nice_max) = nice_bounds(*min, *max);
            nice_min..nice_max
        }
        TransformType::Arcsinh { cofactor: _ } | TransformType::Biexponential { .. } => {
            // Keep the transformed range but we'll format nicely in the formatter
            *plot_range_y.start()..*plot_range_y.end()
        }
    };

    Ok((x_spec.into(), y_spec.into()))
}

/// Calculate percentile bounds for a dataset
///
/// Returns a range that encompasses the specified percentiles of the data,
/// rounded to "nice" numbers for better axis display.
pub fn get_percentile_bounds(
    values: &[f32],
    percentile_low: f32,
    percentile_high: f32,
) -> PlotRange {
    let mut sorted_values = values.to_vec();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let low_index = (percentile_low * sorted_values.len() as f32).floor() as usize;
    let high_index = (percentile_high * sorted_values.len() as f32).ceil() as usize;

    // Ensure indices are within bounds
    let low_index = low_index.clamp(0, sorted_values.len() - 1);
    let high_index = high_index.clamp(0, sorted_values.len() - 1);

    let low_value = sorted_values[low_index];
    let high_value = sorted_values[high_index];

    // Round to nice numbers
    let min_bound = nearest_nice_number(low_value, RoundingDirection::Down);
    let max_bound = nearest_nice_number(high_value, RoundingDirection::Up);

    min_bound..=max_bound
}

fn nice_bounds(min: f32, max: f32) -> (f32, f32) {
    if min.is_infinite() || max.is_infinite() || min.is_nan() || max.is_nan() {
        return (0.0, 1.0); // Fallback for invalid ranges
    }

    let range = max - min;
    if range == 0.0 {
        return (min - 0.5, min + 0.5); // Handle single-point case
    }

    // Find nice step size
    let step_size = 10_f32.powf((range.log10()).floor());
    let nice_min = (min / step_size).floor() * step_size;
    let nice_max = (max / step_size).ceil() * step_size;

    (nice_min, nice_max)
}

enum RoundingDirection {
    Up,
    Down,
}

fn nearest_nice_number(value: f32, direction: RoundingDirection) -> f32 {
    // Handle edge cases
    if value == 0.0 {
        return 0.0;
    }

    let abs_value = value.abs();
    let exponent = abs_value.log10().floor() as i32;
    let factor = 10f32.powi(exponent);

    // Find nearest nice number based on direction
    let nice_value = match direction {
        RoundingDirection::Up => {
            let mantissa = (abs_value / factor).ceil();
            if mantissa <= 1.0 {
                1.0 * factor
            } else if mantissa <= 2.0 {
                2.0 * factor
            } else if mantissa <= 5.0 {
                5.0 * factor
            } else {
                10.0 * factor
            }
        }
        RoundingDirection::Down => {
            let mantissa = (abs_value / factor).floor();
            if mantissa >= 5.0 {
                5.0 * factor
            } else if mantissa >= 2.0 {
                2.0 * factor
            } else if mantissa >= 1.0 {
                1.0 * factor
            } else {
                0.5 * factor
            }
        }
    };

    // Preserve sign
    if value.is_sign_negative() {
        -nice_value
    } else {
        nice_value
    }
}
