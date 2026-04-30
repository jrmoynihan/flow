//! Kuva rendering APIs for Tauri/zero-copy display.
//!
//! Enabled with the `raster` feature. These functions render kuva plots to raw RGBA
//! or PNG for use in Tauri IPC, web canvases, or other zero-copy display paths.
//!
//! ## Gap: density/contour vs kuva
//!
//! Density and contour plots are still rendered by the **plotters** backend
//! ([`crate::render::plotters_backend`]), not by kuva. To fully replace plotters we need:
//!
//! **Density (render_pixels):**
//! - **Option A (flow-plots):** Feed kuva's [Heatmap](https://docs.rs/kuva/latest/kuva/plot/heatmap/struct.Heatmap.html)
//!   with a 2D grid of density values + colormap. Today we only have
//!   [RawPixelData](crate::density_calc::RawPixelData) (sparse (x,y,r,g,b) in data coords);
//!   we'd need either a density matrix from the pipeline or to convert raw pixels into
//!   a dense grid and a compatible value range.
//! - **Option B (kuva):** An "image" or "raster" plot type that accepts a pre-rendered
//!   RGB/RGBA buffer (width×height) and draws it in the plot area with axes/labels,
//!   so we can keep the current RawPixelData → buffer → output pipeline and only swap
//!   the axis/mesh drawing to kuva.
//!
//! **Contour (render_contour):**
//! - Kuva has a `plot::contour` module. We need to map our [ContourData](crate::contour::ContourData)
//!   (list of paths + outliers) into kuva's contour + scatter types, and build a Layout
//!   with continuous axes (our ranges and transform-aware labels). Requires checking
//!   kuva's contour API and whether it supports continuous (not categorical) axes.
//!
//! ## Raster options
//!
//! | Function | Output | Use case |
//! |----------|--------|----------|
//! | [`render_to_rgba`] | `(width, height, Vec<u8>)` | Tauri IPC, `ImageData`, zero-copy canvas |
//! | [`render_to_rgba_no_text`] | Same, no text | Fast preview, headless |
//! | [`render_to_png_direct`] | PNG bytes | Direct raster, no SVG round-trip |
//! | [`render_to_png_direct_no_text`] | PNG, no text | Fast PNG export |
//!
//! ## Example (Tauri)
//!
//! ```rust,ignore
//! #[tauri::command]
//! fn render_plot_rgba(plots: ..., layout: ..., scale: f32) -> Result<(u32, u32, Vec<u8>), String> {
//!     flow_plots::kuva::render_to_rgba(plots, layout, scale)
//! }
//! ```
//!
//! Frontend:
//!
//! ```javascript
//! const { width, height, data } = await invoke('render_plot_rgba', {...});
//! const image_data = new ImageData(new Uint8ClampedArray(data), width, height);
//! ctx.putImageData(image_data, 0, 0);
//! ```

#[cfg(feature = "raster")]
pub use kuva::{render_to_raster, render_to_raster_no_text};

/// Re-exported for building plots to pass to the render functions.
#[cfg(feature = "raster")]
pub use kuva::render::layout::Layout;
/// Re-exported for building plots to pass to the render functions.
#[cfg(feature = "raster")]
pub use kuva::render::plots::Plot;

/// Render kuva plots to raw RGBA `(width, height, data)` for zero-copy display.
///
/// Ideal for Tauri IPC and web canvas `ImageData` — no PNG encoding overhead.
#[cfg(feature = "raster")]
pub fn render_to_rgba(
    plots: Vec<Plot>,
    layout: Layout,
    scale: f32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let scene = kuva::render::render::render_multiple(plots, layout);
    kuva::backend::raster::RasterBackend::new()
        .with_scale(scale)
        .render_scene_to_rgba(&scene)
}

/// Like [`render_to_rgba`] but skips text rendering for maximum throughput.
#[cfg(feature = "raster")]
pub fn render_to_rgba_no_text(
    plots: Vec<Plot>,
    layout: Layout,
    scale: f32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let scene = kuva::render::render::render_multiple(plots, layout);
    kuva::backend::raster::RasterBackend::new()
        .with_scale(scale)
        .with_skip_text(true)
        .render_scene_to_rgba(&scene)
}

/// Render kuva plots to PNG via direct raster (no SVG round-trip).
///
/// Alias for [`render_to_raster`].
#[cfg(feature = "raster")]
pub fn render_to_png_direct(
    plots: Vec<Plot>,
    layout: Layout,
    scale: f32,
) -> Result<Vec<u8>, String> {
    kuva::render_to_raster(plots, layout, scale)
}

/// Like [`render_to_png_direct`] but skips text rendering.
///
/// Alias for [`render_to_raster_no_text`].
#[cfg(feature = "raster")]
pub fn render_to_png_direct_no_text(
    plots: Vec<Plot>,
    layout: Layout,
    scale: f32,
) -> Result<Vec<u8>, String> {
    kuva::render_to_raster_no_text(plots, layout, scale)
}

