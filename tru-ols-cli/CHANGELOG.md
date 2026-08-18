# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 (2026-08-18)

### Fixed

- Restore `src/commands.rs` (accidentally emptied during the flow-utils split) and retarget KDE/K-means imports to `flow-density` / `flow-clustering`.
- Construct in-memory `Fcs` fixtures with `Fcs::for_testing` (`flow-fcs` `test-util`) after `columns` became crate-private.
- Rejoin the workspace so `cargo check -p tru-ols` and crates.io publish work again.
- Populate `$PnR` on QC pipeline test fixtures so margin removal does not empty the file; skip doublet detection when no events remain.

### New Features (BREAKING)

 - <csr-id-1f9508f34dab1be6d0195e827b9dbc367c50cdd8/> derive serde on GateParameters/LabelPosition, drop legacy forms
   Replace hand-written Serialize/Deserialize impls with derives so the wire
   tags live in exactly one place — the Rust variant names — and flow through
   ts-rs/specta unedited. Previously the tag list existed in four
   hand-synchronized copies (Rust serde, Rust ts(type=...), the frontend
   interfaces mirror, and the generated binding), which had already drifted.

### Refactor

 - <csr-id-5ddcd7e5e8c9ec8c330c56407d21f961164b610f/> use commands.rs with ScatterPlotData compatibility
   - Rename commands_mine.rs to commands.rs
   - Add .into() for plot.render() to satisfy ScatterPlotData type
   - Remove commands.rs from .gitignore so it is tracked
 - <csr-id-7b3d4ae7ab92af89d94b1f1d0f0832b0f1048faa/> Rename flow-tru-ols-cli to tru-ols and update dependencies
   - Changed package name from "flow-tru-ols-cli" to "tru-ols" in Cargo.toml.
   - Updated Cargo.lock to include new package dependencies for "tru-ols".
   - Adjusted .gitignore to properly ignore the .cursor directory.
   - Cleaned up CHANGELOG.md to reflect recent changes and updates.
 - <csr-id-70008ac39d1d08497c2f59e7fde438d0755433d3/> update for faer-based fcs and tru-ols APIs
   - Add faer-ext for ndarray↔faer conversion at boundaries
   - Update commands.rs: MatRef for apply_spectral_unmixing, spill matrix
   - Update compare_with_julia, export_mixing_matrix, create_mixing_matrix_csv
   - Keep ndarray for downstream plotting/export where needed

### Bug Fixes

 - <csr-id-0c924be8d1e6fd1c48132106ce4ee3cfb1535960/> restore commands.rs and workspace membership
   The flow-utils split emptied the CLI command module, which blocked
   cargo check and a 0.2.0 crates.io publish. Restore it, retarget
   KDE/K-means to flow-density/flow-clustering, and build in-memory
   Fcs fixtures with Fcs::for_testing.
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
 - <csr-id-dadc1dd6069ed7e1d294f5a7549889ce26685c76/> ensure output directories exist before running
   Create plot output and result output dirs upfront so users get clear
   permission/path errors before unmix computations run.
 - <csr-id-264bb789d8447f7f35c0d649594ff478fddbab7f/> update random number generation and DataFrame initialization
   - Replaced instances of `rand::thread_rng()` with `rand::rng()` for consistent random number generation across synthetic data functions.
   - Modified DataFrame initialization to include the number of events, ensuring accurate data representation in synthetic control and mixed sample generation.
   - Adjusted noise generation to utilize the new random range methods for improved clarity and functionality.

