# Changelog

## [0.1.1] — 2026-07-25

### Added

- Versioned portable disk format for [`KnnGraph`]: [`write_knn_graph`] / [`read_knn_graph`]
  (`knn.bin`: magic `FKNN`, packed indices + distances).
- [`KnnError::Io`] for serialize/deserialize failures.

## [0.1.0] — 2026-07-23

### Added

- Initial `flow-knn` crate: portable [`KnnGraph`] / [`NeighborList`], [`compute_knn`].
- Backends: exact (Rayon), usearch HNSW (`hnsw` feature), optional `ann-search-rs` HNSW
  (`ann-search` feature; faer 0.23 pinned to match that stack), kiddo stub (`kdtree`).

## Unreleased

First crates.io release: portable `KnnGraph` / `compute_knn`, usearch HNSW (default), optional ann-search-rs and GPU backends, and bulk graph IO.

### Documentation

 - <csr-id-3c48e73e751a7852b0e07239540448e6ee35a0cf/> refresh crate READMEs and agent guidelines
   Keep the beads export in sync and add the Svelte MCP server to Codex config.
 - <csr-id-92e31b03dc632230809d10422be0c1062e6e9e1b/> consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate
   Rewrites READMEs across the workspace (fcs, flow-clustering,
   flow-control-detection, flow-density, flow-fcs-compress, flow-knn,
   flow-linalg, flow-pacmap, flow-peak-detection, gates, peacoqc-cli,
   peacoqc-rs, tru-ols, tru-ols-cli) to lead with install/quick-start/perf
   for downstream consumers, and adds a new flow-fcs-bench README.
   
   Adds a concrete usage example to peacoqc-py/README.md mirroring the
   docstring in peacoqc/__init__.py.
   
   Removes the superseded utils/ crate (clustering, KDE, and PCA helpers
   now live in their dedicated crates) and syncs beads issue/interaction
   export state.

### New Features

 - <csr-id-c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926/> add shared KNN crate and unify Burn/cubeCL workspace deps
   Introduce flow-knn for portable graphs and ANN backends, pin Burn 0.21 with
   cubeCL 0.10 across GPU consumers, and path-patch pastey/bit-vec for sandbox builds.

### Performance

 - <csr-id-05d8cf13511a8b064b406fd3a385eec95948b8bd/> bulk-write KnnGraph and typed load buffers
   Complete Campaign 2 A/B: keep staged write_all (−99.9%) and LE typed
   read_exact (−6.7%) at 100k×60; document reverted pacmap/compress/peaks
   scratch experiments with Criterion benches retained.
 - <csr-id-2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37/> bulk-load KnnGraph IO; record unsafe micro-opt A/B
   Keep the ~100× faster graph load via read_exact + LE bytemuck cast.
   Add Criterion benches and PERF_AB docs for the six-item A/B campaign;
   revert opts that missed the ≥5% keep rule (BSS, FCS columns/write,
   TRU-OLS SyncPtr, exact KNN / PaCMAP unchecked).

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release over the course of 16 calendar days.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Refresh crate READMEs and agent guidelines ([`3c48e73`](https://github.com/jrmoynihan/flow/commit/3c48e73e751a7852b0e07239540448e6ee35a0cf))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Bulk-write KnnGraph and typed load buffers ([`05d8cf1`](https://github.com/jrmoynihan/flow/commit/05d8cf13511a8b064b406fd3a385eec95948b8bd))
    - Bulk-load KnnGraph IO; record unsafe micro-opt A/B ([`2d3c6fc`](https://github.com/jrmoynihan/flow/commit/2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37))
    - Add shared KNN crate and unify Burn/cubeCL workspace deps ([`c9223d6`](https://github.com/jrmoynihan/flow/commit/c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926))
</details>

