# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- KNN is staged through `flow-knn` (`fit_transform` takes `Option<&KnnGraph>`); PCA init uses `flow-dimensional-reduction`. Optional cubeCL pair-gradient optimization.

### New Features

 - Staged KNN API: public `KnnGraph`, `compute_knn` → `KnnGraph`, and
   optional `knn: Option<&KnnGraph>` on `fit_transform` so callers can build
   the neighbour graph once and reuse it across configs.
 - Depend on workspace crate `flow-knn` for algorithm-agnostic KNN (usearch
   HNSW by default; optional `ann-search` feature for the ann-search-rs stack).
 - <csr-id-55b077057b98e53e2414b3b42cca18efd8b14b1c/> stage KNN via flow-knn and add cubeCL optimize path
   Reuse portable KnnGraph across embeddings, forward ANN features to flow-knn,
   and add Burn/cubeCL CSR pair-gradient optimization with Criterion harnesses.

### Breaking Changes

 - `fit_transform` gains a `knn: Option<&KnnGraph>` argument after `config`
   (pass `None` for previous one-shot behaviour).
 - `compute_knn` now returns `Result<KnnGraph, PaCMAPError>` instead of
   `Result<Vec<NeighborList>, PaCMAPError>`.
 - Cargo features `hnsw` / `kdtree` now forward to `flow-knn`; add
   `ann-search` to enable the ann-search-rs HNSW backend.

### Refactor

 - <csr-id-02ab29154270fb5e288287c36652ed8c30b6b5d5/> use shared flow-dimensional-reduction PCA
   pca_init becomes a two-component specialization of the shared covariance
   PCA. Public signature unchanged; the axis-alignment guard test and the
   pacmap_compare bench are unaffected.
   
   Fixes f32 covariance accumulation: the shared implementation accumulates
   in f64, which matters at the n~1e7 scale this path runs at.

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

### Documentation

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
 - <csr-id-6d051ac5a34e63a997ca85c8819b790c0d161c8a/> scrub changelog neutralize wording after release rewrite
   smart-release reintroduced commit-message phrasing about neutralizing
   attribution; restore neutral release notes and drop the misplaced
   pacmap prep entry from linalg/gates changelogs.

