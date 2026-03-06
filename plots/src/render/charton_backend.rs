// NOTE: charton 0.2.1 depends on polars ^0.49.  The workspace uses polars 0.53
// via flow-fcs.  Cargo can link both versions, but the polars types here (0.49)
// are NOT interchangeable with the workspace polars types (0.53).  If this
// causes unresolvable build errors, the charton dependency version needs to be
// updated upstream to support polars 0.53+.

use crate::contour::ContourData;
use crate::density_calc::RawPixelData;
use crate::options::DensityPlotOptions;
use crate::render::{ProgressInfo, RenderConfig};
use crate::PlotBytes;
use anyhow::Result;
use charton::prelude::*;
use image::RgbImage;
use polars::prelude::*;

/// Convert an SVG string to JPEG-encoded bytes.
///
/// Uses `resvg` to rasterize the SVG, then `image` to encode as JPEG.
fn svg_to_jpeg_bytes(svg_str: &str, width: u32, height: u32) -> Result<Vec<u8>> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg_str, &opts)
        .map_err(|e| anyhow::anyhow!("failed to parse SVG: {e}"))?;

    let int_size = tree.size().to_int_size();
    let target_w = if width > 0 { width } else { int_size.width() };
    let target_h = if height > 0 { height } else { int_size.height() };

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap {}x{}", target_w, target_h))?;

    pixmap.fill(tiny_skia::Color::WHITE);

    let sx = target_w as f32 / int_size.width() as f32;
    let sy = target_h as f32 / int_size.height() as f32;
    let transform = tiny_skia::Transform::from_scale(sx, sy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba_data = pixmap.data();
    let mut rgb_data = Vec::with_capacity((target_w * target_h * 3) as usize);
    for chunk in rgba_data.chunks(4) {
        rgb_data.push(chunk[0]);
        rgb_data.push(chunk[1]);
        rgb_data.push(chunk[2]);
    }

    let img: RgbImage = image::ImageBuffer::from_vec(target_w, target_h, rgb_data)
        .ok_or_else(|| anyhow::anyhow!("RGB buffer size mismatch"))?;

    let mut encoded = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85);
    encoder
        .encode(img.as_raw(), target_w, target_h, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("JPEG encoding failed: {e}"))?;

    Ok(encoded)
}

/// Render density / scatter pixels to JPEG via charton.
///
/// Converts pixel data into a Polars DataFrame, creates a scatter chart using
/// `mark_point()`, renders to SVG, and encodes to JPEG.
pub fn render_pixels(
    pixels: Vec<RawPixelData>,
    options: &DensityPlotOptions,
    render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;
    let height = base.height;

    let setup_start = std::time::Instant::now();

    // Report progress
    let total_pixels = pixels.len();
    let chunk_size = 1000;
    let mut pixel_count = 0;

    for chunk in pixels.chunks(chunk_size) {
        pixel_count += chunk.len();
        let percent = (pixel_count as f32 / total_pixels.max(1) as f32) * 100.0;
        render_config.report_progress(ProgressInfo {
            pixels: chunk.to_vec(),
            percent,
        });
    }

    if pixels.is_empty() {
        let df = DataFrame::new(vec![
            Column::new("x".into(), vec![0.0f64]),
            Column::new("y".into(), vec![0.0f64]),
        ])
        .map_err(|e| anyhow::anyhow!("failed to create empty DataFrame: {e}"))?;

        let svg = Chart::build(&df)?
            .mark_point()
            .encode((x("x"), y("y")))?
            .into_layered()
            .with_size(width, height)
            .to_svg()?;

        return svg_to_jpeg_bytes(&svg, width, height);
    }

    let xs: Vec<f64> = pixels.iter().map(|p| p.x as f64).collect();
    let ys: Vec<f64> = pixels.iter().map(|p| p.y as f64).collect();
    let colors: Vec<String> = pixels
        .iter()
        .map(|p| format!("#{:02x}{:02x}{:02x}", p.r, p.g, p.b))
        .collect();

    let df = DataFrame::new(vec![
        Column::new("x".into(), xs),
        Column::new("y".into(), ys),
        Column::new("color".into(), colors),
    ])
    .map_err(|e| anyhow::anyhow!("failed to create pixel DataFrame: {e}"))?;

    let mut layered = Chart::build(&df)?
        .mark_point()
        .encode((x("x"), y("y"), color("color")))?
        .into_layered()
        .with_size(width, height);

    if let Some(ref x_label) = options.x_axis.label {
        layered = layered.with_x_label(x_label);
    }
    if let Some(ref y_label) = options.y_axis.label {
        layered = layered.with_y_label(y_label);
    }

    let svg = layered.to_svg()?;

    eprintln!(
        "    ├─ Charton render ({} pixels): {:?}",
        total_pixels,
        setup_start.elapsed()
    );

    svg_to_jpeg_bytes(&svg, width, height)
}

