use crate::PlotBytes;
use crate::create_axis_specs;
use crate::density::RawPixelData;
use crate::options::DensityPlotOptions;
use crate::render::{ProgressInfo, RenderConfig};
use flow_fcs::TransformType;

/// Format a value using the transform type
///
/// This replicates the Formattable::format logic since the trait is not exported.
fn format_transform_value(transform: &TransformType, value: &f32) -> String {
    match transform {
        TransformType::Linear => format!("{:.1e}", value),
        TransformType::Arcsinh { cofactor } => {
            // Convert from transformed space back to original space
            let original_value = (value / cofactor).sinh() * cofactor;
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
use std::ops::Range;

/// Render pixels to a JPEG image using the Plotters backend
///
/// This function handles the complete rendering pipeline:
/// 1. Sets up Plotters chart with axes and mesh
/// 2. Writes pixels directly to the buffer
/// 3. Encodes to JPEG format
///
/// Progress reporting is handled via the RenderConfig if provided.
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
