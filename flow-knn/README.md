# flow-knn

Algorithm-agnostic k-nearest-neighbour graphs for large-n flow cytometry data.

Build a [`KnnGraph`] once (`compute_knn`), then pass `&KnnGraph` into PaCMAP,
UMAP, or other embedders without recomputing neighbours. Persist with
[`write_knn_graph`] / [`read_knn_graph`] for cross-run reuse.

## Backends

| Feature | Backend | Notes |
|---------|---------|-------|
| `hnsw` (default) | [usearch](https://crates.io/crates/usearch) | C++ FFI + simsimd |
| `ann-search` | [ann-search-rs](https://crates.io/crates/ann-search-rs) | Pure-Rust HNSW used by manifolds-rs |
| `gpu` | ann-search-rs + cubeCL 0.10 | `GpuExact` / `GpuIvf` / `GpuNnDescent` via wgpu |
| (always) | Exact | Rayon brute-force baseline |
| `kdtree` | kiddo | Currently falls back to exact |

## Example

```rust
use flow_knn::{compute_knn, recommend_method, DistanceMetric, RecommendOpts};

let method = recommend_method(n, d, &RecommendOpts::default());
let graph = compute_knn(
    &data, n, d, /* k */ 60,
    &method,
    DistanceMetric::Euclidean,
)?;
```

## Performance matrix

See [`docs/PERF_MATRIX.md`](docs/PERF_MATRIX.md). Extend timings with:

```bash
cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"
```

CPU vs GPU Criterion:

```bash
cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu
FLOW_KNN_BENCH_PRESSURE=1 cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu
```
