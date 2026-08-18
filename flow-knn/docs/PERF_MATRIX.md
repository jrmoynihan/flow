# KNN performance matrix & method selection

Callers should pick a KNN backend based on **(n events × d dimensions)**, not a
single global default. This crate keeps an append-only dataset and a
[`recommend_method`](https://docs.rs/flow-knn) helper that interpolates it.

## Dataset

File: [`data/knn_perf_matrix.jsonl`](../data/knn_perf_matrix.jsonl)

| Field | Meaning |
|-------|---------|
| `method` | `exact`, `hnsw_usearch`, `hnsw_ann_search`, `exact_gpu`, `ivf_gpu`, `nndescent_gpu` |
| `n`, `d`, `k` | Problem size |
| `median_secs` | Wall time for build + self-query |
| `throughput_elem_per_s` | `n / median_secs` |
| `machine`, `captured_at` | Provenance |

Comment lines (`# …`) are allowed. The shipped file is compiled into the library
via `include_str!` so recommendations work without reading disk at runtime.

## Snapshot matrix (local collect_matrix, 2026-07-24)

Median seconds for build+self-query, k=60. Full Cartesian through 100k; Criterion cells remain for 250k/500k.

| n \\ d | 10 exact / usearch / ann | 15 | 20 |
|--------|--------------------------|----|----|
| 10k | 0.11 / 0.19 / **0.09** | 0.11 / 0.14 / **0.10** | 0.11 / 0.28 / **0.10** |
| 50k | 1.35 / 0.65 / **0.61** | 1.18 / 0.91 / **0.69** | 1.42 / 1.29 / **0.76** |
| 100k | 4.32 / 1.85 / **1.43** | 4.38 / 2.57 / **2.08** | 5.39 / 3.32 / **2.52** |

**Takeaway (CPU):** at FCS scales, `hnsw_ann_search` wins every cell in this matrix; exact only wins below ~5–10k (and is what `recommend_method` returns for `n ≤ 5_000`).

## Spectral / high-d gap

Shipped cells cover **d ≤ 20**. Spectral AF library search often uses **d ∈ {30, 40, 64}**. Extend with:

```bash
cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search"
```

`AnnIndex::search_batch` (query ≠ database) should be timed separately from self-query `compute_knn` when filling that matrix. GPU Exact and IVF now support that API (`--features gpu`); GPU NnDescent does not (`query_nndescent_index_gpu` needs `&mut` index).

## GPU vs CPU (Criterion `knn_cpu_vs_gpu`, 2026-07-24, k=60)

| n × d | exact_cpu | hnsw_ann_cpu | **exact_gpu** | **ivf_gpu** | nndescent_gpu |
|-------|-----------|--------------|---------------|-------------|---------------|
| 10k × 10 | 194 ms | 142 ms | **77 ms** | 155 ms | — |
| 50k × 10 | 2.20 s | 927 ms | **573 ms** | 821 ms | 1.68 s |
| 50k × 20 | 2.95 s | 1.28 s | **601 ms** | 804 ms | 3.29 s |
| 100k × 10 | 7.19 s | 2.40 s | — | **1.67 s** | 3.97 s |
| 100k × 20 | 10.1 s | 3.90 s | — | **1.75 s** | 6.93 s |

**Takeaway (GPU):** `exact_gpu` beats CPU exact and often beats HNSW through 50k; at 100k prefer **`ivf_gpu`** (~1.7 s vs ~2.4–3.9 s HNSW). NN-Descent GPU is competitive only when quality of that graph is required. Enable with `--features gpu`; set `RecommendOpts::allow_gpu = true` to let the selector consider these ids.

## Growing the matrix

```bash
# Full Cartesian: n ∈ {10k,50k,100k,250k,500k} × d ∈ {10,15,20}
cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"

# Faster iteration
FLOW_KNN_MATRIX_SMOKE=1 cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"
FLOW_KNN_MATRIX_MAX_N=50000 cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"

# Alternate output (then merge / replace shipped JSONL)
FLOW_KNN_MATRIX_OUT=/tmp/knn_matrix.jsonl cargo run -p flow-knn --release \
  --example collect_matrix --features "hnsw,ann-search,gpu"
```

`RecommendOpts::allow_gpu` (default `false`) adds GPU method ids when a WGPU
adapter is available.

After a collection run:

1. Deduplicate / keep best median per `(method,n,d,k)` if needed.
2. Copy into `data/knn_perf_matrix.jsonl` (or append new method ids).
3. Re-run `cargo test -p flow-knn --lib`.
4. Optionally refresh Criterion peer A/B via `flow-pacmap`’s `pacmap_compare`
   (`FLOW_PACMAP_BENCH_MATRIX=1` for the denser grid).

## Adding a new method

1. Implement the backend behind a Cargo feature in `flow-knn`.
2. Add a `(method_id, KnnMethod::…)` arm in `examples/collect_matrix.rs`.
3. Record cells into the JSONL with the new `method` string.
4. Teach `select.rs` (`available_method_ids` / `method_from_id`) about the id.
5. Document the id here and in `PERFORMANCE_NOTES.md`.

## Caller API

```rust
use flow_knn::{RecommendOpts, compute_knn, recommend_method, DistanceMetric};

let method = recommend_method(n, d, &RecommendOpts::default());
let graph = compute_knn(&data, n, d, k, &method, DistanceMetric::Euclidean)?;
```

`RecommendOpts::prefer_usearch` forces the usearch HNSW path (quantization /
avoid faer). `exact_ok_factor` keeps exact when it is within that factor of the
best ANN estimate (default `1.25`, and only for `n ≤ 80_000`).

Also re-exported from `flow_pacmap::knn`.

## Graph IO A/B (load + write)

Protocol: [`docs/dev/UNSAFE_MICROOPT_AB.md`](../../docs/dev/UNSAFE_MICROOPT_AB.md).
Bench: `cargo bench -p flow-knn --bench knn_graph_io`.

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| knn_graph_io_load | kept | 3.5105 s | 8.0616 ms | −99.8% | 100k×k=60 | arm64 Apple | 59807616e | 2026-08-02 | bulk `read_exact` + LE bytemuck cast; per-row `to_vec` unchanged |
| knn_graph_io_write | kept | 13.858 s | 17.631 ms | −99.9% | 100k×k=60 | arm64 Apple | 59807616e | 2026-08-02 | staged LE payloads + two `write_all`; `sync_all` kept |
| knn_graph_io_load_typed | kept | 7.8650 ms | 7.1814 ms | −6.7% | 100k×k=60 | arm64 Apple | 59807616e | 2026-08-02 | `read_exact` into `Vec<u32>`/`Vec<f32>` on LE; per-row `to_vec` unchanged |

Secondary write: 50k×60 6.919 s → 12.699 ms (−99.8%). Load secondary: 50k×60 1.806 s → 3.608 ms (−99.8%).

## Unsafe A/B: exact KNN `get_unchecked`

Bench: `cargo bench -p flow-knn --bench exact_knn_micro`.

| Item | Status | Pre median | Post median | Delta | Primary size | Machine | rustc | Date | Notes |
|------|--------|------------|-------------|-------|--------------|---------|-------|------|-------|
| exact_knn_micro | reverted | 50.570 ms | 48.512 ms | −4.1% (noise) | 10k×20 k=30 | arm64 Apple | 59807616e | 2026-08-02 | Criterion: no significant change; &lt;5% keep rule |

Secondary: 5k×20 −3.5% (noise).
