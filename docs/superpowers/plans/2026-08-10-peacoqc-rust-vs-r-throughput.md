# PeacoQC Rust vs R Throughput Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a reusable dual-process harness that times PeacoQC QC-core (headline) and optional e2e on Rust vs R, with a Rust 1-thread / Rayon / GPU matrix, then publish result tables in `peacoqc-rs` and `peacoqc-py` READMEs.

**Architecture:** A `compare_with_r` example prepares shared FCS fixtures (synthetic grid and/or user paths), times Rust configs in **separate processes** (so `RAYON_NUM_THREADS` and GPU feature selection are fair), spawns `Rscript` on a companion `.R` script for the R baseline, and writes `throughput_rust.json`, `throughput_r.json`, and `throughput_report.md`. Docs explain fairness; a sample report is checked in after a real run; both READMEs get a results table linked to that report.

**Tech Stack:** Rust (`peacoqc-rs` + `flow-fcs`), Criterion-style wall timing via `Instant` (not Criterion for cross-language), `serde_json`, R / `Rscript` + Bioconductor `PeacoQC` + `flowCore`.

**Spec:** [`docs/superpowers/specs/2026-08-10-peacoqc-rust-vs-r-throughput-design.md`](../specs/2026-08-10-peacoqc-rust-vs-r-throughput-design.md)  
**Beads:** `flow-crates-edn`

## Global Constraints

- Headline metric is **QC-core** (PeacoQC only on already prepared data); e2e is secondary.
- Default QC-core excludes margins/doublets; `--include-margins-doublets` is optional.
- Headline Rust vs R row is **CPU + default Rayon**; also report Rust `RAYON_NUM_THREADS=1` and optional GPU.
- Do not commit clinical FCS; real paths are CLI-only.
- Do not invent Nx numbers in READMEs without a linked sample report from an actual harness run.
- Prefer `--no-default-features --features flow-fcs` for CPU runs; add `gpu` only for the GPU row.
- Avoid marketing “inspired by Tool X” wording; neutral technical copy + proper academic attribution to Emmaneel et al. / Saeys Lab R package.
- Tests run with `cargo nextest run`; example smoke is `cargo run -p peacoqc-rs … --example compare_with_r`.
- Commits only when the user asks (or when explicitly executing a commit step under user authority).

---

## File structure

| Path | Responsibility |
|------|----------------|
| `peacoqc-rs/examples/compare_with_r.rs` | CLI driver, synthetic FCS writer, Rust timers, orchestrates R + child Rust configs, writes JSON/MD |
| `peacoqc-rs/examples/compare_with_r.R` | R companion: load prepared FCS, time `PeacoQC::PeacoQC`, emit `throughput_r.json` |
| `peacoqc-rs/docs/comparison-with-r.md` | How to run, fairness checklist, how to refresh README tables |
| `peacoqc-rs/docs/throughput_vs_r_sample.md` | Checked-in sample report (filled after Task 6 run) |
| `peacoqc-rs/README.md` | Performance section: matrix table + link to sample + comparison doc |
| `peacoqc-py/README.md` | Same table (bindings share Rust costs) + link to Rust docs |
| `peacoqc-rs/Cargo.toml` | Optional `[[example]]` `required-features = ["flow-fcs"]` |

Pinned CLI (implement exactly):

```text
compare_with_r
  --out <dir>                 # required for full runs; default target/peacoqc-r-compare/<timestamp>
  --fcs <path>                # repeatable; real FCS (optional)
  --synthetic                 # run default grid (default true if no --fcs)
  --no-synthetic              # skip synthetic grid
  --events 50000,200000,1000000
  --channels 5,15,30          # fluorescence channel counts (Time always added)
  --warmup 1                  # default 1; use 0 for smoke
  --reps 5                    # timed reps; use 1 for smoke
  --include-margins-doublets  # optional sensitivity
  --e2e                       # also time end-to-end window
  --rust-only                 # skip R (debug)
  --gpu                       # attempt GPU Rust row (build/run with gpu feature)
  --config <name>             # internal: single Rust config worker (rust-cpu-1|rust-cpu|rust-gpu)
  --case-dir <dir>            # internal: prepared case directory for worker/R
  --smoke                     # alias: --warmup 0 --reps 1, tiny 10k×5 synthetic only
```

Pinned PeacoQC parameters (both sides, unless overridden later): `determine_good_cells=all`, `mad=6`, `IT_limit=0.6`, `consecutive_bins=5`, `remove_zeros=false`, channels = all `FL*-A` (or auto fluorescence list excluding Time/FSC/SSC).

