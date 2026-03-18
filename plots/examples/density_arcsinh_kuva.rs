//! Showcase: density scatter plot with arcsinh pre-transformed data, linear axes,
//! and tick labels showing original values (kuva backend).
//!
//! Run with raster feature: `cargo run -p flow-plots --features raster --example density_arcsinh_kuva`
//!
//! This example:
//! - Generates semi-realistic "raw" fluorescence-like values (two clusters) with `rand`,
//!   then transforms with flow-fcs `TransformType::Arcsinh` for plotting.
//! - Uses linear axes (data and ranges in transformed space).
//! - Axis tick labels show original scale via inverse_transform (e.g. 2e2, 1e3).
//! - Renders with the kuva backend when built with `--features raster`.

use flow_fcs::{TransformType, Transformable};
use flow_plots::colormap::ColorMaps;
use flow_plots::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
use flow_plots::render::RenderConfig;
use flow_plots::{DensityPlot, Plot, ScatterPlotData};
use std::fs;
use std::path::PathBuf;

/// Simple LCG for deterministic semi-realistic spread (no rand dependency in example).
fn next_u32(state: &mut u64) -> u32 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 32) as u32
}

fn uniform_f32(state: &mut u64, lo: f32, hi: f32) -> f32 {
    let u = (next_u32(state) as f32) / (u32::MAX as f32);
    lo + u * (hi - lo)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from("plot_types_output");
    fs::create_dir_all(&out_dir)?;
    println!("Output dir: {}", out_dir.canonicalize()?.display());

    let cofactor = 200.0_f32;
    let transform = TransformType::Arcsinh { cofactor };

    // Semi-realistic raw fluorescence-like values: two clusters, then arcsinh-transform for plotting.
    let mut state = 42_u64;
    let n_per_cluster = 1000_usize;
    let mut raw_x = Vec::with_capacity(n_per_cluster * 2);
    let mut raw_y = Vec::with_capacity(n_per_cluster * 2);

    // Cluster 1: roughly 300–600 x, 500–800 y (raw)
    for _ in 0..n_per_cluster {
        raw_x.push(uniform_f32(&mut state, 280.0, 620.0));
        raw_y.push(uniform_f32(&mut state, 480.0, 820.0));
    }
    // Cluster 2: roughly 1000–1500 x, 1400–1900 y (raw)
    for _ in 0..n_per_cluster {
        raw_x.push(uniform_f32(&mut state, 980.0, 1520.0));
        raw_y.push(uniform_f32(&mut state, 1380.0, 1920.0));
    }

    let xy_transformed: Vec<(f32, f32)> = raw_x
        .into_iter()
        .zip(raw_y)
        .map(|(x, y)| (transform.transform(&x), transform.transform(&y)))
        .collect();

    let x_min = xy_transformed.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let x_max = xy_transformed.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
    let y_min = xy_transformed.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    let y_max = xy_transformed.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);

    let padding_x = (x_max - x_min).max(0.5_f32) * 0.05;
    let padding_y = (y_max - y_min).max(0.5_f32) * 0.05;

    let base = BasePlotOptions::new()
        .width(600u32)
        .height(500u32)
        .title("Density (arcsinh pre-transformed, linear axes, original-value ticks)".to_string())
        .build()?;

    let x_axis = AxisOptions::new()
        .label("Channel A (original scale)".to_string())
        .range((x_min - padding_x)..=(x_max + padding_x))
        .transform(transform.clone())
        .build()?;

    let y_axis = AxisOptions::new()
        .label("Channel B (original scale)".to_string())
        .range((y_min - padding_y)..=(y_max + padding_y))
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

    let bytes = plot.render(data, &options, &mut render_config)?;
    let path = out_dir.join("density_arcsinh_kuva.jpg");
    fs::write(&path, &bytes)?;
    println!("  {} (linear axes, ticks show original values)", path.display());

    Ok(())
}