### Chore

 - <csr-id-9ee153e04507d454f9509a4d2b2a2b3ffe9db17b/> add crates.io metadata for unpublished crates
   Pin flow-pacmap's flow-knn path dependency to a version so cargo publish can succeed.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 26 calendar days.
 - 26 days passed between releases.
 - 7 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Add crates.io metadata for unpublished crates ([`9ee153e`](https://github.com/jrmoynihan/flow/commit/9ee153e04507d454f9509a4d2b2a2b3ffe9db17b))
    - Merge branch 'worktree-pr1-pca-typestate' ([`2cb86fb`](https://github.com/jrmoynihan/flow/commit/2cb86fbea24f6605f550e5af9830c77f8710f17b))
    - Consumer-first README pass across crates, add peacoqc-py usage example, remove legacy utils crate ([`92e31b0`](https://github.com/jrmoynihan/flow/commit/92e31b03dc632230809d10422be0c1062e6e9e1b))
    - Use shared flow-dimensional-reduction PCA ([`02ab291`](https://github.com/jrmoynihan/flow/commit/02ab29154270fb5e288287c36652ed8c30b6b5d5))
    - Bulk-write KnnGraph and typed load buffers ([`05d8cf1`](https://github.com/jrmoynihan/flow/commit/05d8cf13511a8b064b406fd3a385eec95948b8bd))
    - Bulk-load KnnGraph IO; record unsafe micro-opt A/B ([`2d3c6fc`](https://github.com/jrmoynihan/flow/commit/2d3c6fc30fb8bdcc2ddb2c0ca638766e68401e37))
    - Stage KNN via flow-knn and add cubeCL optimize path ([`55b0770`](https://github.com/jrmoynihan/flow/commit/55b077057b98e53e2414b3b42cca18efd8b14b1c))
    - Scrub changelog neutralize wording after release rewrite ([`6d051ac`](https://github.com/jrmoynihan/flow/commit/6d051ac5a34e63a997ca85c8819b790c0d161c8a))
</details>

## 0.1.1 (2026-07-22)

<csr-id-ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4/>
<csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/>

### New Features

 - <csr-id-5df4317832bb258a0adb6a0989a2d01d7ca04404/> add PaCMAP dimensionality reduction crate
   Large-n embedding with faer PCA and optional HNSW/k-d tree KNN for flow cytometry datasets.

### Chore

 - <csr-id-ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4/> prepare 0.1.1 release with Wang et al. attribution
   Bump flow-pacmap to 0.1.1 and pin the README install line for the
   attribution-restoring docs patch.
 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> polish pacmap/linalg/gates for crates.io release
   Add README and publish metadata for flow-pacmap, and refresh install/API
   notes for the upcoming flow-linalg and flow-gates releases.

### Bug Fixes

 - <csr-id-67080e7f832aceb03a85606cb5afd293b3246227/> shorten crates.io keyword under 20 chars
   Replace dimensionality-reduction with dim-reduction so cargo publish
   accepts the package metadata.

### Documentation

 - <csr-id-2db29b5e2ba3f6483b4604c4c2d4cebe81585fca/> restore Wang et al. 2021 academic attribution
   Crate docs, `Cargo.toml` description, and README References now cite
   Wang, Huang, Rudin & Shaposhnik (2021), JMLR 22 (PaCMAP Algorithm 1).
 - <csr-id-1d5806048f15f590ebe7b2ba449501aa73868b95/> polish changelogs for pacmap, linalg, and gates releases

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Shorten crates.io keyword under 20 chars ([`67080e7`](https://github.com/jrmoynihan/flow/commit/67080e7f832aceb03a85606cb5afd293b3246227))
    - Release flow-pacmap v0.1.1, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`b44c4f5`](https://github.com/jrmoynihan/flow/commit/b44c4f5915916ab3ce73a5b6ce421601090f77a5))
    - Prepare 0.1.1 release with Wang et al. attribution ([`ba96d7f`](https://github.com/jrmoynihan/flow/commit/ba96d7fb2b887ab666a3ecdea9f9f49b0cbbf3f4))
    - Restore Wang et al. 2021 attribution ([`2db29b5`](https://github.com/jrmoynihan/flow/commit/2db29b5e2ba3f6483b4604c4c2d4cebe81585fca))
    - Release flow-pacmap v0.1.0, flow-linalg v0.1.2, flow-fcs-compress v0.1.3, flow-plots v0.3.2, flow-gates v0.4.0 ([`e29c820`](https://github.com/jrmoynihan/flow/commit/e29c820dd65493c3a41f437b0e8f850c3cef8102))
    - Polish changelogs for pacmap, linalg, and gates releases ([`1d58060`](https://github.com/jrmoynihan/flow/commit/1d5806048f15f590ebe7b2ba449501aa73868b95))
    - Polish pacmap/linalg/gates for crates.io release ([`c9b7448`](https://github.com/jrmoynihan/flow/commit/c9b7448fef935e2ba6f3ea568ce092f9c777b53b))
    - Add PaCMAP dimensionality reduction crate ([`5df4317`](https://github.com/jrmoynihan/flow/commit/5df4317832bb258a0adb6a0989a2d01d7ca04404))
</details>

## 0.1.0 (2026-07-22)

<csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/>

### Chore

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> polish pacmap/linalg/gates for crates.io release
   Add README and publish metadata for flow-pacmap, and refresh install/API
   notes for the upcoming flow-linalg and flow-gates releases.

### New Features

 - <csr-id-5df4317832bb258a0adb6a0989a2d01d7ca04404/> add PaCMAP dimensionality reduction crate
   Large-n 2D embedding with three-phase pair-weighted optimization, faer PCA
   initialization, and optional HNSW / k-d tree nearest-neighbour search.

### Documentation

 - <csr-id-c9b7448fef935e2ba6f3ea568ce092f9c777b53b/> add README and crates.io publish metadata
   Crate description, repository/readme fields, and a minimal README for the
   first publish.
 - <csr-id-1d5806048f15f590ebe7b2ba449501aa73868b95/> polish changelogs for pacmap, linalg, and gates releases

