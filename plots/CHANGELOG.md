# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Scatter overlay plots** (`PlotType::Scatter`): Discrete gate colors via `ScatterPlotData::with_gates()` and `gate_colors` option
- **Scatter colored by z-axis** (`PlotType::Intensity`): Continuous colormap via `ScatterPlotData::with_z()` and `z_range` option
- **Density point size**: `point_size` option now affects density heatmap (contribution radius per point), matching scatter behavior
- **Contour plots**: KDE-based contour lines with `contour_smoothing`, `draw_outliers`, `contour_level_count`
- **HistogramPlot**: New plot type with filled/unfilled modes, overlaid series, gate colors, baseline separation, `scale_to_peak`
- `ScatterPlotData`, `HistogramData`, `HistogramPlotOptions` types
- **`BasePlotOptions` field promotion**: `DensityPlotOptionsBuilder`, `HistogramPlotOptionsBuilder`, and `SpectralSignaturePlotOptionsBuilder` now expose direct passthrough setters (`.width()`, `.height()`, `.title()`, etc.) for every `BasePlotOptions` field, so common layout options no longer require building a separate `BasePlotOptions` and passing it via `.base(...)`. `.base(...)` still works unchanged.

### Changed

- **Breaking**: `DensityPlot::Data` is now `ScatterPlotData` instead of `Vec<(f32, f32)>`; use `.into()` for simple data
- **Breaking**: `render_batch` and `calculate_density_per_pixel_batch` accept `(ScatterPlotData, DensityPlotOptions)`

### Dependencies

- Added `flow-utils` for KDE contour support
- Added `ndarray` for marching squares contour extraction

## 0.3.2 (2026-07-22)

<csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/>
<csr-id-2fb6aa22990f90582f20d4e46f6bfc0701cd41e9/>
<csr-id-0883e2813c189a443bbe105808e302634a96abf6/>
<csr-id-4e8f876e384c47ce9c63579811b7f384bb84f21a/>
<csr-id-53ce755944243a1fdcef85d5f40a7fc59fd6ef1c/>
<csr-id-32a41aa05c1db8f10bf9cf8150b4bddd1872dd1f/>

### Chore

 - <csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/> bump flow-fcs to 0.4.0, add publish metadata to new crates
   - flow-fcs 0.3.0 → 0.4.0 (new compensation feature + public API)
   - flow-linalg, flow-density, flow-clustering: add repository field
     and smart-release scripts for first publish
   - Update all workspace consumers to ^0.4.0
 - <csr-id-2fb6aa22990f90582f20d4e46f6bfc0701cd41e9/> update Cargo.lock and Cargo.toml with new dependencies
   - Added multiple new dependencies in Cargo.lock, including colorous, core_maths, data-url, euclid, float-cmp, fontdue, image-webp, imagesize, pico-args, resvg, roxmltree, rustybuzz, simplecss, strict-num, svgtypes, and tiny-skia.
   - Updated Cargo.toml to modify the kuva dependency to include the "raster" feature.
   - Ensured consistency in dependency management across the project.
 - <csr-id-0883e2813c189a443bbe105808e302634a96abf6/> update dependencies in Cargo.lock and Cargo.toml
   - Added new dependencies: fontconfig-parser, fontdb, kurbo, kuva, and ttf-parser with their respective versions and checksums in Cargo.lock.
   - Updated Cargo.toml to include the kuva dependency from a Git repository with specific features.
   - Cleaned up formatting in Cargo.toml for consistency.

