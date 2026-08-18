# flow-pacmap

PaCMAP dimensionality reduction for large-n flow cytometry datasets.

[![crates.io](https://img.shields.io/crates/v/flow-pacmap.svg)](https://crates.io/crates/flow-pacmap)
[![docs.rs](https://docs.rs/flow-pacmap/badge.svg)](https://docs.rs/flow-pacmap)
[![MIT](https://img.shields.io/crates/l/flow-pacmap.svg)](LICENSE)

## Overview

`flow-pacmap` is an implementation of PaCMAP (Pairwise Controlled Manifold
Approximation Projection) as described by Wang et al. 2021 (JMLR 22,
Algorithm 1). It embeds high-dimensional event data into 2D using a three-phase
pair-weighted optimization: near-neighbor attraction, mid-near attraction, and
further-pair repulsion. It is designed for large event counts typical in flow
cytometry:

## Features

- Flat `&[f32]` / `Vec<[f32; 2]>` API (no coupling with `ndarray` library needed)
- Optional PCA initialization via `faer` SVD on the `d × d` covariance
- Optional HNSW KNN (`usearch`) or exact k-d tree (`kiddo`)
- Progress reporting and cooperative cancellation

| Feature Flag | Description |
| ------------ | ----------- |
| `hnsw` *(default)* | Forwards to `flow-knn/hnsw` (usearch) |
| `kdtree` *(default)* | Forwards to `flow-knn/kdtree` |
| `ann-search` | Forwards to `flow-knn/ann-search` |
| `cubecl` | Burn + cubeCL pair-gradient path |
| `gpu-knn` | GPU kNN via `flow-knn` (`gpu` + `ann-search`) |

## How it works

1. Optional PCA init on the \(d \times d\) covariance (faer).
2. Build or accept a [`KnnGraph`](../flow-knn/) via [`flow-knn`](../flow-knn/).
3. Construct PaCMAP pairs and optimize the three-phase loss (CPU; optional cubeCL pair gradients).

## Installation

```bash
cargo add flow-pacmap
```

Or add it directly to your `Cargo.toml`:

```toml
[dependencies]
flow-pacmap = "0.1.2"
```

## API usage

```rust
use flow_pacmap::{fit_transform, PaCMAPConfig, PaCMAPError};

fn example(data: &[f32], n: usize, d: usize) -> Result<(), PaCMAPError> {
    let config: PaCMAPConfig = PaCMAPConfig::default();
    let embedding: Vec<[f32; 2]> = fit_transform(
        data, // row-major f32, length n * d
        n,
        d,
        config,
        None, // Option<&KnnGraph> — None = compute via flow-knn
        None, // Option<Sender<PaCMAPProgress>>
        None, // Option<Arc<AtomicBool>> cancel
    )?;
    Ok(())
}
```

### Staged KNN (reuse across runs)

Compute the neighbor graph *once*, then pass it into one or more embeddings:

```rust
use flow_pacmap::{
    compute_knn, fit_transform, DistanceMetric, KnnGraph, KnnMethod, PaCMAPConfig, PaCMAPError,
};

fn example(data: &[f32], n: usize, d: usize) -> Result<(), PaCMAPError> {
    let config: PaCMAPConfig = PaCMAPConfig {
        knn_method: KnnMethod::Exact,
        ..PaCMAPConfig::default()
    };
    let k: usize = KnnGraph::required_k_for_pacmap(n, config.n_neighbors);
    let knn: KnnGraph =
        compute_knn(data, n, d, k, &config.knn_method, DistanceMetric::Euclidean)?;

    let emb_a: Vec<[f32; 2]> =
        fit_transform(data, n, d, config.clone(), Some(&knn), None, None)?;
    let emb_b: Vec<[f32; 2]> =
        fit_transform(data, n, d, config, Some(&knn), None, None)?;
    Ok(())
}
```

`KnnGraph` stores indices and distances; pair construction stays inside PaCMAP.

## Performance

```bash
cargo bench -p flow-pacmap --features ann-search --bench pacmap_compare
cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu
```

See also [`flow-knn` performance matrix](../flow-knn/docs/PERF_MATRIX.md) for neighbor-graph costs.

## References

Wang, Y., Huang, H., Rudin, C., & Shaposhnik, Y. (2021). Understanding How
Dimension Reduction Tools Work: An Empirical Approach to Deciphering t-SNE,
UMAP, TriMap, and PaCMAP for Data Visualization.
*Journal of Machine Learning Research*, 22(201), 1–73.

## License

MIT

## Related crates

- **Algorithm-agnostic nearest-neighbor graphs** → [`flow-knn`](../flow-knn/) Exact, HNSW/usearch, ann-search-rs, optional GPU accel (this crate forwards KNN feature flags and re-exports `KnnGraph` helpers)
- **FCS I/O** → [`flow-fcs`](../fcs/) → load event matrices from FCS files before embedding
- **General-purpose PCA** → tracked separately; this crate only uses `faer` SVD PCA for PaCMAP initialization
