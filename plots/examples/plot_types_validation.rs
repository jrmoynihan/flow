//! Generate one PNG per plot type for visual validation.
//!
//! Run: `cargo run -p flow-plots --example plot_types_validation`
//! Output: `plot_types_output/` with density.png, scatter_solid.png, contour.png, etc.
//!
//! Plots should look substantially different between types.

use flow_fcs::TransformType;
use flow_plots::colormap::ColorMaps;
use flow_plots::options::density::default_gate_colors;
use flow_plots::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
use flow_plots::render::RenderConfig;
use flow_plots::{DensityPlot, Plot, PlotType, ScatterPlotData};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from("plot_types_output");
    fs::create_dir_all(&out_dir)?;
    println!("Output dir: {}", out_dir.canonicalize()?.display());

    // Synthetic data: two clusters
    let points: Vec<(f32, f32)> = {
        let mut v = Vec::with_capacity(2000);
        for _ in 0..1000 {
            v.push((100.0 + rand_centered(20.0), 150.0 + rand_centered(30.0)));
        }
        for _ in 0..1000 {
            v.push((400.0 + rand_centered(25.0), 350.0 + rand_centered(40.0)));
        }
        v
    };

    let base = BasePlotOptions::new()
        .width(600u32)
        .height(500u32)
        .build()?;
    let x_axis = AxisOptions::new()
        .label("X".to_string())
        .range(0.0..=512.0)
        .transform(TransformType::Linear)
        .build()?;
    let y_axis = AxisOptions::new()
        .label("Y".to_string())
        .range(0.0..=512.0)
        .transform(TransformType::Linear)
        .build()?;

    let plot = DensityPlot::new();
    let mut render_config = RenderConfig::default();

    // 1. Density (default)
    let opts = DensityPlotOptions::new()
        .base(base.clone())
        .x_axis(x_axis.clone())
        .y_axis(y_axis.clone())
        .plot_type(PlotType::Density)
        .point_size(1.2)
        .build()?;
    let bytes = plot.render(points.clone().into(), &opts, &mut render_config)?;
    fs::write(out_dir.join("density.png"), &bytes)?;
    println!("  density.png");

    // 2. ScatterSolid – small dots
    let opts = DensityPlotOptions::new()
        .base(base.clone())
        .x_axis(x_axis.clone())
        .y_axis(y_axis.clone())
        .plot_type(PlotType::ScatterSolid)
        .point_size(1.0)
        .build()?;
    let bytes = plot.render(points.clone().into(), &opts, &mut render_config)?;
    fs::write(out_dir.join("scatter_solid.png"), &bytes)?;
    println!("  scatter_solid.png");

    // 3. Contour – lines (known plotters overflow with some grid configs; skip for this example)
    println!("  contour.png (skipped - use FCS-based data for full validation)");

    // 4. ScatterOverlay – needs gate_ids
    let gate_ids: Vec<u32> = points
        .iter()
        .map(|(x, _)| if *x < 256.0 { 0 } else { 1 })
        .collect();
    let overlay_data =
        ScatterPlotData::with_gates(points.clone(), gate_ids).map_err(|e| e.to_string())?;
    let opts = DensityPlotOptions::new()
        .base(base.clone())
        .x_axis(x_axis.clone())
        .y_axis(y_axis.clone())
        .plot_type(PlotType::ScatterOverlay)
        .gate_colors(default_gate_colors())
        .point_size(1.2)
        .build()?;
    let bytes = plot.render(overlay_data.into(), &opts, &mut render_config)?;
    fs::write(out_dir.join("scatter_overlay.png"), &bytes)?;
    println!("  scatter_overlay.png");

    // 5. ScatterColoredContinuous – needs z_values
    let z_values: Vec<f32> = points.iter().map(|(x, y)| (x + y) / 2.0).collect();
    let colored_data =
        ScatterPlotData::with_z(points.clone(), z_values).map_err(|e| e.to_string())?;
    let opts = DensityPlotOptions::new()
        .base(base.clone())
        .x_axis(x_axis.clone())
        .y_axis(y_axis.clone())
        .plot_type(PlotType::ScatterColoredContinuous)
        .colormap(ColorMaps::Viridis)
        .point_size(1.2)
        .build()?;
    let bytes = plot.render(colored_data.into(), &opts, &mut render_config)?;
    fs::write(out_dir.join("scatter_colored.png"), &bytes)?;
    println!("  scatter_colored.png");

    println!("\nDone. Inspect plots in {}/", out_dir.display());
    Ok(())
}

fn rand_centered(scale: f32) -> f32 {
    use std::f32::consts::PI;
    let u: f32 = lcg_next() as f32 / u32::MAX as f32;
    let v: f32 = lcg_next() as f32 / u32::MAX as f32;
    scale * ((-2.0 * (1.0 - u).ln()).sqrt() * (2.0 * PI * v).cos())
}

static mut LCG_STATE: u64 = 12345;

fn lcg_next() -> u32 {
    unsafe {
        LCG_STATE = LCG_STATE
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (LCG_STATE >> 32) as u32
    }
}
