//! **Deprecated:** Prefer the kuva-based rendering API (`flow_plots::render::render_pixels` /
//! `flow_plots::render::render_contour`, backed by the `kuva` crate when the `raster` feature is enabled).
//! This module is kept for compatibility and will be removed in a future release.

use crate::contour::ContourData;
use crate::PlotBytes;
use crate::create_axis_specs;
use crate::density_calc::RawPixelData;
use crate::options::DensityPlotOptions;
use crate::render::{ProgressInfo, RenderConfig};
use flow_fcs::{TransformType, Transformable};
use plotters::prelude::*;

/// Format a value using the transform type
///
/// This replicates the Formattable::format logic since the trait is not exported.
fn format_transform_value(transform: &TransformType, value: &f32) -> String {
    match transform {
        TransformType::Linear => format!("{:.1e}", value),
        TransformType::Arcsinh { cofactor: _ } => {
            // Convert from transformed space back to original space
            let original_value = transform.inverse_transform(value);
            // Make nice rounded labels in original space
            format!("{:.1e}", original_value)
        }
        TransformType::Biexponential { .. } => {
            // Convert from transformed space back to original space using inverse transform
            let original_value = transform.inverse_transform(value);
            // Make nice rounded labels in original space
            format!("{:.1e}", original_value)
        }
    }
}
use anyhow::Result;
use image::RgbImage;
use plotters::{
    backend::BitMapBackend, chart::ChartBuilder, prelude::IntoDrawingArea, style::WHITE,
};