Pinned QC-core fairness: prepare once into `out/cases/<case_id>/prepared.fcs` (compensated+transformed or synthetic already in analysis space). Timed window = load prepared FCS + run PeacoQC only (load can be included or excluded consistently — **exclude load**: read into memory before `Instant::now()`, then time PeacoQC only). Same file for R and Rust.

---

### Task 1: Synthetic prepared FCS + smoke scaffold

**Files:**
- Create: `peacoqc-rs/examples/compare_with_r.rs` (scaffold)
- Modify: `peacoqc-rs/Cargo.toml` (example required-features)

**Interfaces:**
- Produces: `fn write_synthetic_prepared_fcs(path: &Path, n_events: usize, n_fl_channels: usize) -> anyhow::Result<()>`
- Produces: `struct CaseSpec { id: String, n_events: usize, n_channels: usize, prepared_fcs: PathBuf }`
- Consumes: `flow_fcs::{Fcs, write::write_fcs_file, …}`, `polars`, corpus seed pattern from `fcs/benches/lazy_column_access.rs`

- [ ] **Step 1: Add example required-features**

In `peacoqc-rs/Cargo.toml` append:

```toml
[[example]]
name = "compare_with_r"
required-features = ["flow-fcs"]
```

- [ ] **Step 2: Implement synthetic writer + `--smoke` that only writes a case and exits**

Create `peacoqc-rs/examples/compare_with_r.rs` with:

- Columns: `Time` (0..n as f32), optional `FSC-A`/`SSC-A`, then `FL1-A`…`FL{k}-A`.
- Seed from `flow_fcs::corpus::path("int-10000_events_random.fcs")` like the lazy-column bench; set `$PnN` names to match columns; `$DATATYPE=F`, little-endian.
- CLI via `std::env::args` or a tiny manual parse (avoid new deps): recognize `--smoke` and `--out`.
- On `--smoke`: write `out/cases/smoke_10k_x5/prepared.fcs` (10_000 events, 5 FL channels), print path, exit 0.

```rust
// Sketch — expand fully in the file
fn write_synthetic_prepared_fcs(
    path: &Path,
    n_events: usize,
    n_fl_channels: usize,
) -> anyhow::Result<()> {
    // Time + FSC-A + SSC-A + FL1-A..FLn-A
    // write_fcs_file(...)
    Ok(())
}
```

- [ ] **Step 3: Verify smoke write**

Run:

```bash
mkdir -p /tmp/peacoqc-r-smoke && \
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example compare_with_r -- \
  --smoke --out /tmp/peacoqc-r-smoke
```

Expected: exit 0; `prepared.fcs` exists and `Fcs::open` reports height 10000.

- [ ] **Step 4: Commit** (when user authorizes)

```bash
git add peacoqc-rs/Cargo.toml peacoqc-rs/examples/compare_with_r.rs
git commit -m "feat(peacoqc-rs): scaffold compare_with_r synthetic FCS writer"
```

---

### Task 2: Rust QC-core timing + multi-config workers

**Files:**
- Modify: `peacoqc-rs/examples/compare_with_r.rs`

**Interfaces:**
- Consumes: `peacoqc_rs::{peacoqc, PeacoQCConfig, QCMode}`, `PeacoQCData` via `Fcs`
- Produces: `fn time_qc_core(fcs: &Fcs, config: &PeacoQCConfig, warmup: usize, reps: usize) -> TimingStats`
- Produces: `struct TimingStats { mean_s: f64, std_s: f64, events_per_s: f64, pct_removed: f64, reps: usize }`
- Produces: worker mode `--config rust-cpu-1|rust-cpu|rust-gpu --case-dir <dir>` writing `throughput_rust_<config>.json` into case dir or out root

- [ ] **Step 1: Implement `TimingStats` and QC-core timer**

```rust
fn time_qc_core(
    fcs: &flow_fcs::Fcs,
    config: &PeacoQCConfig,
    warmup: usize,
    reps: usize,
) -> anyhow::Result<TimingStats> {
    for _ in 0..warmup {
        let _ = peacoqc(fcs, config)?;
    }
    let mut times = Vec::with_capacity(reps);
    let mut last_pct = 0.0;
    for _ in 0..reps {
        let t0 = Instant::now();
        let result = peacoqc(fcs, config)?;
        times.push(t0.elapsed().as_secs_f64());
        last_pct = result.percentage_removed; // use actual field name from PeacoQCResult
    }
    // mean, std, events_per_s = n_events as f64 / mean_s
    Ok(/* ... */)
}
```

