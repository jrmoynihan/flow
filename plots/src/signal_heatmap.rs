//! Signal heatmap and normalized spectral signature plot generation
//!
//! These functions generate visualizations for flow cytometry spectral signatures.
//! They return plot bytes rather than writing files directly, allowing callers
//! to handle file I/O as needed.

use anyhow::{Context, Result};
use flow_fcs::{Fcs, TransformType, Transformable};
use std::collections::HashMap;

use crate::colormap::ColorMaps;
use crate::plots::Plot;

/// Helper function to calculate geometric mean of positive values
fn calculate_geometric_mean(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }

    let positive_values: Vec<f32> = values.iter().filter(|&&v| v > 0.0).copied().collect();

    if positive_values.is_empty() {
        return None;
    }

    let log_sum: f64 = positive_values.iter().map(|&v| (v as f64).ln()).sum();
    let n = positive_values.len() as f64;
    Some((log_sum / n).exp() as f32)
}

/// Helper function to calculate median
fn _calculate_median(values: &[f32]) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Sort channels by laser type, then wavelength
/// Order: UV > V > B > YG > R, then by wavelength within each group
fn sort_channels_by_laser_and_wavelength(channels: &mut [String]) {
    fn get_laser_order(channel: &str) -> (u8, u32) {
        let upper = channel.to_uppercase();

        let laser_order = if upper.starts_with("UV") {
            1
        } else if upper.starts_with("V") && !upper.starts_with("UV") {
            2
        } else if upper.starts_with("B") {
            3
        } else if upper.starts_with("YG") {
            4
        } else if upper.starts_with("R") {
            5
        } else {
            99
        };

        let wavelength = if upper.starts_with("UV") {
            upper[2..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(9999)
        } else if upper.starts_with("YG") {
            upper[2..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(9999)
        } else if upper.starts_with("V") || upper.starts_with("B") || upper.starts_with("R") {
            upper[1..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(9999)
        } else {
            9999
        };

        (laser_order, wavelength)
    }

    channels.sort_by(|a, b| {
        let (order_a, wave_a) = get_laser_order(a);
        let (order_b, wave_b) = get_laser_order(b);

        match order_a.cmp(&order_b) {
            std::cmp::Ordering::Equal => wave_a.cmp(&wave_b),
            other => other,
        }
    });
}

/// Convert an SVG string to JPEG-encoded bytes (same helper as in charton_backend).
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

    let img: image::RgbImage = image::ImageBuffer::from_vec(target_w, target_h, rgb_data)
        .ok_or_else(|| anyhow::anyhow!("RGB buffer size mismatch"))?;

    let mut encoded = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85);
    encoder
        .encode(
            img.as_raw(),
            target_w,
            target_h,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| anyhow::anyhow!("JPEG encoding failed: {e}"))?;

    Ok(encoded)
}

/// Generate a heatmap visualization of signal intensity across channels
///
/// Shows a density distribution of events across intensity levels for each channel.
/// Each channel is a vertical column where color represents the density of events
/// at each intensity level (y-axis).
///
/// Returns JPEG-encoded bytes rather than writing to a file.
pub fn generate_signal_heatmap(
    _signature_name: &str,
    detector_names: &[String],
    raw_signals: &HashMap<String, f32>,
    fcs_file_path: Option<&std::path::Path>,
    colormap: Option<ColorMaps>,
    _unstained_medians: Option<&HashMap<String, f32>>,
    _positive_medians: Option<&HashMap<String, f32>>,
    _positive_geometric_means: Option<&HashMap<String, f32>>,
) -> Result<Vec<u8>> {
    use charton::prelude::*;
    use polars::prelude::*;

    let colormap = colormap.unwrap_or(ColorMaps::Spectral);

    let mut sorted_detector_names = detector_names.to_vec();
    sort_channels_by_laser_and_wavelength(&mut sorted_detector_names);

    let width = 1600u32;
    let height = 600u32;

    let n_y_bins = 200;

    let arcsinh_cofactor = 200.0f32;
    let arcsinh_transform = TransformType::Arcsinh {
        cofactor: arcsinh_cofactor,
    };

    let channel_densities: Vec<Vec<f32>>;
    let y_min: f32;
    let y_max: f32;
    let max_density: f32;

    if let Some(fcs_path) = fcs_file_path {
        let fcs = Fcs::open(fcs_path.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "FCS file path contains invalid UTF-8: {}",
                fcs_path.display()
            )
        })?)
        .with_context(|| format!("Failed to read FCS file: {}", fcs_path.display()))?;

        let mut global_min = f32::MAX;
        let mut global_max = f32::MIN;
        let mut global_max_raw = f32::MIN;

        for det_name in &sorted_detector_names {
            if let Ok(series) = fcs.data_frame.column(det_name) {
                if let Ok(f32_vals) = series.f32() {
                    for val_opt in f32_vals.iter() {
                        if let Some(val) = val_opt {
                            let transformed_val = arcsinh_transform.transform(&val);
                            global_min = global_min.min(transformed_val);
                            global_max = global_max.max(transformed_val);
                            global_max_raw = global_max_raw.max(val);
                        }
                    }
                }
            }
        }

        let max_signal_cap = 5_000_000.0f32;
        let cap_transformed = arcsinh_transform.transform(&max_signal_cap);

        let effective_max = if global_max_raw > max_signal_cap {
            global_max
        } else {
            cap_transformed
        };

        y_min = 0.0f32.max(global_min * 0.9);
        y_max = effective_max * 1.1;
        let y_bin_size = (y_max - y_min) / n_y_bins as f32;

        let mut densities: Vec<Vec<f32>> = Vec::new();

        for det_name in &sorted_detector_names {
            let mut density = vec![0.0f32; n_y_bins];

            if let Ok(series) = fcs.data_frame.column(det_name) {
                if let Ok(f32_vals) = series.f32() {
                    for val_opt in f32_vals.iter() {
                        if let Some(val) = val_opt {
                            let transformed_val = arcsinh_transform.transform(&val);
                            if transformed_val >= y_min && transformed_val <= y_max {
                                let bin_idx = (((transformed_val - y_min) / y_bin_size) as usize)
                                    .min(n_y_bins - 1);
                                if bin_idx < n_y_bins {
                                    density[bin_idx] += 1.0;
                                }
                            }
                        }
                    }
                }
            }

            densities.push(density);
        }

        let mut max_log_density = 0.0f32;
        for density in &mut densities {
            for count in density.iter_mut() {
                if *count > 0.0 {
                    *count = (*count + 1.0).log10();
                    max_log_density = max_log_density.max(*count);
                }
            }
        }
        max_log_density = max_log_density.max(1.0);

        channel_densities = densities;
        max_density = max_log_density;
    } else {
        let max_signal = raw_signals.values().fold(0.0f32, |a, &b| a.max(b)).max(1.0);
        let max_signal_cap = 5_000_000.0f32;

        let capped_signal = if max_signal > max_signal_cap {
            max_signal
        } else {
            max_signal_cap
        };

        let capped_transformed = arcsinh_transform.transform(&capped_signal);

        y_min = 0.0f32;
        y_max = capped_transformed * 1.1;
        let y_bin_size = (y_max - y_min) / n_y_bins as f32;

        let mut densities: Vec<Vec<f32>> = Vec::new();

        for det_name in &sorted_detector_names {
            let signal = raw_signals.get(det_name).copied().unwrap_or(0.0);
            let mut density = vec![0.0f32; n_y_bins];

            if signal > 0.0 {
                let std_dev = signal * 0.1;
                let mean = signal;

                for bin_idx in 0..n_y_bins {
                    let y_center = y_min + (bin_idx as f32 + 0.5) * y_bin_size;
                    let diff = (y_center - mean) / std_dev;
                    let density_value = (-0.5 * diff * diff).exp();
                    density[bin_idx] = density_value;
                }
            } else {
                let baseline_signal = 100.0;
                let std_dev = baseline_signal * 0.1;

                for bin_idx in 0..n_y_bins {
                    let y_center = y_min + (bin_idx as f32 + 0.5) * y_bin_size;
                    if y_center < baseline_signal * 2.0 {
                        let diff = (y_center - baseline_signal) / std_dev;
                        let density_value = (-0.5 * diff * diff).exp();
                        density[bin_idx] = density_value;
                    }
                }
            }

            densities.push(density);
        }

        let mut max_log_density = 0.0f32;
        for density in &mut densities {
            for count in density.iter_mut() {
                if *count > 0.0 {
                    *count = (*count + 1.0).log10();
                    max_log_density = max_log_density.max(*count);
                }
            }
        }
        max_log_density = max_log_density.max(1.0);

        channel_densities = densities;
        max_density = max_log_density;
    }

    let y_bin_size = (y_max - y_min) / n_y_bins as f32;

    // Build a DataFrame with columns: channel_idx, y_center, color_hex
    // Each row = one coloured rectangle in the heatmap.
    let mut rect_channel: Vec<f64> = Vec::new();
    let mut rect_y: Vec<f64> = Vec::new();
    let mut rect_y2: Vec<f64> = Vec::new();
    let mut rect_color: Vec<String> = Vec::new();

    for (idx, density) in channel_densities.iter().enumerate() {
        let x_center = idx as f64;

        for (bin_idx, &density_value) in density.iter().enumerate() {
            if density_value <= 0.0 {
                continue;
            }

            let y_bottom = y_min as f64 + bin_idx as f64 * y_bin_size as f64;
            let y_top = y_min as f64 + (bin_idx + 1) as f64 * y_bin_size as f64;

            let normalized_log_density = if max_density > 0.0 {
                (density_value / max_density).min(1.0).max(0.0)
            } else {
                0.0
            };

            let inverted_density = 1.0 - normalized_log_density;
            let c = colormap.map(inverted_density);

            rect_channel.push(x_center);
            rect_y.push(y_bottom);
            rect_y2.push(y_top);
            rect_color.push(format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2));
        }
    }

    if rect_channel.is_empty() {
        // Nothing to draw
        let df = DataFrame::new(vec![
            Column::new("channel".into(), vec![0.0f64]),
            Column::new("y".into(), vec![0.0f64]),
        ])
        .map_err(|e| anyhow::anyhow!("empty heatmap DataFrame: {e}"))?;

        let svg = Chart::build(&df)?
            .mark_point()
            .encode((x("channel"), y("y")))?
            .into_layered()
            .with_size(width, height)
            .to_svg()?;

        return svg_to_jpeg_bytes(&svg, width, height);
    }

    let df = DataFrame::new(vec![
        Column::new("channel".into(), rect_channel),
        Column::new("y".into(), rect_y),
        Column::new("y2".into(), rect_y2),
        Column::new("color".into(), rect_color),
    ])
    .map_err(|e| anyhow::anyhow!("heatmap DataFrame: {e}"))?;

    let layered = Chart::build(&df)?
        .mark_rect()
        .encode((x("channel"), y("y"), y2("y2"), color("color")))?
        .into_layered()
        .with_size(width, height)
        .with_x_label("Channel")
        .with_y_label("Signal Intensity (arcsinh transformed)");

    let svg = layered.to_svg()?;
    svg_to_jpeg_bytes(&svg, width, height)
}

