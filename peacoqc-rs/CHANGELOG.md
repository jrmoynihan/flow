# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.4 (2026-02-27)

### Added

- **Plot aesthetics**: Larger default fonts; configurable axis, legend, and title styling; grid and MAD shown as dashed lines; legend with semi-transparent background; spline in blue and MAD in green; scatter alpha for readability.
- **QCPlotConfig**: New `caption_font_size` and `font_family` options for plot typography.

### Fixed

- **GPU**: Corrected tensor dtype for Burn backend (use f32 consistently).

### Bug Fixes

 - <csr-id-bf4438343008fc1f4501b1ba13e661cfe9d0c6f1/> remove duplicate entry for bad events visualization in QC plots

### New Features

 - <csr-id-7aad630ce2e5e9ebcda2273cef86f22777efc05b/> plot aesthetics, GPU fix, CLI plot options
   - peacoqc-rs: Plot improvements (issue #16): larger fonts, axis/legend/title
     config; grid and MAD dashed lines; legend semi-transparent background;
     spline blue/MAD green; scatter alpha. Add caption_font_size and
     font_family to QCPlotConfig. Fix GPU tensor shape (Burn Float is f32).
   - peacoqc-cli: Apply --hide-spline-mad and --show-bin-boundaries; add
     --plot-width, --plot-height, --plot-title-size, --plot-axis-size,
     --plot-tick-size, --plot-legend-size, --plot-font; document in README.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Plot aesthetics, GPU fix, CLI plot options ([`7aad630`](https://github.com/jrmoynihan/flow/commit/7aad630ce2e5e9ebcda2273cef86f22777efc05b))
    - Remove duplicate entry for bad events visualization in QC plots ([`bf44383`](https://github.com/jrmoynihan/flow/commit/bf4438343008fc1f4501b1ba13e661cfe9d0c6f1))
</details>

## 0.2.3 (2026-02-26)

<csr-id-2180ed40f60f01489aa7549809c69bd4efba2be0/>

### New Features

 - <csr-id-489796d785339fdf8d2c56bd61bf383d60986106/> add example for generating QC plots with synthetic data
   - Introduced a new example `demo_qc_plot.rs` that demonstrates how to create a QC plot using synthetic FCS-like data.

### Chore

 - <csr-id-2180ed40f60f01489aa7549809c69bd4efba2be0/> changelog and README for 0.2.3 (QC plot bad-events fix)

### Bug Fixes

 - QC channel plots now draw "bad" (unstable) events in addition to "good" events.
   - Previously only good events (grey points) were drawn; purple rectangles marked unstable regions but the underlying bad-event data was invisible.
   - Bad events are now drawn in a distinct color (red by default), configurable via `QCPlotConfig::bad_color`.
- Bad events are now drawn in a distinct color (red by default), configurable via `QCPlotConfig::bad_color`.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.2.3 ([`7600d54`](https://github.com/jrmoynihan/flow/commit/7600d54b5bdbedb4c5e8189265a6b5f20a1970cf))
    - Changelog and README for 0.2.3 (QC plot bad-events fix) ([`2180ed4`](https://github.com/jrmoynihan/flow/commit/2180ed40f60f01489aa7549809c69bd4efba2be0))
    - Add example for generating QC plots with synthetic data ([`489796d`](https://github.com/jrmoynihan/flow/commit/489796d785339fdf8d2c56bd61bf383d60986106))
    - Release flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`4ab83fe`](https://github.com/jrmoynihan/flow/commit/4ab83fe18e7f67bba8c1ce2bf8163e8652a9a592))
</details>

## 0.2.2 (2026-02-26)

<csr-id-ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c/>

### Chore

 - <csr-id-ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c/> update changelogs and READMEs for flow-fcs, flow-plots, peacoqc-rs patch release

### New Features

<csr-id-d2faa60c34aacf4bc627e86e314ba3e4ef550797/>

 - <csr-id-6916425895d246583109af0b35b9fc60e487ba5c/> enforce minimum y-scale for plots and improve mesh drawing
   - Implemented a minimum y-scale of 1 event/sec for plots, adjusting the y-range accordingly.

### Bug Fixes

 - <csr-id-b8041c2bdc2ed688ecbdf3d0babc3fceaad43e63/> improve compensation handling in FCS processing
   - Updated compensation logic to skip processing when $SPILLOVER keyword is missing, using arcsinh fallback instead.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 9 calendar days.
 - 10 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`d8a6922`](https://github.com/jrmoynihan/flow/commit/d8a6922a47b2196a6dcf8362bab067b176757908))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Update changelogs and READMEs for flow-fcs, flow-plots, peacoqc-rs patch release ([`ec0fcf8`](https://github.com/jrmoynihan/flow/commit/ec0fcf8823f4d35e47d7da935f1e70d1927f0f0c))
    - Enforce minimum y-scale for plots and improve mesh drawing ([`6916425`](https://github.com/jrmoynihan/flow/commit/6916425895d246583109af0b35b9fc60e487ba5c))
    - Add plot_index parameter for selective plot rendering ([`d2faa60`](https://github.com/jrmoynihan/flow/commit/d2faa60c34aacf4bc627e86e314ba3e4ef550797))
    - Improve compensation handling in FCS processing ([`b8041c2`](https://github.com/jrmoynihan/flow/commit/b8041c2bdc2ed688ecbdf3d0babc3fceaad43e63))
</details>

## 0.2.1 (2026-02-16)

### Documentation

 - <csr-id-a6df2248b415f4cb6650b4a3efc96a6f80f2a1ee/> improve formatting and clarity in DEV_NOTES.md
   - Updated table formatting for batched operations to enhance readability.

### New Features

 - <csr-id-442c53cc3018e84661fca51603e175c67be11531/> add Serialize/Deserialize and Clone/PartialEq derives
   - Add Serialize/Deserialize to PeacoQCResult, PeakInfo, ChannelPeakFrame

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.2.1, flow-utils v0.1.1, flow-gates v0.2.2, flow-tru-ols v0.1.0 ([`c3d9774`](https://github.com/jrmoynihan/flow/commit/c3d97742b3f83d01f1b831eea6eb662a2511adb9))
    - Improve formatting and clarity in DEV_NOTES.md ([`a6df224`](https://github.com/jrmoynihan/flow/commit/a6df2248b415f4cb6650b4a3efc96a6f80f2a1ee))
    - Add Serialize/Deserialize and Clone/PartialEq derives ([`442c53c`](https://github.com/jrmoynihan/flow/commit/442c53cc3018e84661fca51603e175c67be11531))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
</details>

## 0.2.0 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-e62b423812b0ce71a2b355a60da926fd588cbf0a/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-ef08072701d707a79303fb5ffcb14127d3d22930/>
<csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/>

### Chore

 - <csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/> update dependencies and align workspace configurations
   - Updated various dependencies in Cargo.toml files across multiple crates to their latest versions for improved functionality and compatibility.
   - Changed several dependencies to use workspace references for consistency and to reduce duplication.
   - Notable updates include polars to version 0.53.0, faer to version 0.24, and ndarray-linalg to version 0.18.1.
   - Adjusted dev-dependencies to utilize workspace settings for better management.
 - <csr-id-e62b423812b0ce71a2b355a60da926fd588cbf0a/> use workspace ndarray dependency
   Replace ndarray 0.17 with workspace ndarray 0.16 for consistency
 - <csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/> clean up unused imports and variables
   - Remove unused imports in clustering and gating modules
   - Fix unreachable code warning in DBSCAN
   - Remove unused mut keywords
   - Clean up warnings for better code quality
 - <csr-id-ef08072701d707a79303fb5ffcb14127d3d22930/> update benchmarks to use current APIs
   - Replace deprecated criterion::black_box with std::hint::black_box
   - Update rand API: thread_rng() -> rng(), gen_range() -> random_range()
   - Fix csaps compatibility by converting ndarray to slices

### Chore

 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release

### Documentation

 - <csr-id-5fba05b5e2747e3cfdda979dabb4ddde2bb384e1/> consolidate GPU performance documentation
   - Move GPU performance results from GPU_PERFORMANCE.md to DEV_NOTES.md

### New Features

<csr-id-6e66d118e5c249b95ba7262d1b2f5ee8393bf3cf/>

 - <csr-id-6a911f32c2ae53a4cc4e956e6ce4efc6df49aa66/> remove GPU thresholds, always use GPU when available
   - Remove GPU_THRESHOLD, GPU_FFT_THRESHOLD, GPU_MATRIX_THRESHOLD constants

### Bug Fixes

 - <csr-id-75937c28e6df8843143cb9222cea163894b2f0a0/> fix integration regression tests
   - Add missing PeacoQCData trait import

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 15 commits contributed to the release over the course of 24 calendar days.
 - 27 days passed between releases.
 - 9 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Use workspace ndarray dependency ([`e62b423`](https://github.com/jrmoynihan/flow/commit/e62b423812b0ce71a2b355a60da926fd588cbf0a))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
    - Merge pull request #10 from jrmoynihan/gpu-acceleration ([`69363eb`](https://github.com/jrmoynihan/flow/commit/69363eb3a664b1aa6cd0be9b980ec08fc03b7955))
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
    - Update benchmarks to use current APIs ([`ef08072`](https://github.com/jrmoynihan/flow/commit/ef08072701d707a79303fb5ffcb14127d3d22930))
    - Fix integration regression tests ([`75937c2`](https://github.com/jrmoynihan/flow/commit/75937c28e6df8843143cb9222cea163894b2f0a0))
    - Consolidate GPU performance documentation ([`5fba05b`](https://github.com/jrmoynihan/flow/commit/5fba05b5e2747e3cfdda979dabb4ddde2bb384e1))
    - Remove GPU thresholds, always use GPU when available ([`6a911f3`](https://github.com/jrmoynihan/flow/commit/6a911f32c2ae53a4cc4e956e6ce4efc6df49aa66))
    - Add GPU acceleration for KDE and feature matrix operations ([`6e66d11`](https://github.com/jrmoynihan/flow/commit/6e66d118e5c249b95ba7262d1b2f5ee8393bf3cf))
</details>

## 0.1.3 (2026-01-18)

<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>
<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-ccd3cb1557065cd0c1ba4637c35d937bac39c9f6/>
<csr-id-1b41cd165c4cd315e9759b437e6b4e2a2839af99/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-c91cd7fd5ad0b9c912c5ca05ff7540655a37d304/>
<csr-id-be95b5180e4ffe4826bcb9a3833295d35a9b7ced/>
<csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/>
<csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/>
<csr-id-005a1cc9bd0bef0c9354d1f16b1fa077828359a3/>

### Chore

 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.
 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-ccd3cb1557065cd0c1ba4637c35d937bac39c9f6/> add reference materials and cargo config
   - Add PeacoQC paper PDF for reference
   - Add cargo config for build settings
 - <csr-id-1b41cd165c4cd315e9759b437e6b4e2a2839af99/> remove R reference files and test artifacts
   - Remove R reference implementation files (moved to separate location)
   - Remove test plot images
   - Clean up repository for production use
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Documentation

<csr-id-2721f1f10fe2edd01034e4bd5340dd9cc6fe6b2e/>
<csr-id-76d800d1b9a5b40c8f4628b46074320bd9e51630/>

 - <csr-id-c6e843a730bc3352229c02a60be3b167e9f2d14d/> md formatting on dev notes
 - <csr-id-06a15cc61b34171896102c8de48c275fb811e78d/> consolidate and clean up documentation
   - Create DEV_NOTES.md consolidating technical implementation details

### New Features

<csr-id-1164c5de5cd34a0806cf2b89bd87f51e905b8aed/>
<csr-id-9bfc1e2f00f85a894ae962a8a1b7bbe0bb019b10/>

 - <csr-id-d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f/> add QC plot generation functionality
   Add comprehensive QC plot generation for visualizing PeacoQC results.
 - <csr-id-b5ef7a7b3515f707310cd932617bbf32125b0690/> implement FFT-based kernel density estimation
   Replace naive O(n×m) KDE implementation with FFT-based O(n log n) version
   for significant performance improvements.
   
   - Add realfft dependency for efficient FFT operations

### Bug Fixes

 - <csr-id-0f4f447a4dbc2a47b6e5959f07da70ef465f95b9/> remove duplicate debug module declaration in qc mod.rs

### Other

 - <csr-id-c91cd7fd5ad0b9c912c5ca05ff7540655a37d304/> remove paper pdf for release
 - <csr-id-be95b5180e4ffe4826bcb9a3833295d35a9b7ced/> :pushpin: merging cargo.toml

### Refactor

 - <csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/> clean up unused imports and improve code readability
   - Removed unused imports from write.rs and peaks.rs.
   - Updated the loop in isolation_tree.rs to ignore unused variables for clarity.
   - Standardized string conversion in plots.rs for consistency.
 - <csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/> improve code quality and add features
   - Improve QC algorithm implementations
   - Add plot generation functionality
   - Enhance error handling
   - Update dependencies
   - Improve code organization

### Test

 - <csr-id-005a1cc9bd0bef0c9354d1f16b1fa077828359a3/> add comprehensive test suite
   - Add regression tests for critical fixes
   - Add algorithm correctness tests
   - Add integration tests with known outputs
   - Add R compatibility tests
   - Add spline comparison tests
   - Add peak detection tests
   - Add test documentation in tests/README.md
   - Add debug utilities for QC development

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 27 commits contributed to the release over the course of 3 calendar days.
 - 4 days passed between releases.
 - 20 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.3, peacoqc-cli v0.1.2 ([`572393e`](https://github.com/jrmoynihan/flow/commit/572393e435342b438c398b2c51b680af50da1b68))
    - Remove paper pdf for release ([`c91cd7f`](https://github.com/jrmoynihan/flow/commit/c91cd7fd5ad0b9c912c5ca05ff7540655a37d304))
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Clean up unused imports and improve code readability ([`6da76b7`](https://github.com/jrmoynihan/flow/commit/6da76b758d02b9da1abcd3052323f81992dc3fdd))
    - Remove duplicate debug module declaration in qc mod.rs ([`0f4f447`](https://github.com/jrmoynihan/flow/commit/0f4f447a4dbc2a47b6e5959f07da70ef465f95b9))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`f08823c`](https://github.com/jrmoynihan/flow/commit/f08823cabcae5223efe4250471dd75ea7fcaa936))
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - :pushpin: merging cargo.toml ([`be95b51`](https://github.com/jrmoynihan/flow/commit/be95b5180e4ffe4826bcb9a3833295d35a9b7ced))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Md formatting on dev notes ([`c6e843a`](https://github.com/jrmoynihan/flow/commit/c6e843a730bc3352229c02a60be3b167e9f2d14d))
    - Add reference materials and cargo config ([`ccd3cb1`](https://github.com/jrmoynihan/flow/commit/ccd3cb1557065cd0c1ba4637c35d937bac39c9f6))
    - Improve code quality and add features ([`5bd48e4`](https://github.com/jrmoynihan/flow/commit/5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7))
    - Add comprehensive test suite ([`005a1cc`](https://github.com/jrmoynihan/flow/commit/005a1cc9bd0bef0c9354d1f16b1fa077828359a3))
    - Remove R reference files and test artifacts ([`1b41cd1`](https://github.com/jrmoynihan/flow/commit/1b41cd165c4cd315e9759b437e6b4e2a2839af99))
    - Consolidate and clean up documentation ([`06a15cc`](https://github.com/jrmoynihan/flow/commit/06a15cc61b34171896102c8de48c275fb811e78d))
    - Add QC plot generation functionality ([`d262a61`](https://github.com/jrmoynihan/flow/commit/d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f))
    - Implement FFT-based kernel density estimation ([`b5ef7a7`](https://github.com/jrmoynihan/flow/commit/b5ef7a7b3515f707310cd932617bbf32125b0690))
    - Add export formats documentation ([`2721f1f`](https://github.com/jrmoynihan/flow/commit/2721f1f10fe2edd01034e4bd5340dd9cc6fe6b2e))
    - Add convenience export methods to PeacoQCResult ([`1164c5d`](https://github.com/jrmoynihan/flow/commit/1164c5de5cd34a0806cf2b89bd87f51e905b8aed))
    - Add export module for QC results ([`9bfc1e2`](https://github.com/jrmoynihan/flow/commit/9bfc1e2f00f85a894ae962a8a1b7bbe0bb019b10))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - Update README files for peacoqc-cli and peacoqc-rs to include license information, enhance function documentation, and improve contribution guidelines. ([`76d800d`](https://github.com/jrmoynihan/flow/commit/76d800d1b9a5b40c8f4628b46074320bd9e51630))
</details>

## 0.1.2 (2026-01-18)

<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-ccd3cb1557065cd0c1ba4637c35d937bac39c9f6/>
<csr-id-1b41cd165c4cd315e9759b437e6b4e2a2839af99/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-be95b5180e4ffe4826bcb9a3833295d35a9b7ced/>
<csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/>
<csr-id-005a1cc9bd0bef0c9354d1f16b1fa077828359a3/>
<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>
<csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/>

### Chore

 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-ccd3cb1557065cd0c1ba4637c35d937bac39c9f6/> add reference materials and cargo config
   - Add PeacoQC paper PDF for reference
   - Add cargo config for build settings
 - <csr-id-1b41cd165c4cd315e9759b437e6b4e2a2839af99/> remove R reference files and test artifacts
   - Remove R reference implementation files (moved to separate location)
   - Remove test plot images
   - Clean up repository for production use
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Refactor

 - <csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/> clean up unused imports and improve code readability
   - Removed unused imports from write.rs and peaks.rs.
   - Updated the loop in isolation_tree.rs to ignore unused variables for clarity.
   - Standardized string conversion in plots.rs for consistency.

### Bug Fixes

 - <csr-id-0f4f447a4dbc2a47b6e5959f07da70ef465f95b9/> remove duplicate debug module declaration in qc mod.rs

### Chore

 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.

### Documentation

<csr-id-2721f1f10fe2edd01034e4bd5340dd9cc6fe6b2e/>
<csr-id-76d800d1b9a5b40c8f4628b46074320bd9e51630/>

 - <csr-id-c6e843a730bc3352229c02a60be3b167e9f2d14d/> md formatting on dev notes
 - <csr-id-06a15cc61b34171896102c8de48c275fb811e78d/> consolidate and clean up documentation
   - Create DEV_NOTES.md consolidating technical implementation details

### New Features

<csr-id-1164c5de5cd34a0806cf2b89bd87f51e905b8aed/>
<csr-id-9bfc1e2f00f85a894ae962a8a1b7bbe0bb019b10/>

 - <csr-id-d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f/> add QC plot generation functionality
   Add comprehensive QC plot generation for visualizing PeacoQC results.
 - <csr-id-b5ef7a7b3515f707310cd932617bbf32125b0690/> implement FFT-based kernel density estimation
   Replace naive O(n×m) KDE implementation with FFT-based O(n log n) version
   for significant performance improvements.
   
   - Add realfft dependency for efficient FFT operations

### Other

 - <csr-id-be95b5180e4ffe4826bcb9a3833295d35a9b7ced/> :pushpin: merging cargo.toml

### Refactor

 - <csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/> improve code quality and add features
   - Improve QC algorithm implementations
   - Add plot generation functionality
   - Enhance error handling
   - Update dependencies
   - Improve code organization

### Test

 - <csr-id-005a1cc9bd0bef0c9354d1f16b1fa077828359a3/> add comprehensive test suite
   - Add regression tests for critical fixes
   - Add algorithm correctness tests
   - Add integration tests with known outputs
   - Add R compatibility tests
   - Add spline comparison tests
   - Add peak detection tests
   - Add test documentation in tests/README.md
   - Add debug utilities for QC development

## 0.1.1 (2026-01-14)

<csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/>

### Chore

 - <csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/> :hammer: Add cargo scripts for testing and release management for each crate
   Granular control at the crate level.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.1 ([`947c991`](https://github.com/jrmoynihan/flow/commit/947c991bff21beb7b7d60f1f637279bd86b9ab66))
    - :hammer: Add cargo scripts for testing and release management for each crate ([`a6a4ff7`](https://github.com/jrmoynihan/flow/commit/a6a4ff733ae38acaec36d3327f4952d6fded3c0f))
    - Adjusting changelogs prior to release of peacoqc-rs v0.1.1 ([`a84b627`](https://github.com/jrmoynihan/flow/commit/a84b6271257f16432464aff091fb9c34eadf16f0))
    - Release peacoqc-cli v0.1.0 ([`ee76027`](https://github.com/jrmoynihan/flow/commit/ee760271b139b2a192d7065d08063fe5ecf0ffbf))
</details>

## 0.1.0 (2026-01-14)

<csr-id-9eceaee3506dcde315676f0d99dc893acc7430b7/>
<csr-id-deb1cae21a7b99c937335413c7f0ab6ee339365c/>
<csr-id-56accd5d225e545fe0c79e84922ecc8c21272a7e/>
<csr-id-5ac6927216aefa9779c9185841c9e4b6ee12355a/>
<csr-id-1347675f8a5648b939e368949cd30f5b6ec4b379/>
<csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/>
<csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/>
<csr-id-734654c97306d477fe98eda2ed151a92c1e49050/>

### Chore

 - <csr-id-9eceaee3506dcde315676f0d99dc893acc7430b7/> remove R source files and example files
   - Deleted PeacoQC Helper Functions, Plot Helper Functions, and main R script files as they are no longer in use.
   - Removed associated example FCS file and QC plot image to clean up the repository.
 - <csr-id-deb1cae21a7b99c937335413c7f0ab6ee339365c/> remove test report JSON file
   Remove unused test_report.json file from examples directory.
 - <csr-id-56accd5d225e545fe0c79e84922ecc8c21272a7e/> remove .DS_Store files from git tracking
   - Remove macOS .DS_Store files that were previously tracked
   - These files are already in .gitignore and should not be committed

### Chore

 - <csr-id-734654c97306d477fe98eda2ed151a92c1e49050/> Update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### Chore

 - <csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### Chore

 - <csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Documentation

<csr-id-e20e140346192a329fe65bb1d669036344471a39/>

 - <csr-id-42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f/> add FCS specification PDF and example QC plot
   - Add FCS 3.1 implementation guidance PDF for reference

### New Features

<csr-id-89520c5f677d2ca74c9777765b160554cca49eb5/>
<csr-id-bcf9880c6dcff0414818a09746adf8a315d14444/>

 - <csr-id-e08f165f1b18fdee7d303db125685066f6846ac2/> add QC plotting functionality
   - Reformatted Cargo.toml for improved readability and added new dependencies `plotters` and `image` for plotting.

### Bug Fixes

 - <csr-id-1cb95844e0c987752bf9f12854f03457c26bc408/> implement dynamic grid sizing for QC plots
   - Add calculate_grid_dimensions function that creates square-ish grids

### Refactor

 - <csr-id-5ac6927216aefa9779c9185841c9e4b6ee12355a/> extract CLI functionality to separate crate
   - Remove CLI binary from peacoqc-rs/src/bin/
   - Update examples to reflect library-only usage
   - Update Cargo.toml to remove binary targets

### Test

 - <csr-id-1347675f8a5648b939e368949cd30f5b6ec4b379/> add R compatibility tests
   - Add comprehensive R compatibility test suite
   - Ensure algorithm parity with original R implementation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 20 commits contributed to the release over the course of 7 calendar days.
 - 14 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.0 ([`ae4bc91`](https://github.com/jrmoynihan/flow/commit/ae4bc91414dde199edfdac0965c9df44e9036f2f))
    - Update CHANGELOG for upcoming release ([`734654c`](https://github.com/jrmoynihan/flow/commit/734654c97306d477fe98eda2ed151a92c1e49050))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`3292c46`](https://github.com/jrmoynihan/flow/commit/3292c46b282d226aa48c2a83bc17c50896bb8341))
    - Update CHANGELOG for upcoming release ([`037f74e`](https://github.com/jrmoynihan/flow/commit/037f74e0e364ebfc8d68cf672dca0f758a3f2952))
    - Remove R source files and example files ([`9eceaee`](https://github.com/jrmoynihan/flow/commit/9eceaee3506dcde315676f0d99dc893acc7430b7))
    - Merge pull request #7 from jrmoynihan/feat/cli-plot-generation ([`e0cd286`](https://github.com/jrmoynihan/flow/commit/e0cd286f9faa58d264eb27cc6dc6b57958389f78))
    - Remove test report JSON file ([`deb1cae`](https://github.com/jrmoynihan/flow/commit/deb1cae21a7b99c937335413c7f0ab6ee339365c))
    - Implement dynamic grid sizing for QC plots ([`1cb9584`](https://github.com/jrmoynihan/flow/commit/1cb95844e0c987752bf9f12854f03457c26bc408))
    - Merge pull request #6 from jrmoynihan/flow-gates ([`dcec55b`](https://github.com/jrmoynihan/flow/commit/dcec55bc4f08bb2bd3d6db1bfe4b603a014c3beb))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Add QC plotting functionality ([`e08f165`](https://github.com/jrmoynihan/flow/commit/e08f165f1b18fdee7d303db125685066f6846ac2))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Remove .DS_Store files from git tracking ([`56accd5`](https://github.com/jrmoynihan/flow/commit/56accd5d225e545fe0c79e84922ecc8c21272a7e))
    - Add FCS specification PDF and example QC plot ([`42a6b5d`](https://github.com/jrmoynihan/flow/commit/42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f))
    - Add R compatibility tests ([`1347675`](https://github.com/jrmoynihan/flow/commit/1347675f8a5648b939e368949cd30f5b6ec4b379))
    - Add documentation, R helper functions, and update examples ([`e20e140`](https://github.com/jrmoynihan/flow/commit/e20e140346192a329fe65bb1d669036344471a39))
    - Extract CLI functionality to separate crate ([`5ac6927`](https://github.com/jrmoynihan/flow/commit/5ac6927216aefa9779c9185841c9e4b6ee12355a))
    - Refactor and improve QC algorithms ([`89520c5`](https://github.com/jrmoynihan/flow/commit/89520c5f677d2ca74c9777765b160554cca49eb5))
    - Initialize PeacoQC library for flow cytometry quality control ([`bcf9880`](https://github.com/jrmoynihan/flow/commit/bcf9880c6dcff0414818a09746adf8a315d14444))
</details>

