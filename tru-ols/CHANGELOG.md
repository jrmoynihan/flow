# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Chore

 - <csr-id-af6097fbd09f00657eaf82ea8367fffd3ee72baf/> default test commands to cargo-nextest (flow-crates-9xv)
   Nextest runs each test in its own process and reports per-test timing, so
   make it the default runner everywhere the project tells a human or an agent
   how to run tests, rather than leaving it as an opt-in each caller remembers.
   
   Adds .config/nextest.toml (default profile fails fast; a ci profile runs the
   whole suite) and a `cargo nt` alias, since Cargo cannot alias the built-in
   `test` subcommand. Doctests stay on the built-in harness because nextest
   cannot run them.
 - <csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/> bump flow-fcs to 0.4.0, add publish metadata to new crates
   - flow-fcs 0.3.0 → 0.4.0 (new compensation feature + public API)
   - flow-linalg, flow-density, flow-clustering: add repository field
     and smart-release scripts for first publish
   - Update all workspace consumers to ^0.4.0
 - <csr-id-fd1cc4a76af40804018e24792dce407860302857/> Release
 - <csr-id-bd8837a275f83cee37e59ab665d6dd2ec293bbfc/> clean up Cargo.toml formatting and remove unused keyword
   - Removed "compensation" from keywords.
   - Standardized formatting for dependencies and dev-dependencies for consistency.

### Documentation

 - <csr-id-92e31b03dc632230809d10422be0c1062e6e9e1b/> consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate
   Rewrites READMEs across the workspace (fcs, flow-clustering,
   flow-control-detection, flow-density, flow-fcs-compress, flow-knn,
   flow-linalg, flow-pacmap, flow-peak-detection, gates, peacoqc-cli,
   peacoqc-rs, tru-ols, tru-ols-cli) to lead with install/quick-start/perf
   for downstream consumers, and adds a new flow-fcs-bench README.
   
   Adds a concrete usage example to peacoqc-py/README.md mirroring the
   docstring in peacoqc/__init__.py.
   
   Removes the superseded utils/ crate (clustering, KDE, and PCA helpers
   now live in their dedicated crates) and syncs beads issue/interaction
   export state.
 - <csr-id-a915f69bce32ad10dbdb5e1c166f40cbda1681ff/> add PeacoQC and TRU-OLS CPU vs GPU bench result notes
 - <csr-id-a9d4aaaed740a9f5928c2ec9225b23f69b7d1675/> add benchmark reports, flamegraphs, and performance analysis
   Root benchmark_output/: quality metrics report (JSON + markdown + CSVs)
   from synthetic unmixing scenarios.
   tru-ols/benchmark_output/: e2e plate throughput logs, perf ceiling
   hypotheses, before/after comparison, and samply flamegraph profiles.
 - <csr-id-cdff38f488388888dfe146cf6672ff23086c5014/> move long-form docs into docs/
   - Add crate README; centralize comparison, validation, and dev notes

