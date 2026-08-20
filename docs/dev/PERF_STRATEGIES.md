# Optimization strategies (retry shortlist)

When writing a new hot path, or when [`PERF_GAP.md`](PERF_GAP.md) is 3× or worse
off napkin math, **try a named tactic that already kept** before inventing a new
one.

Per-crate `PERF_AB.md` / [`UNSAFE_MICROOPT_AB.md`](UNSAFE_MICROOPT_AB.md) are the
dated measurement logs. Promote a tactic here only when it is reusable. A later
keep (≥5% on the primary size) that generalizes gets a new card or an extra
evidence line. One-off keeps stay in the crate log.

Protocol: [`PERF_PGD.md`](PERF_PGD.md). Primitive costs: [`PERF_LATENCIES.md`](PERF_LATENCIES.md).

---

## `hoist-factor-once`

**Targets:** complexity, arithmetic.

**When to try:** Many events share one mixing matrix `M` (or one `M` per AF
candidate). The inner loop currently runs QR / Cholesky / LU **per event**.

**When not to try:** `M` changes every event (TRU-OLS truncation). Then cache by
**mask**, not one global factor (`unmix-cache` in `flow-tru-ols`).

**Evidence:**

- Residual AF match, 10,000 events × 8 detectors × 32 AF spectra: 26.19 ms →
  6.17 ms (**−76%**). [`flow-autospectral/docs/PERF_AB.md`](../../flow-autospectral/docs/PERF_AB.md)
- Shared-matrix OLS: 1.096 ms → 0.505 ms (**−54%**). Same file, `OlsFactor`.

**Apply:** Factor `MᵀM` (or QR of `M`) once per distinct `M`. Per event, apply
the factor to that event’s detector vector. Store factors in a small array
indexed by AF or variant id. Do not rebuild `M` inside the cell loop unless a
column actually changed.

## `score-with-dots`

**Targets:** complexity.

**When to try:** Choosing among library columns (AF spectra, fluorophore
variants) currently builds a new least-squares problem per candidate.

**When not to try:** The candidate set is size 1, or you already have a cheap
score.

**Evidence:** Joint unmix AF scoring in `JointPrecomp::score_af`
([`flow-autospectral/src/joint.rs`](../../flow-autospectral/src/joint.rs)).
Full QR per candidate is `O(d F²)`; two dots per column is `O(d)`.

**Apply:** Unmix fluorophores once, form the detector residual, precompute
residual library columns, score with dots. When a variant is accepted, patch one
row/column of the small Gram rather than refactoring the panel.

## `workspace-per-worker`

**Targets:** alloc.

**When to try:** The per-event function allocates many short `Vec`s or returns
an owned row that is then collected. Symptom: 10k events in the same millisecond
range as `n × allocs × 50 ns`.

