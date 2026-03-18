# Plan: Arcsinh pre-transformed data + linear axes with original-value tick labels (kuva backend)

## Goal

Use arcsinh **pre-transformed** data from flow-fcs and render flow plots (density/contour) with the **kuva** backend using **linear axes** and **custom tick formatting** so tick labels show **original** (untransformed) values.

## Current state

### flow-fcs

- **TransformType** (`fcs/src/transform.rs`): `Linear`, `Arcsinh { cofactor }`, `Biexponential { … }`.
- **Transformable**: `transform(value)` and `inverse_transform(value)`; arcsinh is `asinh(x/cofactor)` and inverse is `sinh(y) * cofactor`.
- **Formattable**: `format(value)` turns a transformed value into a display string by calling `inverse_transform` then e.g. `format!("{:.1e}", original)`.
- Pre-transformed data: callers use e.g. `Fcs::apply_arcsinh_transform` or `apply_default_arcsinh_transform`; the resulting dataframe holds values in **transformed** space.

### flow-plots

- **Data contract**: `ScatterPlotData` holds (x, y) pairs. For arcsinh workflows these are already in **transformed** space; no second transform is applied in the plot pipeline.
- **Axis options** (`options::AxisOptions`): `range` (in **transformed** space when using helpers), `transform` (used only for **label formatting**), `label`.
- **Helpers** (`density_options_from_fcs`): Builds ranges by transforming raw values and taking percentiles, so `x_axis.range` / `y_axis.range` are in **transformed** space; sets `transform` to `Arcsinh { cofactor }` (or Linear for FSC/SSC/Time) for formatting.
- **Plotters backend** (current, deprecated): Uses `create_axis_specs`; for Arcsinh keeps range as-is (transformed). Draws **linear** axes in data coordinates. Tick formatter: `format_transform_value(transform, value)` → `inverse_transform(value)` → format to string (e.g. scientific). So axes are linear in transformed space, labels show original values.
- **Kuva backend** (`render/kuva_backend.rs`): Today only exposes `render_to_rgba`, `render_to_png_direct`, etc., taking `Vec<Plot>` and `Layout`. Density/contour are **not** yet implemented with kuva; they still use plotters (see module doc “Gap: density/contour vs kuva”).

### Kuva API (from docs.rs)

- **Layout**: `x_range`, `y_range` (f64 pairs), `log_x`/`log_y` (we keep `false` for linear), `x_tick_format`, `y_tick_format` (`TickFormat`), `x_label`, `y_label`, etc.
- **TickFormat**: `Auto` | `Fixed(usize)` | `Integer` | `Sci` | `Percent` | **`Custom(Arc<dyn Fn(f64) -> String + Send + Sync>)`**. So we can pass a closure that receives the tick value in **data (transformed)** space and returns the label string (original value).
- **Plot types**: e.g. `Heatmap`, `Contour`, `Histogram2d`, etc. Whichever we use for density/contour, we will build a `Layout` with linear axes and custom tick formatters.

## Design: linear axes + original-value ticks

1. **Axes**: Linear. Data and `DensityPlotOptions.x_axis.range` / `y_axis.range` are in **transformed** space; kuva `Layout`’s `x_range` / `y_range` are set from these (no log scale).
2. **Tick formatter**: For each axis, use `TickFormat::Custom`. The closure receives the tick value in **transformed** space (same as data). Call `flow_fcs::TransformType::inverse_transform` (with the axis’s `transform` from `AxisOptions`) to get the original value, then format it (e.g. `format!("{:.1e}", original)` or reuse flow-fcs `Formattable::format` if we expose it). So tick **positions** are linear in transformed space; tick **labels** show original values.
3. **flow-fcs**: No API change required. We already have `Transformable::inverse_transform` and `Formattable::format`. If the formatter in flow-plots should match flow-fcs exactly, we can either duplicate the formatting logic (as plotters_backend does with `format_transform_value`) or add a small public helper / re-export of `Formattable` in flow-fcs and use it from flow-plots.
4. **Biexponential**: Same idea: if an axis uses `TransformType::Biexponential`, use its `inverse_transform` in the custom tick formatter so labels show original scale.

