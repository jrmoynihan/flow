# flow-crates Agent Guidelines

## Release Workflow

For pull requests, merge branches, and crate releases, use the **rust-release-workflow** skill. Key steps:

1. **Dry-run first**: `cargo smart-release <crate-name> --update-crates-index` or `cargo run -p <pkg> dry-release`
2. **Polish changelogs**: `cargo changelog --write <crate-name>` then edit by hand
3. **Update READMEs** with new versions
4. **Execute** only after review: add `--execute` to the smart-release command

Pre-1.0 version policy: minor for large features, patch for all other changes (no strict semver for breaking changes). See `.cursor/rules/release-versioning.mdc`.

---

## SvelteKit Overview

You are able to use the Svelte MCP server, where you have access to comprehensive Svelte 5 and SvelteKit documentation. Here's how to use the available tools effectively:

## Available MCP Tools

### 1. list-sections

Use this FIRST to discover all available documentation sections. Returns a structured list with titles, use_cases, and paths.
When asked about Svelte or SvelteKit topics, ALWAYS use this tool at the start of the chat to find relevant sections.

### 2. get-documentation

Retrieves full documentation content for specific sections. Accepts single or multiple sections.
After calling the list-sections tool, you MUST analyze the returned documentation sections (especially the use_cases field) and then use the get-documentation tool to fetch ALL documentation sections that are relevant for the user's task.

### 3. svelte-autofixer

Analyzes Svelte code and returns issues and suggestions.
You MUST use this tool whenever writing Svelte code before sending it to the user. Keep calling it until no issues or suggestions are returned.

### 4. playground-link

Generates a Svelte Playground link with the provided code.
After completing the code, ask the user if they want a playground link. Only call this tool after user confirmation and NEVER if code was written to files in their project.

## Cursor Cloud specific instructions

### Project structure

- **Rust workspace** (9 crates): `flow-fcs`, `flow-plots`, `flow-gates`, `flow-utils`, `flow-tru-ols`, `peacoqc-rs`, `peacoqc-cli`, `peacoqc-py`, `tru-ols-cli`
- **SvelteKit docs site**: root `package.json`, uses bun, Svelte 5, Tailwind CSS v4, mdsvex

### Rust crates

- Build/test commands: `cargo check --workspace`, `cargo test --workspace --lib --bins`, `cargo clippy --workspace`
- The TRU-OLS CLI crate path is `tru-ols-cli/`; Cargo package name is `tru-ols` (`cargo run -p tru-ols`, `cargo check -p tru-ols`).
- `peacoqc-rs` tests that use GPU/KDE (via WGPU/Vulkan) will fail in headless VMs without a GPU adapter. Use `--no-default-features --features flow-fcs` when running peacoqc-rs examples to skip GPU backend.
- System deps `libfontconfig1-dev` and `pkg-config` are required by the `plotters` crate.

### SvelteKit docs site

- Install: `bun install`
- Dev server: `bun run dev` (port 5173)
- Lint: `bun run lint` (prettier + eslint)
- Type-check: `bun run check` (svelte-check)
- **Known issue**: `svelte-kit sync` fails because both `src/routes/+page.svelte` and `src/routes/+page.svx` exist. This prevents SSR and `bun run check` from working until one is removed. The `prepare` script suppresses this error with `|| echo ''`.

### Running tests

- `cargo test -p flow-fcs --lib` — 74 tests, all pass
- `cargo test -p flow-plots --lib` — 2 tests, all pass
- `cargo test -p flow-gates --lib` — 64 tests, all pass
- `cargo test -p flow-tru-ols --lib` — 13 tests, all pass
- `cargo test -p peacoqc-rs --lib` — some tests fail without GPU; non-GPU tests pass
- Demo example: `cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example demo_qc_plot`

**Optional QC debug artifacts (local / CI when enabled)**

- `FLOW_GATES_QC_TEST_PLOTS=1` — `flow-gates` integration test `qc_plot_smoke_env_gated` writes a short summary under `CARGO_TARGET_TMPDIR` (or temp).
- `TRU_OLS_QC_TEST_PLOTS=1` — `tru-ols` unit test `pipeline_debug_plot_bundle_env_gated` writes `scatter_post_debris.png` under the target temp dir (PeacoQC overview export may still be skipped if metadata/plot prerequisites are missing).

---