Confirm `PeacoQCResult` field names in `peacoqc-rs/src/qc/peacoqc.rs` before coding (`percentage_removed` or similar).

- [ ] **Step 2: Worker mode for one config**

When `--config rust-cpu-1` is set **before any rayon work**, set `std::env::set_var("RAYON_NUM_THREADS", "1")` at the top of `main` (must be first). For `rust-cpu`, leave unset (or set to available parallelism). For `rust-gpu`, require `--features gpu` build; if GPU context unavailable, write JSON with `"skipped": true, "reason": "..."`.

Worker: load `case-dir/prepared.fcs`, build `PeacoQCConfig` with FL channels, call `time_qc_core`, write JSON:

```json
{
  "config": "rust-cpu",
  "case_id": "...",
  "phase": "qc_core",
  "mean_s": 0.0,
  "std_s": 0.0,
  "events": 0,
  "channels": 0,
  "events_per_s": 0.0,
  "pct_removed": 0.0,
  "rayon_num_threads": "...",
  "rustc": "...",
  "peacoqc_rs_version": "..."
}
```

- [ ] **Step 3: Orchestrator spawns child processes for each Rust config**

Parent must **not** initialize Rayon before spawning. Use `std::process::Command` with same binary (`std::env::current_exe()`), args `--config … --case-dir … --warmup … --reps …`, and for `rust-cpu-1` set `.env("RAYON_NUM_THREADS", "1")` on the child.

- [ ] **Step 4: Smoke-time one synthetic case (rust-only)**

```bash
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example compare_with_r -- \
  --smoke --rust-only --out /tmp/peacoqc-r-smoke2
```

Expected: JSON files for `rust-cpu-1` and `rust-cpu` with positive `mean_s`.

- [ ] **Step 5: Commit** (when authorized)

```bash
git commit -m "feat(peacoqc-rs): time Rust QC-core matrix in compare_with_r"
```

---

### Task 3: R companion script + driver integration

**Files:**
- Create: `peacoqc-rs/examples/compare_with_r.R`
- Modify: `peacoqc-rs/examples/compare_with_r.rs`

**Interfaces:**
- Produces: R CLI via `commandArgs`: `--case-dir`, `--warmup`, `--reps`, `--channels` (comma-separated), `--out-json`
- Consumes: prepared FCS at `case-dir/prepared.fcs`
- Driver spawns: `Rscript path/to/compare_with_r.R ...`

- [ ] **Step 1: Write `compare_with_r.R`**

```r
#!/usr/bin/env Rscript
suppressPackageStartupMessages({
  library(flowCore)
  library(PeacoQC)
})

# Parse --case-dir, --warmup, --reps, --channels, --out-json (simple argv loop)
# ff <- read.FCS(file.path(case_dir, "prepared.fcs"), transformation = FALSE, truncate_max_range = FALSE)
# channels <- strsplit(channels_arg, ",")[[1]]
# for (i in seq_len(warmup)) PeacoQC::PeacoQC(ff, channels = channels, determine_good_cells = "all",
#   plot = FALSE, save_fcs = FALSE, output_directory = tempdir())
# times <- numeric(reps)
# for (i in seq_len(reps)) {
#   t0 <- proc.time()[["elapsed"]]
#   res <- PeacoQC::PeacoQC(...)
#   times[i] <- proc.time()[["elapsed"]] - t0
# }
# Write throughput_r.json with mean_s, std_s, events, channels, events_per_s,
# pct_removed, R.version.string, PeacoQC + flowCore packageVersion
```

Match PeacoQC args to defaults used in Rust (`mad`, `IT_limit`, etc.). Use `plot=FALSE` / no PNG side effects. Prefer writing any PeacoQC file outputs under `tempdir()` so the case dir stays clean.

- [ ] **Step 2: Driver discovers Rscript and example `.R` path**

Resolve `.R` next to the example source via `CARGO_MANIFEST_DIR`:

```rust
let r_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("examples/compare_with_r.R");
```

If `Rscript` missing or `--rust-only`: skip with clear message. If PeacoQC missing, R script should `quit(status=2)` with stderr hint to install Bioconductor PeacoQC.

- [ ] **Step 3: End-to-end secondary timer (optional `--e2e`)**

For e2e: time (open raw/synthetic unprepared FCS → `preprocess_fcs` or R compensate+estimateLogicle → peacoqc). Synthetic cases can use the same prepared file as “raw” only if documented; prefer a separate `raw.fcs` vs `prepared.fcs` when e2e is on. For the first publishable run, **QC-core is enough**; implement `--e2e` but default off.