**When not to try:** The real bound is zstd, KDE, or a solver (see
[Do not retry](#do-not-retry-without-new-evidence)). Scratch reuse **regressed**
in those cases.

**Evidence:** Joint unmix `EventScratch`, 10,000 events × 20 detectors × 8
fluorophores × 8 AF: 4.464 ms → 2.096 ms (**−56%**); AF-only control **−49%**.
[`flow-autospectral/docs/PERF_AB.md`](../../flow-autospectral/docs/PERF_AB.md)

**Apply:** One struct of arrays per Rayon worker (`thread_local` or
`with_thread_scratch`). `ensure` grows if the panel shape changed; `begin_event`
clears flags. Allocate output tables (`n × F`) once and write row `i` in place.
Do not return a freshly owned dense vector from the per-event function.

## `copy-on-commit`

**Targets:** alloc, memory traffic.

**When to try:** Most events keep the master spectra, but the code clones `M`
(and the Gram) at the top of every event.

**When not to try:** Every event always mutates `M`.

**Evidence:** Joint unmix flags `cell_s_copied` / `ensure_cell_s` — copy from
the shared panel only when `try_commit_variant` first needs a writable copy.
Same A/B as `workspace-per-worker`.

**Apply:** Keep a read-only view while scoring. Allocate the cell-local mixing
matrix on the first accepted variant, then patch in place.

## `match-layout-gemv`

**Targets:** layout, SIMD.

**When to try:** Matrix–vector products index `matrix[(row, col)]` with the
inner loop walking the **non-contiguous** axis (row-inner on a column-major
store). Symptom: 3–10× off the FMA roofline on a stream that should be L1/L2.

**When not to try:** The kernel is already a BLAS/faer `gemv`.

**Evidence:** `gemv` / `col_slice` in
[`flow-autospectral/src/joint.rs`](../../flow-autospectral/src/joint.rs) stream
packed columns. Part of the joint-unmix −56% keep (with scratch reuse).

**Apply:** Store so the inner loop streams contiguous detector samples (faer
column-major `col(j).try_as_col_major()`; R `d × F` and `S %*% alpha`). Keep
both orientations if different products need different axes.

## `parallel-after-precomp`

**Targets:** occupancy.

**When to try:** Shared factors exist, events only write their own output rows,
and `n` is large enough that pool wake is not the whole cost.

**When not to try:** `n` below a few hundred events; or BLAS is already using
all cores inside each event (nested pools).

**Evidence:** Residual match 31.95 ms sequential → 6.17 ms parallel at 32 AF
spectra (**−81%**). Shared-matrix OLS 0.892 ms → 0.505 ms (**−43%**). GPU
scatter-clean at 10,000 events is **not** this tactic (launch-bound).

**Apply:** Build `JointPrecomp` / `OlsFactor` / KNN index on one thread (or a
small team). Then Rayon over events. Default threshold 256. Pin BLAS to one
thread under that pool (`OMP_NUM_THREADS=1`).
`FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1` / `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` for
A/B.

## `bulk-syscall-io`

**Targets:** syscall, IO.

**When to try:** A write/read loop issues one syscall per edge, per event, or
per integer. Symptom: seconds of wall time for tens of megabytes.

**When not to try:** The file is already two `write_all`s and the remaining time
is compression.

**Evidence:** `write_knn_graph` 13.858 s → 17.631 ms (**−99.9%**) at 100,000
events × 60 neighbors. Load 3.5105 s → 8.0616 ms (**−99.8%**).
[`flow-knn/docs/PERF_MATRIX.md`](../../flow-knn/docs/PERF_MATRIX.md)

**Apply:** Pack little-endian payloads in memory, then `write_all` / `read_exact`
a handful of times. Durability (`sync_all`) stays once per file.

## `typed-bulk-load`

**Targets:** syscall, layout.

**When to try:** After bulk IO, values are still parsed one `read_u32` at a time
or copied through an untyped byte buffer.

**When not to try:** Host endian or alignment does not match; then a single
safe convert pass is still better than per-value syscalls.

**Evidence:** Already-bulk KNN load into `Vec<u32>` / `Vec<f32>`: 7.8650 ms →
7.1814 ms (**−6.7%**, kept) at 100,000 events × 60 neighbors.

**Apply:** `read_exact` into a typed buffer; `bytemuck` cast on little-endian.
Building per-event `Vec` neighbor lists can stay; that is not the I/O.

## `gpu-after-amortize`

**Targets:** occupancy (GPU launch).

**When to try:** A CPU kernel is bandwidth- or `n²`-bound at **large n**, and a
GPU backend already exists. Compare at the **primary size**, not a 10k smoke.

**When not to try:** n ≈ 10k d=2 scatter-clean (measured wash). Full PeacoQC e2e
when only the KDE microbench wins. No kernel for the path (joint unmix).

**Evidence:** Scatter-clean `KnnMethod::GpuExact`: 12.58 ms vs 12.36 ms at
10,000 events (skip as default); 300.0 ms → 98.0 ms at 50,000 events (**−67%**,
keep optional). KNN `exact_gpu` wins through ~50k; `ivf_gpu` at 100k.
[`flow-autospectral/docs/PERF_MATRIX.md`](../../flow-autospectral/docs/PERF_MATRIX.md),
[`flow-knn/docs/PERF_MATRIX.md`](../../flow-knn/docs/PERF_MATRIX.md)

**Apply:** Keep CPU as default under the measured crossover. Feature-gate GPU.
Do not quote GPU as the headline when it loses the published sizes.

## `type-width-f32`

**Targets:** SIMD occupancy, DRAM traffic.

**When to try:** A hot path is already on the right complexity (factor once, workspace
reuse) and the remaining tax is IEEE width. `f64` uses half the NEON lanes of `f32`
and twice the bytes. Primary sizes that leave L2 (or sit in DRAM either way) show
this more than tiny occupancy-bound files.

**When not to try:** The published comparison is `double` (R AutoSpectral, Julia).
Small n where a full `f64→f32` cast of the event table dominates the kernel.
Ill-conditioned panels until abundances and AF/variant indices match the `f64` path.

**Evidence:** Joint unmix, Apple M5 Max, 2026-08-20, paired Criterion in one binary.
200,000 events × 64 detectors: 295 ms `f64` → 241 ms `f32` (**−18%**). 10,000 events
× 20 detectors: **+157%** (skip as default). [`flow-autospectral/docs/PERF_AB.md`](../../flow-autospectral/docs/PERF_AB.md)

**Apply:** Keep `f64` as the default. Expose `f32` (or mixed factor-`f64` / apply-`f32`)
behind config. Convert the panel once; do not change the public abundance type.
Quality-check indices and relative abundances before any keep.

---

## Do not retry without new evidence

These **reverted**. A new attempt needs a different size, a profile that shows
the old bound is gone, or a different primitive — not the same patch.

### Counted-loop `get_unchecked`

LLVM already hoists bounds checks in counted loops. BSS split/unsplit, FCS
column extract, exact KNN, and PaCMAP gradient were all **&lt;5% or regressions**.
[`UNSAFE_MICROOPT_AB.md`](UNSAFE_MICROOPT_AB.md) campaign 1.

### `unsafe` SyncPtr scatter (TRU-OLS)

Solver-bound. +1% noise at 100,000 events. Do not scatter with raw pointers
until the factorization is no longer the limiter.

### Scratch `Vec` reuse when the bound is elsewhere

- PaCMAP gradient buffer reuse: **+12%** at 50,000 events.
- Compress encode payload reuse: **+8%** (zstd-bound) at 16 × 64k.
- peacoqc peaks bin slices: noise at 100,000 events (KDE-bound).

`workspace-per-worker` still applies when **allocation is the limiter** (joint
unmix). Profile first: if zstd/KDE/QR is 90% of the sample, do not shuffle
`Vec::clear`.
