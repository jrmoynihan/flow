# TRU-OLS profiling and A/B controls

## Goals (what we measure and why)

| Priority | What | How |
|----------|------|-----|
| **Primary** | **TRU-OLS wall time** (`TruOls::unmix`) before vs after optimizations, at **50k–1M** events. | Criterion groups `unmixing`, `parameter_sweep_unmix`, `parallel_vs_sequential`; [`profile_hot_path`](../examples/profile_hot_path.rs) mode **`tru_ols_unmix`** + sampling profilers. Compare against a **saved Criterion baseline** or a **git revision** (`--save-baseline` / `--baseline`, or check out an old commit and re-run the same command). |
| **Secondary** | **Speed of plain OLS kernels** on a **fixed** mixing matrix (no truncation). | `run_ols` vs `run_ols_normal_equations` vs optional GPU RHS helper — see [Glossary](#glossary-qr-vs-normal-equations-vs-gpu-rhs--cpu). Benches: `ols_method_compare`, `ols_vs_normal_equations`. |
| **Quality (TRU-OLS vs OLS)** | **Spread and fit**, not raw speed. | [`run_comparison`](../src/benchmark.rs) → [`ComparisonReport`](../src/metrics.rs); format with [`comparison_report_markdown`](../src/benchmark.rs) or run **`cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report`**. Sample tables: [`quality_comparison_report.md`](quality_comparison_report.md). Metrics: **rSD**, **CV**, **R²**, **residual** summaries, **USE**, dimensionality. |

Typical FCS-scale workloads are **tens of thousands to ~one million events** per file. **Small-`n` timings** are mainly regression smoke, not the main optimization target.

### `flow-tru-ols` Cargo features (profiling relevance)

| Feature | Effect |
|---------|--------|
| **`large-panels`** | Allows more than [`MAX_ENDMEMBERS_DEFAULT`](../src/lib.rs) (**128**) endmembers; default builds use `u128` active-set keys only. |
| **`unmix-cache`** | Adds a bounded [`quick_cache`](https://docs.rs/quick_cache) store of Gram/Cholesky factors keyed by active-endmember bitmask (default capacity **512** masks). When enabled, inner solves prefer this path over per-iteration faer QR / BLAS least squares. |
| **`blas`** | Uses ndarray-linalg for [`solve_linear_system`](../src/preprocessing.rs) where applicable; with outer Rayon, set **`OMP_NUM_THREADS=1`** unless nested parallelism is intended. |

## TRU-OLS `unmix`: before/after optimization (throughput + memory)

Use the same machine, CPU governor, and dependency set when comparing commits. Record **`rustc -Vv`**, **`RUSTFLAGS`** (e.g. `-C target-cpu=native`), **`--features`**, and **`RAYON_NUM_THREADS` / `OMP_NUM_THREADS`** (set BLAS threads to `1` when benchmarking outer Rayon + `blas`).

### Throughput (wall time and events/sec)

1. **Criterion baselines** (recommended for regression tracking):

   ```bash
   # Once on the pre-change revision:
   cargo bench -p flow-tru-ols --no-default-features --bench unmixing_benchmark -- --save-baseline tru-ols-pre

   # After changes, on the same host:
   cargo bench -p flow-tru-ols --no-default-features --bench unmixing_benchmark -- --baseline tru-ols-pre
   ```

   Focus groups: **`unmixing`** (2k / 50k / 100k events), **`parameter_sweep_unmix`** if present, and (optional long run) set **`FLOW_TRU_OLS_BENCH_1M=1`** for 250k / 1M-style rows. Criterion HTML under **`target/criterion/report/index.html`** shows time and **throughput**; the baseline diff highlights regressions/improvements.

2. **End-to-end hot path** (complements Criterion):

   ```bash
   cargo build -p flow-tru-ols --no-default-features --release --example profile_hot_path
   ./target/release/examples/profile_hot_path tru_ols_unmix --n-events 100000 --iter 40
   ```

   Divide **`n-events` × `iter`** by wall time for approximate **events/sec** (rough; includes `TruOls::new` once per process—see mode docs). For **`unmix`-only** emphasis, align with the example’s documented split or time **`unmix`** in a small harness.

### Memory (allocator churn and peak RSS)

 Allocator-focused optimizations need **both** “how much heap” and “how often we allocate”:

| Signal | Tool / method |
|--------|----------------|
| **Peak resident set** | POSIX: **`/usr/bin/time -l`** (macOS) or **`time -v`** / **`/usr/bin/time -v`** (Linux **`Maximum resident set size`**). Wrap the **release** `profile_hot_path tru_ols_unmix` (or a one-shot `unmix` driver) with the same `n_events` × `iter` as throughput runs. |
| **Allocations / temporary heap** | **macOS Instruments → Allocations** (or **Leaks** for growth). **Linux:** **heaptrack** / **massif** / **DHAT** (Valgrind) on the same binary + args. |
| **Cache (`unmix-cache`)** | With **`--features unmix-cache`**, use [`TruOls::unmix_factor_cache_hits_misses`](../src/unmixing.rs) after a run to read **`quick_cache`** hit/miss counts and judge reuse across events. |

Save a short **before/after** note in the PR or changelog: machine model, baseline name, representative **events/sec** (or Criterion ratio vs baseline), **max RSS** from `time -l`, and one sentence on allocator profiles if you sampled them.

**Julia side:** REPL commands (`using LinearAlgebra`, `BLAS.get_config()`), `@code_native` / `@code_llvm`, and macOS OpenBLAS vs the Cargo **`blas`** feature are documented in [julia-and-blas-on-macos.md](julia-and-blas-on-macos.md).

## Assembly / LLVM inspection (SIMD and codegen)

To compare **what the compiler emitted** for a hot path vs **calls into OpenBLAS**:

| Script | Purpose |
|--------|---------|
| [`tru-ols/scripts/inspect_codegen_julia.jl`](../scripts/inspect_codegen_julia.jl) | Julia: BLAS config, pure-loop `@code_*`, and `A \\ b` wrapper (expect **BLAS call**, not vector loops in Julia IR). |
| [`tru-ols/scripts/inspect_codegen_rust.sh`](../scripts/inspect_codegen_rust.sh) | Rust: `cargo asm` for [`solve_linear_system`](../src/preprocessing.rs) with **`flow-fcs`** (faer) vs **`flow-fcs,blas`** (ndarray-linalg + system OpenBLAS). Requires **`cargo install cargo-show-asm`**. |

Default **`RUSTFLAGS=-C target-cpu=native`** in the shell script matches tuned SIMD to the host CPU. The **faer** path shows large inlined regions (QR/LU, `faer`, `dyn_stack`); the **blas** path typically shows **calls** to LAPACK/OpenBLAS. Fallback: the script prints a **`cargo rustc --emit=llvm-ir,asm`** one-liner; artifacts land under `target/**/deps/` with mangled names.

## Glossary: QR vs normal equations vs GPU RHS + CPU

All three solve the **same** least-squares problem for a **fixed** mixing matrix **M** (detectors × endmembers) and many observation rows (events × detectors): for each event, find abundances **x** minimizing \(\| M x - b \|\) where **b** is that event’s detector vector (overdetermined case: more detectors than endmembers). Implementations differ only in **how** they factor and reuse work across events.

| Name in docs / benches | What it is in code | Work breakdown |
|------------------------|-------------------|----------------|
| **QR (per-event OLS)** | [`run_ols`](../src/benchmark.rs) → [`solve_linear_system`](../src/preprocessing.rs): **one QR (or BLAS SVD)** per event on the **full** **M**. | Many small factorizations; parallelized with Rayon over events. Stable; no assumption that \(M^\top M\) is well-conditioned. |
| **Normal equations (CPU)** | [`run_ols_normal_equations`](../src/batched_ols.rs): form **Gram** \(M^\top M\) once, **Cholesky** once, then for all events one **GEMM** `B M` for stacked RHS and **triangular solves** (parallel over events when `n_events` is large). | Asymptotically cheaper when **M** is fixed and well-conditioned; **not** the same code path as TRU-OLS truncation. |
| **GPU RHS + CPU** | [`run_ols_normal_equations_gpu_rhs`](../src/gpu/mod.rs): **`f32` GEMM** on GPU for the RHS block, then **CPU `f64` Cholesky + triangular solves** (parallel like CPU NE). | Same math pattern as CPU NE; extra H2D/D2H/sync; only interesting when GPU arithmetic wins over transfer. |

**TRU-OLS** (`TruOls::unmix`) is **not** any of the rows above: it **drops columns** of **M** per event (truncation loop) and calls **`solve_linear_system`** on **smaller** matrices repeatedly. So OLS benches are **building blocks** and **upper bounds** on “full-matrix” work, not a substitute for profiling **`unmix`**.

## Baseline (pre-batched RHS, historical OLS-only)

One **release** Criterion run on a **10×10** panel **before** batched RHS + parallel Cholesky in `run_ols_normal_equations`. **Machine-specific.**

| n_events | QR (`run_ols`) | CPU normal equations | GPU RHS + CPU |
|--------:|---------------|----------------------|---------------|
| 2 000 | fastest | ~1.2× slower than QR | ~1.6× slower than QR |
| 20 000 | fastest | ~1.2× slower than QR | ~1.0× slower than QR |
| 80 000 | fastest | ~1.7× slower than QR | ~1.4× slower than QR |

## Current measurements (post-optimization, OLS kernels)

**Host (example session):** Apple M1 Max, aarch64-apple-darwin, `rustc 1.93.1` (2026-02-11). Re-run locally; ratios are more portable than milliseconds.

### `ols_method_compare` — `ols_method_matrix`, 10×10, `--no-default-features --features cubecl`

| n_events | QR | CPU normal equations | GPU RHS + CPU | Fastest |
|--------:|---:|---------------------:|--------------:|:--------|
| 50 000 | ~13.25 ms | ~4.22 ms | ~6.2 ms | **CPU NE** (~3× vs QR) |
| 200 000 | ~43 ms | ~14 ms | ~22 ms | **CPU NE** (~3× vs QR) |

### `unmixing_benchmark` — `ols_vs_normal_equations`, 100 000 rows

| `run_ols` (QR) | `run_ols_normal_equations` |
|---------------:|---------------------------:|
| ~25 ms | ~7 ms |

### Hypotheses vs outcomes (OLS kernels)

| Expectation | Outcome (this host) |
|-------------|---------------------|
| Batched RHS + parallel solves fix large-`n` CPU NE. | **Yes** — CPU NE **beats** QR at 50k–200k; ranking **flips** vs the historical table. |
| GPU beats CPU NE at 10×10 here. | **No** — GPU between QR and CPU NE. |
| TRU-OLS still dominated by truncation / column changes. | **Yes** — measure with **`tru_ols_unmix`** and `unmixing` benches, not `ols_method_compare` alone. |

## Quality metrics: TRU-OLS vs OLS

Use **[`run_comparison`](../src/benchmark.rs)** with a [`BenchmarkConfig`](../src/benchmark.rs) (dataset label, endmember names, cutoff, autofluorescence index). The returned **[`ComparisonReport`](../src/metrics.rs)** includes:

- **Spread** ([`SpreadMetrics`](../src/metrics.rs)): **std_dev**, **robust_sd** (MAD-based rSD), **cv** (coefficient of variation), mean/median per endmember for **OLS** vs **TRU-OLS** abundances.
- **Fit** ([`FitMetrics`](../src/metrics.rs)): **R²** per event and mean/median, **residual_abs_mean** / **median** / **max** (per-event detector residuals vs **M**·abundances). There is no separate **RMSE** field; use these residual summaries (or compute RMSE in your own layer from stored residuals) alongside R².
- **USE** ([`UnmixingSpreadingError`](../src/metrics.rs)), **dimensionality** ([`DimensionalityMetrics`](../src/metrics.rs)).

For narrative validation examples elsewhere in this repo, see [`docs/comparison-with-julia.md`](comparison-with-julia.md) and related validation notes.

## Environment variables

| Variable | Effect |
|----------|--------|
| `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` | Disables Rayon for independent-event loops and `TruOls::unmix` scheduling (current `flow-tru-ols`). **Older revisions** (before this flag gated `unmix`) still used Rayon for `unmix` whenever `n_events > 10_000`, so **`unset` env** is required for apples-to-apples throughput vs those commits. |
| `RAYON_NUM_THREADS` | Rayon worker count. |
| `OMP_NUM_THREADS` / BLAS | With `blas`, set inner threads to `1` when using outer Rayon. |
| `FLOW_TRU_OLS_BENCH_1M=1` | Adds 1M OLS grid and 250k unmix benches (long runs). |

### Parallel vs sequential `unmix` (A/B)

```bash
cargo bench -p flow-tru-ols --no-default-features --bench unmixing_benchmark -- parallel_vs_sequential
FLOW_TRU_OLS_FORCE_SEQUENTIAL=1 cargo bench -p flow-tru-ols --no-default-features --bench unmixing_benchmark -- parallel_vs_sequential
```

### `unmix` only (reuse `compare_with_julia` / e2e export CSVs)

To time **only** `TruOls::from_preprocessed` + `unmix` (no FCS I/O, no mixing-matrix generation), point the helper at a directory that already contains **`mixing_matrix.csv`**, **`unstained_data.csv`**, **`stained_data.csv`**, **`rust_cutoffs.csv`**, **`rust_nonspecific.csv`**, and **`endmember_names.csv`** (or pass **`--af-index`**):

```bash
cargo build -p flow-tru-ols --release --no-default-features --example unmix_profile_csv
./target/release/examples/unmix_profile_csv /path/to/e2e_export --af-index 12 --iter 1
# optional: --stats for truncation-iteration stats without a timed unmix loop
```

Sampling profilers (`samply`, Instruments) should attach to this binary so stacks are not dominated by CLI control-plane work.

## `TruOls::new` vs preprocessing (profiling and CLI)

[`TruOls::new`](../src/unmixing.rs) runs [`CutoffCalculator::calculate`](../src/preprocessing.rs) and [`NonspecificObservation::calculate`](../src/preprocessing.rs) every time — the same least-squares-on-unstained work as an explicit preprocess step. Pipelines that already computed cutoffs and nonspecific observation should call [`TruOls::from_preprocessed`](../src/unmixing.rs) instead so wall-time comparisons (e.g. Julia vs Rust) do not **double-count** preprocessing.

To sample only that constructor work with a synthetic panel, use [`profile_hot_path`](../examples/profile_hot_path.rs) mode **`tru_ols_new`** (see the example file for `samply` / `cargo flamegraph` invocations). Mode **`tru_ols_unmix`** includes one `TruOls::new` plus repeated `unmix`.

## Thresholds (crate internals)

- **Independent work** (including normal-equation solves): Rayon when **n_events > 256** (unless forced sequential).
- **`TruOls::unmix`**: Rayon when **n_events > 10_000** (unless forced sequential).

## Criterion benchmarks and HTML plots

```bash
cargo bench -p flow-tru-ols --no-default-features
cargo bench -p flow-tru-ols --no-default-features --features cubecl --bench ols_method_compare
```

**Do not pass `--noplot`** if you want Criterion’s **charts** (time + throughput). After a run, open (from the workspace root):

- **Top-level summary:** `target/criterion/report/index.html`
- **OLS method group:** `target/criterion/ols_method_matrix/report/index.html`

Use **`--quick`** for a faster plot refresh while iterating. Plots are written under `target/criterion/` (regenerated each bench run).

## Flame graphs and sampling profilers

### macOS: `samply` (works when `cargo flamegraph` collapses traces badly)

```bash
cargo install samply
cargo build -p flow-tru-ols --no-default-features --release --example profile_hot_path
samply record -s -n -o tru-ols/benchmark_output/flamegraphs/my_profile.json \
  ./target/release/examples/profile_hot_path normal_equations --n-events 100000 --iter 40
```

- **`-s` / `--save-only`**: write profile and exit (no local server).
- **`-n` / `--no-open`**: do not open a browser.

Open **`my_profile.json`** at [https://profiler.firefox.com/](https://profiler.firefox.com/) (Load from file). Example captures checked in under [`benchmark_output/flamegraphs/`](../benchmark_output/flamegraphs/): `normal_equations_samply.json`, `tru_ols_unmix_samply.json`.

### `cargo flamegraph` (alternative)

```bash
cargo install flamegraph
CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -p flow-tru-ols --no-default-features --example profile_hot_path \
  -o tru-ols/benchmark_output/flamegraphs/normal_equations.svg -- normal_equations --n-events 100000
```

On some macOS versions this **fails** while collapsing Instruments XML; use **samply** or **Instruments → Time Profiler** manually. See [`benchmark_output/README.md`](../benchmark_output/README.md).

### `profile_hot_path` modes

| Mode | Purpose |
|------|---------|
| **`tru_ols_unmix`** | **TRU-OLS** end-to-end (primary). |
| `ols_qr` | Per-event QR OLS (secondary). |
| `normal_equations` | CPU normal equations (secondary). |
| `normal_equations_gpu` | GPU RHS + CPU (needs `--features cubecl`). |

Flags: `--n-events`, `--n-det`, `--n-em`, `--iter`.

### Linux

`RUSTFLAGS="-C force-frame-pointers=yes"` with `perf` / `cargo flamegraph` for clearer stacks.

**Target symbols:** `solve_linear_system`, faer QR / GEMM, `unmix_event`, `run_ols_normal_equations`, GPU launch/readback.

## Truncation statistics

`TruOls::summarize_truncation_iterations` — min / max / mean inner loop counts without changing results.

## GPU / cubeCL

`ols_method_compare` for wall-time A/B; instrument `launch_obs_times_mixing_f32` for H2D/kernel/D2H.

## Reproducibility

Record `rustc -Vv`, CPU/GPU, and dependency versions. Criterion stores history under `target/criterion/`; use `--save-baseline` / `--baseline` for named comparisons.

## Unsafe A/B: parallel unmix scatter (SyncPtr)

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md).
Bench: `OMP_NUM_THREADS=1 cargo bench -p flow-tru-ols --no-default-features --bench unmixing_benchmark -- unmixing/`.

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| unmixing SyncPtr scatter | reverted | 30.438 ms | 30.754 ms | +1.0% (noise) | 100k events | arm64 Apple | 59807616e | 2026-08-02 | Direct disjoint writes into faer `Mat`; solver dominates; gather path kept |

Secondary: 50k +3.2% (noise); 2k regressed (below parallel threshold / noise).
