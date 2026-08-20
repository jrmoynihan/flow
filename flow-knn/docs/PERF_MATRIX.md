# KNN performance matrix & method selection

Callers should pick a KNN backend based on **(n events × d dimensions)**, not a
single global default. This crate keeps an append-only dataset and a
`[recommend_method](https://docs.rs/flow-knn)` helper that interpolates it.

## Dataset

File: `[data/knn_perf_matrix.jsonl](../data/knn_perf_matrix.jsonl)`

| Field                    | Meaning                                                                             |
| ------------------------ | ----------------------------------------------------------------------------------- |
| `method`                 | `exact`, `hnsw_usearch`, `hnsw_ann_search`, `exact_gpu`, `ivf_gpu`, `nndescent_gpu` |
| `n`, `d`, `k`            | Problem size                                                                        |
| `median_secs`            | Wall time for build + self-query                                                    |
| `throughput_elem_per_s`  | `n / median_secs`                                                                   |
| `machine`, `captured_at` | Provenance                                                                          |

Comment lines (`# …`) are allowed. The shipped file is compiled into the library
via `include_str!` so recommendations work without reading disk at runtime.

## Snapshot matrix (local collect_matrix, 2026-07-24)

Median seconds for build+self-query, k=60. Full Cartesian through 100,000 events; Criterion *(n, d)* entries remain for 250,000 and 500,000 events. Column headers are detector/feature dimension *d*; row labels are event counts.

| n d  | 10 exact / usearch / ann | 15                     | 20                     |
| ---- | ------------------------ | ---------------------- | ---------------------- |
| 10k  | 0.11 / 0.19 / **0.09**   | 0.11 / 0.14 / **0.10** | 0.11 / 0.28 / **0.10** |
| 50k  | 1.35 / 0.65 / **0.61**   | 1.18 / 0.91 / **0.69** | 1.42 / 1.29 / **0.76** |
| 100k | 4.32 / 1.85 / **1.43**   | 4.38 / 2.57 / **2.08** | 5.39 / 3.32 / **2.52** |

**Takeaway (CPU):** at FCS scales, `hnsw_ann_search` wins every *(n events, d dimensions)* entry in this table; exact only wins below about 5,000–10,000 events (and is what `recommend_method` returns for `n ≤ 5_000`).

## Spectral / high-d gap

Shipped *(n, d)* entries cover **d ≤ 20**. Spectral AF library search often uses **d ∈ {30, 40, 64}**. Extend with:

```bash
cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search"
```

`AnnIndex::search_batch` (query ≠ database) should be timed separately from self-query `compute_knn` when filling that matrix. GPU Exact and IVF now support that API (`--features gpu`); GPU NnDescent does not (`query_nndescent_index_gpu` needs `&mut` index).

## GPU vs CPU (Criterion `knn_cpu_vs_gpu`, 2026-07-24, k=60)

| n × d     | exact_cpu | hnsw_ann_cpu | **exact_gpu** | **ivf_gpu** | nndescent_gpu |
| --------- | --------- | ------------ | ------------- | ----------- | ------------- |
| 10,000 events × 10 dim.  | 194 ms    | 142 ms       | **77 ms**     | 155 ms      | —             |
| 50,000 events × 10 dim.  | 2.20 s    | 927 ms       | **573 ms**    | 821 ms      | 1.68 s        |
| 50,000 events × 20 dim.  | 2.95 s    | 1.28 s       | **601 ms**    | 804 ms      | 3.29 s        |
| 100,000 events × 10 dim. | 7.19 s    | 2.40 s       | —             | **1.67 s**  | 3.97 s        |
| 100,000 events × 20 dim. | 10.1 s    | 3.90 s       | —             | **1.75 s**  | 6.93 s        |

**Takeaway (GPU):** `exact_gpu` beats CPU exact and often beats HNSW through 50,000 events; at 100,000 events prefer `ivf_gpu` (~1.7 s vs ~2.4–3.9 s HNSW). NN-Descent GPU is competitive only when quality of that graph is required. Enable with `--features gpu`; set `RecommendOpts::allow_gpu = true` to let the selector consider these ids.

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
Code: [`src/io.rs`](../src/io.rs) (`write_knn_graph`, `read_knn_graph`).

A `KnnGraph` on disk is a header plus two payloads: neighbor indices (`u32`) and distances (`f32`), each of length *n* × *k*. Primary size: 100,000 events, *k* = 60 neighbors (6,000,000 index values and 6,000,000 distances).

### Write: many small syscalls versus two bulk writes