/// Render contour plot to JPEG via charton.
///
/// Draws contour lines using `mark_line()` and outlier points using `mark_point()`.
pub fn render_contour(
    contour_data: ContourData,
    options: &DensityPlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;
    let height = base.height;

    let mut all_x = Vec::new();
    let mut all_y = Vec::new();
    let mut group_ids = Vec::new();

    for (group_idx, path) in contour_data.contours.iter().enumerate() {
        for &(px, py) in path {
            all_x.push(px);
            all_y.push(py);
            group_ids.push(group_idx as i64);
        }
    }

    let has_contours = !all_x.is_empty();
    let has_outliers = !contour_data.outliers.is_empty();

    if !has_contours && !has_outliers {
        let df = DataFrame::new(vec![
            Column::new("x".into(), vec![0.0f64]),
            Column::new("y".into(), vec![0.0f64]),
        ])
        .map_err(|e| anyhow::anyhow!("empty DataFrame: {e}"))?;

        let svg = Chart::build(&df)?
            .mark_point()
            .encode((x("x"), y("y")))?
            .into_layered()
            .with_size(width, height)
            .to_svg()?;

        return svg_to_jpeg_bytes(&svg, width, height);
    }

    if has_contours {
        let df_contours = DataFrame::new(vec![
            Column::new("x".into(), all_x),
            Column::new("y".into(), all_y),
            Column::new("group".into(), group_ids),
        ])
        .map_err(|e| anyhow::anyhow!("contour DataFrame: {e}"))?;

        let mut layered = Chart::build(&df_contours)?
            .mark_line()
            .encode((x("x"), y("y"), color("group")))?
            .into_layered()
            .with_size(width, height);

        if let Some(ref x_label) = options.x_axis.label {
            layered = layered.with_x_label(x_label);
        }
        if let Some(ref y_label) = options.y_axis.label {
            layered = layered.with_y_label(y_label);
        }

        let svg = layered.to_svg()?;
        return svg_to_jpeg_bytes(&svg, width, height);
    }

    // Only outliers
    let ox: Vec<f64> = contour_data.outliers.iter().map(|&(px, _)| px).collect();
    let oy: Vec<f64> = contour_data.outliers.iter().map(|&(_, py)| py).collect();

    let df_outliers = DataFrame::new(vec![
        Column::new("x".into(), ox),
        Column::new("y".into(), oy),
    ])
    .map_err(|e| anyhow::anyhow!("outlier DataFrame: {e}"))?;

    let mut layered = Chart::build(&df_outliers)?
        .mark_point()
        .encode((x("x"), y("y")))?
        .into_layered()
        .with_size(width, height);

    if let Some(ref x_label) = options.x_axis.label {
        layered = layered.with_x_label(x_label);
    }
    if let Some(ref y_label) = options.y_axis.label {
        layered = layered.with_y_label(y_label);
    }

    let svg = layered.to_svg()?;
    svg_to_jpeg_bytes(&svg, width, height)
}

