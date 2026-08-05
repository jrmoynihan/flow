<img width="1245" height="705" alt="FerrisAtCytometer" src="https://github.com/user-attachments/assets/87278ae7-a1db-4b22-8fe7-557d1e84d25e" />

# flow

Flow cytometry analysis tools, _oxidized_.  The aim of this workspace is to leverage the blazing-fast speed, memory and type safety, and fearless concurrency of the Rust language to scale-up to modern flow cytometry workflows requiring millions of events without breaking a sweat.  Biological data can be unpredictable; the tools to analyze it shouldn't be.

The workspace includes libaries for:

- Reading FCS files
- Creating plots
- Working with gates
- QC'ing data
- Performing unmixing

> **⚠️ Under Construction**: This workspace is actively under development. APIs may change, and some features may be incomplete. Use with caution in production environments.
>

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Repository](https://img.shields.io/badge/github-jrmoynihan%2Fflow-blue)](https://github.com/jrmoynihan/flow)

## Crate Overview

This workspace contains multiple crates for flow cytometry analysis:

### I/O

| Package | Path | Description |
| ------- | ---- | ----------- |
| [`flow-fcs`](fcs/) | `fcs/` | A comprehensive, type-safe API for reading, parsing, writing, and manipulating FCS files. Built on top of [Polars](https://www.pola.rs/) for efficient columnar data operations, with zero-copy data access, and SIMD-accelerated operations. |
| [`flow-fcs-compress`](flow-fcs-compress/) | `flow-fcs-compress/` | Codecs for compression and decompression of FCS files, a novel `.fcz` compressed format |
| [`flow-fcs-bench`](flow-fcs-bench/) | `flow-fcs-bench/` | Synthetic / file harness for compress throughput (unpublished) |

### Shared Primitives

| Package | Path | Description |
| ------- | ---- | ----------- |
| [`flow-linalg`](flow-linalg/) | `flow-linalg/` | Pure-Rust linear algebra primitives for flow cytometry, built on [`faer`](https://crates.io/crates/faer). Spillover compensation, matrix condition / hotspot matrices |
| [`flow-density`](flow-density/) | `flow-density/` | FFT-accelerated KDE (1D/2D) for use with gating, plotting, and high-dimensional reduction and clustering |
| [`flow-clustering`](flow-clustering/) | `flow-clustering/` | K-means, DBSCAN, GMM for automated gating |
| [`flow-knn`](flow-knn/) | `flow-knn/` | Reusable `KnnGraph`: Exact / HNSW / GPU-acceleration (optional) |
| [`flow-peak-detection`](flow-peak-detection/) | `flow-peak-detection/` | Histogram peak isolation, e.g. for single-stain medians |
| [`flow-control-detection`](flow-control-detection/) | `flow-control-detection/` | Filename heuristics for unstained / single-stain roles |

### Analysis

| Package | Path | Description |
| ------- | ---- | ----------- |
| [`flow-gates`](gates/) | `gates/` | Manual + GatingML gates, hierarchies, automated scatter/doublets/debris |
| [`flow-plots`](plots/) | `plots/` | Density, scatter, histogram, and spectral plot rendering to static images |
| [`peacoqc-rs`](peacoqc-rs/) | `peacoqc-rs/` | PeacoQC time-bin QC (IT / MAD / margins / doublets) |
| [`flow-tru-ols`](tru-ols/) | `tru-ols/` | Truncated re-unmixing OLS core + quality metrics |
| [`flow-pacmap`](flow-pacmap/) | `flow-pacmap/` | PaCMAP embedding (KNN via `flow-knn`) |

### Apps and Bindings

| Package | Path | Description |
| ------- | ---- | ----- |
| [`peacoqc-cli`](peacoqc-cli/) | `peacoqc-cli/` | CLI for performing PeacoQC on FCS files |
| [`tru-ols-cli`](tru-ols-cli/) | `tru-ols-cli/` | CLI for performing TRU-OLS spectral unmixing on FCS files |
| [`peacoqc-py`](peacoqc-py/) | `peacoqc-py/` | Python bindings for `peacoqc-rs` |

## Building and Testing

```bash
cargo check --workspace
cargo test --workspace --lib --bins
cargo clippy --workspace
```

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgments

- [Polars](https://www.pola.rs/) for columnar storage of events/parameters
- [faer](https://github.com/sarah-ek/faer) Linear algebra
- [PeacoQC](https://github.com/saeyslab/PeacoQC) (Saeys lab) for the QC algorithm reimplemented in `peacoqc-rs`
- [TRU-OLS](https://github.com/De-Novo-Research/TRU-OLS) (DeNovo Research) for the unmixing algorithm.

## Related Projects

- [Polars](https://www.pola.rs/) — High-performance dataframe library
- [faer](https://github.com/sarah-ek/faer) — Pure-Rust linear algebra library
- [PeacoQC](https://github.com/saeyslab/PeacoQC) — Peak-based selection of high quality cytometry data
- [ISAC FCS](https://flowcyt.sourceforge.net/) — Flow Cytometry Standard

## Contributing

Contributions are welcome!  Please submit an Issue or Pull Request to the repository!