**Problem.** The previous writer issued a [write](https://en.wikipedia.org/wiki/System_call) (or equivalent) per neighbor list or per integer: on the order of *n* × *k* kernel transitions.

**Solution.** Pack little-endian indices and distances into two buffers, then `write_all` twice (`write_knn_graph`). `sync_all` is unchanged so the file is durable.

**What changed in operation.**

- Before: thousands to millions of small writes.
- After: two payload writes after an in-memory pack.
- Difference: 13.858 s → 17.631 ms (**−99.9%**) at 100,000 events × 60 neighbors. Secondary: 50,000 events × 60 neighbors 6.919 s → 12.699 ms (**−99.8%**).

### Load: many small reads versus one `read_exact` per payload

**Problem.** The previous reader called `read` per row or per value.

**Solution.** One `read_exact` into a contiguous `u32`/`f32` buffer (little-endian [bytemuck](https://docs.rs/bytemuck) cast when the host is LE). Building each event’s `Vec` neighbor list (`to_vec` per row) is unchanged; that is not the I/O.

**What changed in operation.**

- Before: I/O cost scaled with *n* × *k* syscalls.
- After: two large reads, then in-memory slices into per-event lists.
- Difference: 3.5105 s → 8.0616 ms (**−99.8%**) at 100,000 events × 60 neighbors. Typed-buffer load (already bulk) 7.8650 ms → 7.1814 ms (**−6.7%**, kept). Secondary load: 50,000 events × 60 neighbors 1.806 s → 3.608 ms (**−99.8%**).

| What we changed | Status | Before | After | Delta | Size | Date |
|-----------------|--------|--------|-------|-------|------|------|
| Bulk `read_exact` + LE cast on load | kept | 3.5105 s | 8.0616 ms | −99.8% | 100,000 events × 60 neighbors | 2026-08-02, arm64 Apple, rustc 59807616e |
| Staged LE payloads + two `write_all` | kept | 13.858 s | 17.631 ms | −99.9% | 100,000 events × 60 neighbors | 2026-08-02, arm64 Apple, rustc 59807616e |
| `read_exact` into `Vec<u32>` / `Vec<f32>` on LE | kept | 7.8650 ms | 7.1814 ms | −6.7% | 100,000 events × 60 neighbors | 2026-08-02, arm64 Apple, rustc 59807616e |

## Unsafe A/B: exact KNN `get_unchecked`

Bench: `cargo bench -p flow-knn --bench exact_knn_micro`.

**Problem.** Exact k-NN indexes a dense *n* × *d* table many times; safe indexing theoretically pays a bounds check per access.

**Solution tried.** `get_unchecked` on those loads.

**What changed in operation.** Median 50.570 ms → 48.512 ms (**−4.1%**, under the 5% keep rule) at 10,000 events × 20 dimensions, *k* = 30. Secondary 5,000 events × 20 dimensions **−3.5%**. The arithmetic and memory traffic dominate; checks do not.

**Decision:** reverted. Safe indexing remains.

| What we changed | Status | Before | After | Delta | Size | Date |
|-----------------|--------|--------|-------|-------|------|------|
| Exact KNN `get_unchecked` | reverted | 50.570 ms | 48.512 ms | −4.1% (noise) | 10,000 events × 20 dimensions, k=30 | 2026-08-02, arm64 Apple, rustc 59807616e |

## Cost model (napkin vs measured)

Workspace protocol: [`docs/dev/PERF_PGD.md`](../../docs/dev/PERF_PGD.md). Index: [`docs/dev/PERF_GAP.md`](../../docs/dev/PERF_GAP.md).

**Exact self-query** (100,000 events × 20 dimensions, k=60): `O(n² d)` distance work ≈ 2×10¹¹ FMA-equivalents. Six P-cores at ~16 GFLOP/s `f32` → **~2 s** if the kernel were a dense GEMM. Measured exact CPU **10.1 s** (**~5×**). That is a **gather** kernel (row-wise distances), not GEMM: host random gather is ~8 ns vs 0.85 ns sequential ([`PERF_HOST.md`](../../docs/dev/PERF_HOST.md)). Vs the gather roofline this is **1–3×**. Do not `get_unchecked` (reverted). Strategy: use HNSW / `ivf_gpu` (`gpu-after-amortize`), not a tighter exact loop.

**HNSW `ann-search`** at the same size: **2.52 s**. Complexity `O(n log n)` with a large constant; this is the intended CPU path above ~5,000–10,000 events (`recommend_method`).

**Graph write** after bulk IO: 17.6 ms for ~48 MiB (100,000 × 60 × `u32`+`f32`). `memcpy` 64 MiB is 2.6 ms on this host → **~7×** including pack + `sync_all` (**3–10×**). Pre-keep was 13.9 s (**>100×**, syscalls; `bulk-syscall-io` kept). Load 8.06 ms is the same story (`typed-bulk-load`).