/// Generate normalized spectral signature line plot
///
/// Shows the normalized signature (0-1 range) as a line plot connecting peaks across channels.
/// If detector_signals is empty, calculates normalized signature from FCS file.
///
/// Returns JPEG-encoded bytes rather than writing to a file.
pub fn generate_normalized_spectral_signature_plot(
    signature_name: &str,
    detector_names: &[String],
    detector_signals: &HashMap<String, f64>,
    fcs_file_path: Option<&std::path::Path>,
) -> Result<Vec<u8>> {
    let mut sorted_detector_names = detector_names.to_vec();
    sort_channels_by_laser_and_wavelength(&mut sorted_detector_names);

    let spectrum_data: Vec<(usize, f64)> = if detector_signals.is_empty() && fcs_file_path.is_some()
    {
        let fcs_path = fcs_file_path.ok_or_else(|| anyhow::anyhow!("FCS file path is None"))?;
        let fcs = Fcs::open(fcs_path.to_str().ok_or_else(|| {
            anyhow::anyhow!(
                "FCS file path contains invalid UTF-8: {}",
                fcs_path.display()
            )
        })?)
        .with_context(|| "Failed to read FCS file for normalized signature")?;

        let arcsinh_cofactor = 200.0f32;
        let arcsinh_transform = TransformType::Arcsinh {
            cofactor: arcsinh_cofactor,
        };

        let mut transformed_geometric_means = HashMap::new();
        for det_name in &sorted_detector_names {
            if let Ok(series) = fcs.data_frame.column(det_name) {
                if let Ok(f32_vals) = series.f32() {
                    let transformed_values: Vec<f32> = f32_vals
                        .iter()
                        .filter_map(|v| v.map(|x| arcsinh_transform.transform(&x)))
                        .collect();

                    if let Some(geo_mean) = calculate_geometric_mean(&transformed_values) {
                        transformed_geometric_means.insert(det_name.clone(), geo_mean);
                    }
                }
            }
        }

        let max_signal = transformed_geometric_means
            .values()
            .fold(0.0f32, |a, &b| a.max(b));

        sorted_detector_names
            .iter()
            .enumerate()
            .map(|(idx, det_name)| {
                let normalized = if max_signal > 0.0 {
                    transformed_geometric_means
                        .get(det_name)
                        .copied()
                        .unwrap_or(0.0)
                        / max_signal
                } else {
                    0.0
                };
                (idx, normalized as f64)
            })
            .collect()
    } else {
        sorted_detector_names
            .iter()
            .enumerate()
            .map(|(idx, det_name)| {
                let normalized = detector_signals.get(det_name).copied().unwrap_or(0.0);
                (idx, normalized)
            })
            .collect()
    };

    let channel_names = sorted_detector_names;

    let mut render_config = crate::render::RenderConfig::default();
    let plot = crate::plots::SpectralSignaturePlot::new();

    let base_opts = crate::options::BasePlotOptions::new()
        .width(1600u32)
        .height(600u32)
        .title(format!(
            "Normalized Spectral Signature - {}",
            signature_name
        ))
        .build()?;

    let options = crate::options::SpectralSignaturePlotOptions::new()
        .base(base_opts)
        .x_axis(Some(
            crate::options::AxisOptions::new()
                .label("Detector Channel".to_string())
                .build()?,
        ))
        .y_axis(Some(
            crate::options::AxisOptions::new()
                .label("Normalized Intensity (0.0 to 1.0)".to_string())
                .build()?,
        ))
        .line_color("#1f77b4".to_string())
        .line_width(2.5)
        .show_grid(true)
        .build()?;

    plot.render((spectrum_data, channel_names), &options, &mut render_config)
}
