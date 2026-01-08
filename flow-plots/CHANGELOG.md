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
<csr-id-94934619d4cea454e9c38cddcc8f8d6d9ffbe068/>
<csr-id-45efa1279eed93d24d598682e3c2875a5859f05a/>
<csr-id-7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c/>
<csr-id-2638feaae082a369694370c9ba633c4c0ed7f083/>
<csr-id-670c81054b4e1a4455e5050f7888e5f96f1a35cb/>
<csr-id-2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e/>

### Chore

 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

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

 - 30 commits contributed to the release over the course of 1 calendar day.
 - 25 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Remove outdated keywords from Cargo.toml for flow-plots ([`62ee764`](https://github.com/jrmoynihan/flow/commit/62ee7640139a377207b7a6b5a5590081d473b0a4))
    - Release flow-plots v0.1.0 ([`7d7bd39`](https://github.com/jrmoynihan/flow/commit/7d7bd396e4a6571f80c4cfb8a61096f2beee777d))
    - Update dependencies and enhance documentation ([`670c810`](https://github.com/jrmoynihan/flow/commit/670c81054b4e1a4455e5050f7888e5f96f1a35cb))
    - Merge pull request #3 from jrmoynihan:flow-plots ([`91674e1`](https://github.com/jrmoynihan/flow/commit/91674e13a6dc21b9c1979d63bbaa161f28f9dc2b))
    - Merge branch 'main' into flow-plots ([`5977fb3`](https://github.com/jrmoynihan/flow/commit/5977fb309ee7e726e5e7cefca902278f155b79f8))
    - Update flow-fcs dependency version to 0.1.1 in Cargo.toml ([`2671217`](https://github.com/jrmoynihan/flow/commit/2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e))
    - Add CHANGELOG.md for project documentation ([`9493461`](https://github.com/jrmoynihan/flow/commit/94934619d4cea454e9c38cddcc8f8d6d9ffbe068))
    - Clean up unused imports in helper and density plot files ([`45efa12`](https://github.com/jrmoynihan/flow/commit/45efa1279eed93d24d598682e3c2875a5859f05a))
    - Swap to hybrid flow-fcs dependency entry ([`7d23a3f`](https://github.com/jrmoynihan/flow/commit/7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c))
    - Dependency updates ([`2638fea`](https://github.com/jrmoynihan/flow/commit/2638feaae082a369694370c9ba633c4c0ed7f083))
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

