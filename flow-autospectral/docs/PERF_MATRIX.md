# flow-autospectral n × K × d throughput matrix

Fill from Criterion (`match_matrix`, `discover_and_match`, `scatter_clean`,
`joint_unmix`). Interleave variants; keep `match_events_nn` / `match_nn_control`
as the untouched residual-match control (see beads memory
`benchmark-a-b-on-this-machine-apple-m5`). README vs-R claims belong in
`docs/comparison-with-r.md` (QC-core rust/R ratios), not these Criterion `Melem/s`
rows.

## Groups

| Bench | Group | Notes |
|-------|-------|--------|
| `match_matrix` | `match_residual_naive` | `reuse_af_factors=false`, Rayon threshold 256 |
| `match_matrix` | `match_residual_factored` | `reuse_af_factors=true`, Rayon threshold 256 |
| `match_matrix` | `match_residual_seq` | `reuse_af_factors=true`, `parallel_event_threshold=usize::MAX` |
| `match_matrix` | `match_nn_control` | `NearestNeighbor` only — do not mix residual |
| `match_matrix` | `unmix_ols` | IDs `n{n}_d{d}_{naive|factored|seq}` via `OlsUnmixConfig` |
| `discover_and_match` | `match_events_nn` | Untouched NN control (narrow n sweep, d=8) |
| `scatter_clean` | `scatter_clean_cpu` | `KnnMethod::Exact`, d_scatter=2 |
| `scatter_clean` | `scatter_clean_gpu` | `KnnMethod::GpuExact`; skipped if AnnIndex GPU missing / no adapter |
| `joint_unmix` | `joint_unmix` | QC-core joint pipeline; IDs `n{n}_d{d}_F{F}_K{K}` (`f64`) and `…_f32` |
| `joint_unmix` | `joint_af_only` | Empty variants (AF matching-pursuit only) — `f64` control vs full joint |

## Env

| Variable | Effect |
|----------|--------|
| *(unset)* | match: n=10k, K∈{1,4,8,32}, d∈{8,20}; scatter: 10k and 50k |
| `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1` | match: +n 50k/100k, +d 40, +K 16/64; scatter: +100k |
| `FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1` | match: +n 250k; joint: +n 1_000_000 (do not combine casually) |
| `FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1` | disables Rayon in library paths (not a bench grid gate) |

IDs: match `n{n}_d{d}_K{K}`; scatter `n{n}`. Throughput is `Elements(n)` (events).

## Commands

```bash
# Default (10k × K∈{1,4,8,32} × d∈{8,20})
cargo bench -p flow-autospectral --bench match_matrix

# Smoke subset used for the snapshot below
cargo bench -p flow-autospectral --bench match_matrix -- \
  'n10000_d8_K(1|8|32)$|n10000_d8_(naive|factored|seq)$'

# Denser grid: +50k/100k, d=40, K=16,64
FLOW_AUTOSPECTRAL_BENCH_MATRIX=1 cargo bench -p flow-autospectral --bench match_matrix

# +250k (slow)
FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1 cargo bench -p flow-autospectral --bench match_matrix

cargo bench -p flow-autospectral --bench scatter_clean
cargo bench -p flow-autospectral --bench scatter_clean --features gpu

# Joint QC-core (default 10k × 20 × 8 fluors, K_AF=8)
cargo bench -p flow-autospectral --bench joint_unmix

# Joint denser grid: +n 50k/200k, +d 40/64
FLOW_AUTOSPECTRAL_BENCH_MATRIX=1 cargo bench -p flow-autospectral --bench joint_unmix
```

## Provenance (smoke snapshot)

| Field | Value |
|-------|--------|
| Machine | Apple M5 Max |
| rustc | 1.95.0 (59807616e 2026-04-14) |
| Features | default `hnsw`; scatter GPU used `--features gpu` |
| Date | 2026-08-18 |
| Env | filter `n=10_000`, `d=8`, `K∈{1,8,32}` plus scatter `n∈{10k,50k}` |
| Criterion | sample_size 10, warmup 1s, measure 2s (match) / 3s (scatter) |

Medians below. Throughput is events/s (`Elements(n)`). Cells marked *not run* need `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1`.

## Match residual vs NN (n=10_000, d=8)

