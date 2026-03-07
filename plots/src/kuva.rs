//! Kuva rendering APIs for Tauri/zero-copy display.
//!
//! Enabled with the `raster` feature. These functions render kuva plots to raw RGBA
//! or PNG for use in Tauri IPC, web canvases, or other zero-copy display paths.
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
pub use kuva::{
    render_to_raster, render_to_raster_no_text,
};

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
