# Changelog

## 0.1.3 (2026-11-12)

## [0.1.3] — 2026-11-12

### Bug Fixes

 - <csr-id-5a739f84782af506b668f8dcd07bcfe10f35bb50/> one cubeCL client with Burn tensors
   Derive the shared WGPU client from Burn's device so CSR kernels and Adam
   see the same handles under BURN_DEVICE. Clarify flow-knn Unreleased timings
   as interleaved Criterion medians, not matrix rows, and update agent GPU pins.
 - <csr-id-424a0faf58cdfb4db3270be641be66fe84d1b9b0/> drop the circular density deprecation
   render_pixels and render_contour were deprecated in favor of the same functions. Kuva still does not draw density or contour, so this backend stays. Also remove the unused colormap macros and keep prefer_usearch only for the ann-search build.

### New Features

 - <csr-id-500179053b2999f71805645e564d680350382dd8/> port GPU crates to cubeCL 0.11 / Burn 0.22
   Align workspace wgpu/ann-search stacks and record interleaved A/B
   Performance notes so callers know what moved on this host.
 - <csr-id-175b9749a8e82e7619c7c6da061846d5879f13ca/> add e-means (default) and agglomerative metaclustering
   Ship mini-batch EMA k-means with a same-kernel Lloyd baseline, measured
   to beat Lloyd from ~5k events, plus kodama linkage for over-cluster merges.

### Chore

 - <csr-id-9407a6dc3adca636bf51c241b71cb513ed3cfbab/> prep clustering/autospectral cubeCL port publish
   Merge main (FCS 0.6, TRU-OLS 0.3.0, typed PaCMAP KNN dim mismatch), bump
   six port crates with polished changelogs and README install pins; flow-clustering
   0.3.0 minor for e-means/agglomerative.

### Changed

- `recommend_method` / `available_method_ids`: only the ann-search build consults
  the USearch `prefer_usearch` flag.

### Performance

- Workspace bump to cubeCL **0.11.4** / Burn **0.22.1** / ann-search-rs **0.12.2** /
  wgpu **30** (was 0.10 / 0.21 / 0.4.4 on `d21d0ff`). Medians below are from
  **interleaved Criterion A/B** on Apple M5 Max (`OMP_NUM_THREADS=1`, features
  `hnsw,ann-search,gpu`); they are not copied from `data/knn_perf_matrix.jsonl` rows.
  Self-query **exact_gpu** at 50k×10 **410 ms → 284 ms**; **exact_cpu** at 50k×10
  unchanged (~1.08 s). **ivf_gpu** at 100k×10 **1.06 s → 108 ms** in that same A/B run
  (the matrix file’s latest **ivf_gpu** row at 100k×10 is ~1.67 s — different harness
  and capture date). **hnsw_ann_cpu** moved **556 ms → 309 ms** with ann-search-rs
  **0.4.4 → 0.12.2**, not from cubeCL alone.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release over the course of 9 calendar days.
 - 9 days passed between releases.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Prep clustering/autospectral cubeCL port publish ([`9407a6d`](https://github.com/jrmoynihan/flow/commit/9407a6dc3adca636bf51c241b71cb513ed3cfbab))
    - Merge origin/main into release/clustering-autospectral-dry ([`713ac1c`](https://github.com/jrmoynihan/flow/commit/713ac1cc46dc2115e32684f4909c60eeecaa0152))
    - Merge pull request #30 from jrmoynihan/feature/cubecl-0.11 ([`fe7594c`](https://github.com/jrmoynihan/flow/commit/fe7594c96aa61e9f2a32fadc6310a59e86f1f9a3))
    - One cubeCL client with Burn tensors ([`5a739f8`](https://github.com/jrmoynihan/flow/commit/5a739f84782af506b668f8dcd07bcfe10f35bb50))
    - Port GPU crates to cubeCL 0.11 / Burn 0.22 ([`5001790`](https://github.com/jrmoynihan/flow/commit/500179053b2999f71805645e564d680350382dd8))
    - Merge pull request #29 from jrmoynihan/feature/emeans-prototype ([`7c8f826`](https://github.com/jrmoynihan/flow/commit/7c8f8267258a21d0cd5c5264b691808a5f14f15c))
    - Add e-means (default) and agglomerative metaclustering ([`175b974`](https://github.com/jrmoynihan/flow/commit/175b9749a8e82e7619c7c6da061846d5879f13ca))
    - Merge pull request #26 from jrmoynihan/feature/tru-ols-path-b-protected-af ([`a51cd25`](https://github.com/jrmoynihan/flow/commit/a51cd25879f62c31a9fb6cb3e787697136f8fb00))
    - Drop the circular density deprecation ([`424a0fa`](https://github.com/jrmoynihan/flow/commit/424a0faf58cdfb4db3270be641be66fe84d1b9b0))
    - Merge pull request #23 from jrmoynihan/release/clustering-autospectral-dry ([`b26691b`](https://github.com/jrmoynihan/flow/commit/b26691b458ca8b0e99e6d39b34e3b4cd700500df))
</details>

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

 - 7 commits contributed to the release.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-knn v0.1.2 ([`5ba917d`](https://github.com/jrmoynihan/flow/commit/5ba917dd3cb05139eaee0653dc4979e9a8e2065c))
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

