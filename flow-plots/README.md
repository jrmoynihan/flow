# flow-plots

A library for creating visualizations of flow cytometry data.

## Overview

This library provides a flexible, extensible API for creating different types of plots from flow cytometry data. The architecture is designed to be easily extended with new plot types while maintaining clean separation of concerns.

## Features

- **Extensible Architecture**: Easy to add new plot types by implementing the `Plot` trait
- **Builder Pattern**: Type-safe configuration using the builder pattern
- **Progress Reporting**: Optional progress callbacks for streaming/progressive rendering
- **Flexible Rendering**: Applications can inject their own execution and progress logic

## Basic Usage

### Simple Density Plot

```rust
use flow_plots::{DensityPlot, DensityPlotOptions};
use flow_plots::render::RenderConfig;

let plot = DensityPlot::new();
let options = DensityPlotOptions::new()
    .width(800)
    .height(600)
    .title("My Density Plot")
    .build()?;

let data: Vec<(f32, f32)> = vec![(100.0, 200.0), (150.0, 250.0)];
let mut render_config = RenderConfig::default();
let bytes = plot.render(data, &options, &mut render_config)?;
```

### With FCS File Initialization

This example shows the complete workflow from opening an FCS file to generating a plot:

```rust
use flow_plots::{DensityPlot, helpers};
use flow_plots::render::RenderConfig;
use flow_fcs::Fcs;

// Step 1: Open the FCS file from a file path
let fcs = Fcs::open("path/to/your/file.fcs")?;

// Step 2: Select parameters for the x and y axes
// You can find parameters by their channel name (e.g., "FSC-A", "SSC-A", "FL1-A")
let x_parameter = fcs.find_parameter("FSC-A")?;
let y_parameter = fcs.find_parameter("SSC-A")?;

// Step 3: Use the helper function to create plot options with sensible defaults
// This analyzes the FCS file and parameters to determine appropriate ranges and transforms
let mut builder = helpers::density_options_from_fcs(
    &fcs,
    x_parameter,
    y_parameter,
)?;

// Step 4: Customize the options further if needed
let options = builder
    .width(800)
    .height(600)
    .build()?;

// Step 5: Extract the data for plotting
// The helper function uses the parameter's transform to calculate appropriate ranges,
// so we should use the same data (raw or transformed) for plotting
let data: Vec<(f32, f32)> = fcs.get_xy_pairs("FSC-A", "SSC-A")?;

// Step 6: Create and render the plot
let plot = DensityPlot::new();
let mut render_config = RenderConfig::default();
let bytes = plot.render(data, &options, &mut render_config)?;

// Step 7: Use the bytes (JPEG-encoded image) as needed
// e.g., save to file, send over network, display in UI, etc.
```

**Key Points:**

- **Opening FCS files**: `Fcs::open(path)` opens and parses an FCS file from a file path. The path must have a `.fcs` extension. This function:
  - Memory-maps the file for efficient access
  - Parses the header, text segment, and data segment
  - Loads event data into a Polars DataFrame
  - Returns a fully parsed `Fcs` struct ready for use

- **Finding parameters**: `fcs.find_parameter(channel_name)` finds a parameter by its channel name (e.g., "FSC-A", "SSC-A", "FL1-A"). Returns a `Result<&Parameter>` - an error if the parameter doesn't exist. To list all available parameters, use `fcs.get_parameter_names_from_dataframe()` which returns a `Vec<String>` of all channel names.

- **Automatic option configuration**: `helpers::density_options_from_fcs()` analyzes the FCS file and parameters to automatically:
  - **Determine plot ranges** based on parameter type:
    - **FSC/SSC**: Uses default range (0 to 200,000)
    - **Time**: Uses the actual maximum time value from the data
    - **Fluorescence**: Calculates percentile bounds (1st to 99th percentile) after applying the parameter's transform to the raw values
  - **Apply transformations**: For fluorescence parameters, it transforms the raw values using the parameter's `TransformType` (typically Arcsinh) before calculating percentile bounds, ensuring the plot range reflects the transformed data scale
  - **Set axis transforms**: Automatically sets Linear transform for FSC/SSC, and the parameter's default transform (usually Arcsinh) for fluorescence
  - **Extract metadata**: Gets the file name from the `$FIL` keyword for the plot title

- **Extracting data**: `fcs.get_xy_pairs(x_param, y_param)` extracts (x, y) coordinate pairs for plotting. This returns raw (untransformed) values, which is appropriate since the plot options handle transformation during rendering and axis labeling.

**Error Handling:**

All operations return `Result` types. Here's a more complete example with error handling:

