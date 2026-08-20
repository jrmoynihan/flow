# Comparing flow-autospectral joint unmix with AutoSpectralRcpp

This document describes the dual-process harness in
`examples/compare_with_r.rs` + `examples/compare_with_r.R`.

The algorithm under test is AutoSpectral v1.6 `pipeline = "joint"`
(Burton *et al.*, *bioRxiv* [2025.10.27.684855](https://doi.org/10.1101/2025.10.27.684855)):
per-cell AF matching-pursuit, then fluorophore-variant coordinate descent.

## Prerequisites

- Rust toolchain (`cargo`)
- Optional: `Rscript` on `PATH` with `AutoSpectralRcpp` and `flowCore`
- Use `--rust-only` when R packages are missing

## Fairness checklist

1. **QC-core (headline):** events already in RAM; time only `unmix_autospectral_joint` / `unmix.autospectral.rcpp(..., pipeline="joint")`.
2. **e2e (secondary):** `Fcs::open` + joint unmix + `write_fcs_file` (Rust) vs `read.FCS` + joint (R).
3. **io_only:** open + materialize the fluorescence matrix (sanity that I/O is not the QC-core clock).
4. Shared fixture per case under `out/cases/<id>/prepared.fcs` plus `spectra.csv`, `af.csv`, `variants/`. Detector columns are named `FL1-A`… to match synthetic FCS (AutoSpectralRcpp requires identical colnames on `raw.data` and `spectra`).
5. Record n, d, F, K_AF, `n_variants_mean`, machine, `rustc`, R / AutoSpectral / AutoSpectralRcpp versions, **matched 1-thread and N-thread** rows.
6. Prefer **release** builds (`cargo run --release …`).
7. README may quote **QC-core rust/R ratios** from the sample report below. Do not paste Criterion `Melem/s` as a vs-R claim, and do not mix absolute events/s from different sessions into one table without a date footnote.
8. Real FCS: `--fcs` CLI only. Reports use anonymous `real_01`, … ids — never embed source paths.
9. If the R sidecar skips, the harness prints the JSON `reason` (package load failure, column mismatch, missing joint entry point).

## Commands

Smoke (synthetic 10k × 20 detectors × 8 fluors):

```bash
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- \
  --smoke --out /tmp/autospectral-r-smoke
# optional: cap both Rayon and R OpenMP
#   --threads 8
```

`--smoke` pins d=20, F=8, and (unless you pass `--events`) n=10k. It does **not** override an explicit `--events` list.

Matched-thread scaling (same panel, larger n — do **not** combine with `--smoke` if you want the default 10k pin):

```bash
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- \
  --events 10000,50000,200000 --warmup 1 --reps 2 --out /tmp/autospectral-r-scale
```

Rust-only:

```bash
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- \
  --smoke --rust-only --e2e --out /tmp/autospectral-r-smoke
```

MATRIX-sized grid (50k/200k events, d=40/64, F=42). Cases with `d < F+1` are skipped (underdetermined mixing matrix).

```bash
FLOW_AUTOSPECTRAL_BENCH_MATRIX=1 cargo run -p flow-autospectral --example compare_with_r \
  --features fcs --release -- --warmup 1 --reps 2 --out target/autospectral-r-matrix
```

1M events, same F=8 d=20 panel (include a small-n control in the same process so session drift is visible):

```bash
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- \
  --events 10000,1000000 --warmup 1 --reps 2 --out target/autospectral-r-1m
```

1M wide panel (`d ≥ F+1`; F=42 sets K_AF=100):

```bash
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- \
  --events 50000,1000000 --detectors 64 --fluors 42 --warmup 1 --reps 2 \
  --out target/autospectral-r-1m-wide
```

`FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1` / `--large` only **appends** n=1M to the current events list. Combined with `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1` that also forces F=42 and d∈{40,64} — skip `d=40` (`d < F+1`).

Artifacts under `--out`:

- `cases/<id>/prepared.fcs`, `spectra.csv`, `af.csv`, `variants/`, `meta.json`
- `throughput_merged.json`
- `throughput_report.md`

## Interpreting QC-core vs e2e

- **QC-core:** algorithm cost only (headline for vs-R claims). Throughput = events/s.
- **e2e:** includes FCS read and write; I/O often dominates at small n.
- **Agreement (smoke):** cosine of unmixed fluor columns and AF-index match rate when R is present (`unmixed_r.csv`, n ≤ 50k). R `AF Index` is treated as 1-based unless a 0 is present.
- **Quality (not speed):** MAD of the collinear partner on true-A cells. Those cells are generated from a peak-bumped A variant (not `A+λB`, which lies in the master span so OLS already fits). Joint should commit the variant and drop partner MAD vs OLS.
- **Threads:** each case times **1 thread and N threads** for both Rust (Rayon) and AutoSpectralRcpp (OpenMP `threads=N`). N defaults to `available_parallelism` (`--threads`). R sidecar sets `OMP_NUM_THREADS=N` and BLAS libraries to 1 so the event OpenMP pool does not nest with BLAS. `FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1` skips the Rust multi-thread row only.

## Sample report (Apple M5 / aarch64)

d=20, F=8, K_AF=8 unless noted. AutoSpectralRcpp 1.2.1, AutoSpectral 1.7.1, R 4.6.0, rustc 1.95.0, 18 hardware threads. Warmup 1, reps 2. Sidecar elapsed via `Sys.time()`.

- **2026-08-19** abs rates: `target/autospectral-r-scale/`, `target/autospectral-r-matrix/` (10k–200k).
- **2026-08-20** 1M ratios: `target/autospectral-r-1m-recheck/`, `target/autospectral-r-1m-wide/`. A 10k (F=8) / 50k (F=42) control in the same process was ~2× slower in events/s than 19 Aug (endpoint-security CPU load, ~2.5 cores). rust/R **ratios** still matched the 19 Aug 1-thread pair. Do not splice 20 Aug absolute events/s into the 19 Aug columns.

Agreement (n≤50k, determined panels): mean cosine 1.000, AF-index match 1.000. Variant commit on true-A cells is 1.000 on the F=8 panel.

### QC-core (headline)

Events/s, matched threads. Ratio is rust/R.

| n | rust 1 | R 1 | rust/R 1 | rust 18 | R 18 | rust/R 18 |
|---|--------|-----|----------|---------|------|-----------|
| 10k | **1.74M** | 0.79M | **2.2×** | 7.26M | 2.55M | 2.8× |
| 50k | **1.72M** | 0.91M | **1.9×** | 5.26M | 4.96M | 1.1× |
| 200k | **1.72M** | 0.89M | **1.9×** | 13.2M | 3.78M | 3.5× |
| 1M | — | — | **~2.1×** | — | — | ~2.0× |

1-thread QC-core is the publishable pair: Rust ~2× AutoSpectralRcpp on this panel from 10k through 1M. 50k / 18-thread is two reps and almost a tie — treat **200k** as the stabler mt row. 1M abs events/s from 20 Aug are omitted (see session note above); the 1-thread ratio in that run was 1.08M / 0.514M rust/R.

Wide panel (F=42, K_AF=100, d=64; `d ≥ F+1`):

| n | rust 1 | R 1 | rust/R 1 | rust 18 | R 18 | rust/R 18 |
|---|--------|-----|----------|---------|------|-----------|
| 50k | 0.166M | 0.075M | **2.2×** | 1.97M | 0.322M | 6.1× |
| 200k | 0.157M | 0.073M | **2.2×** | 1.73M | 0.325M | 5.3× |
| 1M | — | — | **~2.2×** | — | — | ~3.5× |

1M wide 1-thread in the 20 Aug run was 85.2k / 38.6k rust/R (ratio 2.2×). The 50k control in that process was 111k / 49.9k (still 2.2×; abs ~⅓ below 19 Aug).

`n50k_d40_F42` is underdetermined (40 detectors, 43 mixing columns). Joint still runs, but R agreement was cosine 0.57 / AF-index 0.004 — not a vs-R quality case. The harness now skips `d < F+1`.

### e2e (secondary)

FCS open + joint + write (Rust) vs `read.FCS` + joint (R). I/O dominates; 10k Rust 18-thread e2e was *slower* than 1-thread (write-bound). 1-thread e2e:

| n | rust 1 | R 1 |
|---|--------|-----|
| 10k | 0.54M | 0.46M |
| 50k | 0.86M | 0.41M |
| 200k | 0.97M | 0.56M |

### Collinear-pair spillover MAD (true-A cells)

| n | d | F | OLS MAD | joint MAD | variant commit |
|---|---|---|---------|-----------|----------------|
| 10k | 20 | 8 | 6.52 | 0.10 | 1.000 |
| 50k | 20 | 8 | 6.52 | 0.10 | 1.000 |
| 200k | 20 | 8 | 5.44 | 0.15 | 1.000 |
| 50k | 64 | 42 | 12.48 | 0.34 | 0.373 |
| 200k | 64 | 42 | 12.48 | 0.34 | 0.375 |
| 1M | 20 | 8 | 6.52 | 0.10 | 1.000 |
| 1M | 64 | 42 | 12.48 | 0.34 | 0.375 |

## Criterion

```bash
cargo bench -p flow-autospectral --bench joint_unmix
FLOW_AUTOSPECTRAL_BENCH_MATRIX=1 cargo bench -p flow-autospectral --bench joint_unmix
```

Default: 10k × 20 × 8 fluors. Keep `joint_af_only` as the AF-matching-pursuit control inside this bench; do not mix `match_matrix` IDs into `joint_unmix`. MATRIX snapshot: 10k×20 joint **1.66 ms (6.03 M/s)**; A/B keep remains 2.10 ms vs `joint-alloc-pre`. Criterion `--events` is not a bench flag — use `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1`.

## Why the Rust path is faster

The single-thread ~2× versus AutoSpectralRcpp is not a “Rust is faster” claim. The same operational changes apply in Rcpp/C++ or Julia. [`PERF_AB.md`](PERF_AB.md) states each one as a problem, a solution, and how the per-event work differs, with definitions of factorization, Gram matrices, and related terms.

- **Decompose once:** `JointPrecomp::build` and `OlsFactor` factor mixing matrices that depend only on the panel. Each event then applies those factors instead of running QR again.
- **Rank with residual columns:** `score_af` and `unmix_fluor_event` test AF and variant candidates with dot products against precomputed residual columns, not a new least-squares solve per candidate.
- **One workspace per worker:** `EventScratch` in [`joint.rs`](../src/joint.rs) is a set of arrays allocated once and overwritten. `ensure_cell_s` copies the mixing matrix only when `try_commit_variant` first needs a writable copy (`cell_` there means “this event’s copy,” not a matrix entry).
- **Contiguous columns:** `gemv` adds one emitter column at a time so the CPU loads adjacent detector samples.
- **Independent events:** after that shared work, events write only into their own output rows, so the loop can run in parallel with BLAS held to one thread (the R sidecar already sets `OMP_NUM_THREADS` that way).

On the 10,000-event × 20-detector × 8-fluorophore joint A/B, reusing the workspace, copying the mixing matrix only when a variant is accepted, writing into pre-sized tables, and using a column-wise matrix–vector product cut median time 56% (`joint-alloc-pre`). The AF-only control dropped 49%, so most of that 56% is fewer per-event allocations, not a different variant-search algorithm.
