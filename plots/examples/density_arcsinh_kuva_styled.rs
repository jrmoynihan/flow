//! Example: density plot with arcsinh + kuva, optional title/colorbar and appearance options.
//!
//! Run: `cargo run -p flow-plots --features raster --example density_arcsinh_kuva_styled`
//!
//! Shows a second density plot with:
//! - Title and colorbar turned off (optional labels).
//! - Custom text sizes and grid visibility.
//! - More events and Gaussian (blob) spread for Poisson-like count distribution.

use flow_fcs::{TransformType, Transformable};
use flow_plots::colormap::ColorMaps;
use flow_plots::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
use flow_plots::render::RenderConfig;
use flow_plots::{DensityPlot, Plot, ScatterPlotData};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn next_u32(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 32) as u32
}

/// Box–Muller: two uniforms -> two N(0,1). Uses ln(1-u) to avoid ln(0).
fn normal_f32(state: &mut u64) -> (f32, f32) {
    let u1 = (next_u32(state) as f32) / (u32::MAX as f32);
    let u2 = (next_u32(state) as f32) / (u32::MAX as f32);
    let u1 = if u1 <= 0.0_f32 { f32::MIN_POSITIVE } else { u1 };
    let r = (-2.0 * (1.0 - u1).ln()).sqrt();
    let theta = 2.0 * std::f32::consts::PI * u2;
    (r * theta.cos(), r * theta.sin())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from("plot_types_output");
    fs::create_dir_all(&out_dir)?;
    println!("Output dir: {}", out_dir.canonicalize()?.display());

    let cofactor = 200.0_f32;
    let transform = TransformType::Arcsinh { cofactor };

    // Two clusters with more events and Gaussian (blob) spread for Poisson-like density.
    let mut state = 123_u64;
    let n_per_cluster = 5_000_usize;
    let mut raw_x = Vec::with_capacity(n_per_cluster * 2);
    let mut raw_y = Vec::with_capacity(n_per_cluster * 2);
    // Cluster 1: center (450, 650), sigma ~80
    for _ in 0..n_per_cluster {
        let (n1, n2) = normal_f32(&mut state);
        raw_x.push(450.0_f32 + 80.0 * n1);
        raw_y.push(650.0_f32 + 80.0 * n2);
    }
    // Cluster 2: center (1250, 1650), sigma ~120
    for _ in 0..n_per_cluster {
        let (n1, n2) = normal_f32(&mut state);
        raw_x.push(1250.0_f32 + 120.0 * n1);
        raw_y.push(1650.0_f32 + 120.0 * n2);
    }

    let xy_transformed: Vec<(f32, f32)> = raw_x
        .into_iter()
        .zip(raw_y)
        .map(|(x, y)| (transform.transform(&x), transform.transform(&y)))
        .collect();

    let x_min = xy_transformed
        .iter()
        .map(|p| p.0)
        .fold(f32::INFINITY, f32::min);
    let x_max = xy_transformed
        .iter()
        .map(|p| p.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let y_min = xy_transformed
        .iter()
        .map(|p| p.1)
        .fold(f32::INFINITY, f32::min);
    let y_max = xy_transformed
        .iter()
        .map(|p| p.1)
        .fold(f32::NEG_INFINITY, f32::max);
    let pad_x = (x_max - x_min).max(0.5_f32) * 0.05;
    let pad_y = (y_max - y_min).max(0.5_f32) * 0.05;

    let base = BasePlotOptions::new()
        .width(500u32)
        .height(450u32)
        .title("Styled density".to_string())
        .show_title(false)
        .show_colorbar(false)
        .title_size(16u32)
        .label_size(11u32)
        .tick_size(9u32)
        .show_grid(true)
        .build()?;

    let x_axis = AxisOptions::new()
        .label("X (original)".to_string())
        .range((x_min - pad_x)..=(x_max + pad_x))
        .transform(transform.clone())
        .build()?;

    let y_axis = AxisOptions::new()
        .label("Y (original)".to_string())
        .range((y_min - pad_y)..=(y_max + pad_y))
        .transform(transform)
        .build()?;

    let options = DensityPlotOptions::new()
        .base(base)
        .x_axis(x_axis)
        .y_axis(y_axis)
        .colormap(ColorMaps::Viridis)
        .build()?;

    let data: ScatterPlotData = xy_transformed.into();
    let plot = DensityPlot::new();
    let mut render_config = RenderConfig::default();

    let t0 = Instant::now();
    let bytes = plot.render(data, &options, &mut render_config)?;
    let render_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let path = out_dir.join("density_arcsinh_kuva_styled.jpg");
    fs::write(&path, &bytes)?;
    let total_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!(
        "  {} (no title/colorbar, custom sizes)",
        path.display()
    );
    println!(
        "  Render (no disk): {:.1} ms, total with write: {:.1} ms",
        render_ms, total_ms
    );



    Ok(())
}