/// Render pixels to a JPEG image using the Plotters backend.
///
/// **Deprecated:** Use [`crate::render::render_pixels`] instead.
///
/// This function handles the complete rendering pipeline:
/// 1. Sets up Plotters chart with axes and mesh
/// 2. Writes pixels directly to the buffer
/// 3. Encodes to JPEG format
///
/// Progress reporting is handled via the RenderConfig if provided.
#[deprecated(
    since = "0.3.2",
    note = "Use flow_plots::render::render_pixels instead. This plotters backend will be removed once kuva-backed rendering is implemented; see flow_plots::render::kuva_backend module doc."
)]
pub fn render_pixels(
    pixels: Vec<RawPixelData>,
    options: &DensityPlotOptions,
    render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;
    let height = base.height;
    let margin = base.margin;
    let x_label_area_size = base.x_label_area_size;
    let y_label_area_size = base.y_label_area_size;

    let setup_start = std::time::Instant::now();
    // Use RGB buffer (3 bytes per pixel) since we'll encode to JPEG which doesn't support alpha
    let mut pixel_buffer = vec![255; (width * height * 3) as usize];

    let (plot_x_range, plot_y_range, x_spec, y_spec) = {
        let backend = BitMapBackend::with_buffer(&mut pixel_buffer, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| anyhow::anyhow!("failed to fill plot background: {e}"))?;

        // Create appropriate ranges based on transform types
        let (x_spec, y_spec) = create_axis_specs(
            &options.x_axis.range,
            &options.y_axis.range,
            &options.x_axis.transform,
            &options.y_axis.transform,
        )?;

        let mut chart = ChartBuilder::on(&root)
            .margin(margin)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(x_spec.start..x_spec.end, y_spec.start..y_spec.end)?;

        // Clone transforms to avoid lifetime issues with closures
        let x_transform_clone = options.x_axis.transform.clone();
        let y_transform_clone = options.y_axis.transform.clone();

        // Create owned closures for formatters
        let x_formatter =
            move |x: &f32| -> String { format_transform_value(&x_transform_clone, x) };
        let y_formatter =
            move |y: &f32| -> String { format_transform_value(&y_transform_clone, y) };

        let mut mesh = chart.configure_mesh();
        mesh.x_max_light_lines(4)
            .y_max_light_lines(4)
            .x_labels(10)
            .y_labels(10)
            .x_label_formatter(&x_formatter)
            .y_label_formatter(&y_formatter);

        // Add axis labels if provided
        if let Some(ref x_label) = options.x_axis.label {
            mesh.x_desc(x_label);
        }
        if let Some(ref y_label) = options.y_axis.label {
            mesh.y_desc(y_label);
        }

        let mesh_start = std::time::Instant::now();
        mesh.draw()
            .map_err(|e| anyhow::anyhow!("failed to draw plot mesh: {e}"))?;
        eprintln!("    ├─ Mesh drawing: {:?}", mesh_start.elapsed());

        // Get the plotting area bounds (we'll use these after Plotters releases the buffer)
        let plotting_area = chart.plotting_area();
        let (plot_x_range, plot_y_range) = plotting_area.get_pixel_range();

        root.present()
            .map_err(|e| anyhow::anyhow!("failed to present plotters buffer: {e}"))?;

        (plot_x_range, plot_y_range, x_spec, y_spec)
    }; // End Plotters scope - pixel_buffer is now released and we can write to it

    // DIRECT PIXEL BUFFER WRITING - 10-50x faster than Plotters series rendering
    // Now that Plotters has released pixel_buffer, we can write directly
    let series_start = std::time::Instant::now();

    let plot_x_start = plot_x_range.start as f32;
    let plot_y_start = plot_y_range.start as f32;
    let plot_width = (plot_x_range.end - plot_x_range.start) as f32;
    let plot_height = (plot_y_range.end - plot_y_range.start) as f32;

    // Calculate scale factors from data coordinates to screen pixels
    let data_width = x_spec.end - x_spec.start;
    let data_height = y_spec.end - y_spec.start;

    // Stream pixel chunks during rendering using configurable chunk size
    let mut pixel_count = 0;
    let total_pixels = pixels.len();
    let chunk_size = 1000; // Default chunk size for progress reporting

    // Write each pixel directly to the buffer
    for pixel in &pixels {
        let data_x = pixel.x;
        let data_y = pixel.y;

        // Transform data coordinates to screen pixel coordinates
        let rel_x = (data_x - x_spec.start) / data_width;
        let rel_y = (y_spec.end - data_y) / data_height; // Flip Y (screen coords go down)

        let screen_x = (plot_x_start + rel_x * plot_width) as i32;
        let screen_y = (plot_y_start + rel_y * plot_height) as i32;

        // Bounds check
        if screen_x >= plot_x_range.start
            && screen_x < plot_x_range.end
            && screen_y >= plot_y_range.start
            && screen_y < plot_y_range.end
        {
            let px = screen_x as u32;
            let py = screen_y as u32;

            // Write to pixel buffer (RGB format - 3 bytes per pixel)
            let idx = ((py * width + px) * 3) as usize;

            if idx + 2 < pixel_buffer.len() {
                pixel_buffer[idx] = pixel.r;
                pixel_buffer[idx + 1] = pixel.g;
                pixel_buffer[idx + 2] = pixel.b;
            }
        }

        pixel_count += 1;

        // Emit progress every chunk_size pixels
        if pixel_count % chunk_size == 0 || pixel_count == total_pixels {
            let percent = (pixel_count as f32 / total_pixels as f32) * 100.0;

            // Create a small sample of pixels for this chunk (for visualization)
            let chunk_start = (pixel_count - chunk_size.min(pixel_count)).max(0);
            let chunk_end = pixel_count;
            let chunk_pixels: Vec<RawPixelData> = pixels
                .iter()
                .skip(chunk_start)
                .take(chunk_end - chunk_start)
                .map(|p| RawPixelData {
                    x: p.x,
                    y: p.y,
                    r: p.r,
                    g: p.g,
                    b: p.b,
                })
                .collect();

            render_config.report_progress(ProgressInfo {
                pixels: chunk_pixels,
                percent,
            });
        }
    }

    eprintln!(
        "    ├─ Direct pixel writing: {:?} ({} pixels)",
        series_start.elapsed(),
        pixels.len()
    );
    eprintln!("    ├─ Total plotting: {:?}", setup_start.elapsed());

    let img_start = std::time::Instant::now();
    let img: RgbImage = image::ImageBuffer::from_vec(width, height, pixel_buffer)
        .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;
    eprintln!("    ├─ Image buffer conversion: {:?}", img_start.elapsed());

    let encode_start = std::time::Instant::now();

    // Pre-allocate Vec with estimated JPEG size
    // RGB buffer is (width * height * 3) bytes
    // JPEG at quality 85 typically compresses to ~10-15% of raw size for density plots
    let raw_size = (width * height * 3) as usize;
    let estimated_jpeg_size = raw_size / 8; // Conservative estimate (~12.5% of raw)
    let mut encoded_data = Vec::with_capacity(estimated_jpeg_size);

    // JPEG encoding is faster and produces smaller files for density plots
    // Quality 85 provides good visual quality with ~2x smaller file size vs PNG
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded_data, 85);
    encoder
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("failed to JPEG encode plot: {e}"))?;
    eprintln!("    └─ JPEG encoding: {:?}", encode_start.elapsed());

    // Return the JPEG-encoded bytes directly
    Ok(encoded_data)
}

