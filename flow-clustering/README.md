# flow-clustering

Clustering algorithms for flow cytometry: K-means, DBSCAN, and Gaussian Mixture Models.

[![crates.io](https://img.shields.io/crates/v/flow-clustering.svg)](https://crates.io/crates/flow-clustering)
[![docs.rs](https://docs.rs/flow-clustering/badge.svg)](https://docs.rs/flow-clustering)
[![MIT](https://img.shields.io/crates/l/flow-clustering.svg)](LICENSE)

## Overview

`flow-clustering` provides:

- Unsupervised clustering algorithms (K-means, DBSCAN, GMM) commonly used in automated gating or analysis
- Clustering quality/validation metrics (Silhouette score).
- *(Future)* FlowSOM-style self-organizing maps
- *(Future)* Hierarchical clustering / dendrograms
- *(Future)* Cluster merging heuristics for automated gating

It uses thin wrappers around [`linfa`](https://crates.io/crates/linfa) clustering with shared result types (`labels`, centroids/means, optional noise for DBSCAN).

## Installation

```bash
cargo add flow-clustering
```

Or add directly to your Cargo.toml:

```toml
[dependencies]
flow-clustering = "0.1.2"
```

| Feature | Description |
| ------- | ----------- |
| `kmeans` *(default)* | K-means clustering |
| `dbscan` *(default)* | Density-based spatial clustering (DBSCAN) |
| `gmm` *(default)* | Gaussian Mixture Model fitting |

## Usage

### K-Means

Lloyd's algorithm via `linfa-clustering`. Supports row-major `ndarray::Array2` input and `fit_from_rows` for pre-separated channel vectors.

```rust
use flow_clustering::{ClusteringResult, KMeans, KMeansConfig, KMeansResult};
use ndarray::Array2;

fn example(data: &Array2<f64>) -> ClusteringResult<()> {
    let config: KMeansConfig = KMeansConfig {
        n_clusters: 3,
        max_iterations: 300,
        ..Default::default()
    };
    let result: KMeansResult = KMeans::fit(data, &config)?;
    let assignments: &Vec<usize> = &result.assignments;
    let centroids: &Array2<f64> = &result.centroids;
    let iterations: usize = result.iterations;
    let inertia: f64 = result.inertia;
    Ok(())
}
```

### DBSCAN

Density-based clustering that identifies noise points. Useful for scatter gating where populations have irregular shapes.  Expectation-maximization for Gaussian mixtures. Models multi-modal populations common in fluorescence channels.

```rust
use flow_clustering::{ClusteringResult, Dbscan, DbscanConfig, DbscanResult};
use ndarray::Array2;

fn example(data: &Array2<f64>) -> ClusteringResult<()> {
    let config: DbscanConfig = DbscanConfig {
        eps: 0.5,
        min_samples: 5,
    };
    // Note: `Dbscan::fit` currently returns `ClusteringFailed` (linfa API limitation).
    let result: DbscanResult = Dbscan::fit(data, &config)?;
    let assignments: &Vec<i32> = &result.assignments; // -1 = noise
    let n_clusters: usize = result.n_clusters;
    let n_noise: usize = result.n_noise;
    Ok(())
}
```

### Gaussian mixture model

```rust
use flow_clustering::{ClusteringResult, Gmm, GmmConfig, GmmResult};
use ndarray::Array2;

fn example(data: &Array2<f64>) -> ClusteringResult<()> {
    let config: GmmConfig = GmmConfig {
        n_components: 2,
        max_iterations: 100,
        ..Default::default()
    };
    let result: GmmResult = Gmm::fit(data, &config)?;
    let assignments: &Vec<usize> = &result.assignments;
    let means: &Array2<f64> = &result.means;
    let log_likelihood: f64 = result.log_likelihood;
    Ok(())
}
```

### Cluster Validation

Silhouette scores are a quality metric (−1 to +1) of clustering. Full O(n²) and sampled O(n·k) variants available.

```rust
use flow_clustering::{
    silhouette_scores, silhouette_scores_sampled, ClusteringResult, SilhouetteResult,
};
use ndarray::Array2;

fn example(data: &Array2<f64>, labels: &[usize]) -> ClusteringResult<()> {
    let scores: SilhouetteResult = silhouette_scores(data, labels)?;
    let mean: f64 = scores.mean_score;
    let per_point: &Vec<f64> = &scores.scores;

// For large datasets, use sampling:
    let sampled: SilhouetteResult = silhouette_scores_sampled(data, labels, 1000)?;
    Ok(())
}
```

## Tests

```bash
cargo test -p flow-clustering
```

4 unit tests covering silhouette score correctness for well-separated and overlapping clusters.

## License

MIT

## Related crates

- **KDE / Contour Detection** → [`flow-density`](../flow-density/) — density primitives used alongside clustering in gating
- **Gate Geometry and GatingML** → [`flow-gates`](../gates/) - Consumes this crate in `automated` feature
- **Dimensionality Reduction** → [`flow-pacmap`](../flow-pacmap/)
- [`flow-fcs`](../fcs/) - Reading and parsing flow cytometry standard (FCS) files
- [`tru-ols`](../tru-ols-cli/) CLI — may use clustering in control / QC workflows