### New Features

 - <csr-id-fe642f65aa7d03fac6d688f4598143d8c2955137/> split flow-utils into flow-density + flow-clustering crates
   flow-utils bundled unrelated concerns (KDE, clustering, PCA). Split into
   focused crates so consumers only pull what they need. flow-utils removed
   from workspace members; existing code updated to import from new crates.
 - <csr-id-7674f5f49c84ab9f4147ae0fbd79dd9a823bfb29/> raw-scale AF medians plot for unstained control
   The normalised spectral plot (max=1.0) hides the actual AF values that
   get subtracted from every fluorophore control, so the user can't tell
   whether "AF=1.0 on B3-A" is 500 or 50,000 raw units. For the unstained
   control, the concrete per-detector medians are what matter.
   
   Add `generate_af_medians_plot`, written as step 08 under the unstained
   control's plot folder, with:
   - y-axis in raw detector units (not normalised)
   - y-axis range set to `[0, 1.1 * max_median]` so the peak isn't pinned
   to the frame
   - the max median printed in the title so the bar heights have a clear
   anchor
   
   To enable this, `render_spectral_signature` now honours
   `y_axis.range` when it differs from `AxisOptions::default()`. The
   default is treated as "unset" so existing normalised-spectrum call
   sites (all of them use a plain `AxisOptions::new()` without a range)
   keep their 0..1 axis unchanged.
 - <csr-id-e68b500d7a7c6f3874c16862e722addbcdf34b59/> contour paths API and density normalization percentile
   - Add contour_paths_at_threshold for ordered closed paths from KDE
   - Optional density_normalization_percentile for colormap scaling
   - Wire plotters backend options as needed
 - <csr-id-d09dc93e60217d9cd87f787d8ce63b2ec89e10d2/> expose kuva raster APIs for Tauri/zero-copy display
   - Add optional 'raster' feature with kuva fork dependency
   - New kuva module: render_to_rgba, render_to_rgba_no_text,
   render_to_png_direct, render_to_png_direct_no_text
   - Re-export render_to_raster, render_to_raster_no_text, Layout, Plot
   - Document raster feature in README
 - <csr-id-8d79dd17b3a38a8bcdc26126333abf8d2555fcd9/> implement contour path clipping to axis range
   - Added `clip_contour_paths` function to clamp contour path points to specified x and y axis ranges, dropping degenerate paths.
   - Updated `calculate_contours` to utilize the new clipping function, ensuring contour paths do not exceed the chart's axis range.
   - Enhanced documentation for `x_range` and `y_range` parameters to clarify their purpose.
   - Added regression tests to verify clipping behavior and prevent panics when rendering contours with out-of-range data.