### New Features

 - <csr-id-26a0a5421d948b319977d0b359ed6a0e8d536d32/> retire delimiter workaround now that flow-fcs handles escaping
   Remove SAFE_TEXT_DELIMITER and ensure_delimiter_survives_provenance
   which worked around the writer's inability to escape delimiters in
   keyword values. flow-crates-1xb now handles delimiter escaping in
   flow-fcs's writer at FCS 3.1+ (3.2 is the version tru-ols stamps
   products as), so this containment mechanism is no longer needed.
 - <csr-id-0cc2d3beabceb00542ea645e7daa84d234396c3e/> record full unmixing provenance and stamp products FCS3.2
   Closes flow-crates-x17.6 and x17.7. An unmixed .fcs could not be audited,
   reproduced, or even attributed from the file alone.
   
   What was recorded before: $UNMIXED = "TRU-OLS", and — on only one of the two
   write paths — $RAW_DATASOURCE_GUID. The mixing matrix, the unstained control
   identity, the cutoff percentile, the strategy, the AF index, the software
   version and the fit metrics were all dropped. $SPILLOVER, the one slot a
   reader would inspect, is overwritten with an identity matrix. That is correct
   behaviour — the detector basis is gone, so downstream tools must not
   re-compensate — but it carries zero information about the transform that
   replaced it.
   
   The core defect was the split: export_unmixed_fcs set the GUID pair, but the
   apply_tru_ols_unmixing* trait methods did not. A caller using the trait plus
   write_fcs_file produced a file that inherited the raw file's $GUID with no
   parent pointer — indistinguishable from the source it came from.
   
   Provenance is now written unconditionally from
   build_unmixed_fcs_from_unmixed_abundances, the single chokepoint both paths
   already flow through, so neither path can omit it. The parameters arrive as an
   UnmixProvenance struct rather than more positional arguments; the function was
   already #[allow(clippy::too_many_arguments)].
   
   The matrix goes in $TRUOLS_MIXMAT (MixedKeyword::MixingMatrix) rather than
   $SPILLOVER, whose square n×n encoding cannot represent a rectangular
   detector×endmember matrix. Unstained-control description uses the spec-native
   $UNSTAINEDINFO.
   
   read_provenance(&Fcs) -> Option<UnmixProvenance> reads it back, which is what
   makes the traceability usable rather than merely recorded — and testable.
   
   Products are stamped FCS3.2 via flow_fcs::upgrade::stamp_v3_2: $ORIGINALITY =
   DataModified, $LAST_MODIFIER, $LAST_MODIFIED, ISO 8601 datetimes derived from
   the source's $DATE/$BTIM/$ETIM, and carrier keywords migrated from the
   deprecated plate/well forms. Raw passthrough writes are untouched and keep
   their source version — 3.2 is not the global writer default.
 - <csr-id-cd6b4d9ffb17f3ecaa0aa60393b45255d77962a1/> harden unmixed FCS export metadata and channel labels
   Preserve signed abundances, stamp $UNMIXED, mint a fresh GUID, strip orphan
   Pn* keywords, and write fluor/target labels plus identity spillover for plots.
 - <csr-id-c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926/> add shared KNN crate and unify Burn/cubeCL workspace deps
   Introduce flow-knn for portable graphs and ANN backends, pin Burn 0.21 with
   cubeCL 0.10 across GPU consumers, and path-patch pastey/bit-vec for sandbox builds.
 - <csr-id-4fa5791892b1403996979a5d5a695a83c03fa050/> include Autofluorescence as mixing-matrix endmember
   Append AF as a max-normalized column when set so cosine similarity and
   hotspot/SIF reflect AF collinearity with the panel.
 - <csr-id-c31b59dca115999cf7f9a4ff074ff39b12a1c4ae/> attach hotspot/SIF to mixing matrix assemble
 - <csr-id-1c50f77e55c25456a7da29e78e472422a669d780/> add mixing matrix assembly and shared pipeline helpers
   Build MixingMatrix with condition/cosine QC and UI-agnostic AF resolution / FCS export helpers, depending on flow-linalg.
 - <csr-id-cd548b992d65013fe718124cb5d4bab5db0c5505/> parallelize unmixing pipeline with scratch buffers and batch caching
   Major performance refactor of the TRU-OLS unmixing engine:
   - Rayon parallelization for preprocessing and unmixing (threshold-gated)
   - Scratch-buffer least-squares solvers (faer QR/Cholesky, BLAS paths)
   - SharedMaskFactorCache for batch runs (new unmix-cache feature)
   - Comprehensive quality metrics (spread, USE, dimensionality, fit)
   - FCS reconstruction from unmixed abundances (build_unmixed_fcs)
   
   New modules: batched_ols, benchmark, metrics, unmix_buffer, unmix_cache, gpu.
   New docs: profiling guide, Julia/BLAS notes, quality comparison report.
 - <csr-id-d043a3591c9d66858d3b03fdf0753214817565db/> FCS naming, unmix export clamping, bench targets
   - Derive display names from endmember stems without splitting markers
   - Clamp unmixed abundances to non-negative for FCS export
   - Register unmixing_benchmark harness; split sequential vs parallel benches
   - Use flow-plots without default raster feature
 - <csr-id-8d79dd17b3a38a8bcdc26126333abf8d2555fcd9/> implement contour path clipping to axis range
   - Added `clip_contour_paths` function to clamp contour path points to specified x and y axis ranges, dropping degenerate paths.
   - Updated `calculate_contours` to utilize the new clipping function, ensuring contour paths do not exceed the chart's axis range.
   - Enhanced documentation for `x_range` and `y_range` parameters to clarify their purpose.
   - Added regression tests to verify clipping behavior and prevent panics when rendering contours with out-of-range data.
 - <csr-id-8b26c1418137646bb311d45a678d1d43ef05a22d/> scatter overlay, z-axis coloring, density point size, contours, histograms
   - ScatterPlotData: discrete gate colors (ScatterOverlay), continuous z-axis (ScatterColoredContinuous)
   - Density plots: point_size affects contribution radius (matches scatter behavior)
   - Contour plots: KDE-based contours, draw_outliers, contour_smoothing
   - HistogramPlot: filled/unfilled, overlaid with gate colors, baseline separation, scale_to_peak
   - Breaking: DensityPlot::Data is now ScatterPlotData; use .into() for Vec<(f32,f32)>
   - Updated tru-ols, tru-ols-cli, gates for new API