## Learned User Preferences

- When writing or editing this codebase, do not describe implementations as derived from or inspired by a named third-party tool or publication in code comments, API documentation, commit messages, or user-facing copy; use neutral technical descriptions of behavior instead.
- For TRU-OLS performance and benchmarking, prioritize large event counts (on the order of tens of thousands to roughly one million events per file); very small event sizes are not the main optimization target.
- When comparing TRU-OLS to ordinary least squares, treat quality metrics (for example rSD, coefficient of variation, RMSE) as the primary comparison; speed or throughput is secondary.
- For TRU-OLS performance work (profiling, throughput, GPU experiments), prioritize before-and-after comparisons on TRU-OLS itself; head-to-head unmix speed versus plain OLS is useful but secondary.

---

## Learned Workspace Facts

- Older guidance to exclude the TRU-OLS CLI from workspace builds is obsolete; `tru-ols-cli` is a normal workspace member and the Cargo package name is `tru-ols`.
- `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` disables Rayon for independent-event loops and for `TruOls::unmix` (useful for A/B profiling vs parallel builds). Independent-event paths use Rayon when there are more than 256 events; `unmix` uses Rayon above 10_000 events. When benchmarking outer Rayon together with a multithreaded BLAS backend, set `OMP_NUM_THREADS=1` (and vendor-specific BLAS thread limits) unless nested parallelism is intentional, to reduce oversubscription.
- For `tru-ols unmix` when `--stained` is a directory, `TRU_OLS_BATCH_SHARED_FACTOR_CACHE` selects a shared mask-factor cache across stained files (default) or a fresh cache per file (`0`/`false`/`no`) for A/B timing; pair with `OMP_NUM_THREADS=1` when benchmarking alongside multithreaded BLAS.
- TRU-OLS **vs** plain OLS **quality** (spread, fit, USE, dimensionality) is evaluated with `run_comparison` / `ComparisonReport`, `comparison_report_markdown`, or `cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report`; Criterion benches measure **throughput**, not that quality comparison.
- TRU-OLS **profiling** and A/B notes live in `tru-ols/docs/PROFILING.md`; end-to-end hot-path sampling uses the `profile_hot_path` example mode `tru_ols_unmix`. On macOS, **samply** is documented when `cargo flamegraph` trace collapse fails.
- Optional GPU paths in `flow-tru-ols` are behind the `cubecl` Cargo feature (WGPU). GPU-related benches and `profile_hot_path` modes such as `normal_equations_gpu` need `--no-default-features --features cubecl` and may fail without a suitable GPU adapter (similar to other WGPU usage).
- `tru-ols/docs/comparison-with-julia.md` and `tru-ols-cli/examples/compare_with_julia.rs` validate numerical agreement (CSVs) and can emit wall-clock throughput sidecars (`throughput_rust.json`, `throughput_julia.json`, `throughput_report.md`, `julia_blas_info.txt`) for same-input runs; there is no fixed CI regression for Rust-vs-Julia timing—document machine, BLAS, and thread-related env when publishing numbers. For Rust wall times comparable to typical optimized Julia runs, use `cargo run --release --example compare_with_julia`; dev/debug builds are not representative of hot-path performance.
- `tru-ols/docs/julia-and-blas-on-macos.md` supplements the comparison doc with Julia REPL basics (e.g. `using LinearAlgebra` before `BLAS.get_config()`), BLAS/JLL inspection, and native/LLVM inspection notes useful when comparing Julia and Rust builds on Apple Silicon.
- `TruOls::unmix` runs a variable inner loop (repeated least-squares solves on shrinking column subsets until cutoffs stabilize), not a fixed two-pass workflow; comparing throughput to single-factorization OLS paths only makes sense when solver paths and per-event iteration counts are aligned.
- The `compare_with_julia` example takes four positional paths only (stained FCS, unstained FCS, reference controls directory, output directory); it does not parse `--controls_dir` or `--output_dir`, and passing those tokens as arguments is treated as literal paths and produces confusing errors.
- In `flow-tru-ols`, the optional Cargo `blas` feature pins `ndarray` to 0.17.x in `tru-ols/Cargo.toml` so it matches `ndarray-linalg`; other workspace crates may use `ndarray` 0.16—do not assume one shared version when fixing trait or dependency errors for `--features flow-fcs,blas`.
