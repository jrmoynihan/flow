# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.0 (2026-01-08)

<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-d3588b056c11325475ed470006c2829b7d0c1528/>
<csr-id-27e7e939f80820adc297ed7193ba87f3a4e759bb/>
<csr-id-9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e/>
<csr-id-71b90a5b4f798e27fff5634048ad12a9ff57684a/>

### Chore

 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

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
- Added `RawPixelData` type to facilitate optimized density calculations, featuring key performance improvements and a streamlined structure for pixel data representation.
- Both types are designed to enhance rendering performance and memory efficiency in the flow-plots library.
- Introduced a comprehensive README.md file detailing the flow-plots library, including an overview, features, and basic usage examples for creating density plots.
- Documented the architecture of the library, outlining the organization of modules and the process for adding new plot types.
- Included detailed instructions for using the library with FCS files, highlighting key functionalities and error handling.
- Enhanced user experience by providing clear examples and explanations of the API, facilitating easier adoption and integration.
- Added support for the `colorgrad` library to provide a variety of perceptually uniform colormaps for density plots.
- Expanded the `ColorMaps` enum to include new colormap options such as `Plasma`, `Inferno`, `Magma`, and others, improving visualization capabilities.
- Updated the `map` method to utilize `colorgrad` for color mapping based on normalized values, enhancing the accuracy and aesthetics of density visualizations.
- Modified dependencies in `Cargo.toml` to include `colorgrad` and other necessary libraries for improved functionality.
- Introduced `density_options_from_fcs` function to generate a `DensityPlotOptionsBuilder` with default configurations based on FCS file parameters.
- Implemented logic to determine appropriate plot ranges and transformations for x and y axes, enhancing usability for density plot creation.
- Included detailed documentation and examples for the new helper function, improving developer experience.
- Expanded the test suite for density plots, including comprehensive tests for `BasePlotOptions`, `AxisOptions`, and `DensityPlotOptions`.
- Introduced a new module for organizing plot options, encapsulating axis, base, and density options with builder patterns for improved usability.
- Added helper functions and tests for creating test FCS data, ensuring robust validation of density plot configurations and rendering.
- Implemented tests for percentile bounds and axis specifications, enhancing the reliability of the plotting framework.
- Introduced `ProgressInfo` struct to encapsulate pixel data and rendering progress percentage.
- Added `ProgressCallback` type for reporting rendering progress, allowing for error handling without interrupting the rendering process.
- Introduced `RenderConfig` struct to manage rendering configuration and progress reporting.
- Implemented `render_pixels` function to handle the complete rendering pipeline for density plots, including pixel buffer management and JPEG encoding.
- Integrated progress reporting during pixel rendering, allowing applications to track rendering status.
- Added utility functions for formatting transform values for axis labels.
- Added a new `DensityPlot` struct to create 2D density plots from (x, y) coordinate pairs.
- Implemented the `render` method to calculate pixel density and render the plot using specified options.
- Included an example in the documentation to demonstrate usage of the `DensityPlot` and `DensityPlotOptions` for configuration.
- Introduced a new `DensityPlotOptions` struct to encapsulate options for creating density plots, including base layout, axis configurations, and color map selection.
- Implemented a builder pattern for constructing `DensityPlotOptions`, improving usability.
- Marked the old `PlotOptions` struct as deprecated, encouraging users to transition to the new structure.
- Included an example in the documentation to demonstrate usage.
- Introduced a new `AxisOptions` struct to define options for configuring a plot axis, including range, transformation, and label.
- Implemented a builder pattern for creating instances of `AxisOptions`, improving usability.
- Provided default values for axis range and transformation, enhancing the flexibility of axis configuration.
- Included an example in the documentation to demonstrate usage.
- Added a new `Plot` trait that defines the interface for all plot types, specifying their options and data types.
- Included an example implementation to demonstrate how to create custom plot types with specific options and rendering logic.
- This addition enhances the flexibility and extensibility of the plotting framework.
- Introduced a new `BasePlotOptions` struct to encapsulate common layout and display settings for plots.
- Implemented a builder pattern for creating instances of `BasePlotOptions`, enhancing usability.
- Provided default values for plot dimensions, margins, and title.
- Marked the old `PlotOptions` struct as deprecated, encouraging users to transition to the new structure.
- Introduced a new `DensityPlot` module for visualizing flow cytometry data.
- Added `RawPixelData` and `BinaryPixelChunk` structures for efficient pixel data handling.
- Implemented optimized density calculation methods, significantly improving performance and memory usage.
- Created a `ColorMaps` enum for various colormap options, enhancing visualization capabilities.
- Updated `Cargo.toml` to include new dependencies for colormap and image processing.
- Added comprehensive tests for the new plotting functionality and options.

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

 - 21 commits contributed to the release over the course of 1 calendar day.
 - 19 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Add CHANGELOG.md for project documentation ([`9493461`](https://github.com/jrmoynihan/flow/commit/94934619d4cea454e9c38cddcc8f8d6d9ffbe068))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`46431c0`](https://github.com/jrmoynihan/flow/commit/46431c0431afb4b7fa7de240595ac5726e693242))
    - :truck: Renamed density calculation module to clarify from density plot implementation ([`71b90a5`](https://github.com/jrmoynihan/flow/commit/71b90a5b4f798e27fff5634048ad12a9ff57684a))
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

<csr-unknown>
 add README documentation for flow-plots library integrate colorgrad for enhanced colormap options in density plots add helper function for creating DensityPlotOptions from FCS data enhance testing framework for density plots and options add ProgressInfo struct and callback type for rendering progress add rendering capabilities for density plots implement DensityPlot for 2D density visualization add DensityPlotOptions struct for density plot configuration add AxisOptions struct for plot axis configuration introduce Plot trait for customizable plot types add BasePlotOptions struct for plot configuration implement density plotting with optimized pixel rendering<csr-unknown/>