- [ ] **Step 4: Integration smoke with R**

```bash
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example compare_with_r -- \
  --smoke --out /tmp/peacoqc-r-with-r
```

Expected: `throughput_r.json` present; `% removed` within a loose band of Rust (document; do not fail the harness on small parity drift).

- [ ] **Step 5: Commit** (when authorized)

```bash
git commit -m "feat(peacoqc-rs): add R companion for PeacoQC throughput comparison"
```

---

### Task 4: Report aggregation + docs

**Files:**
- Modify: `peacoqc-rs/examples/compare_with_r.rs`
- Create: `peacoqc-rs/docs/comparison-with-r.md`
- Create: `peacoqc-rs/docs/throughput_vs_r_sample.md` (placeholder stub)

**Interfaces:**
- Produces: `fn write_report(out: &Path, rust_rows: &[…], r_rows: &[…])` → `throughput_report.md` + merged JSON
- Speedup columns: `vs_r`, `vs_rust_cpu_1`

- [ ] **Step 1: Aggregate JSON into markdown table**

For each case, rows: R, rust-cpu-1, rust-cpu, rust-gpu (if present). Columns: mean_s, events/s, speedup vs R, speedup vs rust-cpu-1. Footer: machine (`sysctl -n machdep.cpu.brand_string` on macOS / `/proc/cpuinfo` model on Linux), OS, rustc, R/PeacoQC/flowCore versions, date, warmup/reps, feature flags.

- [ ] **Step 2: Write `comparison-with-r.md`**

Include: prerequisites, example commands (smoke, synthetic grid, real `--fcs`), fairness checklist from the spec, how to interpret QC-core vs e2e, how to refresh README tables (points at Task 7).

- [ ] **Step 3: Placeholder sample report**

`throughput_vs_r_sample.md` starts with:

```markdown
# Sample: PeacoQC Rust vs R throughput

**Status:** placeholder — replace with output from `compare_with_r` after Task 6.
```

- [ ] **Step 4: Full synthetic dry run (CPU)**

```bash
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs --example compare_with_r -- \
  --out target/peacoqc-r-compare/dry-run \
  --events 50000,200000 \
  --channels 5,15 \
  --warmup 1 --reps 3
```

(Use smaller grid than full 1M×30 for CI-less local dry run; full publishable grid is Task 6.)

Expected: `throughput_report.md` with multiple cases.

- [ ] **Step 5: Commit** (when authorized)

```bash
git commit -m "docs(peacoqc-rs): comparison-with-r guide and report aggregation"
```

---

### Task 5: Margins/doublets flag + GPU row wiring

**Files:**
- Modify: `peacoqc-rs/examples/compare_with_r.rs`
- Modify: `peacoqc-rs/examples/compare_with_r.R` (if R should include RemoveMargins/RemoveDoublets when flag set)
- Modify: `peacoqc-rs/docs/comparison-with-r.md`

- [ ] **Step 1: `--include-margins-doublets`**

When set, timed window on Rust: `remove_margins` + `remove_doublets` + `peacoqc` on prepared data (or document that prepared skips compensation). Mirror in R with the PeacoQC package helpers used in their vignette (`RemoveMargins`, `RemoveDoublets`) before `PeacoQC`, all inside the timed section.

- [ ] **Step 2: `--gpu` child**

Orchestrator runs an additional child built/invoked with GPU features when `--gpu` is passed. Document:

```bash
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs,gpu --example compare_with_r -- \
  --gpu --out …
```

If adapter missing, row skipped, report notes it.

- [ ] **Step 3: Verify flag help / rust-only margins path**

```bash
cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example compare_with_r -- \
  --smoke --rust-only --include-margins-doublets --out /tmp/peacoqc-r-md
```

Expected: completes; report or JSON mentions margins_doublets phase or flag.

- [ ] **Step 4: Commit** (when authorized)

```bash
git commit -m "feat(peacoqc-rs): margins/doublets and GPU rows in R comparison harness"
```

---

### Task 6: Publishable harness run + sample report

**Files:**
- Replace: `peacoqc-rs/docs/throughput_vs_r_sample.md`
- Optionally copy raw JSON under `peacoqc-rs/bench_results/vs_r/` (small JSON only; no FCS)

- [ ] **Step 1: Ask user for real FCS paths** (if available)

If provided, run with `--fcs …` in addition to synthetic. If not, publishable table uses synthetic grid only and states that clearly in the sample footer.

- [ ] **Step 2: Run full publishable matrix**

