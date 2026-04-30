# Plate-scale TRU-OLS: benchmark snapshot, performance ceilings, and hypotheses

**Run date:** 2026-04-15  
**Machine:** Apple M1 Max, 8P+8E cores (sysctl), 32 GiB RAM  
**Rust:** `rustc 1.93.1` (aarch64-apple-darwin)  
**Dataset:** Exported CSVs under  
`~/Downloads/Plate_001/Unmixed/e2e_bench_after_perf`  
(200 000 stained events × 67 detectors × 13 endmembers; truncation **mean inner iterations = 1** on this export.)

---

## 1. Benchmarks (this run)

### A. `unmix` only — real plate matrices (`unmix_profile_csv`, `--iter 5`, `--af-index 12`)


| Mode                                               | Wall (5× full `unmix`) | Per single `unmix` (÷5) | Reported agg. throughput¹                 |
| -------------------------------------------------- | ---------------------- | ----------------------- | ----------------------------------------- |
| **Parallel** (default)                             | 1.14–1.26 s (3 runs)   | **~0.23–0.25 s**        | ~0.79–0.88 M stained events/s (aggregate) |
| **Sequential** (`FLOW_TRU_OLS_FORCE_SEQUENTIAL=1`) | ~6.83–7.11 s (3 runs)  | **~1.37–1.42 s**        | ~0.14–0.15 M stained events/s (aggregate) |


¹ Example prints `(n_events × iter) / total_secs` — for 5 passes over 200 k rows, divide by 5 for **per-pass** stained events/s (~**0.83–0.88 M/s** at the parallel best).

### B. Synthetic same shape — `profile_hot_path` (200 k × 67 × 13, `--iter 1`)


| Mode                                                                            | Wall time (one batch / one unmix) | Throughput (stained events/s)² |
| ------------------------------------------------------------------------------- | --------------------------------- | ------------------------------ |
| `normal_equations` (batch OLS, one Gram + Cholesky + stacked RHS)               | **58.8 ms**                       | ~3.4 M                         |
| `ols_qr` (QR-style batch path in `run_ols`)                                     | **182 ms**                        | ~1.1 M                         |
| `tru_ols_unmix` (synthetic; `**TruOls::new` + one `unmix`** in the timed block) | **397 ms**                        | ~0.5 M (not unmix-only)        |


² For NE/QR, “events/s” is **all rows solved in one call**, not TRU-OLS. The synthetic `tru_ols_unmix` line **includes preprocessing inside `new`**; compare **unmix-only** work to `**unmix_profile_csv`** on the plate export (~**0.23 s** parallel), not to this 397 ms line alone.

---

## 2. Hypothesis (a) — Upper bound of performance on this machine

TRU-OLS is **not** the same problem as batch OLS: the active endmember set changes per event, so you **cannot** factor **one** Gram matrix for the whole file (except when optional **mask caching** hits).

**Layers of “ceiling”:**


| Layer                               | What it means                                                                        | Order of magnitude (this box, 200 k × 67 × 13)                                                         |
| ----------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| **A. Batch OLS (wrong model)**      | `run_ols_normal_equations`: one M^\top M, one Cholesky, then cheap solves per row    | **~59 ms** total → **~3.4 M stained events/s** — **not reachable** for full TRU-OLS semantics          |
| **B. Batch OLS QR (`run_ols`)**     | Single large least-squares structure per batch                                       | **~182 ms** → **~1.1 M/s** — still **not** TRU-OLS (no truncation)                                     |
| **C. TRU-OLS parallel, real plate** | One inner truncation round on average; faer QR + column bookkeeping per event, Rayon | **~0.23 s** → **~0.87 M stained events/s** — **empirical “good” line** for current code on this export |
| **D. TRU-OLS sequential**           | Same math, no Rayon in `unmix`                                                       | **~1.37 s** → **~0.15 M/s** — floor for debugging / A/B vs BLAS threading                              |


