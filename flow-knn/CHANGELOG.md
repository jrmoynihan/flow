# Changelog

## Unreleased

### Performance

- Workspace bump to cubeCL **0.11.4** / Burn **0.22.1** / ann-search-rs **0.12.2** /
  wgpu **30** (was 0.10 / 0.21 / 0.4.4 on `d21d0ff`). Interleaved Criterion on Apple
  M5 Max, `OMP_NUM_THREADS=1`, `hnsw,ann-search,gpu`: self-query **exact_gpu** at
  50k×10 **410 ms → 284 ms**; **ivf_gpu** at 100k×10 **1.06 s → 108 ms**;
  **exact_cpu** control at 50k×10 unchanged (~1.08 s). The **hnsw_ann_cpu** arm also
  changed (**556 ms → 309 ms**) because ann-search-rs **0.4.4 → 0.12.2** on HEAD,
  not from cubeCL alone. `collect_matrix` appended rows to
  `data/knn_perf_matrix.jsonl` (smoke plus `FLOW_KNN_MATRIX_MAX_N=50000` on this host).

## [0.1.2] — 2026-11-03

### Added

- [`AnnIndex`] / [`build_ann_index`] for query-vs-library nearest-neighbour search
  (Exact, usearch HNSW, ann-search-rs; GPU Exact/IVF behind `gpu`). Used by
  stained-vs-AF-library matching in `flow-autospectral`.

### Performance

- Keep bulk [`KnnGraph`] IO: staged `write_all` and little-endian typed
  `read_exact` load buffers (see crate `docs/PERF_AB.md`).

### Documentation

- README covers `AnnIndex` for query ≠ database workloads alongside self-query
  [`compute_knn`].

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

## v0.1.2 (2026-11-03)

<csr-id-964879b6c4d52736bd5dd54e0d80572f655a9557/>

### Chore

 - <csr-id-964879b6c4d52736bd5dd54e0d80572f655a9557/> polish changelogs for knn 0.1.2, clustering 0.2.0, autospectral 0.1.0
   Pin flow-knn to ^0.1.2 for AnnIndex consumers and ready scoped
   smart-release publishes in dependency order.

### Documentation

 - <csr-id-e33bbe75d3777e0b8f7c8f5ebe0d71259ab9b4ef/> add performance-guided development map and host calibrator
   Give agents a napkin-math floor, this-machine snapshot, gap index, and
   retry shortlist so hot-path work starts from hardware cost rather than
   another 5% unsafe A/B.

### New Features

 - <csr-id-560f8fca8b1e67ed0c6ad0df057054c1ad8794fe/> factor-once OLS match and n×K×d benches
   Reuse per-AF Gram/QR factors and event-level Rayon so residual matching
   scales with K; add env-gated Criterion axes and GPU query-vs-library
   AnnIndex (Exact/IVF) so keep/skip is measured rather than guessed.
 - <csr-id-f6bccfbfccca0c76ddf9bb39c1a5091c56875967/> add AF library discovery and matching crate
   Introduce flow-autospectral for multi-AF spectral library discovery from
   unstained controls and per-event matching against stained data. Extend
   flow-knn with a reusable AnnIndex API (Exact, usearch, ann-search-rs)
   for query-vs-database search used by spectral AF library matching.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-knn v0.1.2 ([`00e1ffc`](https://github.com/jrmoynihan/flow/commit/00e1ffcf73ae1ea33e15670704d3f38c4a50e432))
    - Polish changelogs for knn 0.1.2, clustering 0.2.0, autospectral 0.1.0 ([`964879b`](https://github.com/jrmoynihan/flow/commit/964879b6c4d52736bd5dd54e0d80572f655a9557))
    - Merge pull request #22 from jrmoynihan/feature/flow-autospectral ([`35007ba`](https://github.com/jrmoynihan/flow/commit/35007ba7841c164cf8e4ffae220a782dcb87d7ce))
    - Add performance-guided development map and host calibrator ([`e33bbe7`](https://github.com/jrmoynihan/flow/commit/e33bbe75d3777e0b8f7c8f5ebe0d71259ab9b4ef))
    - Factor-once OLS match and n×K×d benches ([`560f8fc`](https://github.com/jrmoynihan/flow/commit/560f8fca8b1e67ed0c6ad0df057054c1ad8794fe))
    - Add AF library discovery and matching crate ([`f6bccfb`](https://github.com/jrmoynihan/flow/commit/f6bccfbfccca0c76ddf9bb39c1a5091c56875967))
</details>

## v0.1.1 (2026-08-18)

### Documentation

 - <csr-id-8953fb69a59e69ae363e1b2d1ef28ce5f67289ee/> drop user Unreleased summaries that break smart-release
   Leave Keep-a-Changelog sections cargo-smart-release already knows how to rewrite.
 - <csr-id-2af7c38d871243877def4efd66ae004c3af6ce5f/> put changelog summaries under Keep-a-Changelog headings
   cargo-smart-release rejects free-form Unreleased paragraphs that are not a known section.
 - <csr-id-3b118cede174e11c18a64c25e913416654b8c26e/> polish release notes and README install versions
   Add first-publish summaries for unpublished crates and bump install examples to the versions cargo-smart-release will publish.
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

 - 8 commits contributed to the release over the course of 16 calendar days.
 - 8 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Drop user Unreleased summaries that break smart-release ([`8953fb6`](https://github.com/jrmoynihan/flow/commit/8953fb69a59e69ae363e1b2d1ef28ce5f67289ee))
    - Put changelog summaries under Keep-a-Changelog headings ([`2af7c38`](https://github.com/jrmoynihan/flow/commit/2af7c38d871243877def4efd66ae004c3af6ce5f))
    - Polish release notes and README install versions ([`3b118ce`](https://github.com/jrmoynihan/flow/commit/3b118cede174e11c18a64c25e913416654b8c26e))
    - Refresh crate READMEs and agent guidelines ([`3c48e73`](https://github.com/jrmoynihan/flow/commit/3c48e73e751a7852b0e07239540448e6ee35a0cf))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Bulk-write KnnGraph and typed load buffers ([`05d8cf1`](https://github.com/jrmoynihan/flow/commit/05d8cf13511a8b064b406fd3a385eec95948b8bd))
    - Bulk-load KnnGraph IO; record unsafe micro-opt A/B ([`2d3c6fc`](https://github.com/jrmoynihan/flow/commit/2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37))
    - Add shared KNN crate and unify Burn/cubeCL workspace deps ([`c9223d6`](https://github.com/jrmoynihan/flow/commit/c9223d6d29e19c5d2a4513514c0cdaf8d3fe2926))
</details>