// -----------------------------------------------------------------------------
// Density plot rendering with linear axes and original-value tick labels
// -----------------------------------------------------------------------------

/// Render a density plot via kuva using arcsinh pre-transformed data, linear axes,
/// and custom tick formatting so labels show original (untransformed) values.
///
/// Uses a one-cell-per-pixel grid (bins = width×height), like the plotters backend:
/// no blur, no spreading into empty regions, and no visible grid banding between
/// adjacent cells because each cell is one pixel.
#[cfg(feature = "raster")]
pub fn render_density_kuva(
    xy: &[(f32, f32)],
    options: &crate::options::DensityPlotOptions,
    _render_config: &mut crate::render::RenderConfig,
) -> Result<crate::PlotBytes, String> {
    use crate::colormap::ColorMaps;
    use crate::options::PlotOptions;
    use crate::render::kuva_axis::{
        apply_density_layout_overrides, tick_format_heatmap_cell_index,
    };
    use image::ImageDecoder;

    let base = options.base();
    let width = base.width as usize;
    let height = base.height as usize;

    if width == 0 || height == 0 {
        return Err("width and height must be positive".to_string());
    }

    let x_min = *options.x_axis.range.start() as f64;
    let x_max = *options.x_axis.range.end() as f64;
    let y_min = *options.y_axis.range.start() as f64;
    let y_max = *options.y_axis.range.end() as f64;

    let span_x = (x_max - x_min).abs().max(1e-10);
    let span_y = (y_max - y_min).abs().max(1e-10);
    let eps = 1e-9;
    let x_max_k = x_max + eps * span_x;
    let y_max_k = y_max + eps * span_y;

    let data: Vec<(f64, f64)> = xy.iter().map(|&(a, b)| (a as f64, b as f64)).collect();

    // One cell per pixel (bins = width × height) so no banding. Match plotters: each point
    // contributes to a neighborhood of cells (radius from point_size), not just one cell,
    // so density is visible and empty regions stay 0 (no blur).
    let bins_x = width;
    let bins_y = height;
    let point_size = options.point_size.max(1.0).min(4.0);
    let radius_cells = ((point_size - 1.0) / 3.0 * 3.0).round() as usize;
    let radius_cells = radius_cells.min(3);

    let scale_x = bins_x as f64 / span_x;
    let scale_y = bins_y as f64 / span_y;

    let mut grid = vec![vec![0.0_f64; bins_x]; bins_y];
    for &(x, y) in &data {
        if x < x_min || x >= x_max_k || y < y_min || y >= y_max_k {
            continue;
        }
        let px = ((x - x_min) * scale_x).floor() as i32;
        let py = ((y - y_min) * scale_y).floor() as i32;
        let cx = px.clamp(0, (bins_x as i32) - 1) as usize;
        let cy = py.clamp(0, (bins_y as i32) - 1) as usize;

        for dy in -(radius_cells as i32)..=(radius_cells as i32) {
            for dx in -(radius_cells as i32)..=(radius_cells as i32) {
                let j = (cx as i32 + dx).clamp(0, (bins_x as i32) - 1) as usize;
                let i = (cy as i32 + dy).clamp(0, (bins_y as i32) - 1) as usize;
                grid[i][j] += 1.0;
            }
        }
    }

    // Log-scale density so colormap range matches plotters (log10(count+1))
    for row in grid.iter_mut() {
        for v in row.iter_mut() {
            *v = (*v + 1.0).log10();
        }
    }

    let heatmap_cmap = match options.colormap {
        ColorMaps::Inferno => kuva::plot::ColorMap::Inferno,
        ColorMaps::Viridis => kuva::plot::ColorMap::Viridis,
        _ => kuva::plot::ColorMap::Viridis,
    };

    let heatmap = kuva::plot::Heatmap::new()
        .with_data(grid)
        .with_color_map(heatmap_cmap);

    let plots = vec![Plot::Heatmap(heatmap)];
    let mut layout = kuva::render::layout::Layout::auto_from_plots(&plots);
    apply_density_layout_overrides(&mut layout, options);
    layout.x_tick_format =
        tick_format_heatmap_cell_index(x_min, x_max, bins_x, &options.x_axis.transform);
    layout.y_tick_format =
        tick_format_heatmap_cell_index(y_min, y_max, bins_y, &options.y_axis.transform);
    layout.width = Some(base.width as f64);
    layout.height = Some(base.height as f64);

    let png_bytes = kuva::render_to_raster(plots, layout, 1.0)?;

    // Match plotters backend: return JPEG for density (smaller, same as render_pixels)
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(&png_bytes))
        .map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    let img = image::DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    let rgb = img.to_rgb8();
    let mut jpeg = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85);
    enc.encode(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| e.to_string())?;
    Ok(jpeg)
}