/// Render contour plot to JPEG using Plotters LineSeries.
///
/// **Deprecated:** Use [`crate::render::render_contour`] instead.
///
/// Draws contour lines from KDE density estimation plus optional outlier scatter points.
#[deprecated(
    since = "0.3.2",
    note = "Use flow_plots::render::render_contour instead. This plotters backend will be removed once kuva-backed rendering is implemented; see flow_plots::render::kuva_backend module doc."
)]
pub fn render_contour(
    contour_data: ContourData,
    options: &DensityPlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;

    let base = options.base();
    let width = base.width;
    let height = base.height;
    let margin = base.margin;
    let x_label_area_size = base.x_label_area_size;
    let y_label_area_size = base.y_label_area_size;

    let (x_spec, y_spec) = create_axis_specs(
        &options.x_axis.range,
        &options.y_axis.range,
        &options.x_axis.transform,
        &options.y_axis.transform,
    )?;

    let mut pixel_buffer = vec![255; (width * height * 3) as usize];

    {
        let backend = BitMapBackend::with_buffer(&mut pixel_buffer, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| anyhow::anyhow!("failed to fill plot background: {e}"))?;

        let x_transform_clone = options.x_axis.transform.clone();
        let y_transform_clone = options.y_axis.transform.clone();
        let x_formatter = move |x: &f64| -> String {
            format_transform_value(&x_transform_clone, &(*x as f32))
        };
        let y_formatter = move |y: &f64| -> String {
            format_transform_value(&y_transform_clone, &(*y as f32))
        };

        let mut chart = ChartBuilder::on(&root)
            .margin(margin)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(
                x_spec.start as f64..x_spec.end as f64,
                y_spec.start as f64..y_spec.end as f64,
            )?;

        let mut mesh = chart.configure_mesh();
        mesh.x_max_light_lines(4)
            .y_max_light_lines(4)
            .x_labels(10)
            .y_labels(10)
            .x_label_formatter(&x_formatter)
            .y_label_formatter(&y_formatter);
        if let Some(ref x_label) = options.x_axis.label {
            mesh.x_desc(x_label);
        }
        if let Some(ref y_label) = options.y_axis.label {
            mesh.y_desc(y_label);
        }
        mesh.draw()
            .map_err(|e| anyhow::anyhow!("failed to draw plot mesh: {e}"))?;

        let stroke_width = options.contour_line_thickness.max(0.5).min(5.0) as u32;
        let contour_color = RGBColor(60, 60, 60);

        // Chart axis bounds for defensive clamping.  Even though
        // calculate_contours now clips paths, belt-and-suspenders clamping
        // here protects against any code path that supplies raw ContourData
        // directly.  Plotters panics with integer overflow when coordinates
        // are far outside the chart range.
        let x_lo = x_spec.start as f64;
        let x_hi = x_spec.end as f64;
        let y_lo = y_spec.start as f64;
        let y_hi = y_spec.end as f64;

        // Draw contour lines
        for path in &contour_data.contours {
            if path.len() < 2 {
                continue;
            }
            let points: Vec<(f64, f64)> = path
                .iter()
                .map(|&(x, y)| (x.clamp(x_lo, x_hi), y.clamp(y_lo, y_hi)))
                .collect();
            chart
                .draw_series(LineSeries::new(
                    points,
                    contour_color.stroke_width(stroke_width),
                ))
                .map_err(|e| anyhow::anyhow!("failed to draw contour: {e}"))?;
        }

        // Draw outlier points if present
        if !contour_data.outliers.is_empty() {
            let outlier_color = RGBColor(150, 150, 150);
            chart
                .draw_series(
                    contour_data
                        .outliers
                        .iter()
                        .map(|&(x, y)| {
                            Circle::new(
                                (x.clamp(x_lo, x_hi), y.clamp(y_lo, y_hi)),
                                2,
                                outlier_color.filled(),
                            )
                        }),
                )
                .map_err(|e| anyhow::anyhow!("failed to draw outliers: {e}"))?;
        }

        root.present()
            .map_err(|e| anyhow::anyhow!("failed to present plotters buffer: {e}"))?;
    }

    let img: RgbImage =
        image::ImageBuffer::from_vec(width, height, pixel_buffer)
            .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;

    let mut encoded_data = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded_data, 85);
    encoder
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("failed to JPEG encode plot: {e}"))?;

    Ok(encoded_data)
}