**Interpretation:** The **meaningful** upper bound for **TRU-OLS on this panel** is not (A) or (B), because truncation forbids a single global factorization. A **theoretical** ceiling in the same ballpark as (C) might be approached if every event shared the same mask (then caching could approach batch-like behavior) or if inner work shrank to a **minimal** solve path with maximal hardware utilization. **Rough headroom:** parallel plate `**unmix` only** (~~**0.23 s**) is **not** directly comparable to the 397 ms **synthetic** line (that includes `TruOls::new`). Versus **batch** paths on the same shape: **~~0.23 s** (parallel TRU-OLS unmix, real plate) vs **182 ms** (batch QR) vs **59 ms** (batch NE) — the gap to (A)/(B) is **algorithmic** (per-event truncation + different factorization pattern), not a single missing micro-optimization.

---

## 3. Hypothesis (b) — Changes that could move toward peak


| Direction                                              | Rationale                                                                   | Risk / cost                                              |
| ------------------------------------------------------ | --------------------------------------------------------------------------- | -------------------------------------------------------- |
| `**unmix-cache` + repeated masks**                     | If many events share the same active set, reuse Gram/Cholesky for that mask | Memory; must validate hit rate on real panels            |
| **System BLAS (`blas` feature) for inner solves**      | Faster GEMM/QR at moderate k on large n_\mathrm{det}                        | Threading vs Rayon; set `OMP_NUM_THREADS=1` when nesting |
| **Larger batched normal equations when mask is fixed** | If algorithm can batch events with identical active columns                 | Algorithm change; not always applicable                  |
| **GPU RHS / blocks (`cubecl` experiments)**            | Existing OLS GPU path pattern; extend to repeated block solves              | Adapter, transfer overhead; only wins at large n         |
| **Fewer truncation rounds**                            | Lower mean inner iterations (cutoffs, panel design)                         | Science / gating quality                                 |
| **Profile-guided micro-opts**                          | Reduce copies in `UnmixScratch`, fuse passes, `target-cpu=native`           | Diminishing returns once parallel QR dominates           |


**Not** a silver bullet: moving from **~0.23 s** toward **~0.06 s** (same order as batch NE) would require **changing the problem** (e.g. fixed full panel OLS) or **massive** mask reuse — not a small refactor.

---

## 4. Reproduce

```bash
cargo build -p flow-tru-ols --release --no-default-features --examples unmix_profile_csv profile_hot_path

# Plate unmix only (parallel vs sequential)
./target/release/examples/unmix_profile_csv \
  "~/Downloads/Plate_001/Unmixed/e2e_bench_after_perf" --af-index 12 --iter 5

FLOW_TRU_OLS_FORCE_SEQUENTIAL=1 ./target/release/examples/unmix_profile_csv \
  "~/Downloads/Plate_001/Unmixed/e2e_bench_after_perf" --af-index 12 --iter 5

# Synthetic ceilings (same n_det × n_em; not TRU-OLS semantics)
./target/release/examples/profile_hot_path normal_equations --n-events 200000 --n-det 67 --n-em 13 --iter 1
./target/release/examples/profile_hot_path ols_qr --n-events 200000 --n-det 67 --n-em 13 --iter 1
./target/release/examples/profile_hot_path tru_ols_unmix --n-events 200000 --n-det 67 --n-em 13 --iter 1
```

---

## 5. Takeaway

- On this plate export, **parallel `unmix` is already in the high hundreds of thousands of stained events/s** with **one** inner iteration on average — a **strong** result relative to sequential and relative to synthetic TRU-OLS on random data.
- **Batch OLS NE at ~3.4 M/s** is an **upper bound for a different problem**; use it to calibrate expectations, not as a TRU-OLS SLA.
- Further gains are likely **incremental** (cache, BLAS, GPU blocks) unless the **science** allows fewer truncations or more repeated masks.

