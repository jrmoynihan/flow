use rustc_hash::FxHashMap;
use serde::Serialize;
use ts_rs::TS;

use crate::options::density::default_gate_colors;
use crate::options::{DensityPlotOptions, PlotOptions};
use crate::plots::PlotType;
use crate::scatter_data::ScatterPlotData;

/// Optimized density calculation with pixel-based rendering
///
/// Key optimizations (validated with Criterion benchmarks):
/// 1. FIX: Corrected y-axis scale calculation bug
/// 2. Eliminated overplotting: Creates ONE pixel per screen coordinate (not per data point)
/// 3. Array-based density building: 7x faster than HashMap (cache locality)
/// 4. Sequential processing: Parallel overhead dominates for typical FCS sizes
/// 5. Converts to sparse HashMap only for non-zero pixels
/// 6. Result: 10-50x faster, uses 5-10x less memory than previous implementation
/// Raw pixel data for direct buffer writing
#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct RawPixelData {
    pub x: f32,
    pub y: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Binary pixel chunk for efficient data transfer
/// Contains raw RGB data with metadata for direct canvas rendering
#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct BinaryPixelChunk {
    /// Raw RGB pixel data: [r,g,b,r,g,b,...] for each pixel
    pub pixels: Vec<u8>,
    /// Width of the pixel chunk
    pub width: u32,
    /// Height of the pixel chunk  
    pub height: u32,
    /// X offset within the full plot
    pub offset_x: u32,
    /// Y offset within the full plot
    pub offset_y: u32,
    /// Full plot width for coordinate context
    pub total_width: u32,
    /// Full plot height for coordinate context
    pub total_height: u32,
}

/// Convert a region of RawPixelData to a binary pixel chunk
/// This creates a rectangular region of pixels for efficient transfer
pub fn create_binary_chunk(
    pixels: &[RawPixelData],
    plot_width: u32,
    plot_height: u32,
    _chunk_size: u32,
) -> Option<BinaryPixelChunk> {
    if pixels.is_empty() {
        return None;
    }

    // Find bounding box of pixels
    let mut min_x = pixels[0].x;
    let mut max_x = pixels[0].x;
    let mut min_y = pixels[0].y;
    let mut max_y = pixels[0].y;

    for pixel in pixels {
        min_x = min_x.min(pixel.x);
        max_x = max_x.max(pixel.x);
        min_y = min_y.min(pixel.y);
        max_y = max_y.max(pixel.y);
    }

    // Convert to pixel coordinates (coords are already pixel-space floats)
    let chunk_x = min_x.max(0.0).floor() as u32;
    let chunk_y = min_y.max(0.0).floor() as u32;
    let chunk_width = (max_x - min_x).max(0.0).floor() as u32 + 1;
    let chunk_height = (max_y - min_y).max(0.0).floor() as u32 + 1;

    // Create RGB buffer for this chunk (use usize with saturation to avoid overflow)
    let total_px: usize = (chunk_width as usize)
        .saturating_mul(chunk_height as usize)
        .min((plot_width as usize).saturating_mul(plot_height as usize));
    let buf_len = total_px.saturating_mul(3);
    let mut rgb_data = vec![0u8; buf_len];

    // Fill the buffer with pixel data
    for pixel in pixels {
        let local_x = (pixel.x - min_x).round().max(0.0) as u32;
        let local_y = (pixel.y - min_y).round().max(0.0) as u32;

        if local_x < chunk_width && local_y < chunk_height {
            let idx = ((local_y as usize)
                .saturating_mul(chunk_width as usize)
                .saturating_add(local_x as usize))
            .saturating_mul(3);
            if idx + 2 < rgb_data.len() {
                rgb_data[idx] = pixel.r;
                rgb_data[idx + 1] = pixel.g;
                rgb_data[idx + 2] = pixel.b;
            }
        }
    }

    Some(BinaryPixelChunk {
        pixels: rgb_data,
        width: chunk_width,
        height: chunk_height,
        offset_x: chunk_x,
        offset_y: chunk_y,
        total_width: plot_width,
        total_height: plot_height,
    })
}