/// Render spectral signature plot to JPEG
///
/// Creates a line plot showing normalized spectral signatures (0.0 to 1.0) across detector channels.
pub fn render_spectral_signature(
    data: (Vec<(usize, f64)>, Vec<String>),
    options: &crate::options::spectral::SpectralSignaturePlotOptions,
    _render_config: &mut RenderConfig,
) -> Result<PlotBytes> {
    use crate::options::PlotOptions;
    use plotters::prelude::*;

    let (spectrum_data, channel_names) = data;
    let base = options.base();
    let width = base.width;
    let height = base.height;
    let margin = base.margin;
    // Reserve enough space for rotated x-axis labels when showing every channel
    let x_label_area_size = if channel_names.len() > 10 {
        base.x_label_area_size.max(80)
    } else {
        base.x_label_area_size
    };
    let y_label_area_size = base.y_label_area_size;

    // Create RGB buffer
    let mut pixel_buffer = vec![255; (width * height * 3) as usize];

    // Determine x and y ranges (use f32 to match plotters expectations)
    let x_min = 0.0f32;
    let x_max = spectrum_data
        .iter()
        .map(|(idx, _)| *idx as f32)
        .fold(0.0f32, f32::max)
        .max(1.0);
    let y_min = 0.0f32;
    let y_max = 1.0f32;

    // Clone channel_names for the closure
    let channel_names_clone = channel_names.clone();

    {
        let backend = BitMapBackend::with_buffer(&mut pixel_buffer, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| anyhow::anyhow!("failed to fill plot background: {e}"))?;

        let mut chart = ChartBuilder::on(&root)
            .margin(margin)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)
            .map_err(|e| anyhow::anyhow!("failed to build chart: {e}"))?;

        // Create formatter for x-axis labels if channel names are provided
        let x_formatter: Option<Box<dyn Fn(&f32) -> String>> = if !channel_names_clone.is_empty()
            && channel_names_clone.len() == spectrum_data.len()
        {
            let channel_names_for_formatter = channel_names_clone.clone();
            Some(Box::new(move |x: &f32| -> String {
                // Find closest channel index
                let idx = x.round() as usize;
                if idx < channel_names_for_formatter.len() {
                    channel_names_for_formatter[idx].clone()
                } else {
                    format!("{:.0}", x)
                }
            }))
        } else {
            None
        };

        // Configure mesh
        let mut mesh = chart.configure_mesh();
        if options.show_grid {
            mesh.x_max_light_lines(4).y_max_light_lines(4);
        }

        // Set axis labels
        if let Some(ref x_axis) = options.x_axis {
            if let Some(ref label) = x_axis.label {
                mesh.x_desc(label);
            }
        } else {
            mesh.x_desc("Channel");
        }

        if let Some(ref y_axis) = options.y_axis {
            if let Some(ref label) = y_axis.label {
                mesh.y_desc(label);
            }
        } else {
            mesh.y_desc("Normalized Intensity");
        }

        // Apply x-axis formatter if provided
        if let Some(ref formatter) = x_formatter {
            mesh.x_label_formatter(formatter);
        }

        // Show every channel bin on the x-axis; rotate labels 90° so they fit without overlapping
        let x_label_count = if !channel_names_clone.is_empty() {
            channel_names_clone.len()
        } else {
            10
        };

        mesh.x_labels(x_label_count)
            .y_labels(10);

        // Rotate x-axis labels 90° when showing all channels (plotters has no 45° option)
        if x_label_count > 1 {
            use plotters::style::{FontTransform, TextStyle};
            let rotated = TextStyle::from(("sans-serif", 12).into_font())
                .transform(FontTransform::Rotate90);
            mesh.x_label_style(rotated);
        }

        mesh.draw()
            .map_err(|e| anyhow::anyhow!("failed to draw mesh: {e}"))?;

        // Draw the spectral signature line
        if !spectrum_data.is_empty() {
            // Parse hex color (e.g., "#1f77b4" or "1f77b4")
            let line_color = if options.line_color.starts_with('#') {
                let hex = &options.line_color[1..];
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(31);
                    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(119);
                    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(180);
                    RGBColor(r, g, b)
                } else {
                    RGBColor(31, 119, 180) // Default blue
                }
            } else {
                RGBColor(31, 119, 180) // Default blue
            };

            chart
                .draw_series(LineSeries::new(
                    spectrum_data
                        .iter()
                        .map(|(idx, val)| (*idx as f32, *val as f32)),
                    line_color.stroke_width(options.line_width as u32),
                ))
                .map_err(|e| anyhow::anyhow!("failed to draw line series: {e}"))?;
        }
    } // Backend is dropped here, pixel_buffer is now available

    // Convert to image and encode
    let img: RgbImage = image::ImageBuffer::from_vec(width, height, pixel_buffer)
        .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;

    let mut encoded_data = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded_data, 85);
    encoder
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("failed to JPEG encode plot: {e}"))?;

    Ok(encoded_data)
}

