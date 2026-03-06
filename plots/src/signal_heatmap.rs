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

/// Generate a heatmap visualization of signal intensity across channels
///
/// Shows a density distribution of events across intensity levels for each channel.
/// Each channel is a vertical column where color represents the density of events
/// at each intensity level (y-axis).
///
/// Returns PNG-encoded bytes rather than writing to a file.
pub fn generate_signal_heatmap(
    _signature_name: &str,
    detector_names: &[String],
    raw_signals: &HashMap<String, f32>,
    fcs_file_path: Option<&std::path::Path>,
    colormap: Option<ColorMaps>,
    unstained_medians: Option<&HashMap<String, f32>>,
    positive_medians: Option<&HashMap<String, f32>>,
    positive_geometric_means: Option<&HashMap<String, f32>>,
) -> Result<Vec<u8>> {
    use image::RgbImage;

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

    let _y_bin_size = (y_max - y_min) / n_y_bins as f32;

    // Build a pixel buffer directly using the image crate for the heatmap cells,
    // since kuva's high-level Heatmap may not support per-cell custom coloring from
    // our colormap. We render colormapped rectangles into an RGB pixel buffer.
    let mut pixel_buffer = vec![255u8; (width * height * 3) as usize];

    let margin_left = 80u32;
    let margin_right = 40u32;
    let margin_top = 40u32;
    let margin_bottom = 80u32;
    let plot_width = width - margin_left - margin_right;
    let plot_height = height - margin_top - margin_bottom;
    let n_channels = sorted_detector_names.len();

    let col_width = if n_channels > 0 {
        plot_width as f32 / n_channels as f32
    } else {
        plot_width as f32
    };

    for (idx, density) in channel_densities.iter().enumerate() {
        let x_start_px = margin_left as f32 + idx as f32 * col_width + col_width * 0.1;
        let x_end_px = margin_left as f32 + (idx + 1) as f32 * col_width - col_width * 0.1;

        for (bin_idx, &density_value) in density.iter().enumerate() {
            if density_value <= 0.0 {
                continue;
            }

            let normalized_log_density = if max_density > 0.0 {
                (density_value / max_density).min(1.0).max(0.0)
            } else {
                0.0
            };

            let inverted_density = 1.0 - normalized_log_density;
            let color = colormap.map(inverted_density);

            let y_frac_bottom = bin_idx as f32 / n_y_bins as f32;
            let y_frac_top = (bin_idx + 1) as f32 / n_y_bins as f32;
            let y_px_top = margin_top as f32 + (1.0 - y_frac_top) * plot_height as f32;
            let y_px_bottom = margin_top as f32 + (1.0 - y_frac_bottom) * plot_height as f32;

            for py in (y_px_top as u32)..(y_px_bottom as u32).min(height) {
                for px in (x_start_px as u32)..(x_end_px as u32).min(width) {
                    let pixel_idx = ((py * width + px) * 3) as usize;
                    if pixel_idx + 2 < pixel_buffer.len() {
                        pixel_buffer[pixel_idx] = color.0;
                        pixel_buffer[pixel_idx + 1] = color.1;
                        pixel_buffer[pixel_idx + 2] = color.2;
                    }
                }
            }
        }
    }

    // Draw overlay lines for geometric means and medians into the pixel buffer
    if let Some(positive_geo) = positive_geometric_means {
        let geo_points: Vec<(usize, f32)> = sorted_detector_names
            .iter()
            .enumerate()
            .filter_map(|(idx, det_name)| {
                positive_geo
                    .get(det_name)
                    .map(|&val| (idx, val))
            })
            .collect();

        draw_overlay_line(
            &mut pixel_buffer,
            width,
            &geo_points,
            (255, 165, 0),
            margin_left,
            margin_top,
            plot_width,
            plot_height,
            col_width,
            y_min,
            y_max,
        );
    }

    if let Some(positive_med) = positive_medians {
        let med_points: Vec<(usize, f32)> = sorted_detector_names
            .iter()
            .enumerate()
            .filter_map(|(idx, det_name)| {
                positive_med
                    .get(det_name)
                    .map(|&val| (idx, val))
            })
            .collect();

        draw_overlay_line(
            &mut pixel_buffer,
            width,
            &med_points,
            (0, 0, 255),
            margin_left,
            margin_top,
            plot_width,
            plot_height,
            col_width,
            y_min,
            y_max,
        );
    }

    if let Some(unstained) = unstained_medians {
        let unstained_points: Vec<(usize, f32)> = sorted_detector_names
            .iter()
            .enumerate()
            .filter_map(|(idx, det_name)| {
                unstained.get(det_name).map(|&median| {
                    let transformed_median = arcsinh_transform.transform(&median);
                    (idx, transformed_median)
                })
            })
            .collect();

        draw_overlay_line(
            &mut pixel_buffer,
            width,
            &unstained_points,
            (180, 180, 180),
            margin_left,
            margin_top,
            plot_width,
            plot_height,
            col_width,
            y_min,
            y_max,
        );
    }

    let img: RgbImage = image::ImageBuffer::from_vec(width, height, pixel_buffer)
        .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;

    let mut encoded_data = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut encoded_data);
    image::ImageEncoder::write_image(
        encoder,
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| anyhow::anyhow!("failed to PNG encode plot: {e}"))?;

    Ok(encoded_data)
}

/// Draw an overlay line connecting points on the heatmap pixel buffer
fn draw_overlay_line(
    buffer: &mut [u8],
    img_width: u32,
    points: &[(usize, f32)],
    color: (u8, u8, u8),
    margin_left: u32,
    margin_top: u32,
    _plot_width: u32,
    plot_height: u32,
    col_width: f32,
    y_min: f32,
    y_max: f32,
) {
    if points.len() < 2 {
        return;
    }

    let y_range = y_max - y_min;
    if y_range <= 0.0 {
        return;
    }

    let to_pixel = |ch_idx: usize, val: f32| -> (i32, i32) {
        let px = margin_left as f32 + ch_idx as f32 * col_width + col_width * 0.5;
        let y_frac = (val - y_min) / y_range;
        let py = margin_top as f32 + (1.0 - y_frac) * plot_height as f32;
        (px as i32, py as i32)
    };

    for i in 0..points.len() - 1 {
        let (x0, y0) = to_pixel(points[i].0, points[i].1);
        let (x1, y1) = to_pixel(points[i + 1].0, points[i + 1].1);
        draw_line_bresenham(buffer, img_width, x0, y0, x1, y1, color);
    }
}

/// Draw a line using Bresenham's algorithm
fn draw_line_bresenham(
    buffer: &mut [u8],
    img_width: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8),
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;

    loop {
        if cx >= 0 && cy >= 0 {
            let px = cx as u32;
            let py = cy as u32;
            let pixel_idx = ((py * img_width + px) * 3) as usize;
            if pixel_idx + 2 < buffer.len() {
                buffer[pixel_idx] = color.0;
                buffer[pixel_idx + 1] = color.1;
                buffer[pixel_idx + 2] = color.2;
                // Draw 2px wide line
                if px + 1 < img_width {
                    let idx2 = pixel_idx + 3;
                    if idx2 + 2 < buffer.len() {
                        buffer[idx2] = color.0;
                        buffer[idx2 + 1] = color.1;
                        buffer[idx2 + 2] = color.2;
                    }
                }
            }
        }

        if cx == x1 && cy == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Generate normalized spectral signature line plot
///
/// Shows the normalized signature (0-1 range) as a line plot connecting peaks across channels.
/// If detector_signals is empty, calculates normalized signature from FCS file.
///
/// Returns PNG-encoded bytes rather than writing to a file.
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
