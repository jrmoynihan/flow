# flow-clustering

Clustering for flow cytometry: K-means, DBSCAN, GMM, SOM / FlowSOM, optional PARC, experimental e-means, and agglomerative metaclustering.

[![crates.io](https://img.shields.io/crates/v/flow-clustering.svg)](https://crates.io/crates/flow-clustering)
[![docs.rs](https://docs.rs/flow-clustering/badge.svg)](https://docs.rs/flow-clustering)
[![MIT](https://img.shields.io/crates/l/flow-clustering.svg)](LICENSE)

## Highlights

- **K-means / GMM / DBSCAN** — thin [`linfa`](https://crates.io/crates/linfa) wrappers with shared error types
- **SOM / FlowSOM** *(feature `som`, default)* — batch SOM codebook, then k-means metaclusters
- **PARC** *(feature `parc`)* — HNSW k-NN → local + Jaccard prune → Leiden, for large phenotypic datasets
- **Faster than Python PARC** — same-host clustering-only wall times are about **4–8×** the
  [ShobiStassen/PARC](https://github.com/ShobiStassen/PARC) reference on 5k–100k events
  (see [Performance](#performance))
- **Silhouette** — full and sampled cluster validation
- **Shared k-NN** — PARC accepts a prebuilt [`flow-knn`](../flow-knn/) `KnnGraph` (same pattern as PaCMAP)
- **Progress / cancel** — optional PaCMAP-shaped stage events and cooperative cancellation
- **e-means** *(feature `emeans`, default)* — preferred row-major `f32` k-means (Bessa et al., arXiv:2611.09501); about **6–13×** faster than same-kernel Lloyd@25 at 1M events with matched held-out MSE; wins from ~5k events (see [Performance](#performance))
- **Agglomerative** *(feature `agglomerative`)* — kodama linkage for merging over-clusters into metaclusters

## Installation

```bash
cargo add flow-clustering
# Optional PARC (pulls flow-knn + leiden-rs):
cargo add flow-clustering --features parc
# Optional agglomerative metaclustering:
cargo add flow-clustering --features agglomerative
```

```toml
[dependencies]
flow-clustering = { version = "0.3.0", features = ["parc"] }
```

| Feature | Description |
| ------- | ----------- |
| `emeans` *(default)* | Row-major `f32` EMA k-means + Lloyd baseline (preferred; 3 epochs, CPU) |
| `dbscan` *(default)* | Density-based spatial clustering (DBSCAN) |
| `gmm` *(default)* | Gaussian Mixture Model fitting |
| `som` *(default)* | Batch SOM and FlowSOM (codebook metaclusters via Lloyd; pulls `emeans`) |
| `kmeans` | Optional linfa `Array2<f64>` K-means (legacy) |
| `parc` | PARC: k-NN graph prune + Leiden community detection |
| `emeans-faer` | Optional faer product assignment kernel (A/B vs CPU loop) |
| `emeans-gpu` | cubeCL nearest-centroid; **not recommended by default** (see PERF_EMEANS) |
| `agglomerative` | Average/complete/single/ward linkage (kodama) |

## Usage

### K-Means (legacy linfa; feature `kmeans`)

Prefer [`EMeans`](#e-means-default) for new code. The linfa wrapper stays
available behind `--features kmeans` for the `Array2<f64>` API.

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

### FlowSOM

Batch self-organizing map, then k-means on the codebook (metaclusters). Event labels come from each event's best-matching unit.

```rust
use flow_clustering::{ClusteringResult, FlowSom, FlowSomConfig, SomConfig};
use ndarray::Array2;

fn example(data: &Array2<f64>) -> ClusteringResult<()> {
    let config = FlowSomConfig {
        som: SomConfig {
            width: 10,
            height: 10,
            n_epochs: 10,
            ..SomConfig::default()
        },
        n_metaclusters: 8,
        ..FlowSomConfig::default()
    };
    let result = FlowSom::fit(data, &config)?;
    let labels: &Vec<usize> = &result.event_metaclusters;
    let centroids: &Array2<f64> = &result.metacluster_centroids;
    let _ = (labels, centroids);
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
    // Optional progress + cancel (PaCMAP-shaped):
    // Parc::fit_with_knn(data, &config, None, Some(tx), Some(cancel))?;
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

### e-means (default)

Preferred row-major `f32` mini-batch EMA k-means for practical event counts.
Defaults: 3 epochs, CPU assignment, last-epoch-mean finalisation, published
scaling laws for batch size and peak step. Use [`prefer_emeans`] when choosing
between e-means and same-kernel Lloyd (true for `n ≥ 5k` or `n × d ≥ 50k`).

```rust
use flow_clustering::{ClusteringResult, EMeans, EMeansConfig, EMeansResult, prefer_emeans};

fn example(data: &[f32], n: usize, d: usize) -> ClusteringResult<()> {
    let _use_emeans = prefer_emeans(n, d); // true for n ≥ 5k or n×d ≥ 50k
    let config: EMeansConfig = EMeansConfig {
        n_clusters: 256,
        epochs: 3, // default
        seed: Some(42),
        ..Default::default()
    };
    let result: EMeansResult = EMeans::fit(data, n, d, &config)?;
    let _ = (
        &result.centroids,
        &result.assignments,
        result.inertia,
        result.batch_size,
        result.alpha0,
    );
    Ok(())
}
```

`Lloyd` (same feature) is the fair baseline and is what FlowSOM uses for
codebook metaclusters. Linfa `KMeans` is optional (`kmeans` feature) for the
`Array2<f64>` API. `Agglomerative` / `metacluster` (feature `agglomerative`)
merge over-clusters.

## Performance

### e-means vs Lloyd (headline)

Same-kernel assignment, synthetic unequal Gaussian populations, Apple M5 Max
(2026-11-12). Lower-bound grid: e-means wins from about **5k events** (or
`n × d ≥ 50k`); at n ≥ 10k it won 26/27 cells. At 1M events it matches
Lloyd@25 held-out MSE in 1–2 epochs with about **6–13×** less wall time.
Details: [`docs/PERF_EMEANS.md`](docs/PERF_EMEANS.md).

| n × d × k | Epochs to match | Speedup vs Lloyd@25 |
|-----------|-----------------|---------------------|
| 10k × 30 × 256 | 2 | ~2.9× |
| 50k × 30 × 256 | 1 | ~6× |
| 1M × 30 × 256 | 1 | ~10× |
| 1M × 60 × 2048 | 1 | ~13× |

GPU assignment wins only for large batches (e.g. 16k events × 2048 clusters);
keep CPU as the default.

### vs Python PARC (headline)

Same-host clustering-only wall times vs PyPI `parc` 0.40 (ShobiStassen reference),
Apple M5 Max, `rustc` 1.98.1, interleaved reps. Full sample:
[`docs/throughput_vs_python_sample.md`](docs/throughput_vs_python_sample.md).
Harness: [`docs/comparison-with-python.md`](docs/comparison-with-python.md).

| n × d | Rust median | Python median | Speedup |
|-------|-------------|---------------|---------|
| 5k × 20 | 0.40 s | 1.54 s | **3.9×** |
| 20k × 20 | 0.81 s | 3.54 s | **4.4×** |
| 50k × 20 | 1.29 s | 10.1 s | **7.8×** |
| 100k × 20 | 2.70 s | 19.5 s | **7.2×** |

Throughput claim is wall-clock only; HNSW backends differ (`flow-knn` vs
`hnswlib`), so cluster counts need not match.

### PARC internals (Exact vs HNSW)

Synthetic Criterion on Apple M5 Max (2026-08-18). Full tables, Rayon A/B, peak
RSS: [`docs/PERF_PARC.md`](docs/PERF_PARC.md).

| Size | HNSW median | HNSW evt/s | vs Exact |
|------|-------------|------------|----------|
| 5k × 20, k=20 | 212 ms | 23.6k | 0.96× (Exact slightly faster) |
| 20k × 20, k=30 | 690 ms | 29.0k | 0.93× |
| 50k × 20, k=30 | 943 ms | 53.0k | **1.63×** |
| 100k × 20, k=30 | 1.65 s | 60.4k | **2.75×** |

HNSW is the production default from ~50k events upward. Peak RSS on Exact k-NN
is roughly 0.2–0.4 GiB at 50k–100k events (Rayon prune adds ~10–16%).

### FlowSOM

Synthetic Criterion on Apple M5 Max (2026-11-06), 10×10 SOM / 10 metaclusters.
Details: [`docs/PERF_FLOWSOM.md`](docs/PERF_FLOWSOM.md).

| Case | Median | Throughput |
|------|--------|------------|
| 20k × 10 | 238 ms | 83.9k evt/s |
| 50k × 20 | 858 ms | 58.3k evt/s |
| 100k × 20 | 1.68 s | 59.5k evt/s |
| 100k × 30 | 2.36 s | 42.3k evt/s |

### Reproduce

```bash
# Same-host vs Python PARC (needs venv with parc; see comparison-with-python.md)
PARC_PYTHON=... cargo run -p flow-clustering --release --example compare_with_python \
  --features parc -- --warmup 1 --reps 3

cargo bench -p flow-clustering --bench parc_throughput --features parc
cargo bench -p flow-clustering --bench flowsom_throughput
cargo run -p flow-clustering --release --example parc_rss --features parc

# e-means quality / lower-bound grids + assignment crossover
cargo run -p flow-clustering --example compare_emeans_lloyd --release
FLOW_CLUSTERING_BENCH_LOWER_BOUND=1 cargo run -p flow-clustering --example compare_emeans_lloyd --release
cargo bench -p flow-clustering --bench emeans_assign_cpu_vs_gpu --features emeans-gpu
```

## Tests

```bash
cargo nextest run -p flow-clustering --lib
cargo nextest run -p flow-clustering --features parc --lib
cargo nextest run -p flow-clustering --features agglomerative --lib
```

Unit tests cover silhouette scores, SOM / FlowSOM recovery of two well-separated
blobs, e-means / Lloyd on two blobs, and average-linkage cut parity.

## Acknowledgments

PARC algorithm: Stassen et al., *Bioinformatics* 36(9):2778–2786 (2020),
doi:[10.1093/bioinformatics/btaa042](https://doi.org/10.1093/bioinformatics/btaa042).
Reference implementation: [ShobiStassen/PARC](https://github.com/ShobiStassen/PARC) (MIT).

Van Gassen S *et al.* (2015). FlowSOM. *Cytometry Part A*, 87(7), 636–645.

e-means: Bessa, Mazaré, Szilvasy, Touvron, Picard, Douze and Jégou,
[arXiv:2611.09501](https://arxiv.org/abs/2611.09501).

## License

MIT

## Related crates

- **KDE / Contour Detection** → [`flow-density`](../flow-density/)
- **Gate Geometry and GatingML** → [`flow-gates`](../gates/)
- **k-NN graphs** → [`flow-knn`](../flow-knn/)
- **Dimensionality Reduction** → [`flow-pacmap`](../flow-pacmap/)
- [`flow-fcs`](../fcs/) — FCS I/O
- [`flow-autospectral`](../flow-autospectral/) — uses GMM / k-means / FlowSOM for AF libraries
