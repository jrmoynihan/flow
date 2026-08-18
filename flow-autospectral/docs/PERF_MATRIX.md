# flow-autospectral n × K × d throughput matrix

Fill from Criterion (`match_matrix`, `discover_and_match`, `scatter_clean`).
Interleave variants; keep `match_events_nn` / `match_nn_control` as the untouched
control (see beads memory `benchmark-a-b-on-this-machine-apple-m5`).

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

## Env

| Variable | Effect |
|----------|--------|
| *(unset)* | match: n=10k, K∈{1,4,8,32}, d∈{8,20}; scatter: 10k and 50k |
| `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1` | match: +n 50k/100k, +d 40, +K 16/64; scatter: +100k |
| `FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1` | match: +n 250k (do not combine casually) |
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

| Item | Status | Notes |
|------|--------|-------|
| Factor-once residual | **keep** | 1.9× (K=1) … 4.2× (K=32) vs naive at n=10k d=8 |
| Rayon residual (vs seq, factored) | **keep** | 1.26× at K=1, 4.0× at K=8, 5.2× at K=32 |
| Factor-once `unmix_events_ols` | **keep** | 2.2× vs per-event QR |
| Rayon `unmix_events_ols` | **keep** | 1.76× vs sequential at n=10k |
| GPU `AnnIndex` query (Exact, IVF) | **keep API** | query-vs-library works; pad features to `LINE_SIZE` |
| GPU `AnnIndex` NnDescent | **skip** | `query_nndescent_index_gpu` needs `&mut` index |
| GPU scatter-clean n=10k d=2 | **skip as default** | +1.7% median, overlapping ranges |
| GPU scatter-clean n=50k d=2 | **keep optional** | 3.1× vs CPU Exact; use `--features gpu` |

Still unmeasured: d=20/40, n=50k–250k match residual, K=64, IVF scatter, ANN shortlist vs exhaustive at K>32.
