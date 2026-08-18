# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Changed

- CLI 0.3.0: depends on `flow-fcs` 0.5.x and `peacoqc-rs` 0.3.x; removal-reason visualization options from the library.

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

### New Features

 - <csr-id-907a77cbe2adf5c7654c9a86e3a6bae15c344026/> removal reason visualization and export from mask
   - Add RemovalReason and per-bin reason data to PeacoQCResult
   - Color-code removal reasons (IT, MAD, Consecutive) in QC time/channel plots
   - Add export_csv_boolean_from_mask and export_csv_numeric_from_mask
   - Improve legend legibility; optional reason-specific colors in QCPlotConfig
   - peacoqc-cli: tempfile dep, README and CLI updates for new options

### Bug Fixes

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

### Refactor

 - <csr-id-f179bb3fb82f79a0f64667577b7ebd5340ba479c/> migrate to structured tracing and improve GPU diagnostics
   Replaces all eprintln! debug output with tracing::debug! for
   production-ready structured logging across QC modules (mad, debug,
   export, plots). Enhances trajectory and bin-level diagnostics.
   
   Improves GPU batch operation error handling and context lifecycle.
   Expands regression test coverage and adds synthetic drift test.
   CLI: refactored argument parsing and improved output formatting.
   Python bindings: updated for API changes.