/// Convert scatter points with discrete gate colors to RawPixelData (ScatterOverlay).
pub fn scatter_to_pixels_overlay(
    data: &ScatterPlotData,
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
) -> Vec<RawPixelData> {
    let gate_ids = match &data.gate_ids {
        Some(ids) => ids,
        None => return scatter_to_pixels(data.xy(), width, height, options),
    };
    let colors = if options.gate_colors.is_empty() {
        default_gate_colors()
    } else {
        options.gate_colors.clone()
    };

    // Point size 1.0–4.0: 1 = 1 pixel (radius 0), 2→1, 3→2, 4→3
    let point_size = options.point_size.max(1.0).min(4.0);
    let radius_px = ((point_size - 1.0) / 3.0 * 3.0).round() as usize;
    let radius_px = radius_px.min(3);

    let scale_x = width as f32 / (*options.x_axis.range.end() - *options.x_axis.range.start());
    let scale_y = height as f32 / (*options.y_axis.range.end() - *options.y_axis.range.start());

    let mut pixels = Vec::new();
    for (i, &(x, y)) in data.xy().iter().enumerate() {
        let gate_id = gate_ids.get(i).copied().unwrap_or(0) as usize;
        let (r, g, b) = colors
            .get(gate_id)
            .copied()
            .unwrap_or((60, 60, 60));

        let pixel_x = (((x - *options.x_axis.range.start()) * scale_x).floor() as isize)
            .clamp(0, (width - 1) as isize) as usize;
        let pixel_y = (((y - *options.y_axis.range.start()) * scale_y).floor() as isize)
            .clamp(0, (height - 1) as isize) as usize;

        for dy in -(radius_px as i32)..=(radius_px as i32) {
            for dx in -(radius_px as i32)..=(radius_px as i32) {
                let px = (pixel_x as i32 + dx).clamp(0, (width - 1) as i32) as usize;
                let py = (pixel_y as i32 + dy).clamp(0, (height - 1) as i32) as usize;

                let data_x = (px as f32 / scale_x) + *options.x_axis.range.start();
                let data_y = (py as f32 / scale_y) + *options.y_axis.range.start();

                pixels.push(RawPixelData {
                    x: data_x,
                    y: data_y,
                    r,
                    g,
                    b,
                });
            }
        }
    }
    pixels
}

/// Convert scatter points with continuous z-axis coloring to RawPixelData (ScatterColoredContinuous).
pub fn scatter_to_pixels_colored(
    data: &ScatterPlotData,
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
) -> Vec<RawPixelData> {
    let z_values = match &data.z_values {
        Some(z) => z,
        None => return scatter_to_pixels(data.xy(), width, height, options),
    };

    let (z_min, z_max) = match options.z_range {
        Some((min, max)) => (min, max),
        None => {
            let min = z_values
                .iter()
                .copied()
                .fold(f32::INFINITY, f32::min);
            let max = z_values
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            if min >= max {
                (min, min + 1.0)
            } else {
                (min, max)
            }
        }
    };
    let z_range = z_max - z_min;

    // Point size 1.0–4.0: 1 = 1 pixel (radius 0), 2→1, 3→2, 4→3
    let point_size = options.point_size.max(1.0).min(4.0);
    let radius_px = ((point_size - 1.0) / 3.0 * 3.0).round() as usize;
    let radius_px = radius_px.min(3);

    let scale_x = width as f32 / (*options.x_axis.range.end() - *options.x_axis.range.start());
    let scale_y = height as f32 / (*options.y_axis.range.end() - *options.y_axis.range.start());

    let mut pixels = Vec::new();
    for (i, &(x, y)) in data.xy().iter().enumerate() {
        let z = z_values.get(i).copied().unwrap_or(0.0);
        let t = (z - z_min) / z_range;
        let normalized = t.max(0.0).min(1.0);
        let color = options.colormap.map(normalized);
        let (r, g, b) = (color.0, color.1, color.2);

        let pixel_x = (((x - *options.x_axis.range.start()) * scale_x).floor() as isize)
            .clamp(0, (width - 1) as isize) as usize;
        let pixel_y = (((y - *options.y_axis.range.start()) * scale_y).floor() as isize)
            .clamp(0, (height - 1) as isize) as usize;

        for dy in -(radius_px as i32)..=(radius_px as i32) {
            for dx in -(radius_px as i32)..=(radius_px as i32) {
                let px = (pixel_x as i32 + dx).clamp(0, (width - 1) as i32) as usize;
                let py = (pixel_y as i32 + dy).clamp(0, (height - 1) as i32) as usize;

                let data_x = (px as f32 / scale_x) + *options.x_axis.range.start();
                let data_y = (py as f32 / scale_y) + *options.y_axis.range.start();

                pixels.push(RawPixelData {
                    x: data_x,
                    y: data_y,
                    r,
                    g,
                    b,
                });
            }
        }
    }
    pixels
}

