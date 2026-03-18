//! Kuva layout and tick formatting for flow plots with arcsinh pre-transformed data.
//!
//! Linear axes in transformed space with custom tick formatters that display
//! original (untransformed) values using flow-fcs `TransformType::inverse_transform`.

#![cfg(feature = "raster")]

use crate::options::{DensityPlotOptions, PlotOptions};
use flow_fcs::{TransformType, Transformable};
use std::sync::Arc;

/// Build a kuva `TickFormat::Custom` that shows original-axis values.
///
/// The closure receives the tick value in **transformed** (data) space and
/// returns a string for the original scale (e.g. fluorescence units).
pub fn tick_format_from_transform(transform: &TransformType) -> kuva::render::layout::TickFormat {
    let t = transform.clone();
    kuva::render::layout::TickFormat::Custom(Arc::new(move |value: f64| {
        let original = t.inverse_transform(&(value as f32));
        format!("{:.1e}", original)
    }))
}

/// Build a tick formatter for kuva Heatmap, where tick values are cell indices (0.5, 1.5, ...).
/// Maps index to data coordinate then to original scale via inverse_transform.
pub fn tick_format_heatmap_cell_index(
    data_min: f64,
    data_max: f64,
    n_cells: usize,
    transform: &TransformType,
) -> kuva::render::layout::TickFormat {
    let t = transform.clone();
    let n = n_cells as f64;
    let span = data_max - data_min;
    kuva::render::layout::TickFormat::Custom(Arc::new(move |value: f64| {
        let data = data_min + (value - 0.5) / n * span;
        let original = t.inverse_transform(&(data as f32));
        format!("{:.1e}", original)
    }))
}

/// Apply density-plot overrides to an existing kuva `Layout` (e.g. from `Layout::auto_from_plots`).
///
/// Sets tick formatters for original-value labels, axis labels, title, colorbar,
/// and optional visibility/appearance from options.
pub fn apply_density_layout_overrides(
    layout: &mut kuva::render::layout::Layout,
    options: &DensityPlotOptions,
) {
    layout.log_x = false;
    layout.log_y = false;

    layout.x_tick_format = tick_format_from_transform(&options.x_axis.transform);
    layout.y_tick_format = tick_format_from_transform(&options.y_axis.transform);

    if let Some(ref label) = options.x_axis.label {
        layout.x_label = Some(label.clone());
    }
    if let Some(ref label) = options.y_axis.label {
        layout.y_label = Some(label.clone());
    }

    let base = options.base();
    if base.show_title {
        layout.title = Some(base.title.clone());
    } else {
        layout.title = None;
    }
    layout.show_colorbar = base.show_colorbar;

    layout.show_grid = base.show_grid;
    if let Some(ref f) = base.font_family {
        layout.font_family = Some(f.clone());
    }
    if let Some(s) = base.title_size {
        layout.title_size = s;
    }
    if let Some(s) = base.label_size {
        layout.label_size = s;
    }
    if let Some(s) = base.tick_size {
        layout.tick_size = s;
    }
}

/// Build a kuva `Layout` for a density plot with linear axes and original-value tick labels.
///
/// Uses `options.x_axis.range` / `y_axis.range` (transformed space), sets
/// `log_x`/`log_y` to false, and applies custom tick formatters so labels
/// show original values via `inverse_transform`.
pub fn density_layout_from_options(options: &DensityPlotOptions) -> kuva::render::layout::Layout {
    let x_min = *options.x_axis.range.start() as f64;
    let x_max = *options.x_axis.range.end() as f64;
    let y_min = *options.y_axis.range.start() as f64;
    let y_max = *options.y_axis.range.end() as f64;

    let mut layout = kuva::render::layout::Layout::new((x_min, x_max), (y_min, y_max));
    apply_density_layout_overrides(&mut layout, options);
    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_format_arcsinh() {
        let transform = TransformType::Arcsinh { cofactor: 200.0 };
        let fmt = tick_format_from_transform(&transform);
        let s0 = fmt.format(0.0);
        let s1 = fmt.format(1.0);
        let s5 = fmt.format(5.0);
        // 0 in transformed -> 0 in original
        assert!(s0.contains('0'));
        // 1 in transformed -> sinh(1)*200 ≈ 235
        assert!(s1.contains('e'));
        // 5 in transformed -> sinh(5)*200 ≈ 2.9e4
        assert!(s5.contains('e'));
    }

    #[test]
    fn test_tick_format_linear() {
        let transform = TransformType::Linear;
        let fmt = tick_format_from_transform(&transform);
        assert_eq!(fmt.format(100.0), "1.0e2");
        assert_eq!(fmt.format(0.5), "5.0e-1");
    }
}
