# Plate_001 end-to-end TRU-OLS throughput (before vs after perf work)

**Date:** 2026-04-15  
**Host:** macOS, Apple M1 Max (`aarch64-apple-darwin`)  
**Rust:** `rustc 1.93.1` (see `throughput_rust.json` in each output dir for full `rustc -vV`)  
**Environment:** `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` for both runs (recorded in JSON).

## Inputs (local paths)


| Role                     | Path                                                                                                                        |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------- |
| Stained                  | `~/Downloads/Plate_001/Stained_Samples/Donor 12_H1 Full Stain_D12_Plate_001_2025_09_25_15_41_27.fcs`           |
| Unstained                | `~/Downloads/Plate_001/Reference Group/Reference Group_A1 Unstained (Cells)_Plate_001_2025_09_25_14_58_02.fcs` |
| Controls (mixing matrix) | `~/Downloads/Plate_001/Reference Group/`                                                                       |
| Output (before)          | `~/Downloads/Plate_001/Unmixed/e2e_bench_before_head`                                                          |
| Output (after)           | `~/Downloads/Plate_001/Unmixed/e2e_bench_after_perf`                                                           |


**Panel (extracted):** 200000 stained events × 67 detectors; 87665 unstained events; 13 endmembers (see `throughput_rust.json`).

## Harness

Example: `tru-ols-cli/examples/e2e_plate_throughput.rs` (same phases and `throughput_rust.json` / `throughput_report.md` as `compare_with_julia`, plus a `e2e_legacy` feature).

- **Before (pre-optimization `flow-tru-ols`):**  
`cargo build -p tru-ols --release --no-default-features --features e2e_legacy --example e2e_plate_throughput`  
then run the release binary with four positional arguments (stained, unstained, controls dir, output dir).  
Code path: `TruOls::new` after explicit preprocess (`e2e_legacy`; `TruOls::new` recomputes cutoffs/nonspecific internally as in older call patterns).
- **After (perf-plan `flow-tru-ols`):**  
`cargo build -p tru-ols --release --example e2e_plate_throughput`  
Code path: `TruOls::from_preprocessed` + `unmix` (no duplicate cutoffs in `new`).

**CLI crate notes:** `tru-ols-cli` defaults include `cli_benchmark`; use `--no-default-features` for the legacy build so `src/benchmark.rs` (which depends on newer `flow-tru-ols` APIs) is not compiled. Optional `blas` feature forwards to `flow-tru-ols/blas` if you build with `--features blas`.

### Cutoff percentile sort (before run)

A first attempt against **unmodified** `flow-tru-ols` **HEAD** `CutoffCalculator` **panicked** on this plate (`partial_cmp` on NaN during sorting). The **before** numbers below were taken after a **one-line** change to use NaN-safe `total_cmp` for sorting in `CutoffCalculator` (same idea as the optimized tree). Without that, the legacy run does not complete on these files. The optimized **after** run did not require a local patch beyond the restored perf branch.

## Wall-clock and throughput (Rust algorithm only, excluding FCS I/O)

Printed by the example; also stored in `throughput_rust.json`.


| Phase                                   | Before (`e2e_legacy` + `TruOls::new`) | After (`from_preprocessed`) |
| --------------------------------------- | ------------------------------------- | --------------------------- |
| preprocess (cutoffs + nonspecific)      | 1.201561 s                            | 1.990013 s                  |
| TruOls build                            | 1.166450 s (`TruOls::new`)            | ~0 s (`from_preprocessed`)  |
| **unmix**                               | **0.235810 s**                        | **2.153771 s**              |
| preprocess + build + unmix (core)       | 2.603820 s                            | 4.143785 s                  |
| **Throughput unmix (stained events/s)** | **848142**                            | **92860**                   |
| Throughput core (stained / core)        | 76810                                 | 48265                       |
| Rayon threads (after run)               | 10                                    | 10                          |


**Full JSON:**  

- Before: `e2e_bench_before_head/throughput_rust.json`  
- After: `e2e_bench_after_perf/throughput_rust.json`

### Interpretation

### Erratum (parallel vs sequential `unmix`)

The **before** (`e2e_legacy` / older `flow-tru-ols`) build used **Rayon** for `unmix` whenever stained events **> 10 000** and did **not** honor `FLOW_TRU_OLS_FORCE_SEQUENTIAL` for `unmix`. The **after** build honors that variable and **forced sequential** `unmix` when it was set.

So the **~0.24 s vs ~2.15 s unmix** lines were **not** comparable: they correspond to **parallel** vs **sequential** scheduling, not purely to algorithm changes.

**Unmix-only harness** (CSV reuse, no I/O): `flow-tru-ols` example `**unmix_profile_csv`** (see `docs/PROFILING.md`). On the same export directory, **without** `FLOW_TRU_OLS_FORCE_SEQUENTIAL`, `unmix` wall time is **~0.23 s** (parallel), in line with the legacy e2e number; **with** `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1`, expect **~1.4–2.2 s** (sequential) depending on build.

### Solver note

Inner least-squares in `unmix` uses faer; attempting **Gram/Cholesky** before **QR** when the Gram path often fails can add redundant work. Current `flow-tru-ols` uses a **QR-first** path for those inner solves (see `unmixing.rs` / `solve_least_squares_faer_in_place`), which improves the **sequential** `unmix` case.

- **Core wall time** in the original table mixed scheduling modes; use `unmix_profile_csv` + consistent env for A/B.
- **Process memory (`/usr/bin/time -l` on the release binary)** is in the same ballpark; peak footprint is slightly lower **after**.

## Memory (`/usr/bin/time -l`, macOS)


| Metric                            | Before               | After                |
| --------------------------------- | -------------------- | -------------------- |
| Real wall (s)                     | 293.49               | 280.82               |
| Maximum resident set size (bytes) | 773193728 (~737 MiB) | 769736704 (~734 MiB) |
| Peak memory footprint (bytes)     | 701828352 (~669 MiB) | 630590656 (~601 MiB) |


**Note:** Real time includes mixing-matrix generation from controls, FCS read, CSV export, and Julia script generation—not only TRU-OLS.

## Reproduce

```bash
# After (current perf branch)
cd /path/to/flow-crates
export FLOW_TRU_OLS_FORCE_SEQUENTIAL=1
cargo build -p tru-ols --release --example e2e_plate_throughput
/usr/bin/time -l ./target/release/examples/e2e_plate_throughput \
  "$STAINED" "$UNSTAINED" "$CONTROLS_DIR" "$OUT_AFTER"
```

```bash
# Before: stash or checkout older flow-tru-ols, then:
cargo build -p tru-ols --release --no-default-features --features e2e_legacy --example e2e_plate_throughput
/usr/bin/time -l ./target/release/examples/e2e_plate_throughput \
  "$STAINED" "$UNSTAINED" "$CONTROLS_DIR" "$OUT_BEFORE"
```

