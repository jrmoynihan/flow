# flow-pacmap performance notes

Audit of ANN / GPU / accelerant choices relative to peer PaCMAP crates, plus Criterion
timings from `pacmap_compare`.

**Peers (dev-dep comparators):**

| Crate | Version | Role |
|-------|---------|------|
| [`manifolds-rs`](https://crates.io/crates/manifolds-rs) | `0.3.9` | CPU PaCMAP; ANN via `ann-search-rs` |
| [`oxicuda-manifold`](https://crates.io/crates/oxicuda-manifold) | `0.5.1` | Pure-Rust PaCMAP (`f64`); in-crate HNSW/brute |

**Shared KNN:** workspace crate [`flow-knn`](../flow-knn/) owns `KnnGraph` / `compute_knn` (usearch and optional `ann-search-rs`).

**Harness:** `flow-pacmap/benches/pacmap_compare.rs` (+ `benches/README.md`).

---

## GPU boundaries

| Topic | manifolds-rs | oxicuda-manifold | flow crates |
|-------|--------------|------------------|-------------|
| PaCMAP on GPU? | No | No (PTX is t-SNE/UMAP/PCA/MDS/knn_topk) | Optional `OptimizeBackend::Gpu` (Burn Adam + cubeCL CSR) |
| GPU elsewhere | `ann-search-rs` + `cubecl` for UMAP/t-SNE kNN | OxiCUDA PTX | peacoqc / tru-ols / density: Burn 0.21 + cubeCL 0.10 |

Lesson: GPU helps PaCMAP e2e mainly via the Adam + pair-gradient stage (see below).

### Can we benefit? (bead `.9` audit)

**Stage share @ 50k×10 (approx.):** CPU HNSW KNN is ~8–12% of e2e; Adam + pair gradients are ~88–92%. Speeding only KNN therefore caps e2e gains at roughly that fraction unless optimize moves to GPU too.

| Constituent | GPU available today? | Likely benefit for FCS PaCMAP | Notes |
|-------------|----------------------|-------------------------------|-------|
| **KNN graph** | **Yes** — `flow-knn` feature `gpu` → `GpuExact` / `GpuIvf` / `GpuNnDescent` (ann-search-rs + cubeCL 0.10) | **High at mid/large n** | `exact_gpu` wins through ~50k; `ivf_gpu` wins @ 100k vs HNSW in Criterion |
| **Pair sampling** | No | Low | Irregular, once-per-fit; stay on CPU. |
| **PCA init** | oxicuda `pca_center` PTX only | Low | Tiny vs 450-iter optimize. |
| **Adam + pair gradients** | **Yes** — Burn Adam + raw cubeCL CSR on shared `Handle`s | **High** | Zero-copy: ~4–9× vs CPU @ 10k–100k (Criterion) |
| **Full PaCMAP e2e** | Partial (optimize + optional GPU kNN) | High when combining GPU kNN + future zero-copy Adam | |

**Stack:** workspace GPU crates pin **Burn 0.21 + cubeCL 0.10** (unified). Prefer cubecl/wgpu (Metal/Vulkan/CUDA via cubecl) over oxicuda’s NVIDIA-only PTX.

**Practical recommendation:** default CPU HNSW (`recommend_method`). Use **`KnnMethod::GpuExact` / `GpuIvf`** when a WGPU adapter is present and n is large. Prefer **`OptimizeBackend::Gpu`** for the Adam + pair-gradient stage when an adapter is available (zero-copy Burn↔cubeCL).

---

## ANN backends

| Stack | Backends | Notes |
|-------|----------|-------|
| manifolds-rs | `ann-search-rs` | faer 0.23 |
| oxicuda-manifold | in-crate HNSW / brute | f64; e2e PaCMAP uses **brute** KNN |
| **flow-knn** | exact; usearch (`hnsw`); ann-search-rs (`ann-search`) | Portable `KnnGraph` |
| flow-pacmap | depends on `flow-knn` | Features forward; `ann-search` matches manifolds HNSW |

When `--features ann-search` is enabled, `KnnMethod::default()` is `AnnSearchHnsw`. Otherwise default is usearch HNSW.

---

## Staged API

```text
1. knn  = flow_knn::compute_knn(...)?       // shared crate
2. emb  = fit_transform(..., Some(&knn), ...)?
```

Pair construction stays algorithm-specific inside PaCMAP.

---

## Criterion — KNN HNSW grid (local, release)

Aligned params: k=60, m=16, ef_construction=200, ef_search=50. Median wall times.

| n × d | flow usearch | manifolds | **flow ann-search** | oxicuda HNSW |
|-------|--------------|-----------|---------------------|--------------|
| 50k × 10 | 803 ms | 529 ms | **529 ms** | 4.17 s |
| 100k × 15 | 1.82 s | 1.57 s | **1.63 s** | — |
| 250k × 20 | 9.25 s | 5.83 s | **5.76 s** | — |
| 500k × 20 | 21.0 s | 16.3 s | **13.5 s** | — |

**Takeaway:** usearch is ~1.3–1.6× slower than manifolds on this grid. `flow-knn` feature **`ann-search`** matches manifolds at 50–250k and is **faster at 500k**. Prefer `ann-search` for FCS-scale throughput; keep usearch when you need its quantization without pulling faer 0.23.

Exact @ 50k: flow 1.27 s vs manifolds exhaustive 916 ms.

Raw log: `flow-pacmap/bench_results/knn_grid.txt`.

---

## Criterion — e2e fit_transform (450 iters)

Local release medians (`FLOW_PACMAP_BENCH_MAX_N=50000`). oxicuda e2e is brute KNN + f64
(now smoke-only in the harness; number below is from an earlier full run).

| n × d | flow (usearch) | flow (ann-search) | manifolds | oxicuda |
|-------|----------------|-------------------|-----------|---------|
| 50k × 10 | **6.92 s** | 8.53 s | 13.9 s | 67.0 s† |
| 50k × 20 | — | 2.77 s‡ | — | — |
| 100k × 15 | **2.97 s** | 3.73 s | 8.63 s | — (skip) |
| 250k+ | deferred (hours × arms) | deferred | deferred | — |

† oxicuda from an earlier run (harness now smoke-only for oxicuda e2e). On Apple Silicon, oxicuda’s CUDA/PTX path is not usable — expected for an NVIDIA-oriented stack; prefer cubeCL/wgpu here.  
‡ isolated `flow_ann` filter run; treat as directional — three-way 50k×10 prefers the row above.

Absolute e2e wall times vary across sessions (thermal / load); use within-run ordering. Raw logs: `bench_results/e2e_50k.txt`, `e2e_50k_ann.txt`, `e2e_100k.txt`.

```bash
FLOW_PACMAP_BENCH_MAX_N=100000 cargo bench -p flow-pacmap --features ann-search \
  --bench pacmap_compare -- pacmap_fit_transform \
  --warm-up-time 1 --measurement-time 80 --sample-size 10
```

---

## Criterion — GPU vs CPU optimize (Burn Adam + cubeCL CSR, zero-copy)

Feature: `--features cubecl` → `OptimizeBackend::Gpu` runs:
- **Pair gradients:** raw cubeCL CSR kernel (one thread per node; no float atomics)
- **Adam:** Burn `Adam` on the **same** wgpu / cubeCL `Handle` as the embedding & grad tensors

Host sync only on final download. (Pre–zero-copy path synced embd/grad every iter and
stayed at ~parity @ 100k.)

| Workload | CPU (precomputed KNN) | GPU zero-copy | vs prior GPU (sync/iter) |
|----------|----------------------|---------------|--------------------------|
| 10k × 10, 450 iters | 1.83 s | **444 ms** (~4×) | was 1.64 s |
| 50k × 10, 450 iters | 9.49 s | **1.09 s** (~9×) | was 1.81 s |
| 100k × 15, 450 iters | 14.9 s | **2.11 s** (~7×) | was 2.24 s |

Absolute CPU times vary by machine load; the important signal is GPU ≪ CPU after
sharing Burn `CubeTensor` handles with the CSR launch.

```bash
cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu
FLOW_PACMAP_BENCH_PRESSURE=1 cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu
```

Usage:

```rust
let mut config = PaCMAPConfig::default();
config.optimize_backend = OptimizeBackend::Gpu; // needs --features cubecl
```

---

## KNN method matrix & auto-selection

Growing dataset + selector live in **`flow-knn`**:

- Data: [`flow-knn/data/knn_perf_matrix.jsonl`](../../flow-knn/data/knn_perf_matrix.jsonl) (Cartesian through 100k×{10,15,20} collected 2026-07-24; Criterion 250k/500k retained)
- Docs: [`flow-knn/docs/PERF_MATRIX.md`](../../flow-knn/docs/PERF_MATRIX.md)
- API: `flow_knn::recommend_method(n, d, &RecommendOpts::default())` (also `flow_pacmap::knn`)
- Collect: `cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"`
- Criterion: `cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu`

At measured FCS cells on **CPU**, `hnsw_ann_search` wins; with **GPU**, prefer `exact_gpu` (≤50k) or `ivf_gpu` (≥100k). Selector returns Exact only for `n ≤ 5_000` unless `RecommendOpts::allow_gpu`.

---

## Smoke timings (n=1k, short iters — harness only)

| Arm | Median |
|-----|--------|
| flow_hnsw / manifolds_hnsw / oxicuda_hnsw | ~7.4 / 5.4 / 37 ms |
| flow / manifolds / oxicuda e2e (6 iters) | ~9.4 / 11 / 13 ms |
| reuse cold 3× vs hot 1knn+3embed | ~29 vs ~14.5 ms (~2×) |

---

## Ranked follow-ups

| Priority | Work | Status |
|----------|------|--------|
| P0 | Staged `KnnGraph` | **Done** (`flow-knn` + pacmap) |
| P0 | Extract KNN from pacmap | **Done** (`flow-knn`) |
| P1 | Match manifolds HNSW | **Done** via `ann-search` feature |
| P1 | Fill e2e 50–100k table | **Done** (250k+ deferred) |
| P1 | Perf matrix + `recommend_method` | **Done** (`flow-knn` JSONL + selector) |
| P2 | Optional default `ann-search` in more apps | Documented / selector prefers it |
| P2 | GPU Adam + pair gradients (cubecl/wgpu) | **Done** — zero-copy Burn↔cubeCL; ~4–9× vs CPU |
| P3 | GPU kNN (`ann-search` cubecl 0.10) | **Done** — `flow-knn` `gpu` feature; see `PERF_MATRIX.md` |
| P3b | Zero-copy Burn↔cubeCL Adam buffers | **Done** |

```bash
cargo bench -p flow-pacmap --features ann-search --bench pacmap_compare -- \
  pacmap_knn --warm-up-time 1 --measurement-time 6 --sample-size 10

FLOW_PACMAP_BENCH_MAX_N=100000 cargo bench -p flow-pacmap --features ann-search \
  --bench pacmap_compare -- pacmap_fit_transform
```
