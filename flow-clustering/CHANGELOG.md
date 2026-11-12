# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [0.3.0] — 2026-11-12

### Added

- `emeans` feature (now **default**): mini-batch EMA k-means (`EMeans`) and
  same-kernel Lloyd baseline (`Lloyd`) on row-major `f32`. Optional `emeans-gpu`
  (cubeCL assignment) and `emeans-faer` (matrix-product assignment).
- `prefer_emeans(n, d)` size heuristic (≥5k events or n×d ≥ 50k).
- `agglomerative` feature: kodama linkage with `Dendrogram::cut` and `metacluster`.
- Example `compare_emeans_lloyd`, Criterion benches, and `docs/PERF_EMEANS.md`.
- `ClusteringError::GpuUnavailable`.

### Changed

- Default features include `emeans` and **exclude** `kmeans`; linfa `KMeans` is
  opt-in (`--features kmeans`) for the `Array2<f64>` API.
- FlowSOM metaclustering uses `Lloyd` instead of linfa `KMeans`; feature `som`
  depends on `emeans`.
- `flow-fcs` ^0.6.0 for synthetic examples.

### Performance

- `emeans-gpu` cubeCL **0.11.4** port: interleaved `emeans_assign_cpu_vs_gpu`,
  Apple M5 Max, `OMP_NUM_THREADS=1`, 65,536 events × 30 dimensions, k = 256:
  **gpu** assign **7.7 ms → 4.4 ms**; **cpu** control unchanged (~7.4 ms).

### Dependencies

- `flow-knn` ^0.1.3 (PARC / AnnIndex consumers)

## v0.2.1 (2026-11-06)

<csr-id-6562b53fa71b9f285f8646c91c7a88a563696d11/>

### Chore

 - <csr-id-6562b53fa71b9f285f8646c91c7a88a563696d11/> polish 0.2.1 changelog and README install version