/// Convert scatter (x,y) points to RawPixelData for solid scatter plots
///
/// Each point is drawn as a small square of pixels. point_size (1–4) controls the radius.
/// Uses a single color (dark gray) for all points.
pub fn scatter_to_pixels(
    data: &[(f32, f32)],
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
) -> Vec<RawPixelData> {
    // Point size 1.0–4.0: 1 = 1 pixel (radius 0), 2→1, 3→2, 4→3
    let point_size = options.point_size.max(1.0).min(4.0);
    let radius_px = ((point_size - 1.0) / 3.0 * 3.0).round() as usize;
    let radius_px = radius_px.min(3);

    let scale_x = width as f32 / (*options.x_axis.range.end() - *options.x_axis.range.start());
    let scale_y = height as f32 / (*options.y_axis.range.end() - *options.y_axis.range.start());

    // Single color for scatter solid
    let (r, g, b) = (60u8, 60u8, 60u8);

    let mut pixels = Vec::with_capacity(data.len() * (2 * radius_px + 1).pow(2));

    for &(x, y) in data {
        let pixel_x = (((x - *options.x_axis.range.start()) * scale_x).floor() as isize)
            .clamp(0, (width - 1) as isize) as usize;
        let pixel_y = (((y - *options.y_axis.range.start()) * scale_y).floor() as isize)
            .clamp(0, (height - 1) as isize) as usize;

        for dy in -(radius_px as i32)..=(radius_px as i32) {
            for dx in -(radius_px as i32)..=(radius_px as i32) {
                let px = (pixel_x as i32 + dx).clamp(0, (width - 1) as i32) as usize;
                let py = (pixel_y as i32 + dy).clamp(0, (height - 1) as i32) as usize;

                let data_x = (px as f32 / scale_x) + *options.x_axis.range.start();
                let data_y = (py as f32 / scale_y) + *options.y_axis.range.start();

                pixels.push(RawPixelData {
                    x: data_x,
                    y: data_y,
                    r,
                    g,
                    b,
                });
            }
        }
    }

    pixels
}

/// Dispatch to density, scatter, overlay, or colored scatter based on plot_type and data.
///
/// **Important:** For `Contour` and `ContourOverlay`, this function produces a *density heatmap*
/// (same as `PlotType::Density`), not contour lines. To render contour lines, use
/// [`DensityPlot::render`](crate::plots::density::DensityPlot) or call
/// [`calculate_contours`](crate::contour::calculate_contours) +
/// [`render_contour`](crate::render::plotters_backend::render_contour) directly.
pub fn calculate_plot_pixels(
    data: &ScatterPlotData,
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
) -> Vec<RawPixelData> {
    let xy = data.xy();
    match options.plot_type.canonical() {
        PlotType::ScatterSolid | PlotType::Dot => {
            scatter_to_pixels(xy, width, height, options)
        }
        PlotType::ScatterOverlay => {
            if data.has_gates() {
                scatter_to_pixels_overlay(data, width, height, options)
            } else {
                scatter_to_pixels(xy, width, height, options)
            }
        }
        PlotType::ScatterColoredContinuous => {
            if data.has_z() {
                scatter_to_pixels_colored(data, width, height, options)
            } else {
                calculate_density_per_pixel(xy, width, height, options)
            }
        }
        PlotType::Density
        | PlotType::Contour
        | PlotType::ContourOverlay
        | PlotType::Zebra
        | PlotType::Histogram => calculate_density_per_pixel(xy, width, height, options),
    }
}

