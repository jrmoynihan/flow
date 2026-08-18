# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- First crates.io publish of `flow-gates` 0.5.0: derived serde on `GateParameters`/`LabelPosition` (plural wire tags) and removal of the legacy tuple/`companion` forms.

### Chore

 - <csr-id-af6097fbd09f00657eaf82ea8367fffd3ee72baf/> default test commands to cargo-nextest (flow-crates-9xv)
   Nextest runs each test in its own process and reports per-test timing, so
   make it the default runner everywhere the project tells a human or an agent
   how to run tests, rather than leaving it as an opt-in each caller remembers.
   
   Adds .config/nextest.toml (default profile fails fast; a ci profile runs the
   whole suite) and a `cargo nt` alias, since Cargo cannot alias the built-in
   `test` subcommand. Doctests stay on the built-in harness because nextest
   cannot run them.

### Documentation

 - <csr-id-3c48e73e751a7852b0e07239540448e6ee35a0cf/> refresh crate READMEs and agent guidelines
   Keep the beads export in sync and add the Svelte MCP server to Codex config.
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
 - <csr-id-6d051ac5a34e63a997ca85c8819b790c0d161c8a/> scrub changelog neutralize wording after release rewrite
   smart-release reintroduced commit-message phrasing about neutralizing
   attribution; restore neutral release notes and drop the misplaced
   pacmap prep entry from linalg/gates changelogs.

### Bug Fixes

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
 - <csr-id-8dd7e6d6251dae840e4a40b4b68a9fa2c97b6220/> keep GateParameters wire tags plural (two_channels/no_channels)
   Reconsider the previous commit (2c0efed): the plural tags read better and
   we don't need backward compatibility with the brief window they were live
   yet, so keep two_channels/no_channels going forward instead of reverting to
   singular. Drop the deserialize-side aliases that reverted commit added, and
   update the gate_roundtrip.rs fixtures to match.
 - <csr-id-2c0efed0bcfa742e847f46a22157cb046518a91d/> restore singular GateParameters wire tags (two_channel/no_channel)
   Commit dcb5528 ("style: apply let-chains and formatting cleanups") silently
   renamed the internally-tagged serde variants from TwoChannel/NoChannel to
   TwoChannels/NoChannels, despite claiming no semantic change. This broke
   deserialization of real saved workspace gate JSON, which uses the singular
   tags matching the enum variant names.
   
   Revert serialization to the singular tags, and accept the plural spelling
   on deserialize via #[serde(alias)] for any workspaces saved during the
   2026-08-02 to 2026-08-06 window the bug was live.
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

### Style

 - <csr-id-dcb5528bc256d7dd754481c18660919819d605c7/> apply let-chains and formatting cleanups
   Adopt modern if-let chains and rustfmt across gating XML, hierarchy, and
   transform helpers without changing gate semantics.