```bash
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs --example compare_with_r -- \
  --out target/peacoqc-r-compare/publishable \
  --events 50000,200000,1000000 \
  --channels 5,15,30 \
  --warmup 1 --reps 5

# Optional GPU pass (same out dir or merge manually):
cargo run -p peacoqc-rs --release --no-default-features --features flow-fcs,gpu --example compare_with_r -- \
  --gpu --rust-only \
  --out target/peacoqc-r-compare/publishable-gpu \
  --events 50000,200000,1000000 \
  --channels 5,15,30 \
  --warmup 1 --reps 5
```

- [ ] **Step 3: Copy curated markdown into `docs/throughput_vs_r_sample.md`**

Include machine footer, versions, QC-core headline table (R / rust-cpu-1 / rust-cpu / rust-gpu), and a short note that e2e is secondary if measured.

- [ ] **Step 4: `cargo check -p peacoqc-rs --no-default-features --features flow-fcs --examples`**

Expected: success.

- [ ] **Step 5: Commit sample** (when authorized)

```bash
git commit -m "docs(peacoqc-rs): add Rust vs R throughput sample report"
```

---

### Task 7: Update `peacoqc-rs` and `peacoqc-py` READMEs with results tables

**Files:**
- Modify: `peacoqc-rs/README.md` (Performance section ~356–380)
- Modify: `peacoqc-py/README.md` (Performance section ~89–92)

- [ ] **Step 1: Update `peacoqc-rs/README.md` Performance**

Replace qualitative-only claims with:

1. Short intro: QC-core wall time vs Bioconductor PeacoQC; full method in [`docs/comparison-with-r.md`](docs/comparison-with-r.md).
2. **Results table** copied/summarized from `throughput_vs_r_sample.md` (at least one representative size, e.g. 200k×15 and 1M×30, showing R / Rust-1 / Rust-Rayon / GPU if present, mean seconds and speedup vs R).
3. Link to full sample: [`docs/throughput_vs_r_sample.md`](docs/throughput_vs_r_sample.md).
4. Keep Rayon/GPU bullet context but attribute numbers to the sample report (no orphan Nx).
5. Point Criterion benches / `PERF_AB.md` as internal microbenches, distinct from vs-R.

Example table shape:

```markdown
| Case (events×FL) | R mean (s) | Rust 1-thread (s) | Rust Rayon (s) | GPU (s) | Speedup vs R (Rayon) |
|------------------|------------|-------------------|----------------|---------|----------------------|
| 200k × 15        | …          | …                 | …              | …/n/a   | …×                   |
```

Footer one-liner: machine, date, PeacoQC/R versions, warmup/reps.

- [ ] **Step 2: Update `peacoqc-py/README.md` Performance**

Expand the short Performance section to:

- State bindings share `peacoqc-rs` algorithmic cost (Python overhead is conversion-only).
- **Same results table** (or a trimmed copy) with link to `../peacoqc-rs/docs/throughput_vs_r_sample.md` and `../peacoqc-rs/docs/comparison-with-r.md`.
- Do not invent separate PyPI numbers unless a Python-timed row is added later (out of scope).

- [ ] **Step 3: Sanity read**

Confirm both READMEs agree on the same headline speedup and link to the same sample.

- [ ] **Step 4: Commit** (when authorized)

```bash
git add peacoqc-rs/README.md peacoqc-py/README.md
git commit -m "docs: add PeacoQC Rust vs R throughput tables to READMEs"
```

- [ ] **Step 5: Close bead**

```bash
bd close flow-crates-edn --reason="Harness, sample report, and README tables for Rust vs R throughput"
```

---

## Spec coverage checklist

| Spec requirement | Task |
|------------------|------|
| QC-core headline, e2e secondary | 2, 3 |
| Synthetic + real FCS | 1, 6 |
| R / rust-1 / rust-Rayon / GPU matrix | 2, 5, 6 |
| Dual-process + JSON/MD artifacts | 2–4 |
| `--include-margins-doublets` | 5 |
| `--rust-only`, missing GPU skip | 2, 5 |
| `comparison-with-r.md` | 4 |
| Sample report | 6 |
| README tables (`peacoqc-rs` + `peacoqc-py`) | 7 |
| No CI R gate / no clinical FCS in repo | Global |

## Placeholder / consistency self-review

- CLI flag names pinned in File structure; used consistently across tasks.
- JSON field names: `mean_s`, `std_s`, `events_per_s`, `pct_removed`, `config`, `phase`.
- Config ids: `rust-cpu-1`, `rust-cpu`, `rust-gpu`, `r`.
- `PeacoQCResult` field for % removed must be verified against source in Task 2 (use actual name).