/// Cancelable version of calculate_plot_pixels.
///
/// Same caveat as [`calculate_plot_pixels`]: for Contour/ContourOverlay, this produces
/// density pixels, not contour lines. Use [`DensityPlot::render`](crate::plots::density::DensityPlot)
/// for contour rendering.
pub fn calculate_plot_pixels_cancelable(
    data: &ScatterPlotData,
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
    should_cancel: impl FnMut() -> bool,
) -> Option<Vec<RawPixelData>> {
    let xy = data.xy();
    match options.plot_type.canonical() {
        PlotType::ScatterSolid | PlotType::Dot => {
            Some(scatter_to_pixels(xy, width, height, options))
        }
        PlotType::ScatterOverlay => Some(if data.has_gates() {
            scatter_to_pixels_overlay(data, width, height, options)
        } else {
            scatter_to_pixels(xy, width, height, options)
        }),
        PlotType::ScatterColoredContinuous => {
            if data.has_z() {
                Some(scatter_to_pixels_colored(data, width, height, options))
            } else {
                calculate_density_per_pixel_cancelable(xy, width, height, options, should_cancel)
            }
        }
        PlotType::Density
        | PlotType::Contour
        | PlotType::ContourOverlay
        | PlotType::Zebra
        | PlotType::Histogram => {
            calculate_density_per_pixel_cancelable(xy, width, height, options, should_cancel)
        }
    }
}

pub fn calculate_density_per_pixel(
    data: &[(f32, f32)],
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
) -> Vec<RawPixelData> {
    calculate_density_per_pixel_cancelable(data, width, height, options, || false).expect(
        "calculate_density_per_pixel_cancelable returned None when cancellation is disabled",
    )
}

pub fn calculate_density_per_pixel_cancelable(
    data: &[(f32, f32)],
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
    should_cancel: impl FnMut() -> bool,
) -> Option<Vec<RawPixelData>> {
    calculate_density_per_pixel_cpu(data, width, height, options, should_cancel)
}

/// Calculate density for multiple plots in batch
///
/// Returns vector of RawPixelData vectors, one per plot request.
/// This is the core density calculation function, separate from rendering.
/// Apps can use this to orchestrate their own rendering pipelines.
///
/// # Arguments
/// * `requests` - Vector of (data, options) tuples, where each tuple contains
///   the scatter data and the density plot options for one plot
///
/// # Returns
/// Vector of RawPixelData vectors, one per request
pub fn calculate_density_per_pixel_batch(
    requests: &[(ScatterPlotData, DensityPlotOptions)],
) -> Vec<Vec<RawPixelData>> {
    calculate_density_per_pixel_batch_cancelable(requests, || false)
        .expect("calculate_density_per_pixel_batch_cancelable returned None when cancellation is disabled")
}

/// Calculate density for multiple plots in batch with cancellation support
///
/// Same as `calculate_density_per_pixel_batch` but supports cancellation callbacks.
/// Returns `None` if cancelled.
///
/// # Arguments
/// * `requests` - Vector of (data, options) tuples
/// * `should_cancel` - Closure that returns true if processing should be cancelled
///
/// # Returns
/// `Some(Vec<Vec<RawPixelData>>)` if successful, `None` if cancelled
pub fn calculate_density_per_pixel_batch_cancelable(
    requests: &[(ScatterPlotData, DensityPlotOptions)],
    mut should_cancel: impl FnMut() -> bool,
) -> Option<Vec<Vec<RawPixelData>>> {
    if should_cancel() {
        return None;
    }
    Some(calculate_density_per_pixel_batch_cpu(requests))
}

/// CPU implementation of batched density calculation
fn calculate_density_per_pixel_batch_cpu(
    requests: &[(ScatterPlotData, DensityPlotOptions)],
) -> Vec<Vec<RawPixelData>> {
    requests
        .iter()
        .map(|(data, options)| {
            let base = options.base();
            calculate_plot_pixels(data, base.width as usize, base.height as usize, options)
        })
        .collect()
}

