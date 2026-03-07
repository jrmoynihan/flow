use crate::contour::ContourData;
use crate::density_calc::RawPixelData;
use crate::options::DensityPlotOptions;
use crate::render::RenderConfig;
use crate::PlotBytes;
use anyhow::Result;
use kuva::prelude::*;

/// Render pixels to a PNG image using the kuva backend (direct raster path).
///
/// Uses a single ScatterPlot with per-point colors so the backend can use
/// the CircleBatch fast path. Renders via render_to_raster (no SVG round-trip).
pub fn render_pixels(
    pixels: Vec<RawPixelData>,
    options: &DensityPlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;

    if pixels.is_empty() {
        let scatter = ScatterPlot::new().with_data(vec![(0.0_f64, 0.0)]);
        let plots: Vec<Plot> = vec![scatter.into()];
        let layout = Layout::auto_from_plots(&plots).with_title(&base.title);
        let png_bytes = render_to_raster(plots, layout, 1.0f32)
            .map_err(|e| anyhow::anyhow!("failed to render empty density plot to PNG: {e}"))?;
        return Ok(png_bytes);
    }

    let all_points: Vec<(f64, f64)> = pixels.iter().map(|p| (p.x as f64, p.y as f64)).collect();
    let all_colors: Vec<String> = pixels
        .iter()
        .map(|p| format!("#{:02x}{:02x}{:02x}", p.r, p.g, p.b))
        .collect();
    let point_size = (options.point_size as f64).max(0.5);
    let scatter = ScatterPlot::new()
        .with_data(all_points)
        .with_colors(all_colors)
        .with_size(point_size);
    let plots: Vec<Plot> = vec![scatter.into()];

    let mut layout = Layout::auto_from_plots(&plots).with_title(&base.title);
    if let Some(ref x_label) = options.x_axis.label {
        layout = layout.with_x_label(x_label);
    }
    if let Some(ref y_label) = options.y_axis.label {
        layout = layout.with_y_label(y_label);
    }

    let scale = (width as f32 / 800.0).max(1.0);
    let png_bytes = render_to_raster(plots, layout, scale)
        .map_err(|e| anyhow::anyhow!("failed to render density plot to PNG: {e}"))?;

    #[cfg(feature = "verbose_timing")]
    eprintln!(
        "    ├─ kuva render: {} pixels (raster path)",
        pixels.len()
    );

    Ok(png_bytes)
}

/// Render contour plot to PNG using kuva LinePlot
///
/// Draws contour lines from KDE density estimation plus optional outlier scatter points.
pub fn render_contour(
    contour_data: ContourData,
    options: &DensityPlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;

    let mut plots: Vec<Plot> = Vec::new();

    let contour_color = "#3C3C3C";

    for path in &contour_data.contours {
        if path.len() < 2 {
            continue;
        }
        let points: Vec<(f64, f64)> = path.iter().copied().collect();
        let line = LinePlot::new()
            .with_data(points)
            .with_color(contour_color);
        plots.push(line.into());
    }

    if !contour_data.outliers.is_empty() {
        let outlier_points: Vec<(f64, f64)> = contour_data.outliers.clone();
        let scatter = ScatterPlot::new()
            .with_data(outlier_points)
            .with_color("#969696");
        plots.push(scatter.into());
    }

    if plots.is_empty() {
        let scatter = ScatterPlot::new().with_data(vec![(0.0_f64, 0.0)]);
        plots.push(scatter.into());
    }

    let mut layout = Layout::auto_from_plots(&plots).with_title(&base.title);
    if let Some(ref x_label) = options.x_axis.label {
        layout = layout.with_x_label(x_label);
    }
    if let Some(ref y_label) = options.y_axis.label {
        layout = layout.with_y_label(y_label);
    }

    let scale = (width as f32 / 800.0).max(1.0);
    let png_bytes = render_to_raster(plots, layout, scale)
        .map_err(|e| anyhow::anyhow!("failed to render contour plot to PNG: {e}"))?;

    Ok(png_bytes)
}

/// Render spectral signature plot to PNG
///
/// Creates a line plot showing normalized spectral signatures (0.0 to 1.0) across detector channels.
pub fn render_spectral_signature(
    data: (Vec<(usize, f64)>, Vec<String>),
    options: &crate::options::spectral::SpectralSignaturePlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let (spectrum_data, _channel_names) = data;
    let base = options.base();
    let width = base.width;

    let line_color = if options.line_color.starts_with('#') && options.line_color.len() == 7 {
        options.line_color.clone()
    } else {
        "#1F77B4".to_string()
    };

    let points: Vec<(f64, f64)> = spectrum_data
        .iter()
        .map(|(idx, val)| (*idx as f64, *val))
        .collect();

    let mut plots: Vec<Plot> = Vec::new();

    if !points.is_empty() {
        let line = LinePlot::new()
            .with_data(points)
            .with_color(&line_color);
        plots.push(line.into());
    } else {
        let scatter = ScatterPlot::new().with_data(vec![(0.0_f64, 0.0)]);
        plots.push(scatter.into());
    }

    let mut layout = Layout::auto_from_plots(&plots);

    if let Some(ref x_axis) = options.x_axis {
        if let Some(ref label) = x_axis.label {
            layout = layout.with_x_label(label);
        }
    } else {
        layout = layout.with_x_label("Channel");
    }

    if let Some(ref y_axis) = options.y_axis {
        if let Some(ref label) = y_axis.label {
            layout = layout.with_y_label(label);
        }
    } else {
        layout = layout.with_y_label("Normalized Intensity");
    }

    layout = layout.with_title(&base.title);

    let scale = (width as f32 / 800.0).max(1.0);
    let png_bytes = render_to_raster(plots, layout, scale)
        .map_err(|e| anyhow::anyhow!("failed to render spectral signature to PNG: {e}"))?;

    Ok(png_bytes)
}