### New Features

 - <csr-id-cf0df0a44cf8ea82aab571f4bfe3684d99aaf213/> specta derives, matrix-context gate fields, Polars 0.54
   Restore the pre-peacoqc WIP so path consumers (fast-flow) get optional
   `specta` features, Embedding parameter category, gate spillover/data-context
   ids, and a Polars 0.54 workspace pin compatible with chrono 0.4.42.
 - <csr-id-7bca5d4ffe1467d8d8f1d7e9ecd618163b2a63b5/> QuadrantGate redesign — one gate owning dividers + 4 sub-quadrants
   Replaces the legacy single-corner Quadrant geometry with a GatingML-faithful
   QuadrantGate that owns its dividers and four addressable sub-quadrants. Adds
   corner-selective containment, lossless GatingML round-trip, a per-corner
   label_position (for independent on-plot label dragging), and the
   create_quadrant_gate_geometry constructor. GateHierarchy gains from_gates
   (derived index rebuilt from parent_id). Major version bump to 0.4.0
   (breaking — legacy "Quadrant" data no longer deserializes).
 - <csr-id-fe642f65aa7d03fac6d688f4598143d8c2955137/> split flow-utils into flow-density + flow-clustering crates
   flow-utils bundled unrelated concerns (KDE, clustering, PCA). Split into
   focused crates so consumers only pull what they need. flow-utils removed
   from workspace members; existing code updated to import from new crates.
 - <csr-id-cf81b82caa6124c9a2e58e3a18d49ff6541c6c06/> add throughput reporting and benchmark data generation
   Adds JSON/markdown throughput reports for Rust-vs-Julia wall-clock
   comparisons with CPU detection and environment snapshots.
   Pure-numeric synthetic data generator for profiling without FCS I/O.
   E2E plate throughput example and updated CLI workflows.
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
 - <csr-id-359e3fed28b43032d90384b9c340313e0960eeee/> per-control prefix on QC debug plots, skip primary plots for unstained
   When QC ran on multiple controls, the pipeline's built-in diagnostic
   PNGs (`peacoqc_overview.png`, `scatter_post_debris.png`) were all
   written to the same shared path, so every control overwrote the
   previous one. Only the last-processed control's diagnostics survived
   on disk, which defeats the purpose of the bundle.
   
   Add `QcPipelineConfig::debug_plot_label` plus `sanitized_plot_prefix()`;
   the pipeline now writes `<label>_peacoqc_overview.png` and
   `<label>_scatter_post_debris.png`. Call sites set the label to the
   unstained filename (prefixed with `unstained_`) and to each
   single-stain control's file stem so every control has its own bundle.
   
   The env-gated debug-plot smoke test updates its expected filename
   (`qc_scatter_post_debris.png`, since the default prefix sanitises to
   `qc` when no label is set). Tracking `qc_pipeline.rs` for the first
   time.
   
   Separately, `generate_control_cleanup_debug_plots` now takes
   `primary_idx: Option<usize>`; callers pass `None` for the unstained
   control so plots 06 (primary-vs-SSC-A) and 07 (spectral from peak
   events) are skipped — both are meaningless for an autofluorescence
   control.
 - <csr-id-520e394b880f77c4d6ee73e739c831806364fce0/> graceful unstained failure when AF comes from negatives
   Two issues when the unstained control is so noisy that its QC pipeline
   halts on the scatter/consensus-FSC stage:
   
   1. The whole run aborted even though autofluorescence was going to come
      from single-stain negative events (`--autofluorescence-mode
      negative-events`) or from per-control negatives in universal mode
      with `--use-negative-events`. In those modes the unstained medians
      are only a fallback, so a failed unstained gate should be a warning,
      not a hard stop.
   
   2. No debug plots were written for the unstained control before the
      pipeline bailed, so the user couldn't visually confirm whether the
      input itself was the problem.
 - <csr-id-6897eb46d054149643d23caea77f7c34df97f1e0/> name the failing control in QC pipeline errors
   When `run_qc_pipeline` fails mid-loop over the single-stain controls or
   on the unstained, the error bubbled up without identifying which file
   was responsible, so the user couldn't tell from the log which control
   needed attention.
   
   - Wrap each `run_qc_pipeline` call with `.with_context()` naming the
     file (and endmember / full path for single-stain controls).
   - Emit an `info!` line with the filename immediately before each
     pipeline run so subsequent library-level `warn!`s (margin removal,
     PeacoQC bin count, etc.) appear under a labelled section.
   - Derive the unstained control's display label from its `$FIL`
     keyword, falling back to "unstained control" when absent.
 - <csr-id-2f6da2409f286a2044a0561183830057b2dcb326/> gate unstained control, numbered QC plots, persisted interactive state
   Gate the unstained control through the same QC pipeline as single-stain
   controls when `--auto-gate` is set, so the autofluorescence medians come
   from debris/doublet/margin-filtered events rather than the raw file.
   Previously the unstained was used raw, which over-stated autofluorescence
   whenever the unstained file carried debris and biased the mixing matrix.
   
   Debug control plots (`--debug-control-plots`) rename and extend the
   output set. Filenames are prefix-numbered so `ls` shows them in QC
   execution order:
   
     01_pre_gating_FSC-A_vs_SSC-A_<name>.jpg
     02_post_margin_FSC-A_vs_SSC-A_<name>.jpg
     03_post_doublet_FSC-A_vs_SSC-A_<name>.jpg
     03_pre_doublet_fsca_fsch_FSC-A_vs_FSC-H_<name>.jpg  (new)
     03_post_doublet_fsca_fsch_FSC-A_vs_FSC-H_<name>.jpg (new)
     04_post_debris_FSC-A_vs_SSC-A_<name>.jpg
     05_post_gating_FSC-A_vs_SSC-A_<name>.jpg
     06_primary_vs_ssca_<primary>_vs_SSC-A_<name>.jpg    (new)
     07_spectral_from_peak_events_<name>.jpg
   
   06_primary_vs_ssca plots signal strength on the detector actually used
   for the spectral median, which helps sanity-check the primary detector
   selection. Axis ranges now read $PnR from metadata instead of the
   hardcoded 262144, so plots for data clipped at 4.2e6 are no longer
   scaled to a narrow lower-left corner.
   
   Add per-working-directory state persistence (`.tru-ols-state.json`)
   written on every interactive run. On subsequent runs the CLI offers a
   three-way launch menu: re-run with prior choices, edit a single setting,
   or start fresh. The edit-one flow shows each field's current value
   inline so the user can pick the one to change without walking through
   every prompt. The state file doubles as an audit log of parameters used
   for the most recent run.
   
   When the user doesn't set an explicit mixing-matrix export path, derive
   one automatically (next to the output FCS, else plot dir, else cwd) and
   always write it. Reusing the matrix via `--mixing-matrix` on subsequent
   runs now works without the user having to remember to enable export.
 - <csr-id-edc918fac4d22d115ef44032c5060375d4e9cff6/> interactive prompts and plotters-only flow-plots
   - Add inquire/shellexpand; interactive module and expanded commands
   - Default-features=false for flow-plots; README and example tweaks
   - Refresh workspace Cargo.lock
 - <csr-id-8b26c1418137646bb311d45a678d1d43ef05a22d/> scatter overlay, z-axis coloring, density point size, contours, histograms
   - ScatterPlotData: discrete gate colors (ScatterOverlay), continuous z-axis (ScatterColoredContinuous)
   - Density plots: point_size affects contribution radius (matches scatter behavior)
   - Contour plots: KDE-based contours, draw_outliers, contour_smoothing
   - HistogramPlot: filled/unfilled, overlaid with gate colors, baseline separation, scale_to_peak
   - Breaking: DensityPlot::Data is now ScatterPlotData; use .into() for Vec<(f32,f32)>
   - Updated tru-ols, tru-ols-cli, gates for new API
 - <csr-id-5c6c02a44bcc7abe9a79297d7b33ddbcd15e7fcb/> peak detection, synthetic data, and spectral unmixing
   - Peak detection enabled by default for single-stain control analysis
   - Synthetic FCS data generation with known ground truth
   - Spectral unmixing with --controls auto-detection
   - Examples: generate_synthetic_test_data, compare_with_julia, check_unmixed
   - Peak detection unit tests
 - <csr-id-a086f6c1501996fe7eee5d3b1798f7fab924f853/> integrate automated gating (Task 3.1)
   - Add --auto-gate flag to enable automated preprocessing gates
   - Apply scatter and doublet gates to controls before processing
   - Gate results logged (full filtering requires FCS creation API)
   - Add flow-gates dependency
   - Create comprehensive testing instructions document
   - All compilation errors resolved
 - <csr-id-85475ba5e55685cbc14b6dcea413b0a7110faf23/> integrate peak detection for single-stain controls
   - Add flow-utils dependency for KDE peak detection
   - Add CLI options: --peak-detection, --peak-threshold, --peak-bias
   - Implement calculate_peak_based_median function
   - Replace simple median with peak-based median when enabled
   - Add SingleStainConfig struct for configuration
   - Fallback to simple median if peak detection fails

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
 - <csr-id-d123de32a05868bfc9a25410d9331f22ef90878d/> :memo: changed package name
   Changed to `tru-ols` to make a concise CLI command
 - <csr-id-292bd202b232c6f780a9cc7170cc1d53b443e05e/> add CLI reference and validation reports
   - CLI_ARGUMENTS_REFERENCE: complete argument reference for tru-ols unmix
   - COMPARISON_WITH_JULIA: Rust vs Julia comparison framework
   - PEAK_DETECTION_VALIDATION: peak detection validation report
   - VALIDATION_REPORT: algorithm validation and fixes
   - TRU-OLS vs AutoSpectral: academic comparison
   - UNMIXING_RESULTS_PLATE001: Plate_001 analysis results

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
 - <csr-id-ed58ed43fa56ac62fd3164857d7b47f966091f0d/> Release
 - <csr-id-2bba36cea51cf27174a5572e2597211b95dec140/> add CHANGELOG entry for 0.1.1
 - <csr-id-fd1cc4a76af40804018e24792dce407860302857/> Release
 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release
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

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 50 commits contributed to the release.
 - 33 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Restore commands.rs and workspace membership ([`0c924be`](https://github.com/jrmoynihan/flow/commit/0c924be8d1e6fd1c48132106ce4ee3cfb1535960))
    - Default test commands to cargo-nextest (flow-crates-9xv) ([`af6097f`](https://github.com/jrmoynihan/flow/commit/af6097fbd09f00657eaf82ea8367fffd3ee72baf))
    - Merge branch 'main' into worktree-lazy-fcs-column-loading-stage-a ([`52b5c50`](https://github.com/jrmoynihan/flow/commit/52b5c508956b9888bebe7a1279b47c26932afc7d))
    - Derive serde on GateParameters/LabelPosition, drop legacy forms ([`1f9508f`](https://github.com/jrmoynihan/flow/commit/1f9508f34dab1be6d0195e827b9dbc367c50cdd8))
    - Apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal ([`6986541`](https://github.com/jrmoynihan/flow/commit/6986541e936967c566b3c6caca42c9e0cbf5678f))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Specta derives, matrix-context gate fields, Polars 0.54 ([`cf0df0a`](https://github.com/jrmoynihan/flow/commit/cf0df0a44cf8ea82aab571f4bfe3684d99aaf213))
    - Release peacoqc-rs v0.3.0, safety bump 2 crates ([`604a94e`](https://github.com/jrmoynihan/flow/commit/604a94e13464acb60582292768ce3f97598ea55e))
    - QuadrantGate redesign — one gate owning dividers + 4 sub-quadrants ([`7bca5d4`](https://github.com/jrmoynihan/flow/commit/7bca5d4ffe1467d8d8f1d7e9ecd618163b2a63b5))
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Split flow-utils into flow-density + flow-clustering crates ([`fe642f6`](https://github.com/jrmoynihan/flow/commit/fe642f65aa7d03fac6d688f4598143d8c2955137))
    - Add throughput reporting and benchmark data generation ([`cf81b82`](https://github.com/jrmoynihan/flow/commit/cf81b82caa6124c9a2e58e3a18d49ff6541c6c06))
    - Raw-scale AF medians plot for unstained control ([`7674f5f`](https://github.com/jrmoynihan/flow/commit/7674f5f49c84ab9f4147ae0fbd79dd9a823bfb29))
    - Per-control prefix on QC debug plots, skip primary plots for unstained ([`359e3fe`](https://github.com/jrmoynihan/flow/commit/359e3fed28b43032d90384b9c340313e0960eeee))
    - Graceful unstained failure when AF comes from negatives ([`520e394`](https://github.com/jrmoynihan/flow/commit/520e394b880f77c4d6ee73e739c831806364fce0))
    - Name the failing control in QC pipeline errors ([`6897eb4`](https://github.com/jrmoynihan/flow/commit/6897eb46d054149643d23caea77f7c34df97f1e0))
    - Gate unstained control, numbered QC plots, persisted interactive state ([`2f6da24`](https://github.com/jrmoynihan/flow/commit/2f6da2409f286a2044a0561183830057b2dcb326))
    - Interactive prompts and plotters-only flow-plots ([`edc918f`](https://github.com/jrmoynihan/flow/commit/edc918fac4d22d115ef44032c5060375d4e9cff6))
    - Ensure output directories exist before running ([`dadc1dd`](https://github.com/jrmoynihan/flow/commit/dadc1dd6069ed7e1d294f5a7549889ce26685c76))
    - Release flow-plots v0.3.1 ([`2050584`](https://github.com/jrmoynihan/flow/commit/2050584238b7b516ee209e4f0cb67543d3c3ba09))
    - Merge branch 'cursor/axis-gate-interaction-630e' into main ([`c021235`](https://github.com/jrmoynihan/flow/commit/c021235f1555962be2177f2edd5a49de646effd4))
    - Merge origin/main into cursor/density-plot-point-size-1d39 ([`d1dc0d2`](https://github.com/jrmoynihan/flow/commit/d1dc0d24de430d22d0c434b46a23201a25ee2b2b))
    - Release ([`ed58ed4`](https://github.com/jrmoynihan/flow/commit/ed58ed43fa56ac62fd3164857d7b47f966091f0d))
    - Add CHANGELOG entry for 0.1.1 ([`2bba36c`](https://github.com/jrmoynihan/flow/commit/2bba36cea51cf27174a5572e2597211b95dec140))
    - Use commands.rs with ScatterPlotData compatibility ([`5ddcd7e`](https://github.com/jrmoynihan/flow/commit/5ddcd7e5e8c9ec8c330c56407d21f961164b610f))
    - Push my commands ([`1c0eb47`](https://github.com/jrmoynihan/flow/commit/1c0eb47766324873c64d273e2122fe43735d46e7))
    - Rename flow-tru-ols-cli to tru-ols and update dependencies ([`7b3d4ae`](https://github.com/jrmoynihan/flow/commit/7b3d4ae7ab92af89d94b1f1d0f0832b0f1048faa))
    - Release ([`fd1cc4a`](https://github.com/jrmoynihan/flow/commit/fd1cc4a76af40804018e24792dce407860302857))
    - Scatter overlay, z-axis coloring, density point size, contours, histograms ([`8b26c14`](https://github.com/jrmoynihan/flow/commit/8b26c1418137646bb311d45a678d1d43ef05a22d))
    - :memo: changed package name ([`d123de3`](https://github.com/jrmoynihan/flow/commit/d123de32a05868bfc9a25410d9331f22ef90878d))
    - Release peacoqc-rs v0.2.4, peacoqc-cli v0.2.4 ([`cea03b0`](https://github.com/jrmoynihan/flow/commit/cea03b013c10ae71a83a00fdf96dbea205afc961))
    - Release peacoqc-rs v0.2.3 ([`7600d54`](https://github.com/jrmoynihan/flow/commit/7600d54b5bdbedb4c5e8189265a6b5f20a1970cf))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Update random number generation and DataFrame initialization ([`264bb78`](https://github.com/jrmoynihan/flow/commit/264bb789d8447f7f35c0d649594ff478fddbab7f))
    - Release peacoqc-rs v0.2.1, flow-utils v0.1.1, flow-gates v0.2.2, flow-tru-ols v0.1.0 ([`c3d9774`](https://github.com/jrmoynihan/flow/commit/c3d97742b3f83d01f1b831eea6eb662a2511adb9))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Update for faer-based fcs and tru-ols APIs ([`70008ac`](https://github.com/jrmoynihan/flow/commit/70008ac39d1d08497c2f59e7fde438d0755433d3))
    - Add CLI reference and validation reports ([`292bd20`](https://github.com/jrmoynihan/flow/commit/292bd202b232c6f780a9cc7170cc1d53b443e05e))
    - Peak detection, synthetic data, and spectral unmixing ([`5c6c02a`](https://github.com/jrmoynihan/flow/commit/5c6c02a44bcc7abe9a79297d7b33ddbcd15e7fcb))
    - Integrate automated gating (Task 3.1) ([`a086f6c`](https://github.com/jrmoynihan/flow/commit/a086f6c1501996fe7eee5d3b1798f7fab924f853))
    - Integrate peak detection for single-stain controls ([`85475ba`](https://github.com/jrmoynihan/flow/commit/85475ba5e55685cbc14b6dcea413b0a7110faf23))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
</details>

## 0.1.1 (2026-03-04)

### Fixed

- Compatibility with flow-plots v0.3.0 `ScatterPlotData`: add `.into()` for all density plot renders

### Chore

- Use `commands.rs` as primary module (rename from commands_mine.rs), remove from .gitignore

## 0.1.0 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-70008ac39d1d08497c2f59e7fde438d0755433d3/>
<csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/>

### New Features

<csr-id-a086f6c1501996fe7eee5d3b1798f7fab924f853/>
<csr-id-85475ba5e55685cbc14b6dcea413b0a7110faf23/>

 - <csr-id-5c6c02a44bcc7abe9a79297d7b33ddbcd15e7fcb/> peak detection, synthetic data, and spectral unmixing
   - Peak detection enabled by default for single-stain control analysis
   - Automated scatter and doublet gating (optional `--auto-gate`), peak-based median for single-stain controls, CLI options `--peak-detection`, `--peak-threshold`, `--peak-bias`
   - Synthetic FCS generation example and Julia comparison utilities

### Documentation

 - <csr-id-292bd202b232c6f780a9cc7170cc1d53b443e05e/> add CLI reference and validation reports
   - Complete argument reference for `tru-ols unmix` (CLI_ARGUMENTS_REFERENCE)

### Refactor

 - <csr-id-70008ac39d1d08497c2f59e7fde438d0755433d3/> update for faer-based fcs and tru-ols APIs
   - faer-ext for ndarray↔faer conversion; `MatRef` for spectral unmixing, spill matrix, and CSV export; ndarray retained for plotting/export

### Chore

 - <csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/> update dependencies and align workspace configurations
   - Workspace-wide dependency updates (e.g. polars 0.53, faer 0.24, ndarray-linalg 0.18); more dependencies use workspace references.
 - <csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/> clean up unused imports and variables
   - Removed unused imports in clustering/gating modules, fixed unreachable code in DBSCAN, dropped unnecessary `mut`; general warning cleanup.
 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release

