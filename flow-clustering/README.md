# flow-clustering

Clustering algorithms for flow cytometry: K-means, DBSCAN, and Gaussian Mixture Models.

[![crates.io](https://img.shields.io/crates/v/flow-clustering.svg)](https://crates.io/crates/flow-clustering)
[![docs.rs](https://docs.rs/flow-clustering/badge.svg)](https://docs.rs/flow-clustering)
[![MIT](https://img.shields.io/crates/l/flow-clustering.svg)](LICENSE)

## Overview

`flow-clustering` provides unsupervised clustering algorithms commonly used in automated flow cytometry gating. Each algorithm wraps the [`linfa`](https://crates.io/crates/linfa) ecosystem with flow-cytometry-specific configuration defaults and result types.

## Features

| Feature | Description |
|---------|-------------|
| `kmeans` *(default)* | K-means clustering |
| `dbscan` *(default)* | Density-based spatial clustering (DBSCAN) |
| `gmm` *(default)* | Gaussian Mixture Model fitting |

## Public API

### K-Means

```rust
use flow_clustering::{KMeans, KMeansConfig};

let config = KMeansConfig { n_clusters: 3, max_iterations: 300, ..Default::default() };
let result = KMeans::fit(&data, &config)?;
// result.labels: Vec<usize>, result.centroids: Array2<f64>
```

### DBSCAN

```rust
use flow_clustering::{Dbscan, DbscanConfig};

let config = DbscanConfig { eps: 0.5, min_samples: 5 };
let result = Dbscan::fit(&data, &config)?;
// result.labels: Vec<Option<usize>> (None = noise)
```

### Gaussian Mixture Model

```rust
use flow_clustering::{Gmm, GmmConfig};

let config = GmmConfig { n_clusters: 2, max_iterations: 100, ..Default::default() };
let result = Gmm::fit(&data, &config)?;
// result.labels: Vec<usize>, result.means: Array2<f64>
```

### Cluster Validation

```rust
use flow_clustering::{silhouette_scores, silhouette_scores_sampled};

let scores = silhouette_scores(&data, &labels)?;
// scores.mean_score: f64, scores.per_sample: Vec<f64>

// For large datasets, use sampling:
let scores = silhouette_scores_sampled(&data, &labels, 1000)?;
```

## Algorithms

- **K-Means**: Lloyd's algorithm via `linfa-clustering`. Supports both row-major `Array2` input and convenience `fit_from_rows` for pre-separated channel vectors.
- **DBSCAN**: Density-based clustering that identifies noise points. Useful for scatter gating where populations have irregular shapes.
- **GMM**: Expectation-maximization for Gaussian mixtures. Models multi-modal populations common in fluorescence channels.
- **Silhouette scores**: Cluster quality metric (−1 to +1). Full O(n²) and sampled O(n·k) variants.

## Scope

This crate owns:

- Unsupervised clustering algorithms for cytometry event data
- Cluster quality/validation metrics
- *(Future)* FlowSOM-style self-organizing maps
- *(Future)* Hierarchical clustering / dendrograms
- *(Future)* Cluster merging heuristics for automated gating

It does **not** own: gate geometry, density estimation (see `flow-density`), FCS parsing, or visualization.

## Tests

```bash
cargo test -p flow-clustering
```

4 unit tests covering silhouette score correctness for well-separated and overlapping clusters.

## License

MIT