| K | residual naive | residual factor-once | residual seq | nn control |
|---|----------------|----------------------|--------------|------------|
| 1 | 1.551 ms (6.45 M/s) | **0.811 ms (12.3 M/s)** | 1.020 ms (9.81 M/s) | 4.815 ms (2.08 M/s) |
| 8 | 7.095 ms (1.41 M/s) | **2.015 ms (4.96 M/s)** | 8.107 ms (1.23 M/s) | 6.029 ms (1.66 M/s) |
| 32 | 26.19 ms (0.382 M/s) | **6.168 ms (1.62 M/s)** | 31.95 ms (0.313 M/s) | 11.15 ms (0.897 M/s) |

NN rebuilds `AnnIndex` on every `match_events` call, so it is slower than factored residual at small K. It remains the untouched control for residual A/B, not a residual competitor.

## Unmix OLS (n=10_000, d=8, one mixing matrix)

| Variant | Median | Throughput |
|---------|--------|------------|
| naive (per-event QR, Rayon) | 1.096 ms | 9.12 M/s |
| factor-once (Gram Cholesky, Rayon) | **0.505 ms** | 19.8 M/s |
| factor-once sequential | 0.892 ms | 11.2 M/s |

## Scatter-clean (d=2 Exact; GPU = `KnnMethod::GpuExact`)

| n | CPU Exact | GPU Exact |
|---|-----------|-----------|
| 10_000 | 12.58 ms (0.795 M/s) | 12.36 ms (0.809 M/s) — tie |
| 50_000 | 300.0 ms (0.167 M/s) | **98.0 ms (0.510 M/s)** |

## Keep / skip (A/B ≥5% on primary size)

| What we changed | Status | Notes |
|-----------------|--------|-------|
| Reuse one OLS factorization per AF spectrum (`match_residual_reused`) | **keep** | 1.9× (K=1) … 4.2× (K=32) vs decomposing again for every event at 10,000 events, 8 detectors |
| Process residual matching across events in parallel | **keep** | 1.26× at K=1, 4.0× at K=8, 5.2× at K=32 vs one thread |
| Reuse one factorization in `unmix_events_ols` (`OlsFactor`) | **keep** | 2.2× vs QR once per event |
| Process that unmix across events in parallel | **keep** | 1.76× vs one thread at 10,000 events |
| GPU `AnnIndex` query (Exact, IVF) | **keep API** | query-vs-library works; pad features to `LINE_SIZE` |
| GPU `AnnIndex` NnDescent | **skip** | `query_nndescent_index_gpu` needs `&mut` index |
| GPU scatter-clean at 10,000 events, 2 scatter detectors | **skip as default** | +1.7% median, overlapping ranges |
| GPU scatter-clean at 50,000 events, 2 scatter detectors | **keep optional** | 3.1× vs CPU Exact; use `--features gpu` |
| Reuse `EventScratch` arrays, copy mixing matrices only when a variant is accepted, write into pre-sized tables, column-wise `gemv` | **keep** | −56% median at 10,000 events, 20 detectors, 8 fluorophores, 8 AF spectra (`joint-alloc-pre` → HEAD); AF-only control −49%. Prose: [`PERF_AB.md`](PERF_AB.md). |
| Internal `f32` faer (`JointUnmixPrecision::F32`) | **keep optional** | −18% vs `f64` at 200,000 events × 64 detectors (bead primary). **Skip as default**: +157% at 10,000 events × 20 detectors; vs-R is double. Prose: [`PERF_AB.md`](PERF_AB.md). |

Still unmeasured: d=20/40, n=50k–250k match residual, K=64, IVF scatter, ANN shortlist vs exhaustive at K>32.

## Joint unmix (QC-core)

[`PERF_AB.md`](PERF_AB.md) states each joint-unmix change as a problem, a solution, and how per-event work differs (workspace reuse, copy the mixing matrix only when a variant is accepted, column-wise matrix–vector products, writes into pre-sized tables). It names the functions a C++ / R / Julia port would mirror.

Default Criterion: n=10_000, d=20, F=8, K_AF=8. MATRIX adds n∈{50k,200k} and d∈{40,64}. LARGE adds n=1M.

Provenance for the **A/B keep** (2026-08-19, Apple M5 Max, rustc 1.95.0 59807616e): `cargo bench -p flow-autospectral --bench joint_unmix -- --baseline joint-alloc-pre`. Pre was 4.464 ms joint / 2.877 ms AF-only. Keep at −56% on the primary size.

