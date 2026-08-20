# flow-knn

Algorithm-agnostic k-nearest-neighbour graphs for large-n flow cytometry (and other embedding pipelines).

[![crates.io](https://img.shields.io/crates/v/flow-knn.svg)](https://crates.io/crates/flow-knn)
[![docs.rs](https://docs.rs/flow-knn/badge.svg)](https://docs.rs/flow-knn)
[![MIT](https://img.shields.io/crates/l/flow-knn.svg)](LICENSE)

## Overview

**Build a `KnnGraph` once** (`compute_knn`) and reuse it across embedders — or persist it with `write_knn_graph` / `read_knn_graph`.

For **query ≠ database** search (e.g. stained events vs an AF spectral library), build a reusable `AnnIndex` with `AnnIndex::build` / `search` / `search_batch` (Exact, usearch HNSW, ann-search-rs, or GPU Exact/IVF behind `gpu`; GPU NnDescent is self-query only).

## How it Works

`recommend_method` picks a backend from \((n, d)\). Backends share one `KnnGraph` shape (indices + distances). Exact Rayon brute-force is always available; optional HNSW (usearch), ann-search-rs, kiddo, and GPU paths are feature-gated.

## Related crates

Use a sibling instead when you need:

- **PaCMAP embedding** → [`flow-pacmap`](../flow-pacmap/) — primary consumer; staged KNN before `fit_transform`
- **FCS I/O** → [`flow-fcs`](../fcs/)

## Installation

```bash
cargo add flow-knn
```

Or add it directly to your `Cargo.toml`:

```toml
[dependencies]
flow-knn = { version = "0.1.1", features = ["hnsw"] }
```

| Feature | Backend | Notes |
| ------- | ------- | ----- |
| `hnsw` (default) | [usearch](https://crates.io/crates/usearch) | C++ FFI + simsimd |
| `ann-search` | [ann-search-rs](https://crates.io/crates/ann-search-rs) | Pure-Rust HNSW |
| `gpu` | ann-search-rs + cubeCL | `GpuExact` / `GpuIvf` / `GpuNnDescent` via wgpu |
| (always) | Exact | Rayon brute-force baseline |
| `kdtree` | kiddo | Currently falls back to exact |

## API Usage

```rust
use flow_knn::{
    compute_knn, recommend_method, write_knn_graph, read_knn_graph,
    DistanceMetric, KnnGraph, KnnMethod, RecommendOpts, KnnError,
};
use std::path::Path;

fn example(data: &[f32], n: usize, d: usize) -> Result<(), KnnError> {
    let opts: RecommendOpts = RecommendOpts::default();
    let method: KnnMethod = recommend_method(n, d, &opts);
    let k: usize = 60;
    let graph: KnnGraph = compute_knn(
        data,
        n,
        d,
        k,
        &method,
        DistanceMetric::Euclidean,
    )?;

    let path = Path::new("neighbors.bin");
    write_knn_graph(path, &graph)?;
    let loaded: KnnGraph = read_knn_graph(path)?;
    Ok(())
}
```

## Performance

See [`docs/PERF_MATRIX.md`](docs/PERF_MATRIX.md). Extend timings with:

```bash
cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"
cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu
FLOW_KNN_BENCH_PRESSURE=1 cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu
```

## License

MIT
