# flow-pacmap

PaCMAP dimensionality reduction for large-n flow cytometry datasets.

[![crates.io](https://img.shields.io/crates/v/flow-pacmap.svg)](https://crates.io/crates/flow-pacmap)
[![docs.rs](https://docs.rs/flow-pacmap/badge.svg)](https://docs.rs/flow-pacmap)
[![MIT](https://img.shields.io/crates/l/flow-pacmap.svg)](LICENSE)

## Overview

`flow-pacmap` is an implementation of PaCMAP (Pairwise Controlled Manifold
Approximation Projection) as described by Wang et al. 2021 (JMLR 22,
Algorithm 1). It embeds high-dimensional event data into 2D using a three-phase
pair-weighted optimization: near-neighbour attraction, mid-near attraction, and
further-pair repulsion. It is designed for large event counts typical in flow
cytometry:

- Flat `&[f32]` / `Vec<[f32; 2]>` API (no ndarray coupling)
- PCA initialization via `faer` SVD on the `d × d` covariance
- Optional HNSW KNN (`usearch`) or exact k-d tree (`kiddo`)
- Progress reporting and cooperative cancellation

## Installation

```toml
[dependencies]
flow-pacmap = "0.1.0"
```

## Features

| Feature | Description |
|---------|-------------|
| `hnsw` *(default)* | Approximate nearest neighbours via `usearch` |
| `kdtree` *(default)* | Exact k-d tree search via `kiddo` (good for low-d / moderate-n) |

## Quick start

```rust
use flow_pacmap::{fit_transform, PaCMAPConfig};

let embedding = fit_transform(
    &data, // row-major f32, length n * d
    n,
    d,
    PaCMAPConfig::default(),
    None, // progress channel
    None, // cancel token
)?;
```

## References

Wang, Y., Huang, H., Rudin, C., & Shaposhnik, Y. (2021). Understanding How
Dimension Reduction Tools Work: An Empirical Approach to Deciphering t-SNE,
UMAP, TriMap, and PaCMAP for Data Visualization.
*Journal of Machine Learning Research*, 22(201), 1–73.

## License

MIT