/// Render spectral signature plot to JPEG via charton.
///
/// Creates a line plot (normalized intensity vs channel index).
pub fn render_spectral_signature(
    data: (Vec<(usize, f64)>, Vec<String>),
    options: &crate::options::spectral::SpectralSignaturePlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let (spectrum_data, channel_names) = data;
    let base = options.base();
    let width = base.width;
    let height = base.height;

    if spectrum_data.is_empty() {
        let df = DataFrame::new(vec![
            Column::new("channel".into(), vec![0.0f64]),
            Column::new("intensity".into(), vec![0.0f64]),
        ])
        .map_err(|e| anyhow::anyhow!("empty DataFrame: {e}"))?;

        let svg = Chart::build(&df)?
            .mark_line()
            .encode((x("channel"), y("intensity")))?
            .into_layered()
            .with_size(width, height)
            .to_svg()?;

        return svg_to_jpeg_bytes(&svg, width, height);
    }

    let channel_indices: Vec<f64> = spectrum_data.iter().map(|(idx, _)| *idx as f64).collect();
    let intensities: Vec<f64> = spectrum_data.iter().map(|(_, val)| *val).collect();

    let df = if !channel_names.is_empty() && channel_names.len() == spectrum_data.len() {
        DataFrame::new(vec![
            Column::new("channel".into(), channel_names.clone()),
            Column::new("intensity".into(), intensities),
        ])
        .map_err(|e| anyhow::anyhow!("spectral DataFrame: {e}"))?
    } else {
        DataFrame::new(vec![
            Column::new("channel".into(), channel_indices),
            Column::new("intensity".into(), intensities),
        ])
        .map_err(|e| anyhow::anyhow!("spectral DataFrame: {e}"))?
    };

    let mut layered = Chart::build(&df)?
        .mark_line()
        .encode((x("channel"), y("intensity")))?
        .into_layered()
        .with_size(width, height);

    if let Some(ref x_axis) = options.x_axis {
        if let Some(ref label) = x_axis.label {
            layered = layered.with_x_label(label);
        }
    } else {
        layered = layered.with_x_label("Channel");
    }

    if let Some(ref y_axis) = options.y_axis {
        if let Some(ref label) = y_axis.label {
            layered = layered.with_y_label(label);
        }
    } else {
        layered = layered.with_y_label("Normalized Intensity");
    }

    let svg = layered.to_svg()?;
    svg_to_jpeg_bytes(&svg, width, height)
}

/// Render histogram plot to JPEG via charton.
///
/// Creates a 1D histogram with optional filled area and overlaid series.
pub fn render_histogram(
    data: crate::histogram_data::HistogramData,
    options: &crate::options::HistogramPlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::histogram_data::{bin_values, BinnedHistogram, HistogramData, HistogramSeries};
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;
    let height = base.height;

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
        return render_empty_histogram(options, width, height);
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
    let mut all_x = Vec::new();
    let mut all_y = Vec::new();
    let mut series_ids = Vec::new();
    let mut y_offset = 0.0f64;

    for (binned, gate_id) in &series {
        for (bx, by) in binned.bin_centers.iter().zip(binned.counts.iter()) {
            all_x.push(*bx);
            all_y.push(y_offset + *by);
            series_ids.push(*gate_id as i64);
        }
        y_offset += baseline_sep;
    }

    let df = DataFrame::new(vec![
        Column::new("value".into(), all_x),
        Column::new("count".into(), all_y),
        Column::new("series".into(), series_ids),
    ])
    .map_err(|e| anyhow::anyhow!("histogram DataFrame: {e}"))?;

    // Use mark_area for filled histograms, mark_line for unfilled.
    // Since the two mark types produce different generic types, we render in
    // separate branches and return the SVG string from each.
    let svg = if options.histogram_filled {
        let mut layered = Chart::build(&df)?
            .mark_area()
            .encode((x("value"), y("count"), color("series")))?
            .into_layered()
            .with_size(width, height);

        if let Some(ref label) = options.x_axis.label {
            layered = layered.with_x_label(label);
        } else {
            layered = layered.with_x_label("Value");
        }
        layered = layered.with_y_label("Count");
        layered.to_svg()?
    } else {
        let mut layered = Chart::build(&df)?
            .mark_line()
            .encode((x("value"), y("count"), color("series")))?
            .into_layered()
            .with_size(width, height);

        if let Some(ref label) = options.x_axis.label {
            layered = layered.with_x_label(label);
        } else {
            layered = layered.with_x_label("Value");
        }
        layered = layered.with_y_label("Count");
        layered.to_svg()?
    };

    svg_to_jpeg_bytes(&svg, width, height)
}

fn render_empty_histogram(
    options: &crate::options::HistogramPlotOptions,
    width: u32,
    height: u32,
) -> Result<PlotBytes> {
    let df = DataFrame::new(vec![
        Column::new("value".into(), Vec::<f64>::new()),
        Column::new("count".into(), Vec::<f64>::new()),
    ])
    .map_err(|e| anyhow::anyhow!("empty histogram DataFrame: {e}"))?;

    let mut layered = Chart::build(&df)?
        .mark_line()
        .encode((x("value"), y("count")))?
        .into_layered()
        .with_size(width, height);

    if let Some(ref label) = options.x_axis.label {
        layered = layered.with_x_label(label);
    }
    layered = layered.with_y_label("Count");

    let svg = layered.to_svg()?;
    svg_to_jpeg_bytes(&svg, width, height)
}