### New Features (BREAKING)

 - <csr-id-1f9508f34dab1be6d0195e827b9dbc367c50cdd8/> derive serde on GateParameters/LabelPosition, drop legacy forms
   Replace hand-written Serialize/Deserialize impls with derives so the wire
   tags live in exactly one place — the Rust variant names — and flow through
   ts-rs/specta unedited. Previously the tag list existed in four
   hand-synchronized copies (Rust serde, Rust ts(type=...), the frontend
   interfaces mirror, and the generated binding), which had already drifted.

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

 - 13 commits contributed to the release over the course of 26 calendar days.
 - 26 days passed between releases.
 - 12 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Refresh crate READMEs and agent guidelines ([`3c48e73`](https://github.com/jrmoynihan/flow/commit/3c48e73e751a7852b0e07239540448e6ee35a0cf))
    - Default test commands to cargo-nextest (flow-crates-9xv) ([`af6097f`](https://github.com/jrmoynihan/flow/commit/af6097fbd09f00657eaf82ea8367fffd3ee72baf))
    - Merge branch 'main' into worktree-lazy-fcs-column-loading-stage-a ([`52b5c50`](https://github.com/jrmoynihan/flow/commit/52b5c508956b9888bebe7a1279b47c26932afc7d))
    - Resolve offsets data-set-relative, fold OTHER into CRC range ([`f0b2922`](https://github.com/jrmoynihan/flow/commit/f0b29225fb01d5d2c8060e2b9fdf4b9b87b2dfa7))
    - Address final whole-branch review findings (bounds check, cache warning, feature scoping, version bump, benchmark docs) ([`a565fdf`](https://github.com/jrmoynihan/flow/commit/a565fdf4b372fe74eb6393eb61218a8ea159b6fe))
    - Add Fcs::for_testing constructor, restore cross-crate test-fixture construction ([`6e3d723`](https://github.com/jrmoynihan/flow/commit/6e3d7233683f7c18b858829c83844171fa6adfd1))
    - Derive serde on GateParameters/LabelPosition, drop legacy forms ([`1f9508f`](https://github.com/jrmoynihan/flow/commit/1f9508f34dab1be6d0195e827b9dbc367c50cdd8))
    - Keep GateParameters wire tags plural (two_channels/no_channels) ([`8dd7e6d`](https://github.com/jrmoynihan/flow/commit/8dd7e6d6251dae840e4a40b4b68a9fa2c97b6220))
    - Restore singular GateParameters wire tags (two_channel/no_channel) ([`2c0efed`](https://github.com/jrmoynihan/flow/commit/2c0efed0bcfa742e847f46a22157cb046518a91d))
    - Apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal ([`6986541`](https://github.com/jrmoynihan/flow/commit/6986541e936967c566b3c6caca42c9e0cbf5678f))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Apply let-chains and formatting cleanups ([`dcb5528`](https://github.com/jrmoynihan/flow/commit/dcb5528bc256d7dd754481c18660919819d605c7))
    - Scrub changelog neutralize wording after release rewrite ([`6d051ac`](https://github.com/jrmoynihan/flow/commit/6d051ac5a34e63a997ca85c8819b790c0d161c8a))
</details>

## 0.5.0 (2026-08-06)

### Breaking

 - **`GateParameters` now derives `Serialize`/`Deserialize` instead of hand-written
   impls.** The wire tags follow the variant names, so the tag strings live in
   exactly one place and flow through `ts-rs`/`specta` unedited. Variants renamed
   `TwoChannel` → `TwoChannels` and `NoChannel` → `NoChannels` (`OneChannel` is
   unchanged); the JSON tags change to `"two_channels"` / `"no_channels"`
   accordingly.
 - Removed the `GateParameters::Legacy([String; 2])` accept-only tuple form and
   the `companion` field. A one-channel gate stores only its bounded `channel`;
   consumers needing a paired axis take it from the plot context.
 - **`LabelPosition` no longer accepts the pre-per-channel
   `{ offset_x, offset_y }` form.** The only accepted shape is
   `{ "offsets": { "<channel>": <f32> } }`, and it too now derives its serde impls.
 - Removed `LABEL_LEGACY_X_KEY`, `LABEL_LEGACY_Y_KEY`, and
   `Gate::fixup_label_position`. The fixup had no callers, so legacy blobs were
   already loading with dead sentinel keys and rendering at offset 0 — the
   "compatibility" was a silent no-op, not a migration.
 - **No migration:** serialized gates carrying the old parameter tags or the old
   label-position shape will not deserialize. Callers must handle the load failure
   (log + continue). This converts a silent wrong-render into a loud parse error.

## 0.4.0 (2026-07-22)

<csr-id-1e7cca7cde867e127e8e4a4b253cab187799ceb5/>
<csr-id-713aaaf067f0055296dc31e8027f09dfc7c220d0/>
<csr-id-3cb462369965122ff7d78874f2bfa4f4d7cdf4e4/>
<csr-id-bef89e377dd77e83cf69e03d150fa92d4a1ceaed/>
<csr-id-bbe31765ae740038c394ebefb1e09e825259e8b5/>
<csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/>
<csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/>
<csr-id-fd1cc4a76af40804018e24792dce407860302857/>

### Breaking

 - **`GateGeometry::Quadrant` (single corner) replaced by
   `GateGeometry::QuadrantGate(Box<QuadrantGate>)`.** A quadrant is now ONE gate
   owning its dividers and 4 addressable sub-quadrants (matching GatingML's
   `<gating:QuadrantGate>`), instead of four independent corner gates. Each
   `QuadrantSub` carries a stable id; corner-selective containment is exposed via
   `GateGeometry::contains_point_corner(sub_id, ...)` /
   `contains_points_batch_corner(...)` and `EventIndex::filter_by_quadrant_corner`.
   The gate-level containment traits error for a quadrant gate (no single
   population), like `Boolean`.
 - Removed `QuadrantGroup` struct and `DerivedFrom::QuadrantGroup` — the label and
   dividers now live inside the gate, so no side record is needed.
 - `create_quadrant_corner_geometry(...)` replaced by
   `create_quadrant_gate_geometry(base_id, x_channel, x_value, y_channel, y_value)`.
 - GatingML export/import now supports quadrant gates losslessly (round-trips
   dividers + sub-quadrants + per-position direction).
 - Added `filter_events_by_gate_corner` and `filter_events_by_hierarchy_steps`
   (corner-aware sibling of `filter_events_by_hierarchy`).
 - **No migration:** serialized gates with the old `"type":"Quadrant"` shape will
   not deserialize. Callers must handle the load failure (log + continue).

### Chore

 - <csr-id-ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4/> prepare 0.1.1 release with Wang et al. attribution
   Bump flow-pacmap to 0.1.1, pin the README install line, and scrub changelog
   wording that celebrated neutralizing academic attribution.

### Chore

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> polish pacmap/linalg/gates for crates.io release
   Add README and publish metadata for flow-pacmap, and refresh install/API
   notes for the upcoming flow-linalg and flow-gates releases.
 - <csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/> bump flow-fcs to 0.4.0, add publish metadata to new crates
   - flow-fcs 0.3.0 → 0.4.0 (new compensation feature + public API)
   - flow-linalg, flow-density, flow-clustering: add repository field
     and smart-release scripts for first publish
   - Update all workspace consumers to ^0.4.0
 - <csr-id-fd1cc4a76af40804018e24792dce407860302857/> Release

### New Features

 - <csr-id-b14cbd40e6ccac96d04602e86a7934c51d08a706/> add GateOrigin provenance for compensation-control gates
   Distinguish user, QC, and compensation-control gates so UIs can hide
   control-seeded gates while keeping QC visible; serde skips the User default
   for backward compatibility.
 - <csr-id-cf0df0a44cf8ea82aab571f4bfe3684d99aaf213/> specta derives, matrix-context gate fields, Polars 0.54
   Optional `specta` features, Embedding parameter category, gate
   spillover/data-context ids, and a Polars 0.54 workspace pin.
 - <csr-id-5258357b4c5049d5ba76c0208aa4fb53571d2bd3/> optional `typescript` feature — derive ts_rs::TS on gate types
   Off-by-default `typescript` cargo feature that derives `ts_rs::TS` on public
   gate types for TypeScript binding generation.
 - <csr-id-7bca5d4ffe1467d8d8f1d7e9ecd618163b2a63b5/> QuadrantGate redesign — one gate owning dividers + 4 sub-quadrants
   See Breaking above for API migration notes.
 - <csr-id-bfe56b715fb1bae7b5b03e66deeea2671f505833/> add Mask geometry, NoChannel parameters, overrides, system_managed
   - GateGeometry::Mask { source: MaskSource } for precomputed event masks
   - MaskSource::Qc { file_guid, invert } for QC pass/fail
   - GateParameters::NoChannel for parameter-agnostic gates
   - MaskResolver trait and filter_events_by_hierarchy_with_resolvers()
   - Gate.overrides / Gate.system_managed / Gate.effective_geometry(...)
 - <csr-id-d1f6b53e5e55fab080b0ae1b57b1b304d7a20ad2/> add raw_coords_to_pixels_with_layout for gate rendering
 - <csr-id-c36c7bc9cc05b36ec712275955f04d2ce92d6ab4/> pass actual plot layout to pixel→data conversion
 - <csr-id-f837912c1e9d69251d4c4262044ade4713749558/> add threshold gate geometry, refactor statistics and filtering
 - <csr-id-1ad410a3185f357bbecc11277607a8e2694a8ae3/> add enum plot parameters with legacy migration
 - <csr-id-93835900bc09c3081379143336361b8695034acb/> multi-channel doublet detection and RatioInflectionOrFixed method
 - <csr-id-62c0adda9e1e2e14cf26a6dc5b39ff94b006649a/> add create_range_geometry constructor and GatingML serialization
 - <csr-id-499716c7db8470901168fa933b67022cee9f3bdf/> add Range to EventIndex filtering
 - <csr-id-ba241218da5d1222f83e9864255a40074f6f410e/> add Range variant to GateGeometry enum with trait delegation
 - <csr-id-874d38211d388d890c5731c59c026692b4648acc/> add filter_by_range_batch with test
 - <csr-id-2b8b1c9b96f0ef1bb735854ad19c75b80005caed/> add RangeGateGeometry struct with trait implementations
 - <csr-id-fe642f65aa7d03fac6d688f4598143d8c2955137/> split flow-utils into flow-density + flow-clustering crates
   flow-utils bundled unrelated concerns (KDE, clustering, PCA). Split into
   focused crates so consumers only pull what they need. flow-utils removed
   from workspace members; existing code updated to import from new crates.
 - <csr-id-8b26c1418137646bb311d45a678d1d43ef05a22d/> scatter overlay, z-axis coloring, density point size, contours, histograms
   - ScatterPlotData: discrete gate colors (ScatterOverlay), continuous z-axis (ScatterColoredContinuous)
   - Density plots: point_size affects contribution radius (matches scatter behavior)
   - Contour plots: KDE-based contours, draw_outliers, contour_smoothing
   - HistogramPlot: filled/unfilled, overlaid with gate colors, baseline separation, scale_to_peak
   - Breaking: DensityPlot::Data is now ScatterPlotData; use .into() for Vec<(f32,f32)>
   - Updated tru-ols, tru-ols-cli, gates for new API
 - <csr-id-d36e19b2d86c270e905f84451ccf1757fd16a56c/> add round-trip test for Gate serialization and deserialization
   - Introduced a new test file to validate the serialization and deserialization of the Gate struct using JSON.
   - Implemented a helper function to create a test Gate instance for the round-trip check, ensuring that the original and restored Gate instances are equal.

### Documentation

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> refresh README install pin and GateOrigin feature note
 - <csr-id-1d5806048f15f590ebe7b2ba449501aa73868b95/> polish changelogs for pacmap, linalg, and gates releases

### Bug Fixes

 - <csr-id-09d2f0868510710d31f4bbbcaf677a584c6ac2da/> don't reject Mask gates in hierarchy_steps closure path
 - <csr-id-f683ff6cae4aa54118db491b0d5bdfd2fb5a17f2/> support y-axis range gates in containment, bounds, and batch filtering
 - <csr-id-9684227966218f774a32825c64f3168917549ca5/> use point-in-polygon for density contour scatter mask
 - <csr-id-45e14e82fb8af124e5f08a08dbe2d21ff131e2cf/> ordered density contours for automated scatter gating
 - <csr-id-32f1c91b33433f2fbe0d1c48d20ac4f286ebb5be/> update DataFrame creation in synthetic data visualization example
   - Modified the DataFrame initialization in the visualize_synthetic_data example to include the number of events, ensuring accurate test data representation.
   - This change enhances the reliability of the synthetic data generation for testing purposes.
 - <csr-id-581fd0d8b728198be59a4be79ab64defcb281069/> replace thread_rng with rng for random number generation
   - Updated random number generation in multiple functions to use `rand::rng()` instead of `rand::thread_rng()`, ensuring consistent RNG usage across synthetic data generation methods.

### Refactor

 - <csr-id-1e7cca7cde867e127e8e4a4b253cab187799ceb5/> make MaskSource::Qc.file_guid optional for global QC gates

### Test

 - <csr-id-713aaaf067f0055296dc31e8027f09dfc7c220d0/> add mixed QC status integration test
 - <csr-id-3cb462369965122ff7d78874f2bfa4f4d7cdf4e4/> add mask gate test suite covering QC design scenarios
 - <csr-id-bef89e377dd77e83cf69e03d150fa92d4a1ceaed/> add pixel↔data roundtrip tests with layout params
 - <csr-id-bbe31765ae740038c394ebefb1e09e825259e8b5/> add roundtrip tests for _with_layout coordinate transforms

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 46 commits contributed to the release.
 - 34 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-pacmap v0.1.1, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`b44c4f5`](https://github.com/jrmoynihan/flow/commit/b44c4f5915916ab3ce73a5b6ce421601090f77a5))
    - Prepare 0.1.1 release with Wang et al. attribution ([`ba96d7f`](https://github.com/jrmoynihan/flow/commit/ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4))
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Polish changelogs for pacmap, linalg, and gates releases ([`1d58060`](https://github.com/jrmoynihan/flow/commit/1d5806048f15f590ebe7b2ba449501aa73868b95))
    - Polish pacmap/linalg/gates for crates.io release ([`c9b7448`](https://github.com/jrmoynihan/flow/commit/c9b7448fef935e2ba6f3ea568ce092f9c777b53b))
    - Add GateOrigin provenance for compensation-control gates ([`b14cbd4`](https://github.com/jrmoynihan/flow/commit/b14cbd40e6ccac96d04602e86a7934c51d08a706))
    - Release flow-fcs v0.4.1 ([`597f21b`](https://github.com/jrmoynihan/flow/commit/597f21bef7ea787437071685fc3cce9d2269270f))
    - Specta derives, matrix-context gate fields, Polars 0.54 ([`cf0df0a`](https://github.com/jrmoynihan/flow/commit/cf0df0a44cf8ea82aab571f4bfe3684d99aaf213))
    - Optional `typescript` feature — derive ts_rs::TS on gate types ([`5258357`](https://github.com/jrmoynihan/flow/commit/5258357b4c5049d5ba76c0208aa4fb53571d2bd3))
    - Add mixed QC status integration test ([`713aaaf`](https://github.com/jrmoynihan/flow/commit/713aaaf067f0055296dc31e8027f09dfc7c220d0))
    - Add mask gate test suite covering QC design scenarios ([`3cb4623`](https://github.com/jrmoynihan/flow/commit/3cb462369965122ff7d78874f2bfa4f4d7cdf4e4))
    - Don't reject Mask gates in hierarchy_steps closure path ([`09d2f08`](https://github.com/jrmoynihan/flow/commit/09d2f0868510710d31f4bbbcaf677a584c6ac2da))
    - QuadrantGate redesign — one gate owning dividers + 4 sub-quadrants ([`7bca5d4`](https://github.com/jrmoynihan/flow/commit/7bca5d4ffe1467d8d8f1d7e9ecd618163b2a63b5))
    - Make MaskSource::Qc.file_guid optional for global QC gates ([`1e7cca7`](https://github.com/jrmoynihan/flow/commit/1e7cca7cde867e127e8e4a4b253cab187799ceb5))
    - Add Mask geometry, NoChannel parameters, overrides, system_managed ([`bfe56b7`](https://github.com/jrmoynihan/flow/commit/bfe56b715fb1bae7b5b03e66deeea2671f505833))
    - Add pixel↔data roundtrip tests with layout params ([`bef89e3`](https://github.com/jrmoynihan/flow/commit/bef89e377dd77e83cf69e03d150fa92d4a1ceaed))
    - Add roundtrip tests for _with_layout coordinate transforms ([`bbe3176`](https://github.com/jrmoynihan/flow/commit/bbe31765ae740038c394ebefb1e09e825259e8b5))
    - Add raw_coords_to_pixels_with_layout for gate rendering ([`d1f6b53`](https://github.com/jrmoynihan/flow/commit/d1f6b53e5e55fab080b0ae1b57b1b304d7a20ad2))
    - Pass actual plot layout to pixel→data conversion ([`c36c7bc`](https://github.com/jrmoynihan/flow/commit/c36c7bc9cc05b36ec712275955f04d2ce92d6ab4))
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Add threshold gate geometry, refactor statistics and filtering ([`f837912`](https://github.com/jrmoynihan/flow/commit/f837912c1e9d69251d4c4262044ade4713749558))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Split flow-utils into flow-density + flow-clustering crates ([`fe642f6`](https://github.com/jrmoynihan/flow/commit/fe642f65aa7d03fac6d688f4598143d8c2955137))
    - Add enum plot parameters with legacy migration ([`1ad410a`](https://github.com/jrmoynihan/flow/commit/1ad410a3185f357bbecc11277607a8e2694a8ae3))
    - Multi-channel doublet detection and RatioInflectionOrFixed method ([`9383590`](https://github.com/jrmoynihan/flow/commit/93835900bc09c3081379143336361b8695034acb))
    - Support y-axis range gates in containment, bounds, and batch filtering ([`f683ff6`](https://github.com/jrmoynihan/flow/commit/f683ff6cae4aa54118db491b0d5bdfd2fb5a17f2))
    - Add create_range_geometry constructor and GatingML serialization ([`62c0add`](https://github.com/jrmoynihan/flow/commit/62c0adda9e1e2e14cf26a6dc5b39ff94b006649a))
    - Add Range to EventIndex filtering ([`499716c`](https://github.com/jrmoynihan/flow/commit/499716c7db8470901168fa933b67022cee9f3bdf))
    - Add Range variant to GateGeometry enum with trait delegation ([`ba24121`](https://github.com/jrmoynihan/flow/commit/ba241218da5d1222f83e9864255a40074f6f410e))
    - Add filter_by_range_batch with test ([`874d382`](https://github.com/jrmoynihan/flow/commit/874d38211d388d890c5731c59c026692b4648acc))
    - Add RangeGateGeometry struct with trait implementations ([`2b8b1c9`](https://github.com/jrmoynihan/flow/commit/2b8b1c9b96f0ef1bb735854ad19c75b80005caed))
    - Use point-in-polygon for density contour scatter mask ([`9684227`](https://github.com/jrmoynihan/flow/commit/9684227966218f774a32825c64f3168917549ca5))
    - Ordered density contours for automated scatter gating ([`45e14e8`](https://github.com/jrmoynihan/flow/commit/45e14e82fb8af124e5f08a08dbe2d21ff131e2cf))
    - Release flow-plots v0.3.1 ([`2050584`](https://github.com/jrmoynihan/flow/commit/2050584238b7b516ee209e4f0cb67543d3c3ba09))
    - Merge branch 'cursor/axis-gate-interaction-630e' into main ([`c021235`](https://github.com/jrmoynihan/flow/commit/c021235f1555962be2177f2edd5a49de646effd4))
    - Release ([`fd1cc4a`](https://github.com/jrmoynihan/flow/commit/fd1cc4a76af40804018e24792dce407860302857))
    - Scatter overlay, z-axis coloring, density point size, contours, histograms ([`8b26c14`](https://github.com/jrmoynihan/flow/commit/8b26c1418137646bb311d45a678d1d43ef05a22d))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Update DataFrame creation in synthetic data visualization example ([`32f1c91`](https://github.com/jrmoynihan/flow/commit/32f1c91b33433f2fbe0d1c48d20ac4f286ebb5be))
    - Add round-trip test for Gate serialization and deserialization ([`d36e19b`](https://github.com/jrmoynihan/flow/commit/d36e19b2d86c270e905f84451ccf1757fd16a56c))
    - Replace thread_rng with rng for random number generation ([`581fd0d`](https://github.com/jrmoynihan/flow/commit/581fd0d8b728198be59a4be79ab64defcb281069))
    - Merge PR #15: add PartialEq to Gate, GateNode, GateGeometry and TransformType ([`98455bc`](https://github.com/jrmoynihan/flow/commit/98455bc69a3789f5c8eb9741a3cc024451e63a3e))
    - Add partialeq ([`e2ac3ec`](https://github.com/jrmoynihan/flow/commit/e2ac3ecf031a6a265c482a08f33ebed5c1f35bdd))
</details>

## 0.2.2 (2026-02-16)

<csr-id-1016027ac34ae0cb187d59d0d9562200321d5281/>

### Refactor

 - <csr-id-1016027ac34ae0cb187d59d0d9562200321d5281/> derive PartialEq, Eq, Hash on FilterCacheKey
   Replace manual impls with derive macro

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.2.1, flow-utils v0.1.1, flow-gates v0.2.2, flow-tru-ols v0.1.0 ([`c3d9774`](https://github.com/jrmoynihan/flow/commit/c3d97742b3f83d01f1b831eea6eb662a2511adb9))
    - Derive PartialEq, Eq, Hash on FilterCacheKey ([`1016027`](https://github.com/jrmoynihan/flow/commit/1016027ac34ae0cb187d59d0d9562200321d5281))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
</details>

## 0.2.1 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-bea47e8ee97b86a3120b8097d0fdbe6bc9fce133/>
<csr-id-dcf9154b305c79728dd2a9f61e4440b5a15756ea/>
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

### Documentation

<csr-id-6f6d0f59369453e3f0018b37f1377b204b023223/>
<csr-id-f9eef00689d5c1dbda8bce37ca0d399afae19d46/>

 - <csr-id-69d65c959a392f16431cc98beae9c361ccfed10a/> add implementation status document
   - Document completed features and known limitations

### New Features

<csr-id-0e1ee96078a18b06ce5c0c8776df9892d7861ea8/>
<csr-id-5996edc676f6a606fcd48e2ffc8ed3131f08ce0b/>
<csr-id-547c2ae09f0f263314de70750b8c8e01b4fd4661/>
<csr-id-c0ba8e72f6866bda5d9eec40a6f089ccc7c35107/>
<csr-id-340977390c10a31fdf7694ac9325147f406c5b72/>
<csr-id-6a65bd7077b2a12670c3766248b08447e92ea8b5/>
<csr-id-43a00f6f0e4043d9b973eb8c9ae2c18ff64b780d/>
<csr-id-c89944be9c68a1f688dfb5ee333c7562b28f90b1/>
<csr-id-7b65fbcc9119762ee4cf64cf129c017ece95ff30/>
<csr-id-c998c06382ec30a870452083b7366a74ced5830e/>
<csr-id-6762e5f0d484be7e8d45363205793a50e46b0eb3/>

 - <csr-id-42b46207448be5ca137b0b1067ddaa1222b50ccb/> add hierarchy support and gating improvements
   - Extend hierarchy module

### Bug Fixes

<csr-id-3683d6a9248108834f3be9c6ae7a844d96953b7a/>
<csr-id-a1894b8dd78f86970311dde59e0f863a685ef4ec/>
<csr-id-28677b4de7abaccf198f2a278a38c46a2364f193/>
<csr-id-c8d5ab0e62038fc07f17ffb89e9748c3a159007e/>
<csr-id-6596ed9f6d7916684d38ae65f9284ae7a40a937f/>
<csr-id-38013b28d81af8510a1065745d203bd5e2057518/>
<csr-id-ec337c29858cd506aec01548d0e8431fa6eec9f3/>
<csr-id-7b87699eb278bd7b7d37076aaaa730ff99fc3c53/>
<csr-id-383b476374a707447e655b1b0c0a298e91fd2cc3/>
<csr-id-385c0be364793819279fb9a50f38eb29bbceeab3/>
<csr-id-d33d3616c82ffc04001363ad3f3a9b7ccef0175f/>
<csr-id-161b1334a4a20d5fb0be80aee8134732840e9a6a/>

 - <csr-id-465089a6a99336556e492a02b06757fff54fbb63/> update example generation functions to use Gaussian distributions
   - Replace uniform random (rng.gen_range) with Normal distributions in all functions

### Test

 - <csr-id-bea47e8ee97b86a3120b8097d0fdbe6bc9fce133/> add synthetic FCS file generation for automated gating tests
   - Create test_helpers module with synthetic data generation
   - Support multiple test scenarios: single population, multi-population, doublets, noisy
   - Generate realistic scatter patterns (FSC-A, FSC-H, FSC-W, SSC-A, SSC-H)
   - Remove #[ignore] from all automated gating tests
   - Enable full test suite execution
 - <csr-id-dcf9154b305c79728dd2a9f61e4440b5a15756ea/> add integration tests for automated gating
   - Add tests for scatter gating (ellipse fit, density contour)
   - Add tests for doublet detection (MAD, density-based)
   - Add tests for preprocessing pipeline (automated and interactive)
   - Tests marked with #[ignore] until test data is available

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 38 commits contributed to the release over the course of 24 calendar days.
 - 24 days passed between releases.
 - 33 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Add hierarchy support and gating improvements ([`42b4620`](https://github.com/jrmoynihan/flow/commit/42b46207448be5ca137b0b1067ddaa1222b50ccb))
    - Triple event counts and adjust distributions ([`0e1ee96`](https://github.com/jrmoynihan/flow/commit/0e1ee96078a18b06ce5c0c8776df9892d7861ea8))
    - Update example generation functions to use Gaussian distributions ([`465089a`](https://github.com/jrmoynihan/flow/commit/465089a6a99336556e492a02b06757fff54fbb63))
    - Fix auto_gate parameter passing and regenerate plots ([`3683d6a`](https://github.com/jrmoynihan/flow/commit/3683d6a9248108834f3be9c6ae7a844d96953b7a))
    - Complete synthetic data generation with debris scenario ([`5996edc`](https://github.com/jrmoynihan/flow/commit/5996edc676f6a606fcd48e2ffc8ed3131f08ce0b))
    - Resolve type inference issues in Gaussian distributions ([`a1894b8`](https://github.com/jrmoynihan/flow/commit/a1894b8dd78f86970311dde59e0f863a685ef4ec))
    - Resolve rand version mismatch and complete example ([`28677b4`](https://github.com/jrmoynihan/flow/commit/28677b4de7abaccf198f2a278a38c46a2364f193))
    - Complete migration to Gaussian distributions for synthetic data ([`547c2ae`](https://github.com/jrmoynihan/flow/commit/547c2ae09f0f263314de70750b8c8e01b4fd4661))
    - Add WithDebris scenario and complete Gaussian distribution migration ([`c8d5ab0`](https://github.com/jrmoynihan/flow/commit/c8d5ab0e62038fc07f17ffb89e9748c3a159007e))
    - Improve synthetic data generation with realistic distributions ([`c0ba8e7`](https://github.com/jrmoynihan/flow/commit/c0ba8e72f6866bda5d9eec40a6f089ccc7c35107))
    - Implement peak biasing and negative event extraction ([`3409773`](https://github.com/jrmoynihan/flow/commit/340977390c10a31fdf7694ac9325147f406c5b72))
    - Correct flow-plots API usage in visualization example ([`6596ed9`](https://github.com/jrmoynihan/flow/commit/6596ed9f6d7916684d38ae65f9284ae7a40a937f))
    - Add visualization example for synthetic test data ([`6a65bd7`](https://github.com/jrmoynihan/flow/commit/6a65bd7077b2a12670c3766248b08447e92ea8b5))
    - Add synthetic FCS file generation for automated gating tests ([`bea47e8`](https://github.com/jrmoynihan/flow/commit/bea47e8ee97b86a3120b8097d0fdbe6bc9fce133))
    - Resolve ndarray version mismatch for clustering ([`38013b2`](https://github.com/jrmoynihan/flow/commit/38013b28d81af8510a1065745d203bd5e2057518))
    - Implement clustering-based scatter gating ([`43a00f6`](https://github.com/jrmoynihan/flow/commit/43a00f6f0e4043d9b973eb8c9ae2c18ff64b780d))
    - Add 2D KDE for improved density contours ([`c89944b`](https://github.com/jrmoynihan/flow/commit/c89944be9c68a1f688dfb5ee333c7562b28f90b1))
    - Restore Gate import in doublets module ([`ec337c2`](https://github.com/jrmoynihan/flow/commit/ec337c29858cd506aec01548d0e8431fa6eec9f3))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
    - Add implementation status document ([`69d65c9`](https://github.com/jrmoynihan/flow/commit/69d65c959a392f16431cc98beae9c361ccfed10a))
    - Add comprehensive documentation for flow-utils and research notes ([`6f6d0f5`](https://github.com/jrmoynihan/flow/commit/6f6d0f59369453e3f0018b37f1377b204b023223))
    - Fix final borrow checker error ([`7b87699`](https://github.com/jrmoynihan/flow/commit/7b87699eb278bd7b7d37076aaaa730ff99fc3c53))
    - Fix borrow checker error in comparison module ([`383b476`](https://github.com/jrmoynihan/flow/commit/383b476374a707447e655b1b0c0a298e91fd2cc3))
    - Fix GateHierarchy API usage ([`385c0be`](https://github.com/jrmoynihan/flow/commit/385c0be364793819279fb9a50f38eb29bbceeab3))
    - Fix ellipse geometry creation and error handling ([`d33d361`](https://github.com/jrmoynihan/flow/commit/d33d3616c82ffc04001363ad3f3a9b7ccef0175f))
    - Fix Fcs API usage in automated gating ([`161b133`](https://github.com/jrmoynihan/flow/commit/161b1334a4a20d5fb0be80aee8134732840e9a6a))
    - Add doublet detection method comparison ([`7b65fbc`](https://github.com/jrmoynihan/flow/commit/7b65fbcc9119762ee4cf64cf129c017ece95ff30))
    - Add README for automated gating module ([`f9eef00`](https://github.com/jrmoynihan/flow/commit/f9eef00689d5c1dbda8bce37ca0d399afae19d46))
    - Add integration tests for automated gating ([`dcf9154`](https://github.com/jrmoynihan/flow/commit/dcf9154b305c79728dd2a9f61e4440b5a15756ea))
    - Add enhanced doublet detection module ([`c998c06`](https://github.com/jrmoynihan/flow/commit/c998c06382ec30a870452083b7366a74ced5830e))
    - Add automated scatter gating module ([`6762e5f`](https://github.com/jrmoynihan/flow/commit/6762e5f0d484be7e8d45363205793a50e46b0eb3))
    - Merge pull request #10 from jrmoynihan/gpu-acceleration ([`69363eb`](https://github.com/jrmoynihan/flow/commit/69363eb3a664b1aa6cd0be9b980ec08fc03b7955))
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
</details>

## 0.1.2 (2026-01-21)

<csr-id-e670a9216137c9a2cedde38f3e21894f280fe516/>
<csr-id-a0b4bcdd64294de3a0e40795c6db838cbcb18ac0/>
<csr-id-4bbcfad61b695c86b6b07173486e5580d8b9eeae/>

### New Features

<csr-id-7018701b741c6910e89c93e21ca4249120a1eb1b/>
<csr-id-873cfaee2af2b444fe0cd951ed701fade83febc0/>
<csr-id-b6bf3fcdc9e7466c234ecd30b47db57abc52f643/>
<csr-id-d2068182f96d737d1febfca6854ad89d84a6cbfe/>
<csr-id-e8455560b2f20ff0dda711f866f5eaf71d1d323d/>

 - <csr-id-2b7981fa03249f2052e4078ca6b145371c1a661c/> expand error types for new features
   Add comprehensive error types to support new functionality.
   
   - Add HierarchyCycle error for cycle detection

### Refactor

 - <csr-id-e670a9216137c9a2cedde38f3e21894f280fe516/> update module structure after GPU removal
   - Remove gpu module from lib.rs
   - Update all GPU references to use batch_filtering module
   - Simplify conditional compilation by removing GPU feature flags
 - <csr-id-a0b4bcdd64294de3a0e40795c6db838cbcb18ac0/> remove GPU implementation, use CPU-only batch filtering
   - Remove all GPU code (backend, filter, kernels)
   - Create new batch_filtering module with optimized CPU implementation
   - Remove GPU dependencies (burn, cubecl) from Cargo.toml
   - Update types.rs and filtering/mod.rs to use batch_filtering directly
   - Add GPU_PERFORMANCE_FINDINGS.md documenting why GPU was removed
   - GPU was 2-10x slower than CPU at all batch sizes due to overhead
 - <csr-id-4bbcfad61b695c86b6b07173486e5580d8b9eeae/> update library exports and documentation
   Update public API exports to include new features and improve
   documentation.
   
   - Export GateLinks, GateQuery, and new filtering functions
   - Export BooleanOperation and GateBuilder
   - Export gate geometry traits (GateBounds, GateCenter, etc.)
   - Export GatingML import/export functions
   - Add ParameterSet type alias
   - Update documentation examples to be compilable
   - Fix example code formatting

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 12 commits contributed to the release.
 - 3 days passed between releases.
 - 9 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjusting changelogs prior to release of flow-fcs v0.1.5, flow-plots v0.1.3, flow-gates v0.1.2 ([`9c8f44a`](https://github.com/jrmoynihan/flow/commit/9c8f44a6b5908a262825a2daa8b3963fdea99a11))
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
    - Update module structure after GPU removal ([`e670a92`](https://github.com/jrmoynihan/flow/commit/e670a9216137c9a2cedde38f3e21894f280fe516))
    - Remove GPU implementation, use CPU-only batch filtering ([`a0b4bcd`](https://github.com/jrmoynihan/flow/commit/a0b4bcdd64294de3a0e40795c6db838cbcb18ac0))
    - Merge pull request #9 from jrmoynihan/flow-gates ([`d6e993e`](https://github.com/jrmoynihan/flow/commit/d6e993ea8eb206c676aa0a95d01fc8cfaec882c9))
    - Update library exports and documentation ([`4bbcfad`](https://github.com/jrmoynihan/flow/commit/4bbcfad61b695c86b6b07173486e5580d8b9eeae))
    - Expand error types for new features ([`2b7981f`](https://github.com/jrmoynihan/flow/commit/2b7981fa03249f2052e4078ca6b145371c1a661c))
    - Add gate query builder and filtering helpers ([`7018701`](https://github.com/jrmoynihan/flow/commit/7018701b741c6910e89c93e21ca4249120a1eb1b))
    - Enhance gate hierarchy with reparenting and cloning ([`873cfae`](https://github.com/jrmoynihan/flow/commit/873cfaee2af2b444fe0cd951ed701fade83febc0))
    - Add boolean gate support to GatingML import/export ([`b6bf3fc`](https://github.com/jrmoynihan/flow/commit/b6bf3fcdc9e7466c234ecd30b47db57abc52f643))
    - Add boolean gate support ([`d206818`](https://github.com/jrmoynihan/flow/commit/d2068182f96d737d1febfca6854ad89d84a6cbfe))
    - Add gate linking system ([`e845556`](https://github.com/jrmoynihan/flow/commit/e8455560b2f20ff0dda711f866f5eaf71d1d323d))
</details>

## 0.1.1 (2026-01-18)

<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-fbbef211ba3c7f4dffa75ea7d56f65e249e72384/>

### Chore

 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Other

 - <csr-id-fbbef211ba3c7f4dffa75ea7d56f65e249e72384/> :arrow_up: bump quick-xml version

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release over the course of 4 calendar days.
 - 4 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.2, flow-gates v0.1.1 ([`2c36741`](https://github.com/jrmoynihan/flow/commit/2c367411265c8385e88b2653e278bd1e2d1d2198))
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - :arrow_up: bump quick-xml version ([`fbbef21`](https://github.com/jrmoynihan/flow/commit/fbbef211ba3c7f4dffa75ea7d56f65e249e72384))
</details>

## 0.1.0 (2026-01-14)

<csr-id-5f63c2c2f02f2abaa1862153743e1923c71d8d86/>
<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-f64872e441add42bc9d19280d4411df628ff853e/>
<csr-id-d14cd7b41828c45396709071065c98d9bda5c967/>
<csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/>
<csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/>

### Chore

 - <csr-id-5f63c2c2f02f2abaa1862153743e1923c71d8d86/> add GatingML 2.0 Specification PDF for reference
 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

### Chore

 - <csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/> Update CHANGELOG for upcoming release
   - Documented version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization.

### Chore

 - <csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### New Features

 - <csr-id-7a1233b4426b5c7b5849666b28b75a3bee19e8c7/> introduce flow-gates library for flow cytometry data analysis
   - Added core functionality for creating and managing gates, including Polygon, Rectangle, and Ellipse geometries.

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.

### Test

 - <csr-id-d14cd7b41828c45396709071065c98d9bda5c967/> :white_check_mark: Add GatingML compliance test files
   Added readme, test text, fcs, and xml files to parse and validate

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 13 commits contributed to the release over the course of 7 calendar days.
 - 7 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-gates v0.1.0 ([`869b4c2`](https://github.com/jrmoynihan/flow/commit/869b4c2f123ef2ebbf5a464b4453a71f35a6ad06))
    - Remove extra keywords ([`fbf2fa6`](https://github.com/jrmoynihan/flow/commit/fbf2fa66dbee6a2d6c188a8b9a7f933ca3d2929b))
    - Release flow-plots v0.1.1, flow-gates v0.1.0 ([`b5be6ba`](https://github.com/jrmoynihan/flow/commit/b5be6ba4e2093a8b0e972bd44265fa51b8c6be13))
    - Update CHANGELOG for upcoming release ([`f0f0ab2`](https://github.com/jrmoynihan/flow/commit/f0f0ab21b68eb1a28903957bae137f326b5a082b))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`621d3ad`](https://github.com/jrmoynihan/flow/commit/621d3aded59ff51f953c6acdb75027c4541a8b97))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - :truck: Rnamed folders without the `flow-` prefix. ([`f64872e`](https://github.com/jrmoynihan/flow/commit/f64872e441add42bc9d19280d4411df628ff853e))
    - Introduce flow-gates library for flow cytometry data analysis ([`7a1233b`](https://github.com/jrmoynihan/flow/commit/7a1233b4426b5c7b5849666b28b75a3bee19e8c7))
    - Add GatingML 2.0 Specification PDF for reference ([`5f63c2c`](https://github.com/jrmoynihan/flow/commit/5f63c2c2f02f2abaa1862153743e1923c71d8d86))
    - :white_check_mark: Add GatingML compliance test files ([`d14cd7b`](https://github.com/jrmoynihan/flow/commit/d14cd7b41828c45396709071065c98d9bda5c967))
    - Reorganize workspace into separate crates ([`fd12ce3`](https://github.com/jrmoynihan/flow/commit/fd12ce3ff00c02e75c9ea84848adb58b32c4d66f))
</details>