/// Render histogram plot to JPEG
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
    use plotters::prelude::*;

    let base = options.base();
    let width = base.width;
    let height = base.height;
    let margin = base.margin;
    let x_label_area_size = base.x_label_area_size;
    let y_label_area_size = base.y_label_area_size;

    let x_min = *options.x_axis.range.start() as f64;
    let x_max = *options.x_axis.range.end() as f64;

    // Convert all data to binned series (list of (BinnedHistogram, gate_id))
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
        // Empty plot - still render axes
        return render_empty_histogram(
            options, width, height, margin, x_label_area_size, y_label_area_size, x_min, x_max,
        );
    }

    // Optional: scale each series to its peak (max = 1.0)
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

    // Compute y range
    let (y_min, y_max) = if options.baseline_separation > 0.0 && series.len() > 1 {
        // Stacked: each series gets baseline_separation offset
        let mut max_y = 0.0f64;
        let mut cumulative_offset = 0.0f64;
        for (binned, _) in &series {
            let peak = binned.counts.iter().cloned().fold(0.0f64, f64::max);
            max_y = max_y.max(cumulative_offset + peak);
            cumulative_offset += options.baseline_separation as f64;
        }
        (0.0, (max_y * 1.05).max(0.1))
    } else {
        let global_max = series
            .iter()
            .flat_map(|(b, _)| b.counts.iter())
            .cloned()
            .fold(0.0f64, f64::max);
        (0.0, (global_max * 1.05).max(0.1))
    };

    let mut pixel_buffer = vec![255; (width * height * 3) as usize];

    {
        let backend = BitMapBackend::with_buffer(&mut pixel_buffer, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| anyhow::anyhow!("failed to fill plot background: {e}"))?;

        let mut chart = ChartBuilder::on(&root)
            .margin(margin)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)
            .map_err(|e| anyhow::anyhow!("failed to build histogram chart: {e}"))?;

        let mut mesh = chart.configure_mesh();
        mesh.x_max_light_lines(4).y_max_light_lines(4)
            .x_labels(10)
            .y_labels(10);

        if let Some(ref label) = options.x_axis.label {
            mesh.x_desc(label);
        } else {
            mesh.x_desc("Value");
        }
        mesh.y_desc("Count");

        mesh.draw()
            .map_err(|e| anyhow::anyhow!("failed to draw mesh: {e}"))?;

        let baseline_sep = options.baseline_separation as f64;
        let mut y_offset = 0.0f64;

        for (binned, gate_id) in &series {
            let (r, g, b) = options.gate_color(*gate_id);
            let color = RGBColor(r, g, b);
            let fill_color = RGBColor(r, g, b).mix(0.3);

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

            if options.histogram_filled {
                chart
                    .draw_series(AreaSeries::new(
                        points.iter().copied(),
                        y_offset,
                        fill_color,
                    ))
                    .map_err(|e| anyhow::anyhow!("failed to draw area series: {e}"))?;
                // Draw border line on top
                chart
                    .draw_series(LineSeries::new(
                        points.iter().copied(),
                        color.stroke_width(options.line_width as u32),
                    ))
                    .map_err(|e| anyhow::anyhow!("failed to draw histogram line: {e}"))?;
            } else {
                chart
                    .draw_series(LineSeries::new(
                        points.iter().copied(),
                        color.stroke_width(options.line_width as u32),
                    ))
                    .map_err(|e| anyhow::anyhow!("failed to draw histogram line: {e}"))?;
            }

            y_offset += baseline_sep;
        }
    }

    let img: RgbImage = image::ImageBuffer::from_vec(width, height, pixel_buffer)
        .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;

    let mut encoded_data = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded_data, 85);
    encoder
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("failed to JPEG encode plot: {e}"))?;

    Ok(encoded_data)
}