```rust
use anyhow::Result;

fn create_plot_from_file(path: &str) -> Result<Vec<u8>> {
    // Open the file - returns error if file doesn't exist or is invalid
    let fcs = Fcs::open(path)?;

    // Find parameters - returns error if parameter name doesn't exist
    let x_parameter = fcs.find_parameter("FSC-A")
        .map_err(|e| anyhow::anyhow!("Parameter 'FSC-A' not found: {}", e))?;
    let y_parameter = fcs.find_parameter("SSC-A")
        .map_err(|e| anyhow::anyhow!("Parameter 'SSC-A' not found: {}", e))?;

    // Create options with automatic configuration
    let options = helpers::density_options_from_fcs(&fcs, x_parameter, y_parameter)?
        .width(800)
        .height(600)
        .build()?;

    // Extract data - returns error if parameters don't exist
    let data = fcs.get_xy_pairs("FSC-A", "SSC-A")?;

    // Render the plot
    let plot = DensityPlot::new();
    let mut render_config = RenderConfig::default();
    let bytes = plot.render(data, &options, &mut render_config)?;

    Ok(bytes)
}
```

### With Application Executor and Progress

```rust
use flow_plots::{DensityPlot, DensityPlotOptions, RenderConfig, ProgressInfo};
use crate::plot_executor::with_render_lock;
use crate::commands::PlotProgressEvent;

let options = DensityPlotOptions::new()
    .width(800)
    .height(600)
    // ... configure options
    .build()?;

// Configure rendering with app-specific concerns
let mut render_config = RenderConfig {
    progress: Some(Box::new(move |info: ProgressInfo| {
        channel.send(PlotProgressEvent::Progress {
            pixels: info.pixels,
            percent: info.percent,
        })?;
        Ok(())
    })),
};

// Wrap the render call with your executor's render lock
let bytes = with_render_lock(|| {
    let plot = DensityPlot::new();
    plot.render(data, &options, &mut render_config)
})?;
```

## Architecture

The library is organized into several modules:

- **`options`**: Plot configuration types using the builder pattern
  - `BasePlotOptions`: Layout and display settings
  - `AxisOptions`: Axis configuration (range, transform, label)
  - `DensityPlotOptions`: Complete density plot configuration
- **`plots`**: Plot implementations
  - `DensityPlot`: 2D density plot implementation
  - `Plot` trait: Interface for all plot types
- **`render`**: Rendering infrastructure
  - `RenderConfig`: Configuration for rendering (progress callbacks)
  - `ProgressInfo`: Progress information structure
  - `plotters_backend`: Plotters-based rendering implementation
- **`density`**: Density calculation algorithms
- **`colormap`**: Color map implementations
- **`helpers`**: Helper functions for common initialization patterns

## Adding New Plot Types

To add a new plot type:

1. Create a new options struct (e.g., `DotPlotOptions`) that implements `PlotOptions`
2. Create a new plot struct (e.g., `DotPlot`) that implements the `Plot` trait
3. Implement the `render` method with your plot-specific logic

Example:

```rust
use flow_plots::plots::traits::Plot;
use flow_plots::options::PlotOptions;
use flow_plots::render::RenderConfig;
use flow_plots::PlotBytes;
use anyhow::Result;

struct DotPlotOptions {
    base: BasePlotOptions,
    // ... your plot-specific options
}

impl PlotOptions for DotPlotOptions {
    fn base(&self) -> &BasePlotOptions {
        &self.base
    }
}

struct DotPlot;

impl Plot for DotPlot {
    type Options = DotPlotOptions;
    type Data = Vec<(f32, f32)>;

    fn render(
        &self,
        data: Self::Data,
        options: &Self::Options,
        render_config: &mut RenderConfig,
    ) -> Result<PlotBytes> {
        // ... your rendering logic
        Ok(vec![])
    }
}
```

## Migration Guide

### From Old `PlotOptions` API

**Old API:**

```rust
let options = PlotOptions::new(
    fcs,
    x_parameter,
    y_parameter,
    Some(800),
    Some(600),
    None,
    None,
    None,
    None,
    None,
)?;
```

**New API:**

```rust
// Option 1: Use helper function
let mut builder = helpers::density_options_from_fcs(fcs, x_param, y_param)?;
let options = builder
    .width(800)
    .height(600)
    .build()?;

// Option 2: Manual construction
let options = DensityPlotOptions::new()
    .width(800)
    .height(600)
    .x_axis(|axis| axis
        .range(0.0..=200_000.0)
        .transform(TransformType::Arcsinh { cofactor: 150.0 }))
    .y_axis(|axis| axis
        .range(0.0..=200_000.0)
        .transform(TransformType::Arcsinh { cofactor: 150.0 }))
    .build()?;
```

### From Old `draw_plot` Function

**Old API:**

```rust
let (bytes, _, _, _) = draw_plot(pixels, &options)?;
```

**New API:**

```rust
let plot = DensityPlot::new();
let mut render_config = RenderConfig::default();
let bytes = plot.render(data, &options, &mut render_config)?;
```

## License

MIT