/// Render histogram plot to PNG
///
/// Creates a 1D histogram (x = values, y = count/frequency) with optional fill
/// and support for overlaid multiple series.
pub fn render_histogram(
    data: crate::histogram_data::HistogramData,
    options: &crate::options::HistogramPlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::histogram_data::{bin_values, BinnedHistogram, HistogramData, HistogramSeries};
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;

    let _x_min = *options.x_axis.range.start() as f64;
    let _x_max = *options.x_axis.range.end() as f64;

    let series: Vec<(BinnedHistogram, u32)> = match data {
        HistogramData::RawValues(values) => {
            let binned = bin_values(
                &values,
                options.num_bins,
                *options.x_axis.range.start(),
                *options.x_axis.range.end(),
            );
            match binned {
                Some(b) => vec![(b, 0)],
                None => vec![],
            }
        }
        HistogramData::PreBinned { bin_edges, counts } => {
            let bin_centers: Vec<f64> = bin_edges
                .windows(2)
                .map(|w| (w[0] as f64 + w[1] as f64) / 2.0)
                .collect();
            let counts_f64: Vec<f64> = counts.iter().map(|&c| c as f64).collect();
            vec![(
                BinnedHistogram {
                    bin_centers,
                    counts: counts_f64,
                },
                0,
            )]
        }
        HistogramData::Overlaid(overlaid) => {
            let mut result = Vec::with_capacity(overlaid.len());
            for HistogramSeries { values, gate_id } in overlaid {
                if let Some(binned) = bin_values(
                    &values,
                    options.num_bins,
                    *options.x_axis.range.start(),
                    *options.x_axis.range.end(),
                ) {
                    result.push((binned, gate_id));
                }
            }
            result
        }
    };

    if series.is_empty() {
        return render_empty_histogram(options, width);
    }

    let series: Vec<(BinnedHistogram, u32)> = if options.scale_to_peak && series.len() > 1 {
        series
            .into_iter()
            .map(|(mut binned, gate_id)| {
                let max_count = binned.counts.iter().cloned().fold(0.0f64, f64::max);
                if max_count > 0.0 {
                    binned.counts.iter_mut().for_each(|c| *c /= max_count);
                }
                (binned, gate_id)
            })
            .collect()
    } else {
        series
    };

    let baseline_sep = options.baseline_separation as f64;
    let mut y_offset = 0.0f64;
    let mut plots: Vec<Plot> = Vec::new();

    for (binned, gate_id) in &series {
        let (r, g, b) = options.gate_color(*gate_id);
        let hex_color = format!("#{:02X}{:02X}{:02X}", r, g, b);

        let points: Vec<(f64, f64)> = binned
            .bin_centers
            .iter()
            .zip(binned.counts.iter())
            .map(|(x, c)| (*x, y_offset + *c))
            .collect();

        if points.is_empty() {
            y_offset += baseline_sep;
            continue;
        }

        let line = LinePlot::new()
            .with_data(points)
            .with_color(&hex_color);
        plots.push(line.into());

        y_offset += baseline_sep;
    }

    if plots.is_empty() {
        return render_empty_histogram(options, width);
    }

    let mut layout = Layout::auto_from_plots(&plots).with_title(&base.title);

    if let Some(ref label) = options.x_axis.label {
        layout = layout.with_x_label(label);
    } else {
        layout = layout.with_x_label("Value");
    }
    layout = layout.with_y_label("Count");

    let scale = (width as f32 / 800.0).max(1.0);
    let png_bytes = render_to_raster(plots, layout, scale)
        .map_err(|e| anyhow::anyhow!("failed to render histogram to PNG: {e}"))?;

    Ok(png_bytes)
}

fn render_empty_histogram(
    options: &crate::options::HistogramPlotOptions,
    width: u32,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();

    let scatter = ScatterPlot::new().with_data(vec![(0.0_f64, 0.0)]);
    let plots: Vec<Plot> = vec![scatter.into()];

    let mut layout = Layout::auto_from_plots(&plots).with_title(&base.title);
    if let Some(ref label) = options.x_axis.label {
        layout = layout.with_x_label(label);
    }
    layout = layout.with_y_label("Count");

    let scale = (width as f32 / 800.0).max(1.0);
    let png_bytes = render_to_raster(plots, layout, scale)
        .map_err(|e| anyhow::anyhow!("failed to render empty histogram to PNG: {e}"))?;

    Ok(png_bytes)
}