/// CPU implementation of density calculation
fn calculate_density_per_pixel_cpu(
    data: &[(f32, f32)],
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
    mut should_cancel: impl FnMut() -> bool,
) -> Option<Vec<RawPixelData>> {
    // Calculate scaling factors
    // FIX: Corrected y-axis calculation (was incorrectly using plot_range_x)
    let scale_x = width as f32 / (*options.x_axis.range.end() - *options.x_axis.range.start());
    let scale_y = height as f32 / (*options.y_axis.range.end() - *options.y_axis.range.start());

    // OPTIMIZED DENSITY BUILDING: Use array-based approach for 7x performance
    // Benchmarks show array-based is much faster than HashMap due to cache locality:
    // - 10K events: 19µs (array) vs 134µs (HashMap) = 7x faster
    // - 100K events: ~100µs (array) vs ~500µs (HashMap) = 5x faster
    // Sequential is faster than parallel for typical FCS sizes (parallel overhead dominates)

    // Point size 1.0–4.0: 1 = 1 pixel (radius 0), 2→1, 3→2, 4→3
    let point_size = options.point_size.max(1.0).min(4.0);
    let radius_px = ((point_size - 1.0) / 3.0 * 3.0).round() as usize;
    let radius_px = radius_px.min(3);

    let build_start = std::time::Instant::now();
    let mut density = vec![0.0f32; width * height];

    // Build density map using fast array access
    // Cache-friendly: sequential writes to contiguous memory
    // Each point contributes to a square neighborhood (radius_px), matching scatter plot behavior
    let mut last_progress = std::time::Instant::now();
    for (i, &(x, y)) in data.iter().enumerate() {
        if (i % 250_000) == 0 {
            if should_cancel() {
                eprintln!(
                    "    ├─ Density build cancelled after {} / {} points",
                    i,
                    data.len()
                );
                return None;
            }

            // Only log progress if we're actually slow.
            if last_progress.elapsed().as_secs_f64() >= 2.0 {
                eprintln!(
                    "    ├─ Density build progress: {} / {} points",
                    i,
                    data.len()
                );
                last_progress = std::time::Instant::now();
            }
        }

        let pixel_x = (((x - *options.x_axis.range.start()) * scale_x).floor() as isize)
            .clamp(0, (width - 1) as isize) as usize;
        let pixel_y = (((y - *options.y_axis.range.start()) * scale_y).floor() as isize)
            .clamp(0, (height - 1) as isize) as usize;

        for dy in -(radius_px as i32)..=(radius_px as i32) {
            for dx in -(radius_px as i32)..=(radius_px as i32) {
                let px = (pixel_x as i32 + dx).clamp(0, (width - 1) as i32) as usize;
                let py = (pixel_y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
                let idx = py * width + px;
                density[idx] += 1.0;
            }
        }
    }

    // Convert to HashMap of only non-zero pixels (sparse representation for coloring)
    let mut density_map: FxHashMap<(usize, usize), f32> = FxHashMap::default();
    density_map.reserve(width * height / 10); // Estimate ~10% fill rate

    for (idx, &count) in density.iter().enumerate() {
        if count > 0.0 {
            let px = idx % width;
            let py = idx / width;
            density_map.insert((px, py), count);
        }
    }

    eprintln!(
        "    ├─ Density map building: {:?} ({} unique pixels from {} total)",
        build_start.elapsed(),
        density_map.len(),
        width * height
    );

    // Apply logarithmic transformation to density values (sequential)
    // Benchmarks show sequential is faster for typical pixel counts (1K-50K):
    // - 10K pixels: 42µs (seq) vs 315µs (par) = 7.5x slower when parallel!
    // - 50K pixels: 331µs (seq) vs 592µs (par) = 1.8x slower when parallel!
    // Adding 1.0 before log to avoid log(0) = -Infinity
    let log_start = std::time::Instant::now();
    for (_, count) in density_map.iter_mut() {
        *count = (*count + 1.0).log10();
    }
    eprintln!("    ├─ Log transform: {:?}", log_start.elapsed());

    // Find max density for normalization (sequential - faster for typical sizes)
    let max_start = std::time::Instant::now();
    let max_density_log = density_map
        .values()
        .fold(0.0f32, |max, &val| max.max(val))
        .max(1.0); // Ensure at least 1.0
    eprintln!("    ├─ Find max: {:?}", max_start.elapsed());

    // Create ONE pixel per unique screen coordinate (not per data point!)
    // This is the key optimization: 100K data points → ~20K screen pixels
    // Return raw pixel data for direct buffer writing (bypass Plotters overhead)
    let color_start = std::time::Instant::now();
    let colored_pixels: Vec<RawPixelData> = density_map
        .iter()
        .map(|(&(pixel_x, pixel_y), &dens)| {
            // Normalize density to 0-1 range
            let normalized_density = dens / max_density_log;

            // Get color from colormap
            let color = options.colormap.map(normalized_density);
            let r = color.0;
            let g = color.1;
            let b = color.2;

            // Map pixel coordinates back to data coordinates
            let x = (pixel_x as f32 / scale_x) + *options.x_axis.range.start();
            let y = (pixel_y as f32 / scale_y) + *options.y_axis.range.start();

            RawPixelData { x, y, r, g, b }
        })
        .collect();
    eprintln!("    └─ Pixel coloring: {:?}", color_start.elapsed());

    Some(colored_pixels)
}
// (streaming helpers removed)