fn render_empty_histogram(
    options: &crate::options::HistogramPlotOptions,
    width: u32,
    height: u32,
    margin: u32,
    x_label_area_size: u32,
    y_label_area_size: u32,
    x_min: f64,
    x_max: f64,
) -> Result<PlotBytes> {
    use plotters::prelude::*;

    let mut pixel_buffer = vec![255; (width * height * 3) as usize];

    {
        let backend = BitMapBackend::with_buffer(&mut pixel_buffer, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| anyhow::anyhow!("failed to fill plot background: {e}"))?;

        let mut chart = ChartBuilder::on(&root)
            .margin(margin)
            .x_label_area_size(x_label_area_size)
            .y_label_area_size(y_label_area_size)
            .build_cartesian_2d(x_min..x_max, 0.0f64..1.0f64)
            .map_err(|e| anyhow::anyhow!("failed to build histogram chart: {e}"))?;

        let mut mesh = chart.configure_mesh();
        mesh.x_max_light_lines(4).y_max_light_lines(4);
        if let Some(ref label) = options.x_axis.label {
            mesh.x_desc(label);
        }
        mesh.y_desc("Count")
            .draw()
            .map_err(|e| anyhow::anyhow!("failed to draw mesh: {e}"))?;
    }

    let img: RgbImage = image::ImageBuffer::from_vec(width, height, pixel_buffer)
        .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;

    let mut encoded_data = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded_data, 85);
    encoder
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow::anyhow!("failed to JPEG encode plot: {e}"))?;

    Ok(encoded_data)
}
