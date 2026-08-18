# PARC performance notes (`flow-clustering`, feature `parc`)

Baselines measured on **Apple M5 Max** (18 cores), `rustc 1.95.0 (59807616e 2026-04-14)`,
release / Criterion bench profile. Synthetic two-cloud data. Date: **2026-08-18**.

Default `ParcConfig` uses **HNSW** (`knn_method: None`). Exact is for isolation /
quality A/B, not the production default.

## How to reproduce

```bash
# End-to-end wall + peak RSS
# Filter keys: n_d_k_{exact|hnsw}_{seq|rayon}
cargo run -p flow-clustering --release --example parc_rss --features parc

# Criterion groups (filter with trailing args)
cargo bench -p flow-clustering --bench parc_throughput --features parc -- parc_e2e_knn_ab
cargo bench -p flow-clustering --bench parc_throughput --features parc -- parc_prune_rayon_ab
cargo bench -p flow-clustering --bench parc_throughput --features parc -- parc_e2e_exact
```

`ParcConfig::parallel_prune` toggles Rayon only on local + Jaccard prune.
Leiden (`leiden-rs` with `rayon`) may still use the pool unless
`RAYON_NUM_THREADS=1` for a fully sequential process.

## Exact vs HNSW end-to-end (primary A/B)

Group `parc_e2e_knn_ab` — Exact then HNSW interleaved per size; Rayon prune on.
Medians:

| Size | Exact median | HNSW median | HNSW vs Exact | Exact evt/s | HNSW evt/s |
|------|--------------|-------------|---------------|-------------|------------|
| 5k × 20, k=20 | 202.7 ms | 212.1 ms | **0.96×** (Exact slightly faster) | 24.7k | 23.6k |
| 20k × 20, k=30 | 639.0 ms | 689.6 ms | **0.93×** | 31.3k | 29.0k |
| 50k × 20, k=30 | 1.539 s | 943.4 ms | **1.63×** | 32.5k | 53.0k |
| 100k × 20, k=30 | 4.549 s | 1.654 s | **2.75×** | 22.0k | 60.4k |

At small *n*, HNSW index build + search overhead can outweigh Exact’s O(*n²d*)
lookups. From ~50k events (cytometry-typical), Exact’s quadratic neighbor search
dominates and HNSW pulls ahead sharply; by 100k HNSW is nearly **3×** end-to-end.

## End-to-end throughput (Exact k-NN, Rayon prune on)

Companion matrix when isolating prune/Leiden after an Exact graph build.

### Criterion `parc_e2e_exact` (median)

| Case | Median | Throughput |
|------|--------|------------|
| 2k × 10, k=15 | 21.2 ms | 94.4k evt/s |
| 5k × 10, k=20 | 180.2 ms | 27.7k evt/s |
| 5k × 30, k=20 | 157.9 ms | 31.7k evt/s |
| 20k × 10, k=30 | 650.7 ms | 30.7k evt/s |
| 20k × 30, k=30 | 689.9 ms | 29.0k evt/s |
| 50k × 20, k=30 | 1.61 s | 31.0k evt/s |

### Isolated `parc_rss` single-shot (Exact, rayon)

| n | d | k | Wall | Throughput |
|---|---|---|------|------------|
| 5k | 10 | 20 | 212 ms | ~24k evt/s |
| 20k | 10 | 30 | 688 ms | ~29k evt/s |
| 50k | 20 | 30 | 1.54 s | ~32k evt/s |
| 100k | 20 | 30 | 4.40 s | ~23k evt/s |

## Rayon vs sequential prune (Criterion, precomputed Exact k-NN)

Group `parc_prune_rayon_ab` — seq/rayon interleaved per size. Medians:

| Size | Seq median | Rayon median | Speedup | Events/s (rayon) |
|------|------------|--------------|---------|------------------|
| 5k × 20, k=20 | 205.8 ms | 193.6 ms | **1.06×** | 25.8k |
| 20k × 20, k=30 | 578.8 ms | 483.2 ms | **1.20×** | 41.4k |
| 50k × 20, k=30 | 896.4 ms | 660.7 ms | **1.36×** | 75.7k |

Prune parallelism helps more as *n* grows; at 5k the win is near noise (~6%).
End-to-end Exact gains are smaller than prune-only because Exact k-NN + Leiden
share the cost; with default HNSW the prune share of wall time is larger, so
Rayon prune matters more in production.

## Peak RSS (isolated process per cell)

`getrusage(RUSAGE_SELF).ru_maxrss` (macOS bytes). Fresh process per row via
`PARC_RSS_FILTER` (Exact path):

| n | d | k | Seq peak | Rayon peak | Δ |
|---|---|---|----------|------------|---|
| 5k | 10 | 20 | 19.7 MiB | 21.6 MiB | +10% |
| 20k | 10 | 30 | 78.8 MiB | 86.5 MiB | +10% |
| 50k | 20 | 30 | 181.9 MiB | 197.7 MiB | +9% |
| 100k | 20 | 30 | 323.8 MiB | 376.5 MiB | +16% |

Rayon prune costs modest extra RSS (~10–16%) for the measured sizes.

## Assessment

- **Keep HNSW as default knn** — correct for cytometry scale (≥~50k); Exact only
  for tiny matrices or deterministic neighbor quality checks.
- **Keep `parallel_prune = true` (default)** — clear win from ~20k events upward
  on prune+Leiden path; small regress risk at tiny *n*.
- **RSS** — expect ~0.2–0.4 GiB class footprints at 50k–100k with Exact k-NN and
  k≈30; scale roughly with *n* (and edge lists ~ *n·k*).
- **Follow-ups** — optional Exact↔HNSW peak-RSS A/B; `RAYON_NUM_THREADS=1`
  full-pipeline A/B; f32 end-to-end path to cut copies; neighbor-recall quality
  check if clustering labels diverge under HNSW.

## Control note

Long Criterion sessions can drift on this machine; A/B groups interleave
Exact↔HNSW and seq↔rayon per size. Prefer that pattern for future A/B.