### Refactor

 - <csr-id-4e8f876e384c47ce9c63579811b7f384bb84f21a/> simplify PlotType enum to Scatter, Density, Intensity, Contour, Histogram
   Remove legacy variants (ScatterSolid, Dot, Zebra, ContourOverlay, ScatterOverlay,
   ScatterColoredContinuous) and consolidate into fewer, clearer types. Simplify
   match arms in calculate_plot_pixels accordingly.
 - <csr-id-53ce755944243a1fdcef85d5f40a7fc59fd6ef1c/> improve density KDE, contour extraction, and histogram rendering
   Refines density grid calculations, contour extraction logic, and
   kuva backend histogram/spectral rendering.
 - <csr-id-32a41aa05c1db8f10bf9cf8150b4bddd1872dd1f/> move kuva into render, add kuva_backend and kuva_axis
   - Replace top-level kuva module with render::kuva_backend and render::kuva_axis
   - Add density arcsinh kuva examples and arcsinh-kuva plan doc
   - Update density options, batch_density bench, and plot_types_validation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 19 commits contributed to the release.
 - 11 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Release flow-fcs v0.4.1 ([`597f21b`](https://github.com/jrmoynihan/flow/commit/597f21bef7ea787437071685fc3cce9d2269270f))
    - Simplify PlotType enum to Scatter, Density, Intensity, Contour, Histogram ([`4e8f876`](https://github.com/jrmoynihan/flow/commit/4e8f876e384c47ce9c63579811b7f384bb84f21a))
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Split flow-utils into flow-density + flow-clustering crates ([`fe642f6`](https://github.com/jrmoynihan/flow/commit/fe642f65aa7d03fac6d688f4598143d8c2955137))
    - Improve density KDE, contour extraction, and histogram rendering ([`53ce755`](https://github.com/jrmoynihan/flow/commit/53ce755944243a1fdcef85d5f40a7fc59fd6ef1c))
    - Raw-scale AF medians plot for unstained control ([`7674f5f`](https://github.com/jrmoynihan/flow/commit/7674f5f49c84ab9f4147ae0fbd79dd9a823bfb29))
    - Contour paths API and density normalization percentile ([`e68b500`](https://github.com/jrmoynihan/flow/commit/e68b500d7a7c6f3874c16862e722addbcdf34b59))
    - Move kuva into render, add kuva_backend and kuva_axis ([`32a41aa`](https://github.com/jrmoynihan/flow/commit/32a41aa05c1db8f10bf9cf8150b4bddd1872dd1f))
    - Update Cargo.lock and Cargo.toml with new dependencies ([`2fb6aa2`](https://github.com/jrmoynihan/flow/commit/2fb6aa22990f90582f20d4e46f6bfc0701cd41e9))
    - Merge pull request #19 from jrmoynihan/cursor/kuva-rendering-api-exposure-9206 ([`7f840c8`](https://github.com/jrmoynihan/flow/commit/7f840c81bbe4c6749ed77bd7cd052a18bc64ab5c))
    - Merge branch 'main' into cursor/kuva-rendering-api-exposure-9206 ([`370a4cd`](https://github.com/jrmoynihan/flow/commit/370a4cdadb8481166ef17239d6fbe55c6c0a831a))
    - Update dependencies in Cargo.lock and Cargo.toml ([`0883e28`](https://github.com/jrmoynihan/flow/commit/0883e2813c189a443bbe105808e302634a96abf6))
    - Expose kuva raster APIs for Tauri/zero-copy display ([`d09dc93`](https://github.com/jrmoynihan/flow/commit/d09dc93e60217d9cd87f787d8ce63b2ec89e10d2))
    - Implement contour path clipping to axis range ([`8d79dd1`](https://github.com/jrmoynihan/flow/commit/8d79dd17b3a38a8bcdc26126333abf8d2555fcd9))
</details>

## 0.3.1 (2026-03-05)

<csr-id-9ab231f53ebb8a4aa8cefbc9db2542a69bbd66ca/>

### Chore

 - <csr-id-9ab231f53ebb8a4aa8cefbc9db2542a69bbd66ca/> remove point size UI mapping; use backend values directly

### New Features

 - <csr-id-bf487fdc0b7b021c358f6026e46dd2418080d51c/> point size UI mapping, contour path docs, validation example
   - Add map_point_size_from_ui() and .point_size_from_ui() for 0.05-1.0 -> 0.1-4.0
- Document that Contour/ContourOverlay need DensityPlot::render, not pixel APIs
- Add plot_types_validation example for visual validation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.3.1 ([`2050584`](https://github.com/jrmoynihan/flow/commit/2050584238b7b516ee209e4f0cb67543d3c3ba09))
    - Update CHANGELOG for unreleased features and chore updates ([`44a0537`](https://github.com/jrmoynihan/flow/commit/44a0537157521ed77ee0098f6051f6e64e6f56d0))
    - Merge branch 'cursor/axis-gate-interaction-630e' into main ([`c021235`](https://github.com/jrmoynihan/flow/commit/c021235f1555962be2177f2edd5a49de646effd4))
    - Remove point size UI mapping; use backend values directly ([`9ab231f`](https://github.com/jrmoynihan/flow/commit/9ab231f53ebb8a4aa8cefbc9db2542a69bbd66ca))
    - Point size UI mapping, contour path docs, validation example ([`bf487fd`](https://github.com/jrmoynihan/flow/commit/bf487fdc0b7b021c358f6026e46dd2418080d51c))
</details>

## 0.3.0 (2026-03-04)

<csr-id-fd1cc4a76af40804018e24792dce407860302857/>
<csr-id-a1e9c1ff01eadccf8c24e6d58d39661fb7d8a22b/>

### Chore

 - <csr-id-fd1cc4a76af40804018e24792dce407860302857/> Release
 - <csr-id-a1e9c1ff01eadccf8c24e6d58d39661fb7d8a22b/> update CHANGELOG for v0.3.0 release

### New Features

 - <csr-id-8b26c1418137646bb311d45a678d1d43ef05a22d/> scatter overlay, z-axis coloring, density point size, contours, histograms
   - ScatterPlotData: discrete gate colors (ScatterOverlay), continuous z-axis (ScatterColoredContinuous)
- Density plots: point_size affects contribution radius (matches scatter behavior)
- Contour plots: KDE-based contours, draw_outliers, contour_smoothing
- HistogramPlot: filled/unfilled, overlaid with gate colors, baseline separation, scale_to_peak
- Breaking: DensityPlot::Data is now ScatterPlotData; use .into() for Vec<(f32,f32)>
- Updated tru-ols, tru-ols-cli, gates for new API

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release ([`fd1cc4a`](https://github.com/jrmoynihan/flow/commit/fd1cc4a76af40804018e24792dce407860302857))
    - Update CHANGELOG for v0.3.0 release ([`a1e9c1f`](https://github.com/jrmoynihan/flow/commit/a1e9c1ff01eadccf8c24e6d58d39661fb7d8a22b))
    - Scatter overlay, z-axis coloring, density point size, contours, histograms ([`8b26c14`](https://github.com/jrmoynihan/flow/commit/8b26c1418137646bb311d45a678d1d43ef05a22d))
    - Release flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`4ab83fe`](https://github.com/jrmoynihan/flow/commit/4ab83fe18e7f67bba8c1ce2bf8163e8652a9a592))
</details>

## 0.2.2 (2026-02-26)

<csr-id-9292dc407a5eaab2aa949fdfc1d2abdfcb32798d/>
<csr-id-895fb633cf95fe04939a714f7a41f9e019fce35f/>
<csr-id-ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c/>

### New Features

 - <csr-id-42ff5a7c597cdda6b9340c4f98e3f27f6e5a7feb/> enhance density and scatter plot functionality
   - Added `scatter_to_pixels` function to convert scatter (x,y) points into pixel data for solid scatter plots.

### Refactor

 - <csr-id-9292dc407a5eaab2aa949fdfc1d2abdfcb32798d/> replace matches! with assert_eq! for transform type assertions
   - Updated test assertions to use assert_eq! instead of matches! for checking TransformType values, improving clarity and consistency in test validations.
   - Ensured that the tests remain robust while enhancing readability of the assertions.
 - <csr-id-895fb633cf95fe04939a714f7a41f9e019fce35f/> unify plot options initialization with BasePlotOptions
   - Updated various plot options to utilize BasePlotOptions for consistent width and height settings across DensityPlotOptions and SpectralSignaturePlotOptions.
   - Enhanced documentation examples to reflect the new initialization method, improving clarity for users.
   - Removed redundant width and height setters in favor of a unified base configuration.

### Chore

 - <csr-id-ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c/> update changelogs and READMEs for flow-fcs, flow-plots, peacoqc-rs patch release

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 11 calendar days.
 - 11 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`d8a6922`](https://github.com/jrmoynihan/flow/commit/d8a6922a47b2196a6dcf8362bab067b176757908))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Update changelogs and READMEs for flow-fcs, flow-plots, peacoqc-rs patch release ([`ec0fcf8`](https://github.com/jrmoynihan/flow/commit/ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c))
    - Replace matches! with assert_eq! for transform type assertions ([`9292dc4`](https://github.com/jrmoynihan/flow/commit/9292dc407a5eaab2aa949fdfc1d2abdfcb32798d))
    - Unify plot options initialization with BasePlotOptions ([`895fb63`](https://github.com/jrmoynihan/flow/commit/895fb633cf95fe04939a714f7a41f9e019fce35f))
    - Enhance density and scatter plot functionality ([`42ff5a7`](https://github.com/jrmoynihan/flow/commit/42ff5a7c597cdda6b9340c4f98e3f27f6e5a7feb))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
</details>

## 0.2.1 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/>

### Chore

 - <csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/> update dependencies and align workspace configurations
   - Updated various dependencies in Cargo.toml files across multiple crates to their latest versions for improved functionality and compatibility.
   - Changed several dependencies to use workspace references for consistency and to reduce duplication.
   - Notable updates include polars to version 0.53.0, faer to version 0.24, and ndarray-linalg to version 0.18.1.
   - Adjusted dev-dependencies to utilize workspace settings for better management.
 - <csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/> clean up unused imports and variables
   - Remove unused imports in clustering and gating modules
   - Fix unreachable code warning in DBSCAN
   - Remove unused mut keywords
   - Clean up warnings for better code quality

### Chore

 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release

### New Features

 - <csr-id-adae5a601646d41300edeaa4ec0542c0a665b05f/> add spectral plots and signal heatmap
   - Add SpectralSignaturePlot and spectral plot options

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 24 calendar days.
 - 24 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Add spectral plots and signal heatmap ([`adae5a6`](https://github.com/jrmoynihan/flow/commit/adae5a601646d41300edeaa4ec0542c0a665b05f))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
    - Merge pull request #10 from jrmoynihan/gpu-acceleration ([`69363eb`](https://github.com/jrmoynihan/flow/commit/69363eb3a664b1aa6cd0be9b980ec08fc03b7955))
</details>

## 0.2.0 (2026-01-21)

<csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/>

### Refactor (BREAKING)

 - <csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/> remove GPU acceleration implementation
   - Remove GPU module and all GPU-related code
   - Remove GPU dependencies (burn, cubecl, bytemuck)
   - Remove GPU feature flags from Cargo.toml
   - Update batch functions to use CPU-only implementation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.0, flow-plots v0.2.0 ([`3620154`](https://github.com/jrmoynihan/flow/commit/3620154c694500bb2ff2edbdf0848076287d77d3))
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Remove GPU acceleration implementation ([`fec1c6d`](https://github.com/jrmoynihan/flow/commit/fec1c6d2c50730d98771b7cdc101bad5071baf29))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
</details>

## 0.1.3 (2026-01-21)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 3 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjusting changelogs prior to release of flow-fcs v0.1.5, flow-plots v0.1.3, flow-gates v0.1.2 ([`0fb3ddf`](https://github.com/jrmoynihan/flow/commit/0fb3ddfaf836bf0fb87f5f14dbe542494706f3af))
    - Adjusting changelogs prior to release of flow-fcs v0.1.5, flow-plots v0.1.3, flow-gates v0.1.2 ([`9c8f44a`](https://github.com/jrmoynihan/flow/commit/9c8f44a6b5908a262825a2daa8b3963fdea99a11))
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
</details>

## 0.1.2 (2026-01-18)

<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>
<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/>

### Chore

 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.
 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands
 - <csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/> :hammer: Add cargo scripts for testing and release management for each crate
   Granular control at the crate level.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 12 commits contributed to the release over the course of 4 calendar days.
 - 4 days passed between releases.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.2, flow-gates v0.1.1 ([`2c36741`](https://github.com/jrmoynihan/flow/commit/2c367411265c8385e88b2653e278bd1e2d1d2198))
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`f08823c`](https://github.com/jrmoynihan/flow/commit/f08823cabcae5223efe4250471dd75ea7fcaa936))
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - :hammer: Add cargo scripts for testing and release management for each crate ([`a6a4ff7`](https://github.com/jrmoynihan/flow/commit/a6a4ff733ae38acaec36d3327f4952d6fded3c0f))
</details>

## 0.1.1 (2026-01-14)

<csr-id-8818e480d33513c1bb724432a734b76ac57b95f9/>
<csr-id-f64872e441add42bc9d19280d4411df628ff853e/>
<csr-id-a59079c54a230e816e69cd17e309d9ff66b1bea6/>
<csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/>
<csr-id-14aae61d8d5ccf6b031c3ac9dc310dfb89a383af/>
<csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/>

### Chore

 - <csr-id-8818e480d33513c1bb724432a734b76ac57b95f9/> update plotting backend and bindings
   - Update plotters backend implementation
   - Update TypeScript bindings for pixel data

### Chore

 - <csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/> Update CHANGELOG for upcoming release
   - Documented version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization.

### Chore

 - <csr-id-14aae61d8d5ccf6b031c3ac9dc310dfb89a383af/> Update CHANGELOG for upcoming release
   - documented version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - updated plotting backend and TypeScript bindings for pixel data
   - refactored folder names for better organization

### Chore

 - <csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.
 - <csr-id-a59079c54a230e816e69cd17e309d9ff66b1bea6/> removed unused RawPixelData import

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 5 calendar days.
 - 5 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.1, flow-gates v0.1.0 ([`b5be6ba`](https://github.com/jrmoynihan/flow/commit/b5be6ba4e2093a8b0e972bd44265fa51b8c6be13))
    - Update CHANGELOG for upcoming release ([`f0f0ab2`](https://github.com/jrmoynihan/flow/commit/f0f0ab21b68eb1a28903957bae137f326b5a082b))
    - Update CHANGELOG for upcoming release ([`14aae61`](https://github.com/jrmoynihan/flow/commit/14aae61d8d5ccf6b031c3ac9dc310dfb89a383af))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`621d3ad`](https://github.com/jrmoynihan/flow/commit/621d3aded59ff51f953c6acdb75027c4541a8b97))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Update plotting backend and bindings ([`8818e48`](https://github.com/jrmoynihan/flow/commit/8818e480d33513c1bb724432a734b76ac57b95f9))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - :truck: Rnamed folders without the `flow-` prefix. ([`f64872e`](https://github.com/jrmoynihan/flow/commit/f64872e441add42bc9d19280d4411df628ff853e))
    - Removed unused RawPixelData import ([`a59079c`](https://github.com/jrmoynihan/flow/commit/a59079c54a230e816e69cd17e309d9ff66b1bea6))
</details>

## v0.1.0 (2026-01-08)

<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-d3588b056c11325475ed470006c2829b7d0c1528/>
<csr-id-27e7e939f80820adc297ed7193ba87f3a4e759bb/>
<csr-id-9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e/>
<csr-id-71b90a5b4f798e27fff5634048ad12a9ff57684a/>
<csr-id-94934619d4cea454e9c38cddcc8f8d6d9ffbe068/>
<csr-id-45efa1279eed93d24d598682e3c2875a5859f05a/>
<csr-id-7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c/>
<csr-id-2638feaae082a369694370c9ba633c4c0ed7f083/>
<csr-id-670c81054b4e1a4455e5050f7888e5f96f1a35cb/>
<csr-id-2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e/>
<csr-id-62ee7640139a377207b7a6b5a5590081d473b0a4/>
<csr-id-a236a374302ae611992d7cabec69f7d732c76f54/>
<csr-id-09d31bc88283911ce2856b59311f83fe2dcf5e52/>
<csr-id-f79650c2ce3161b7cc212e87a02738da9c1647a1/>
<csr-id-8fa97683337b2a912ad4ed0d835d4e066099944a/>

### Chore

 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

### Style

 - <csr-id-a236a374302ae611992d7cabec69f7d732c76f54/> :truck: Renamed density calculation module to clarify from density plot implementation

### Chore

 - <csr-id-09d31bc88283911ce2856b59311f83fe2dcf5e52/> remove outdated keywords from Cargo.toml for flow-plots
 - <csr-id-f79650c2ce3161b7cc212e87a02738da9c1647a1/> update dependencies and enhance documentation
   - Added `approx` dependency to Cargo.lock.
   - Created a comprehensive CHANGELOG.md to document notable changes and adhere to Semantic Versioning.
   - Enhanced README documentation for the flow-plots library, detailing features and usage examples.
 - <csr-id-8fa97683337b2a912ad4ed0d835d4e066099944a/> add CHANGELOG.md for project documentation
   - Created a new CHANGELOG.md file to document all notable changes to the flow-plots project.
   - The changelog follows the Keep a Changelog format and adheres to Semantic Versioning.
   - Included detailed entries for version 0.1.0, summarizing new features, refactors, and enhancements made to the library.

### Chore

 - <csr-id-62ee7640139a377207b7a6b5a5590081d473b0a4/> remove outdated keywords from Cargo.toml for flow-plots

### Refactor

 - <csr-id-45efa1279eed93d24d598682e3c2875a5859f05a/> clean up unused imports in helper and density plot files

### Other

 - <csr-id-7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c/> swap to hybrid flow-fcs dependency entry
   allows use of local path during dev and uses the specified version when publishing
 - <csr-id-2638feaae082a369694370c9ba633c4c0ed7f083/> dependency updates
   - enabled the `preset` feature for colorgrad
   - upgraded to flow-fcs 0.1.1

### Chore

 - <csr-id-670c81054b4e1a4455e5050f7888e5f96f1a35cb/> update dependencies and enhance documentation
   - Added `approx` dependency to Cargo.lock.
   - Created a comprehensive CHANGELOG.md to document notable changes and adhere to Semantic Versioning.
   - Enhanced README documentation for the flow-plots library, detailing features and usage examples.
 - <csr-id-2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e/> update flow-fcs dependency version to 0.1.1 in Cargo.toml

### Chore

 - <csr-id-94934619d4cea454e9c38cddcc8f8d6d9ffbe068/> add CHANGELOG.md for project documentation
   - Created a new CHANGELOG.md file to document all notable changes to the flow-plots project.
   - The changelog follows the Keep a Changelog format and adheres to Semantic Versioning.
   - Included detailed entries for version 0.1.0, summarizing new features, refactors, and enhancements made to the library.

### New Features

<csr-id-d807135b00ee17c86bacfebfee220c94a0f4d6bd/>
<csr-id-94d528cc854e4bad71b2cb34df240be2a9c7109d/>
<csr-id-4154d225125c80e22d560c063c679e4063369c63/>
<csr-id-2505a8f0dd3962b24712946402d753bc19e8daa5/>
<csr-id-ed202c7c543ca8a647b5668e24adba7085e94444/>
<csr-id-220489b4562ffb0afe5cb8cae623380ded34a48d/>
<csr-id-a6a1809d4cbe3da0fb712c77763148fa5f260157/>
<csr-id-54c9c93a9f7a4a8157273c467321278399d2b16c/>
<csr-id-3f2afb485498e75aa4b9c5c2b32e0c046a184011/>
<csr-id-042fa281cdc29de70599cac2286bcebf724e9a65/>
<csr-id-dacce4b785a61fc7082889ccb14fe3e76c4e582a/>
<csr-id-cc5fb636e31f055894a5f36c0472c3122b996016/>

 - <csr-id-c27cf93f445a37e318fabb882968a56775d48a8d/> add BinaryPixelChunk and RawPixelData types for optimized pixel handling and binding to frontend (TS) code
   - Introduced `BinaryPixelChunk` type for efficient data transfer, encapsulating raw RGB pixel data along with metadata for canvas rendering.

### Refactor

 - <csr-id-d3588b056c11325475ed470006c2829b7d0c1528/> reorganize plot types and remove legacy density plotting code
   - Deleted the old `plot_types.rs` file, which contained the previous implementation of density plotting.
   - Introduced a new `mod.rs` file in the `plots` directory to better structure plot-related modules and improve organization.
   - Retained the `PlotType` enum for various plot types, ensuring compatibility with existing implementations while enhancing clarity and maintainability.
 - <csr-id-27e7e939f80820adc297ed7193ba87f3a4e759bb/> update density calculation to use DensityPlotOptions
   - Replaced references to the deprecated `PlotOptions` with the new `DensityPlotOptions` struct for improved configuration of density plots.
   - Corrected y-axis scaling calculation to utilize the new axis range properties.
   - Updated color mapping to use the specified colormap from `DensityPlotOptions`, enhancing flexibility in visualization.
 - <csr-id-9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e/> remove executor module for plot job management
   - Deleted the `executor.rs` file, since it deals with application-specific runtime logic

### Style

 - <csr-id-71b90a5b4f798e27fff5634048ad12a9ff57684a/> :truck: Renamed density calculation module to clarify from density plot implementation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 41 commits contributed to the release over the course of 1 calendar day.
 - 29 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.0 ([`43f1292`](https://github.com/jrmoynihan/flow/commit/43f12921cbb9a04491c401b433e90cc7733d51f9))
    - Release flow-plots v0.1.0 ([`3f63b79`](https://github.com/jrmoynihan/flow/commit/3f63b794fbfeb219acdb1115ad12bb1ce8494b73))
    - Remove outdated keywords from Cargo.toml for flow-plots ([`62ee764`](https://github.com/jrmoynihan/flow/commit/62ee7640139a377207b7a6b5a5590081d473b0a4))
    - Remove outdated keywords from Cargo.toml for flow-plots ([`09d31bc`](https://github.com/jrmoynihan/flow/commit/09d31bc88283911ce2856b59311f83fe2dcf5e52))
    - Release flow-plots v0.1.0 ([`7d7bd39`](https://github.com/jrmoynihan/flow/commit/7d7bd396e4a6571f80c4cfb8a61096f2beee777d))
    - Release flow-plots v0.1.0 ([`e6a02b8`](https://github.com/jrmoynihan/flow/commit/e6a02b89a0e26b18f26d9f9d04a5b11530cca8e4))
    - Update dependencies and enhance documentation ([`670c810`](https://github.com/jrmoynihan/flow/commit/670c81054b4e1a4455e5050f7888e5f96f1a35cb))
    - Update dependencies and enhance documentation ([`f79650c`](https://github.com/jrmoynihan/flow/commit/f79650c2ce3161b7cc212e87a02738da9c1647a1))
    - Merge pull request #3 from jrmoynihan:flow-plots ([`91674e1`](https://github.com/jrmoynihan/flow/commit/91674e13a6dc21b9c1979d63bbaa161f28f9dc2b))
    - Merge pull request #3 from jrmoynihan:flow-plots ([`4ab8f89`](https://github.com/jrmoynihan/flow/commit/4ab8f895f8642b59274726cc7f254187e0b14602))
    - Merge branch 'main' into flow-plots ([`5977fb3`](https://github.com/jrmoynihan/flow/commit/5977fb309ee7e726e5e7cefca902278f155b79f8))
    - Merge branch 'main' into flow-plots ([`d7b6226`](https://github.com/jrmoynihan/flow/commit/d7b62269232f1bc6a8b155fd44d905e0a6233887))
    - Update flow-fcs dependency version to 0.1.1 in Cargo.toml ([`2671217`](https://github.com/jrmoynihan/flow/commit/2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e))
    - Add CHANGELOG.md for project documentation ([`9493461`](https://github.com/jrmoynihan/flow/commit/94934619d4cea454e9c38cddcc8f8d6d9ffbe068))
    - Add CHANGELOG.md for project documentation ([`8fa9768`](https://github.com/jrmoynihan/flow/commit/8fa97683337b2a912ad4ed0d835d4e066099944a))
    - Clean up unused imports in helper and density plot files ([`45efa12`](https://github.com/jrmoynihan/flow/commit/45efa1279eed93d24d598682e3c2875a5859f05a))
    - Swap to hybrid flow-fcs dependency entry ([`7d23a3f`](https://github.com/jrmoynihan/flow/commit/7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c))
    - Dependency updates ([`2638fea`](https://github.com/jrmoynihan/flow/commit/2638feaae082a369694370c9ba633c4c0ed7f083))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`987314d`](https://github.com/jrmoynihan/flow/commit/987314dd1120fb723aad0946d8bfb0e882d39454))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`46431c0`](https://github.com/jrmoynihan/flow/commit/46431c0431afb4b7fa7de240595ac5726e693242))
    - :truck: Renamed density calculation module to clarify from density plot implementation ([`a236a37`](https://github.com/jrmoynihan/flow/commit/a236a374302ae611992d7cabec69f7d732c76f54))
    - :truck: Renamed density calculation module to clarify from density plot implementation ([`71b90a5`](https://github.com/jrmoynihan/flow/commit/71b90a5b4f798e27fff5634048ad12a9ff57684a))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`3d994a8`](https://github.com/jrmoynihan/flow/commit/3d994a81aa585e6d5263c5f9d1db7d36106698d2))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`708ddca`](https://github.com/jrmoynihan/flow/commit/708ddca0149fe7f5c6627e052207d78f06b55ed6))
    - Add BinaryPixelChunk and RawPixelData types for optimized pixel handling and binding to frontend (TS) code ([`c27cf93`](https://github.com/jrmoynihan/flow/commit/c27cf93f445a37e318fabb882968a56775d48a8d))
    - Add README documentation for flow-plots library ([`d807135`](https://github.com/jrmoynihan/flow/commit/d807135b00ee17c86bacfebfee220c94a0f4d6bd))
    - Integrate colorgrad for enhanced colormap options in density plots ([`94d528c`](https://github.com/jrmoynihan/flow/commit/94d528cc854e4bad71b2cb34df240be2a9c7109d))
    - Add helper function for creating DensityPlotOptions from FCS data ([`4154d22`](https://github.com/jrmoynihan/flow/commit/4154d225125c80e22d560c063c679e4063369c63))
    - Reorganize plot types and remove legacy density plotting code ([`d3588b0`](https://github.com/jrmoynihan/flow/commit/d3588b056c11325475ed470006c2829b7d0c1528))
    - Enhance testing framework for density plots and options ([`2505a8f`](https://github.com/jrmoynihan/flow/commit/2505a8f0dd3962b24712946402d753bc19e8daa5))
    - Add ProgressInfo struct and callback type for rendering progress ([`ed202c7`](https://github.com/jrmoynihan/flow/commit/ed202c7c543ca8a647b5668e24adba7085e94444))
    - Add rendering capabilities for density plots ([`220489b`](https://github.com/jrmoynihan/flow/commit/220489b4562ffb0afe5cb8cae623380ded34a48d))
    - Implement DensityPlot for 2D density visualization ([`a6a1809`](https://github.com/jrmoynihan/flow/commit/a6a1809d4cbe3da0fb712c77763148fa5f260157))
    - Update density calculation to use DensityPlotOptions ([`27e7e93`](https://github.com/jrmoynihan/flow/commit/27e7e939f80820adc297ed7193ba87f3a4e759bb))
    - Add DensityPlotOptions struct for density plot configuration ([`54c9c93`](https://github.com/jrmoynihan/flow/commit/54c9c93a9f7a4a8157273c467321278399d2b16c))
    - Add AxisOptions struct for plot axis configuration ([`3f2afb4`](https://github.com/jrmoynihan/flow/commit/3f2afb485498e75aa4b9c5c2b32e0c046a184011))
    - Introduce Plot trait for customizable plot types ([`042fa28`](https://github.com/jrmoynihan/flow/commit/042fa281cdc29de70599cac2286bcebf724e9a65))
    - Add BasePlotOptions struct for plot configuration ([`dacce4b`](https://github.com/jrmoynihan/flow/commit/dacce4b785a61fc7082889ccb14fe3e76c4e582a))
    - Remove executor module for plot job management ([`9f7e36c`](https://github.com/jrmoynihan/flow/commit/9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e))
    - Implement density plotting with optimized pixel rendering ([`cc5fb63`](https://github.com/jrmoynihan/flow/commit/cc5fb636e31f055894a5f36c0472c3122b996016))
    - Reorganize workspace into separate crates ([`fd12ce3`](https://github.com/jrmoynihan/flow/commit/fd12ce3ff00c02e75c9ea84848adb58b32c4d66f))
</details>