### Style

 - <csr-id-503255e45d9ac34d127fb4a3a1124e229ab937e6/> simplify let-chains in input and output path handling

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 15 commits contributed to the release over the course of 153 calendar days.
 - 8 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Refresh crate READMEs and agent guidelines ([`3c48e73`](https://github.com/jrmoynihan/flow/commit/3c48e73e751a7852b0e07239540448e6ee35a0cf))
    - Merge branch 'peacoqc-rust-vs-r-throughput' into main ([`536825d`](https://github.com/jrmoynihan/flow/commit/536825ddf5b52f910778eae86b91dfee4f9c319e))
    - Release flow-fcs v0.5.1 ([`ddc9d09`](https://github.com/jrmoynihan/flow/commit/ddc9d094ee0a7390f16daf3e9f23c47eda39e637))
    - Release peacoqc-rs v0.3.2 ([`20d0a9c`](https://github.com/jrmoynihan/flow/commit/20d0a9cb60e2c4ed4efed9f3d55dbc4d9bd54abb))
    - Default test commands to cargo-nextest (flow-crates-9xv) ([`af6097f`](https://github.com/jrmoynihan/flow/commit/af6097fbd09f00657eaf82ea8367fffd3ee72baf))
    - Apply $PnR masking, fix bit-packed stride, add $NEXTDATA traversal ([`6986541`](https://github.com/jrmoynihan/flow/commit/6986541e936967c566b3c6caca42c9e0cbf5678f))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Simplify let-chains in input and output path handling ([`503255e`](https://github.com/jrmoynihan/flow/commit/503255e45d9ac34d127fb4a3a1124e229ab937e6))
    - Release flow-fcs v0.4.1 ([`597f21b`](https://github.com/jrmoynihan/flow/commit/597f21bef7ea787437071685fc3cce9d2269270f))
    - Release peacoqc-rs v0.3.1 ([`5b5e6fd`](https://github.com/jrmoynihan/flow/commit/5b5e6fdb3e1dee945cae3014f84de5baca57f43d))
    - Release peacoqc-rs v0.3.0, safety bump 2 crates ([`604a94e`](https://github.com/jrmoynihan/flow/commit/604a94e13464acb60582292768ce3f97598ea55e))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Migrate to structured tracing and improve GPU diagnostics ([`f179bb3`](https://github.com/jrmoynihan/flow/commit/f179bb3fb82f79a0f64667577b7ebd5340ba479c))
    - Removal reason visualization and export from mask ([`907a77c`](https://github.com/jrmoynihan/flow/commit/907a77cbe2adf5c7654c9a86e3a6bae15c344026))
</details>

## 0.2.4 (2026-02-27)

<csr-id-f78ada41f7d265ee456ff9e08c4299139f4683d4/>

### New Features

 - <csr-id-7aad630ce2e5e9ebcda2273cef86f22777efc05b/> plot aesthetics, GPU fix, CLI plot options
   - peacoqc-rs: Plot improvements (issue #16): larger fonts, axis/legend/title
   config; grid and MAD dashed lines; legend semi-transparent background;
   spline blue/MAD green; scatter alpha. Add caption_font_size and
   font_family to QCPlotConfig. Fix GPU tensor shape (Burn Float is f32).
   - peacoqc-cli: Apply --hide-spline-mad and --show-bin-boundaries; add
   --plot-width, --plot-height, --plot-title-size, --plot-axis-size,
   --plot-tick-size, --plot-legend-size, --plot-font; document in README.

### Documentation

 - <csr-id-aa358ec2cb81c6ba271290606b34b3238977c3b9/> update README to include a citation for PeacoQC algorithm

### Chore

 - <csr-id-f78ada41f7d265ee456ff9e08c4299139f4683d4/> bump version to 0.2.3 in Cargo.toml and Cargo.lock

### Added

- **Plot options**: `--plot-width`, `--plot-height`, `--plot-title-size`, `--plot-axis-size`, `--plot-tick-size`, `--plot-legend-size`, and `--plot-font` to control QC plot dimensions and typography.
- **Plot visibility**: `--hide-spline-mad` and `--show-bin-boundaries` to toggle spline/MAD curves and bin boundaries in QC plots.
- README documentation for the new plot flags.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 11 calendar days.
 - 11 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.2.4, peacoqc-cli v0.2.4 ([`cea03b0`](https://github.com/jrmoynihan/flow/commit/cea03b013c10ae71a83a00fdf96dbea205afc961))
    - Plot aesthetics, GPU fix, CLI plot options ([`7aad630`](https://github.com/jrmoynihan/flow/commit/7aad630ce2e5e9ebcda2273cef86f22777efc05b))
    - Bump version to 0.2.3 in Cargo.toml and Cargo.lock ([`f78ada4`](https://github.com/jrmoynihan/flow/commit/f78ada41f7d265ee456ff9e08c4299139f4683d4))
    - Release peacoqc-rs v0.2.3 ([`7600d54`](https://github.com/jrmoynihan/flow/commit/7600d54b5bdbedb4c5e8189265a6b5f20a1970cf))
    - Release flow-fcs v0.2.2, flow-plots v0.2.2, peacoqc-rs v0.2.2 ([`cb7b98e`](https://github.com/jrmoynihan/flow/commit/cb7b98ecbc3d012df79c2e70bd2aad2f89d9c303))
    - Release peacoqc-rs v0.2.1, flow-utils v0.1.1, flow-gates v0.2.2, flow-tru-ols v0.1.0 ([`c3d9774`](https://github.com/jrmoynihan/flow/commit/c3d97742b3f83d01f1b831eea6eb662a2511adb9))
    - Update README to include a citation for PeacoQC algorithm ([`aa358ec`](https://github.com/jrmoynihan/flow/commit/aa358ec2cb81c6ba271290606b34b3238977c3b9))
    - Merge pull request #14 from jrmoynihan/gpu-acceleration ([`01edbec`](https://github.com/jrmoynihan/flow/commit/01edbecfc222685a8e052eb26b001d3fae4dfe13))
</details>

## 0.2.0 (2026-02-15)

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

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release over the course of 24 calendar days.
 - 27 days passed between releases.
 - 3 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`b758024`](https://github.com/jrmoynihan/flow/commit/b7580243ad5dfba389d80f55d9d2b0a0adf26348))
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
    - Merge pull request #10 from jrmoynihan/gpu-acceleration ([`69363eb`](https://github.com/jrmoynihan/flow/commit/69363eb3a664b1aa6cd0be9b980ec08fc03b7955))
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
</details>

## 0.1.2 (2026-01-18)

<csr-id-2c1548bfe1da6db1af12ecb1a753cdcfca862045/>
<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>
<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-29aae1db8364d6a04f55bc62edb0680eeeb58e4e/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>

### Chore

 - <csr-id-2c1548bfe1da6db1af12ecb1a753cdcfca862045/> standardize formatting in Cargo.toml
   - Remove unnecessary whitespace around key-value pairs in the peacoqc-cli section.
   - Ensure consistent version formatting for dependencies.
 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.
 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-29aae1db8364d6a04f55bc62edb0680eeeb58e4e/> update peacoqc-rs dependency version in Cargo.toml
   - Revert peacoqc-rs version from ^0.1.2 to ^0.1.1 until release is made
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Documentation

 - <csr-id-76d800d1b9a5b40c8f4628b46074320bd9e51630/> Update README files for peacoqc-cli and peacoqc-rs to include license information, enhance function documentation, and improve contribution guidelines.
 - <csr-id-8b64eee4f91acabc724c60ae1f3d380fcac4af92/> Update peacoqc-cli README.md to enhance clarity and provide links to `peacoqc-rs` and `flow-fcs` libraries, and improve attribution formatting for the original authors.

### New Features

<csr-id-d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f/>
<csr-id-de047ef593ff1b1061b5843e439c3990f142bc2b/>

 - <csr-id-12c86f21a3f572f3403cb1d187fd43ac673c38e3/> improve flag interface and implement FCS writing
   - Replace --remove-margins/--no-remove-margins with --keep-margins flag

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 18 commits contributed to the release over the course of 3 calendar days.
 - 4 days passed between releases.
 - 11 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.3, peacoqc-cli v0.1.2 ([`572393e`](https://github.com/jrmoynihan/flow/commit/572393e435342b438c398b2c51b680af50da1b68))
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Standardize formatting in Cargo.toml ([`2c1548b`](https://github.com/jrmoynihan/flow/commit/2c1548bfe1da6db1af12ecb1a753cdcfca862045))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`f08823c`](https://github.com/jrmoynihan/flow/commit/f08823cabcae5223efe4250471dd75ea7fcaa936))
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update peacoqc-rs dependency version in Cargo.toml ([`29aae1d`](https://github.com/jrmoynihan/flow/commit/29aae1db8364d6a04f55bc62edb0680eeeb58e4e))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Improve flag interface and implement FCS writing ([`12c86f2`](https://github.com/jrmoynihan/flow/commit/12c86f21a3f572f3403cb1d187fd43ac673c38e3))
    - Add QC plot generation functionality ([`d262a61`](https://github.com/jrmoynihan/flow/commit/d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f))
    - Add export flags for QC results ([`de047ef`](https://github.com/jrmoynihan/flow/commit/de047ef593ff1b1061b5843e439c3990f142bc2b))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - Update README files for peacoqc-cli and peacoqc-rs to include license information, enhance function documentation, and improve contribution guidelines. ([`76d800d`](https://github.com/jrmoynihan/flow/commit/76d800d1b9a5b40c8f4628b46074320bd9e51630))
    - Update peacoqc-cli README.md to enhance clarity and provide links to `peacoqc-rs` and `flow-fcs` libraries, and improve attribution formatting for the original authors. ([`8b64eee`](https://github.com/jrmoynihan/flow/commit/8b64eee4f91acabc724c60ae1f3d380fcac4af92))
</details>

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
    - Release peacoqc-cli v0.1.1 ([`07c0db8`](https://github.com/jrmoynihan/flow/commit/07c0db8ea865807f08e0b7c965a065d3e05078d5))
    - Release peacoqc-rs v0.1.1 ([`947c991`](https://github.com/jrmoynihan/flow/commit/947c991bff21beb7b7d60f1f637279bd86b9ab66))
    - :hammer: Add cargo scripts for testing and release management for each crate ([`a6a4ff7`](https://github.com/jrmoynihan/flow/commit/a6a4ff733ae38acaec36d3327f4952d6fded3c0f))
    - Adjusting changelogs prior to release of peacoqc-rs v0.1.1 ([`a84b627`](https://github.com/jrmoynihan/flow/commit/a84b6271257f16432464aff091fb9c34eadf16f0))
</details>

## 0.1.0 (2026-01-14)

<csr-id-32d70dc9741a8b5867d784f9e0cfa5f17929cb8c/>
<csr-id-94407a5e6cd66bb753c89c0fbb24c4e026056f35/>
<csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/>

### Chore

 - <csr-id-32d70dc9741a8b5867d784f9e0cfa5f17929cb8c/> update dependency paths in Cargo.toml for peacoqc-cli
   - Changed flow-fcs and peacoqc-rs dependencies to use relative paths and specified versions for better clarity and organization.
 - <csr-id-94407a5e6cd66bb753c89c0fbb24c4e026056f35/> update flow-fcs dependency version in Cargo.toml
   - Changed flow-fcs dependency version from 0.1.0 to 0.1.1 to ensure compatibility with recent updates.

### Chore

 - <csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### New Features

<csr-id-2fb16ca7aab98434c34bd7773295fb6d0b17a8ad/>
<csr-id-395b447bc519ac50168a68589732aace860afc8d/>

 - <csr-id-4a17968a01a3fe08707df80d015650cd3abbb722/> add interactive plot generation to CLI
   - Add --plots and --plot-dir CLI options for plot generation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 13 commits contributed to the release over the course of 7 calendar days.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-cli v0.1.0 ([`ee76027`](https://github.com/jrmoynihan/flow/commit/ee760271b139b2a192d7065d08063fe5ecf0ffbf))
    - Release peacoqc-rs v0.1.0 ([`ae4bc91`](https://github.com/jrmoynihan/flow/commit/ae4bc91414dde199edfdac0965c9df44e9036f2f))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`3292c46`](https://github.com/jrmoynihan/flow/commit/3292c46b282d226aa48c2a83bc17c50896bb8341))
    - Merge pull request #7 from jrmoynihan/feat/cli-plot-generation ([`e0cd286`](https://github.com/jrmoynihan/flow/commit/e0cd286f9faa58d264eb27cc6dc6b57958389f78))
    - Add interactive plot generation to CLI ([`4a17968`](https://github.com/jrmoynihan/flow/commit/4a17968a01a3fe08707df80d015650cd3abbb722))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Implement CLI tool with parallel processing ([`2fb16ca`](https://github.com/jrmoynihan/flow/commit/2fb16ca7aab98434c34bd7773295fb6d0b17a8ad))
    - Update dependency paths in Cargo.toml for peacoqc-cli ([`32d70dc`](https://github.com/jrmoynihan/flow/commit/32d70dc9741a8b5867d784f9e0cfa5f17929cb8c))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - Update flow-fcs dependency version in Cargo.toml ([`94407a5`](https://github.com/jrmoynihan/flow/commit/94407a5e6cd66bb753c89c0fbb24c4e026056f35))
    - Add peacoqc-cli for flow cytometry quality control ([`395b447`](https://github.com/jrmoynihan/flow/commit/395b447bc519ac50168a68589732aace860afc8d))
</details>

