# flow-clustering

Clustering for flow cytometry: K-means, DBSCAN, GMM, and optional PARC (graph community detection).

[![crates.io](https://img.shields.io/crates/v/flow-clustering.svg)](https://crates.io/crates/flow-clustering)
[![docs.rs](https://docs.rs/flow-clustering/badge.svg)](https://docs.rs/flow-clustering)
[![MIT](https://img.shields.io/crates/l/flow-clustering.svg)](LICENSE)

## Highlights

- **K-means / GMM / DBSCAN** — thin [`linfa`](https://crates.io/crates/linfa) wrappers with shared error types
- **PARC** *(feature `parc`)* — HNSW k-NN → local + Jaccard prune → Leiden, for large phenotypic datasets
- **Silhouette** — full and sampled cluster validation
- **Shared k-NN** — PARC accepts a prebuilt [`flow-knn`](../flow-knn/) `KnnGraph` (same pattern as PaCMAP)

## Installation

```bash
cargo add flow-clustering
# Optional PARC (pulls flow-knn + leiden-rs):
cargo add flow-clustering --features parc
```

```toml
[dependencies]
flow-clustering = { version = "0.1.2", features = ["parc"] }
```

| Feature | Description |
| ------- | ----------- |
| `kmeans` *(default)* | K-means clustering |
| `dbscan` *(default)* | Density-based spatial clustering (DBSCAN) |
| `gmm` *(default)* | Gaussian Mixture Model fitting |
| `parc` | PARC: k-NN graph prune + Leiden community detection |

## Usage

### K-Means

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
    let _ = (assignments, centroids, result.iterations, result.inertia);
    Ok(())
}
```

### DBSCAN

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
    let _ = (assignments, result.n_clusters, result.n_noise);
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
    let _ = (&result.assignments, &result.means, result.log_likelihood);
    Ok(())
}
```

### PARC

Phenotyping by Accelerated Refined Community-Partitioning. Prefer when you need
data-driven cluster counts on large cytometry / phenotype matrices. Expects
ready features (e.g. transformed markers or PCA), not raw FCS events.

Defaults follow the Python reference (`dist_std_local = 3`, etc.). Local and
Jaccard prune stages are Rayon-parallel; Leiden input edges are sorted for
seed-stable labels. HNSW comes from `flow-knn` (not hnswlib), so exact label
parity with Python is not guaranteed.

```rust
#[cfg(feature = "parc")]
use flow_clustering::{ClusteringResult, Parc, ParcConfig, ParcResult};
#[cfg(feature = "parc")]
use ndarray::Array2;

#[cfg(feature = "parc")]
fn example(data: &Array2<f64>) -> ClusteringResult<()> {
    let config: ParcConfig = ParcConfig::default();
    let result: ParcResult = Parc::fit(data, &config)?;
    // Or reuse a graph: Parc::fit_with_knn(data, &config, Some(&knn_graph))?;
    let assignments: &Vec<usize> = &result.assignments;
    let n_clusters: usize = result.n_clusters;
    let _ = (assignments, n_clusters);
    Ok(())
}
```

### Cluster validation

Silhouette scores (−1 to +1). API takes row vectors (`&[Vec<f64>]`).

```rust
use flow_clustering::{
    silhouette_scores, silhouette_scores_sampled, ClusteringResult, SilhouetteResult,
};

fn example(data: &[Vec<f64>], labels: &[usize]) -> ClusteringResult<()> {
    let scores: SilhouetteResult = silhouette_scores(data, labels)?;
    let sampled: SilhouetteResult = silhouette_scores_sampled(data, labels, 1000)?;
    let _ = (scores.mean_score, sampled.mean_score);
    Ok(())
}
```

## Performance

See [`docs/PERF_PARC.md`](docs/PERF_PARC.md) for Criterion n×d throughput,
Rayon vs sequential prune A/B, and peak RSS on Apple M5 Max.

```bash
cargo bench -p flow-clustering --bench parc_throughput --features parc
cargo run -p flow-clustering --release --example parc_rss --features parc
```

## Tests

```bash
cargo nextest run -p flow-clustering
cargo nextest run -p flow-clustering --features parc
```

## Acknowledgments

PARC algorithm: Stassen et al., *Bioinformatics* 36(9):2778–2786 (2020),
doi:[10.1093/bioinformatics/btaa042](https://doi.org/10.1093/bioinformatics/btaa042).
Reference implementation: [ShobiStassen/PARC](https://github.com/ShobiStassen/PARC) (MIT).

## License

MIT

## Related crates

- **KDE / Contour Detection** → [`flow-density`](../flow-density/)
- **Gate Geometry and GatingML** → [`flow-gates`](../gates/)
- **k-NN graphs** → [`flow-knn`](../flow-knn/)
- **Dimensionality Reduction** → [`flow-pacmap`](../flow-pacmap/)
- [`flow-fcs`](../fcs/) — FCS I/O
