# e-means performance notes

Experimental mini-batch k-means with exponential-moving-average centroids
(Bessa, Mazaré, Szilvasy, Touvron, Picard, Douze and Jégou,
[arXiv:2611.09501](https://arxiv.org/abs/2611.09501)). Feature `emeans`
(optional `emeans-gpu`).

## Problem

Full-batch Lloyd on multi-million event cytometry matrices (30–60 parameters)
touches every event × every centroid every iteration. At 5M events × 60
parameters × 2,048 clusters that is roughly 6×10¹¹ distance terms per
iteration. Callers need a path that can reach Lloyd-like train or held-out
error in fewer passes without the `Array2<f64>` copy the linfa wrapper forces.

## Solution

`EMeans` takes row-major `&[f32]` (`n × d`), updates per-cluster sums and
counts with a cosine-scheduled EMA, revives dead clusters by donor split
(δ = BF16 ε), and finalises with last-epoch-mean by default. Assignment sits
behind an `Assign` trait so Lloyd, the Rayon CPU loop, and the cubeCL kernel
share one distance kernel. Published batch-size and peak-step scaling laws
are the defaults; every knob is overridable.

## What changes in operation

- **`emeans` is a default Cargo feature.** Use `EMeans::fit` as the primary
  row-major `f32` k-means path. Default config: **3 epochs**, CPU assignment.
- Size heuristic: [`prefer_emeans`](../src/clustering/emeans/schedule.rs)
  (`n ≥ 5_000` or `n × d ≥ 50_000`). Below that both e-means and Lloyd usually
  finish under ~20 ms; Lloyd only clearly wins at toy sizes.
- Linfa `KMeans` is opt-in (`--features kmeans`) for the `Array2<f64>` API;
  FlowSOM metaclusters via `Lloyd`.
- **Do not enable `emeans-gpu` by default.** On this host GPU assignment only
  wins for large per-batch work (see crossover below). Leave the feature off
  unless batch × clusters is roughly ≥ 3×10⁷ (for example 16 384 × 2 048).

## How to measure

```text
cargo run -p flow-clustering --example compare_emeans_lloyd --release
FLOW_CLUSTERING_BENCH_LOWER_BOUND=1 cargo run -p flow-clustering --example compare_emeans_lloyd --release
FLOW_CLUSTERING_BENCH_MATRIX=1 cargo run -p flow-clustering --example compare_emeans_lloyd --release
FLOW_CLUSTERING_BENCH_SMOKE=1 cargo run -p flow-clustering --example compare_emeans_lloyd --release
cargo bench -p flow-clustering --bench emeans_throughput
cargo bench -p flow-clustering --bench emeans_assign_cpu_vs_gpu --features emeans-gpu
```

Interleave baseline and candidate binaries and keep an untouched control
benchmark in the group (this host’s Criterion drift note).

## Decision gate

**Passed (2026-11-12, Apple M5 Max).** At 1M events, every tested `(d, k)` cell
matched Lloyd-at-25 held-out MSE (within 0.5%) with comparable mean F1 in
clearly less wall time on the same assignment kernel—usually by epoch 1, and
by epoch 2 at `k = 2048` for some 100k cells.

## Lower bound vs Lloyd@25 (when to prefer e-means)

Harness: `FLOW_CLUSTERING_BENCH_LOWER_BOUND=1` — 1k–50k events × 10/30/60
features × 8/32/256 clusters, seeds 1–3. Gate: held-out MSE ≤ Lloyd × 1.005;
speed at the earliest matching epoch (1–3).

| regime | verdict |
|--------|---------|
| n ≥ 10k (any d, k in grid) | **e-means** in 26/27 cells; 1 tie (both &lt;20 ms) |
| n ≥ 5k | **e-means** in almost every cell |
| n = 1k, d ≥ 60 | e-means already ahead |
| n = 1k, d = 10, k = 8 | Lloyd slightly faster (~0.87×); both ~5 ms |
| n = 1k, k = 256 | MSE slightly above gate (over-cluster on tiny n) |

**Practical cutoff:** prefer e-means once `n ≥ ~5k` or `n × d ≥ ~50k`. That
covers essentially all real FCS sizes; feature `emeans` is therefore
**default-on**. Absolute walls below ~20 ms are overhead-dominated either way.

## Quality vs Lloyd@25 (synthetic Gaussian populations)

Harness: `compare_emeans_lloyd`. Median over seeds 1–3. “Match” = held-out MSE
≤ Lloyd × 1.005 and mean F1 ≥ Lloyd × 0.98.

| n events | d | k | epochs to match | e-means wall (median) | Lloyd@25 wall | speedup |
|---:|---:|---:|---:|---:|---:|---:|
| 100k | 30 | 32 | 1 | 28 ms | 73 ms | ~2.6× |
| 100k | 30 | 256 | 1 | 40 ms | 305 ms | ~7.6× |
| 100k | 30 | 2048 | 2 | 252 ms | 1.99 s | ~7.9× |
| 100k | 60 | 32 | 1 | 38 ms | 137 ms | ~3.6× |
| 100k | 60 | 256 | 1 | 73 ms | 605 ms | ~8.3× |
| 100k | 60 | 2048 | 2 | 508 ms | 4.22 s | ~8.3× |
| 1M | 30 | 32 | 1 | 107 ms | 611 ms | ~5.7× |
| 1M | 30 | 256 | 1 | 271 ms | 2.74 s | ~10× |
| 1M | 30 | 2048 | 2 | 2.17 s | 20.7 s | ~9.5× |
| 1M | 60 | 32 | 1 | 176 ms | 1.31 s | ~7.5× |
| 1M | 60 | 256 | 1 | 473 ms | 5.76 s | ~12× |
| 1M | 60 | 2048 | 1 | 3.24 s | 43.5 s | ~13× |

Notes:

- At `n = 100k`, `k = 2048`, epoch 1 left many empty clusters (tens to
  hundreds); epoch 2 cleared them. Prefer the default of 3 epochs for
  over-clustering workloads.
- Soft-label mean F1 against ~20–200 generating populations is weak at
  `k = 2048` (over-segmentation); held-out MSE is the primary gate metric.
- Raw JSON/markdown: `target/emeans_compare/` (local; not committed).

## CPU / GPU assignment crossover

Criterion `emeans_assign_cpu_vs_gpu` on Apple M5 Max (unified memory), cubeCL
0.10 / wgpu, interleaved with an untouched control. Times are Criterion
medians (steady state; first-dispatch not separated in this run).

Work per batch ≈ batch × k × d element-wise distance terms.

| batch | d | k | CPU | GPU | winner |
|---:|---:|---:|---:|---:|---|
| 1 024 | 30 | 16 | 176 µs | 1.47 ms | CPU (~8×) |
| 1 024 | 30 | 256 | 298 µs | 2.64 ms | CPU (~9×) |
| 1 024 | 30 | 2 048 | 1.12 ms | 6.38 ms | CPU (~6×) |
| 16 384 | 30 | 16 | 407 µs | 1.68 ms | CPU (~4×) |
| 16 384 | 30 | 256 | 2.04 ms | 3.11 ms | CPU (~1.5×) |
| 16 384 | 30 | 2 048 | 14.6 ms | 6.95 ms | **GPU (~2.1×)** |
| 65 536 | 30 | 16 | 879 µs | 3.47 ms | CPU (~4×) |
| 65 536 | 30 | 256 | 7.91 ms | 6.04 ms | **GPU (~1.3×)** |
| 65 536 | 30 | 2 048 | 57.5 ms | 21.1 ms | **GPU (~2.7×)** |
| 1 024 | 60 | 16 | 254 µs | 1.57 ms | CPU (~6×) |
| 16 384 | 60 | 2 048 | 49.4 ms | 14.0 ms | **GPU (~3.5×)** |
| 65 536 | 60 | 256 | 21.3 ms | 10.7 ms | **GPU (~2.0×)** |
| 65 536 | 60 | 2 048 | 137 ms | 48.0 ms | **GPU (~2.9×)** |

**Recommendation:** keep `AssignBackend::CpuLoop` as the default. Use GPU only
when batch × k is large (on this host, roughly ≥ 16 384 × 2 048 or
65 536 × 256). Discrete GPUs also pay PCIe transfer; re-measure before
shipping a GPU default elsewhere.

## CPU assignment A/B (hand-written vs faer)

Feature `emeans-faer` is available but not yet A/B’d on this host. Default
remains `CpuLoop` until that Criterion pass lands.

| machine | n×d×k | CpuLoop | CpuFaer | control drift | keep |
|---|---|---:|---:|---:|---|
| — | — | — | — | — | pending |

## Defaults

| knob | default | rationale |
|---|---|---|
| Cargo feature `emeans` | **on** | Wins vs Lloyd from ~5k events / n×d ≥ 50k |
| `epochs` | 3 | Paper-scale recipe; covers high-`k` empty-cluster recovery |
| `backend` | `CpuLoop` | GPU loses on small/medium batches |
| `finalization` | `LastEpochMean` | Paper default for train MSE |
| `batch_size` / `alpha0` | scaling laws | Overridable; laws fitted on high-d embeddings |