Later the same day, `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1 cargo bench -p flow-autospectral --bench joint_unmix` (no `--baseline`; Criterion `change` vs the pre-scratch `new` in the workspace target). sample_size 10, warmup 1s, measure 3s. Medians below are that MATRIX pass — **not** a second interleaved A/B. 10k×20 joint 1.658 ms is faster than the A/B post (2.096 ms); treat 2.096 ms as the keep number and this grid as a scaling snapshot.

| n | d | F | K_AF | joint median | AF-only control |
|---|---|---|------|--------------|-----------------|
| 10_000 | 20 | 8 | 8 | **1.658 ms (6.03 M/s)** | 1.245 ms (8.03 M/s) |
| 10_000 | 40 | 8 | 8 | 2.193 ms (4.56 M/s) | — |
| 10_000 | 64 | 8 | 8 | 2.647 ms (3.78 M/s) | — |
| 50_000 | 20 | 8 | 8 | 4.941 ms (10.1 M/s) | — |
| 50_000 | 40 | 8 | 8 | 6.674 ms (7.49 M/s) | — |
| 50_000 | 64 | 8 | 8 | 9.079 ms (5.51 M/s) | — |
| 200_000 | 20 | 8 | 8 | 16.10 ms (12.4 M/s) | — |
| 200_000 | 40 | 8 | 8 | 23.93 ms (8.36 M/s) | — |
| 200_000 | 64 | 8 | 8 | 35.08 ms (5.70 M/s) | — |

AF-only is empty `SpectralVariants` (matching-pursuit, no fluorophore coordinate descent). Full joint at 10k×20 is ~1.33× the AF-only median — expected. Events/s rises with n at fixed d (Rayon occupancy).

Vs AutoSpectralRcpp QC-core (`docs/comparison-with-r.md`): 1-thread rust ~**2×** R on F=8 d=20 (10k–1M) and F=42 d=64 (50k–1M). F=8 200k / 18-thread ~3.5×; F=42 200k / 18-thread ~5.3×. Criterion 1M (`FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1`) was not re-run after the 1M vs-R pass.

This Criterion grid is Rayon occupancy (default thread pool), so 10k×20 at 6.03 M/s is **not** the 1-thread vs-R number (1.74 M/s).

## Cost model (napkin vs measured)

Workspace protocol: [`docs/dev/PERF_PGD.md`](../../docs/dev/PERF_PGD.md). Index: [`docs/dev/PERF_GAP.md`](../../docs/dev/PERF_GAP.md).

**Joint unmix** (Criterion `joint_unmix`, 10,000 events × 20 detectors × 8 fluorophores × 8 AF). Encoding: default `faer::Mat<f64>` in [`joint_inner.rs`](../src/joint_inner.rs) (`JointUnmixPrecision::F64`). Bytes in: 10,000 × 20 × 8 B = 1.6 MiB (L2). Arithmetic floor for a few thousand `f64` FLOPs/event on one P-core is ~1 ms; with Rayon, less. Measured keep: **2.096 ms** (MATRIX snapshot 1.658 ms) → ratio **~2×** vs the `f64` FMA floor (**1–3×**, on the roofline for occupancy). Pre-scratch 4.464 ms was alloc-bound (`workspace-per-worker`, `copy-on-commit`, `match-layout-gemv`, `hoist-factor-once` already applied). Width experiment (`flow-crates-0ap.1`): `F32` is **optional** at 200,000 events × 64 detectors (−18% vs paired `f64`); it lost at 10,000 events × 20 detectors. Default stays `f64` (vs-R double).

**Residual match, factor-once** (`match_residual_factored`, 10,000 events × 8 detectors × 32 AF): 6.17 ms. 32 applies × 10,000 events of a tiny `d=8` factor is tens of ns each; ratio **~2–4×** vs FMA (`hoist-factor-once`, `parallel-after-precomp`). Naive QR-per-pair was **~4×** this (complexity, already kept).

**OLS factor-once** (`unmix_ols` factored, 10,000 events × 8 detectors): 0.505 ms → **~50 ns/event**. On the roofline for a GEMV + triangular solve. Strategy: `hoist-factor-once`.

**Scatter-clean Exact** (50,000 events × 2 detectors): CPU 300 ms vs `n² d` FMA floor ~50 ms on six P-cores (**~6×**, gather occupancy). GPU 98 ms (**~2×**). Strategy: `gpu-after-amortize`. At 10,000 events GPU is a wash (launch).