### Bug Fixes

 - <csr-id-2ea795744687e96f3e4f9be8cfeb9c75ec36ca93/> stop writing empty $PnS in unmixed FCS export (flow-crates-aht)
   FCS 3.1+ forbids empty keyword values because a doubled delimiter is the
   escape for one literal delimiter in those versions; a blank $PnS
   serialises to two adjacent delimiters and the next keyword's bytes get
   absorbed on read. The unmixed-export path always stamps output as FCS
   3.2 (stamp_v3_2) while writing $PnS = "" for scatter/time passthrough
   parameters, autofluorescence, and fluorophore endmembers with no
   resolved marker name — corrupting the TEXT segment on reopen.
   
   Write descriptive, non-empty values instead of omitting or blanking:
   - scatter/time parameters: $PnS = channel name (orig_param.channel_name)
   - fluorophore endmembers: $PnS = target when resolved, else fall back
     to the fluor name already written as $PnN for that parameter
   - autofluorescence: $PnS = af_pn (defaults to "Autofluorescence")
 - <csr-id-a565fdf4b372fe74eb6393eb61218a8ea159b6fe/> address final whole-branch review findings (bounds check, cache warning, feature scoping, version bump, benchmark docs)
   - Fcs::columns() now returns a descriptive Err instead of panicking when a
     parameter's cache index falls outside the column cache (can happen on a
     derived Fcs whose parameters were replaced without resizing the cache,
     e.g. tru-ols's spectral-unmixing output). Added a regression test.
   - Added a `# Warning` doc section to column()/columns()/events() noting the
     cache is only meaningful on an Fcs from open()/open_all() (flow-crates-rkq).
   - Moved flow-fcs's `test-util` feature enablement from [dependencies] to
     [dev-dependencies] in gates, tru-ols, and peacoqc-rs so it's no longer
     forced on in release builds via feature unification.
   - Bumped flow-fcs to 0.5.1 (test-util didn't exist in the published 0.5.0)
     and its dependents' version constraints to ^0.5.1.
   - Documented the benchmark's actual ~8x events_uncached/open_eager_baseline
     gap (extract_columns lacking a uniform-width fast path, not double work
     from open()) in the benchmark source and amended the Stage A plan doc.
     Tracked as flow-crates-3si.
 - <csr-id-6e3d7233683f7c18b858829c83844171fa6adfd1/> add Fcs::for_testing constructor, restore cross-crate test-fixture construction
   Task 4's pub(crate) columns field broke every out-of-crate struct-literal
   construction of Fcs, since a pub(crate) field can't be named externally at
   all. Adds a public, feature-gated constructor and migrates every known
   broken call site (tru-ols, peacoqc-rs, gates, plus flow-fcs's own
   compress-feature tests) to use it instead.
 - <csr-id-6986541e936967c566b3c6caca42c9e0cbf5678f/> apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal
   Fixes four parsing gaps reported in jrmoynihan/flow#21:
   
   - $PnR masking (flow-crates-d35, P0): integer parameters now mask off
     unused high bits per their declared $PnR range before column
     extraction, fixing silently-wrong channel values on instruments
     (Beckman FC500/Gallios/Navios, older BD) that store sub-16-bit ADC
     resolution in wider fields.
   - Bit-packed $PnB stride (flow-crates-bk6, P2): calculate_bytes_per_event
     now sums raw bit widths before rounding once, instead of rounding each
     parameter first — correct for both byte-aligned and bit-packed layouts.
   - $NEXTDATA traversal (flow-crates-1mg, P2): new Fcs::open_all() walks
     the $NEXTDATA chain to read every dataset in a multi-dataset FCS file
     (all Beckman .lmd files use this). open() is unchanged and still
     returns only the first dataset, so existing callers are unaffected.
   - $DATATYPE A (flow-crates-ee0, P3, won't-fix): documented the existing
     Err behavior as a deliberate spec-driven decision (ASCII was
     deprecated due to cross-vendor bit-order disagreement) rather than an
     oversight, and added a test confirming it.
   
   Bumps flow-fcs 0.4.1 -> 0.5.0 and the paired version requirement in
   every workspace crate that depends on it via path (Cargo enforces that
   constraint even for path deps).
 - <csr-id-758d02ec3225382d81b566db15a30e5cd4863e16/> update rand and polars APIs for compatibility
   - Use rand::RngExt for random_range (rand 0.10)
   - Use DataFrame::new_infer_height for polars 0.53

### Performance

 - <csr-id-2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37/> bulk-load KnnGraph IO; record unsafe micro-opt A/B
   Keep the ~100× faster graph load via read_exact + LE bytemuck cast.
   Add Criterion benches and PERF_AB docs for the six-item A/B campaign;
   revert opts that missed the ≥5% keep rule (BSS, FCS columns/write,
   TRU-OLS SyncPtr, exact KNN / PaCMAP unchecked).

### Bug Fixes (BREAKING)

 - <csr-id-f0b29225fb01d5d2c8060e2b9fdf4b9b87b2dfa7/> resolve offsets data-set-relative, fold OTHER into CRC range
   Every FCS offset is measured from the start of the data set that declares
   it, not from the start of the file: HEADER fields (§2.4.3), $BEGINDATA and
   $BEGINANALYSIS (§3.3.3), and $NEXTDATA (§3.3.31). We were treating them all
   as file-absolute.
   
   The bug stayed invisible because a two-data-set file -- which is what every
   .lmd is -- takes exactly one hop, from byte 0, where relative and absolute
   agree. It takes a three-data-set chain to expose it, and no fixture had one.
   
   Fcs gains a public dataset_start; a private absolutize() in file.rs maps a
   declared offset to a file-absolute one. It disambiguates rather than
   assuming, because vendors do emit file-absolute offsets: an offset below
   dataset_start must be relative, and otherwise the relative reading wins
   unless it runs past EOF, in which case we warn and fall back. That keeps the
   existing vendor-style two-data-set fixture green.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 35 commits contributed to the release.
 - 26 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Retire delimiter workaround now that flow-fcs handles escaping ([`26a0a54`](https://github.com/jrmoynihan/flow/commit/26a0a5421d948b319977d0b359ed6a0e8d536d32))
    - Default test commands to cargo-nextest (flow-crates-9xv) ([`af6097f`](https://github.com/jrmoynihan/flow/commit/af6097fbd09f00657eaf82ea8367fffd3ee72baf))
    - Stop writing empty $PnS in unmixed FCS export (flow-crates-aht) ([`2ea7957`](https://github.com/jrmoynihan/flow/commit/2ea795744687e96f3e4f9be8cfeb9c75ec36ca93))
    - Merge branch 'main' into worktree-lazy-fcs-column-loading-stage-a ([`52b5c50`](https://github.com/jrmoynihan/flow/commit/52b5c508956b9888bebe7a1279b47c26932afc7d))
    - Resolve offsets data-set-relative, fold OTHER into CRC range ([`f0b2922`](https://github.com/jrmoynihan/flow/commit/f0b29225fb01d5d2c8060e2b9fdf4b9b87b2dfa7))
    - Address final whole-branch review findings (bounds check, cache warning, feature scoping, version bump, benchmark docs) ([`a565fdf`](https://github.com/jrmoynihan/flow/commit/a565fdf4b372fe74eb6393eb61218a8ea159b6fe))
    - Record full unmixing provenance and stamp products FCS3.2 ([`0cc2d3b`](https://github.com/jrmoynihan/flow/commit/0cc2d3beabceb00542ea645e7daa84d234396c3e))
    - Add Fcs::for_testing constructor, restore cross-crate test-fixture construction ([`6e3d723`](https://github.com/jrmoynihan/flow/commit/6e3d7233683f7c18b858829c83844171fa6adfd1))
    - Apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal ([`6986541`](https://github.com/jrmoynihan/flow/commit/6986541e936967c566b3c6caca42c9e0cbf5678f))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Bulk-load KnnGraph IO; record unsafe micro-opt A/B ([`2d3c6fc`](https://github.com/jrmoynihan/flow/commit/2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37))
    - Add PeacoQC and TRU-OLS CPU vs GPU bench result notes ([`a915f69`](https://github.com/jrmoynihan/flow/commit/a915f69bce32ad10dbdb5e1c166f40cbda1681ff))
    - Harden unmixed FCS export metadata and channel labels ([`cd6b4d9`](https://github.com/jrmoynihan/flow/commit/cd6b4d9ffb17f3ecaa0aa60393b45255d77962a1))
    - Add shared KNN crate and unify Burn/cubeCL workspace deps ([`c9223d6`](https://github.com/jrmoynihan/flow/commit/c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926))
    - Include Autofluorescence as mixing-matrix endmember ([`4fa5791`](https://github.com/jrmoynihan/flow/commit/4fa5791892b1403996979a5d5a695a83c03fa050))
    - Attach hotspot/SIF to mixing matrix assemble ([`c31b59d`](https://github.com/jrmoynihan/flow/commit/c31b59dca115999cf7f9a4ff074ff39b12a1c4ae))
    - Add mixing matrix assembly and shared pipeline helpers ([`1c50f77`](https://github.com/jrmoynihan/flow/commit/1c50f77e55c25456a7da29e78e472422a669d780))
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Release flow-fcs v0.4.1 ([`597f21b`](https://github.com/jrmoynihan/flow/commit/597f21bef7ea787437071685fc3cce9d2269270f))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Add benchmark reports, flamegraphs, and performance analysis ([`a9d4aaa`](https://github.com/jrmoynihan/flow/commit/a9d4aaaed740a9f5928c2ec9225b23f69b7d1675))
    - Parallelize unmixing pipeline with scratch buffers and batch caching ([`cd548b9`](https://github.com/jrmoynihan/flow/commit/cd548b992d65013fe718124cb5d4bab5db0c5505))
    - Move long-form docs into docs/ ([`cdff38f`](https://github.com/jrmoynihan/flow/commit/cdff38f488388888dfe146cf6672ff23086c5014))
    - FCS naming, unmix export clamping, bench targets ([`d043a35`](https://github.com/jrmoynihan/flow/commit/d043a3591c9d66858d3b03fdf0753214817565db))
    - Implement contour path clipping to axis range ([`8d79dd1`](https://github.com/jrmoynihan/flow/commit/8d79dd17b3a38a8bcdc26126333abf8d2555fcd9))
    - Release flow-plots v0.3.1 ([`2050584`](https://github.com/jrmoynihan/flow/commit/2050584238b7b516ee209e4f0cb67543d3c3ba09))
    - Merge branch 'cursor/axis-gate-interaction-630e' into main ([`c021235`](https://github.com/jrmoynihan/flow/commit/c021235f1555962be2177f2edd5a49de646effd4))
    - Release ([`fd1cc4a`](https://github.com/jrmoynihan/flow/commit/fd1cc4a76af40804018e24792dce407860302857))
    - Scatter overlay, z-axis coloring, density point size, contours, histograms ([`8b26c14`](https://github.com/jrmoynihan/flow/commit/8b26c1418137646bb311d45a678d1d43ef05a22d))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Clean up Cargo.toml formatting and remove unused keyword ([`bd8837a`](https://github.com/jrmoynihan/flow/commit/bd8837a275f83cee37e59ab665d6dd2ec293bbfc))
    - Release peacoqc-rs v0.2.1, flow-utils v0.1.1, flow-gates v0.2.2, flow-tru-ols v0.1.0 ([`c3d9774`](https://github.com/jrmoynihan/flow/commit/c3d97742b3f83d01f1b831eea6eb662a2511adb9))
    - Update rand and polars APIs for compatibility ([`758d02e`](https://github.com/jrmoynihan/flow/commit/758d02ec3225382d81b566db15a30e5cd4863e16))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
</details>

## 0.1.0 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-60d00956fa56c883b3c04e4c58bad677b27c6b24/>
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

### Bug Fixes

 - <csr-id-758d02ec3225382d81b566db15a30e5cd4863e16/> update rand and polars APIs for compatibility
   - Use rand::RngExt for random_range (rand 0.10)
   - Use DataFrame::new_infer_height for polars 0.53

### Chore

 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release

### Documentation

 - <csr-id-292bd202b232c6f780a9cc7170cc1d53b443e05e/> add CLI reference and validation reports
   - CLI_ARGUMENTS_REFERENCE: complete argument reference for tru-ols unmix

### New Features

 - <csr-id-9c3354e3667460949ce836783ce02604c972efde/> unmixing, preprocessing, and FCS integration
   - TRU-OLS unmixing with cutoff and iterative removal

### Refactor

 - <csr-id-60d00956fa56c883b3c04e4c58bad677b27c6b24/> replace ndarray with faer for linear algebra
   - Use faer Mat/Col/MatRef/ColRef in TruOls, preprocessing, unmixing
   - solve_linear_system uses faer pure-Rust solver
   - Add optional blas feature for ndarray-linalg least-squares
   - extract_detector_data, apply_tru_ols_unmixing use faer types
   - Update plotting, fcs_integration, tests, benchmarks
   - Update lib doctest to faer API

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 21 calendar days.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Replace ndarray with faer for linear algebra ([`60d0095`](https://github.com/jrmoynihan/flow/commit/60d00956fa56c883b3c04e4c58bad677b27c6b24))
    - Add CLI reference and validation reports ([`292bd20`](https://github.com/jrmoynihan/flow/commit/292bd202b232c6f780a9cc7170cc1d53b443e05e))
    - Unmixing, preprocessing, and FCS integration ([`9c3354e`](https://github.com/jrmoynihan/flow/commit/9c3354e3667460949ce836783ce02604c972efde))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
</details>