## Implementation steps

1. **Tick formatter helper (flow-plots)**  
   - Add a helper that, given `&TransformType`, returns a `TickFormat::Custom(Arc<dyn Fn(f64) -> String + Send + Sync>)` (or the equivalent needed by kuva).  
   - Inside the closure: `inverse_transform(value as f32)` then format (e.g. `{:.1e}`).  
   - Optionally: use flow-fcs `Formattable::format` if we expose it and want one source of truth for label formatting.

2. **Layout builder for density/contour (kuva)**  
   - When we implement kuva-backed density (and contour), build a `Layout` with:  
     - `x_range` / `y_range` from `options.x_axis.range` / `options.y_axis.range` (already in transformed space).  
     - `log_x: false`, `log_y: false`.  
     - `x_tick_format`: from step 1 using `options.x_axis.transform`.  
     - `y_tick_format`: from step 1 using `options.y_axis.transform`.  
     - `x_label` / `y_label` from `options.x_axis.label` / `options.y_axis.label`.

3. **Density/contour → kuva plot type**  
   - Resolve the “Gap” in `kuva_backend.rs`: either feed kuva’s `Heatmap` with a 2D density grid + continuous ranges, or use an image/raster plot type that draws a pre-rendered buffer with the same `Layout` (linear axes + custom tick format).  
   - In both cases, pass the same `Layout` built as in step 2 so that axes are linear and ticks show original values.

4. **Wire RenderConfig / backend selection**  
   - When `render_pixels` / `render_contour` are switched to kuva (feature gate or default), ensure they use the new Layout builder and tick formatters so that arcsinh pre-transformed data always gets linear axes and original-value labels.

5. **Tests**  
   - Unit test: formatter closure for `Arcsinh { cofactor: 200 }` gives expected strings for a few transformed values (e.g. 0, 1, 5) vs known original values.  
   - Integration: one density (and optionally contour) plot rendered via kuva with arcsinh options; sanity-check axis labels (e.g. spot-check that labels increase in original space).

## Data flow summary

- **Upstream**: flow-fcs (or app) produces arcsinh-transformed data and optionally uses `density_options_from_fcs` (or equivalent) so that `DensityPlotOptions` has ranges and transforms in transformed space.
- **flow-plots**: Density pipeline uses (x, y) in transformed space and `options.x_axis.range` / `y_axis.range` (transformed). For kuva, we build linear axes with these ranges and set `x_tick_format` / `y_tick_format` to custom formatters that call `inverse_transform` to show original values.
- **User**: Sees linear spacing of data on screen and axis labels in original (e.g. fluorescence) units.

## Optional: flow-fcs Formattable

- If we want a single place for “transformed value → label string”, consider re-exporting or exposing `Formattable` from flow-fcs and using `transform.format(&value)` in the kuva tick closure instead of duplicating `inverse_transform` + format in flow-plots. The plotters_backend currently duplicates this in `format_transform_value`; we could replace both with flow-fcs `Formattable` in a follow-up.

---

## Implementation status (done)

- **Tick formatter**: `plots/src/render/kuva_axis.rs` — `tick_format_from_transform()` and `density_layout_from_options()`.
- **Kuva density path**: `plots/src/render/kuva_backend.rs` — `render_density_kuva()` using kuva `Histogram2D`, layout from options, JPEG output.
- **Wire-up**: When `raster` feature is enabled, `DensityPlot::render` (and `render_batch`) use `render_density_kuva` for density-type plots; contour still uses plotters.
- **Tests**: `kuva_axis::tests::test_tick_format_arcsinh`, `test_tick_format_linear`.
- **Showcase**: `cargo run -p flow-plots --features raster --example density_arcsinh_kuva` writes `plot_types_output/density_arcsinh_kuva.jpg` with linear axes and original-value tick labels.