### New Features

 - <csr-id-e27facdaf1a14908f4e3cadba2de1440bd7a752b/> PARC progress/cancel, Python speed compare, FlowSOM benches
   Add PaCMAP-shaped progress and cancellation for PARC, same-host throughput
   vs PyPI parc (~4–8×), FlowSOM Criterion baselines, and paper-harness scaffolding.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 3 calendar days.
 - 3 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-clustering v0.2.1 ([`fdaf5d5`](https://github.com/jrmoynihan/flow/commit/fdaf5d5614b07f600cde989a63e36677f308870a))
    - Polish 0.2.1 changelog and README install version ([`6562b53`](https://github.com/jrmoynihan/flow/commit/6562b53fa71b9f285f8646c91c7a88a563696d11))
    - PARC progress/cancel, Python speed compare, FlowSOM benches ([`e27facd`](https://github.com/jrmoynihan/flow/commit/e27facdaf1a14908f4e3cadba2de1440bd7a752b))
    - Merge pull request #23 from jrmoynihan/release/clustering-autospectral-dry ([`b26691b`](https://github.com/jrmoynihan/flow/commit/b26691b458ca8b0e99e6d39b34e3b4cd700500df))
</details>

## v0.2.0 (2026-11-03)

<csr-id-964879b6c4d52736bd5dd54e0d80572f655a9557/>

### Chore

 - <csr-id-964879b6c4d52736bd5dd54e0d80572f655a9557/> polish changelogs for knn 0.1.2, clustering 0.2.0, autospectral 0.1.0
   Pin flow-knn to ^0.1.2 for AnnIndex consumers and ready scoped
   smart-release publishes in dependency order.

### New Features

 - <csr-id-9cc2e13cbc12e04a3474b00a7af9915deac4ee9e/> add batch SOM and FlowSOM metaclustering
   Train a rectangular codebook then k-means the nodes so AF discovery can
   use a FlowSOM backend without gating the rest of Phase 1. Bump to 0.2.0
   and pin path dependents (flow-gates, tru-ols-cli) to match.
 - <csr-id-b953a8b210ccd5fe5ecad413a0dc8cbae1afc5c6/> :sparkles: add PARC clustering algorithm
   - Added optional `parc` feature for PARC clustering, integrating HNSW k-NN via `flow-knn` and Leiden community detection. - Updated README and CHANGELOG to reflect new feature and usage instructions. - Modified Cargo.toml to include dependencies for `flow-knn` and `leiden-rs`. - Enhanced clustering module to support PARC functionality, including pruning and community detection.

### Performance

 - <csr-id-c33e8b220b5108b6be7b14493eb63e3c6fcb9ba2/> add PARC benches and Exact vs HNSW baselines
   Measure e2e Exact↔HNSW and Rayon prune A/B on M5 Max; expose parallel_prune
   for isolation. At ≥50k events HNSW wins clearly and remains the default.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-clustering v0.2.0 ([`7ec31b1`](https://github.com/jrmoynihan/flow/commit/7ec31b1bbd1b297a31e6fcaed4b07e8a0bc82621))
    - Polish changelogs for knn 0.1.2, clustering 0.2.0, autospectral 0.1.0 ([`964879b`](https://github.com/jrmoynihan/flow/commit/964879b6c4d52736bd5dd54e0d80572f655a9557))
    - Merge pull request #22 from jrmoynihan/feature/flow-autospectral ([`35007ba`](https://github.com/jrmoynihan/flow/commit/35007ba7841c164cf8e4ffae220a782dcb87d7ce))
    - Add batch SOM and FlowSOM metaclustering ([`9cc2e13`](https://github.com/jrmoynihan/flow/commit/9cc2e13cbc12e04a3474b00a7af9915deac4ee9e))
    - Merge branch 'feature/parc-clustering' ([`baf4f01`](https://github.com/jrmoynihan/flow/commit/baf4f01043a91470566d16c5d43fc815da17379e))
    - Add PARC benches and Exact vs HNSW baselines ([`c33e8b2`](https://github.com/jrmoynihan/flow/commit/c33e8b220b5108b6be7b14493eb63e3c6fcb9ba2))
    - :sparkles: add PARC clustering algorithm ([`b953a8b`](https://github.com/jrmoynihan/flow/commit/b953a8b210ccd5fe5ecad413a0dc8cbae1afc5c6))
</details>

## v0.1.2 (2026-08-18)

### Documentation

 - <csr-id-3b118cede174e11c18a64c25e913416654b8c26e/> polish release notes and README install versions
   Add first-publish summaries for unpublished crates and bump install examples to the versions cargo-smart-release will publish.
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

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 13 calendar days.
 - 99 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs-compress v0.1.4, flow-linalg v0.1.3, flow-fcs v0.5.2, flow-knn v0.1.1, flow-dimensional-reduction v0.1.0, flow-pacmap v0.1.2, flow-density v0.1.2, flow-plots v0.3.3, flow-clustering v0.1.2, flow-gates v0.5.0, flow-peak-detection v0.1.0, flow-control-detection v0.1.0, flow-tru-ols v0.2.0, peacoqc-rs v0.3.3, peacoqc-cli v0.3.0 ([`63fb593`](https://github.com/jrmoynihan/flow/commit/63fb5937185fba6f342ae3ae5b960a4df4c2bc72))
    - Polish release notes and README install versions ([`3b118ce`](https://github.com/jrmoynihan/flow/commit/3b118cede174e11c18a64c25e913416654b8c26e))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
</details>

## v0.1.1 (2026-05-11)

<csr-id-90e3ee26926e8df26e30fdf12ac25aae632d1dd9/>

### Chore

 - <csr-id-90e3ee26926e8df26e30fdf12ac25aae632d1dd9/> bump to 0.1.1 with readme field for crates.io display

### Documentation

 - <csr-id-e14b4799997458dba9110f4723e88dc50c8dce5b/> add README.md to flow-linalg, flow-density, flow-clustering, flow-fcs-compress
   Each README describes the crate's purpose, public API, algorithms,
   scope boundaries, features, tests, and benchmarks for prospective
   users and contributors.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.1, flow-density v0.1.1, flow-clustering v0.1.1, flow-fcs-compress v0.1.1 ([`966d22a`](https://github.com/jrmoynihan/flow/commit/966d22ae4fbdd6114dc3862d45648fce7ebf53cc))
    - Bump to 0.1.1 with readme field for crates.io display ([`90e3ee2`](https://github.com/jrmoynihan/flow/commit/90e3ee26926e8df26e30fdf12ac25aae632d1dd9))
    - Add README.md to flow-linalg, flow-density, flow-clustering, flow-fcs-compress ([`e14b479`](https://github.com/jrmoynihan/flow/commit/e14b4799997458dba9110f4723e88dc50c8dce5b))
    - Merge branch 'feat/flow-fcs-compress' ([`ef239b2`](https://github.com/jrmoynihan/flow/commit/ef239b24dbacfabc1e68dfa5f4dc8baa49f9704a))
    - Merge pull request #20 from jrmoynihan/feat/flow-fcs-compress ([`f953bc5`](https://github.com/jrmoynihan/flow/commit/f953bc5df8f6978e3fe511538cb2943730a35eff))
</details>

## v0.1.0 (2026-05-11)

<csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/>

### Chore

 - <csr-id-74956f94c544d1fa83f6fffbb18e2d4f5e6072ff/> bump flow-fcs to 0.4.0, add publish metadata to new crates
   - flow-fcs 0.3.0 → 0.4.0 (new compensation feature + public API)
   - flow-linalg, flow-density, flow-clustering: add repository field
     and smart-release scripts for first publish
   - Update all workspace consumers to ^0.4.0

### New Features

 - <csr-id-fe642f65aa7d03fac6d688f4598143d8c2955137/> split flow-utils into flow-density + flow-clustering crates
   flow-utils bundled unrelated concerns (KDE, clustering, PCA). Split into
   focused crates so consumers only pull what they need. flow-utils removed
   from workspace members; existing code updated to import from new crates.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-linalg v0.1.0, flow-density v0.1.0, flow-clustering v0.1.0, flow-fcs-compress v0.1.0, flow-fcs v0.4.0 ([`e8c908e`](https://github.com/jrmoynihan/flow/commit/e8c908ef92fb68b8e2d01d3c1e8d6a294c8c6bda))
    - Bump flow-fcs to 0.4.0, add publish metadata to new crates ([`74956f9`](https://github.com/jrmoynihan/flow/commit/74956f94c544d1fa83f6fffbb18e2d4f5e6072ff))
    - Split flow-utils into flow-density + flow-clustering crates ([`fe642f6`](https://github.com/jrmoynihan/flow/commit/fe642f65aa7d03fac6d688f4598143d8c2955137))
</details>

