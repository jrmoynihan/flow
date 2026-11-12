# Changelog

## 0.1.1 (2026-11-12)

## [0.1.1] — 2026-11-12

### New Features

 - <csr-id-500179053b2999f71805645e564d680350382dd8/> port GPU crates to cubeCL 0.11 / Burn 0.22
   Align workspace wgpu/ann-search stacks and record interleaved A/B
   Performance notes so callers know what moved on this host.

### Chore

 - <csr-id-9407a6dc3adca636bf51c241b71cb513ed3cfbab/> prep clustering/autospectral cubeCL port publish
   Merge main (FCS 0.6, TRU-OLS 0.3.0, typed PaCMAP KNN dim mismatch), bump
   six port crates with polished changelogs and README install pins; flow-clustering
   0.3.0 minor for e-means/agglomerative.

### Changed

- `flow-knn` ^0.1.3, `flow-clustering` ^0.3.0, optional `flow-tru-ols` ^0.3.1.
  Discovery, match, and joint unmix remain CPU-only (faer / Rayon); ann-search-rs
  **0.12.2** may shift `AnnIndex` latency via `flow-knn` only.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 9 calendar days.
 - 9 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Prep clustering/autospectral cubeCL port publish ([`9407a6d`](https://github.com/jrmoynihan/flow/commit/9407a6dc3adca636bf51c241b71cb513ed3cfbab))
    - Merge origin/main into release/clustering-autospectral-dry ([`713ac1c`](https://github.com/jrmoynihan/flow/commit/713ac1cc46dc2115e32684f4909c60eeecaa0152))
    - Merge pull request #30 from jrmoynihan/feature/cubecl-0.11 ([`fe7594c`](https://github.com/jrmoynihan/flow/commit/fe7594c96aa61e9f2a32fadc6310a59e86f1f9a3))
    - Port GPU crates to cubeCL 0.11 / Burn 0.22 ([`5001790`](https://github.com/jrmoynihan/flow/commit/500179053b2999f71805645e564d680350382dd8))
    - Release flow-clustering v0.2.1 ([`fdaf5d5`](https://github.com/jrmoynihan/flow/commit/fdaf5d5614b07f600cde989a63e36677f308870a))
    - Merge pull request #27 from jrmoynihan/release/row-io-path-b-bumps ([`ec6ccbd`](https://github.com/jrmoynihan/flow/commit/ec6ccbda8015e46d2c629c7e3b90c1cd2c218d8c))
    - Release flow-linalg 0.2.0, flow-fcs-compress 0.2.0, flow-fcs 0.6.0, flow-tru-ols 0.3.0 ([`1b24dcf`](https://github.com/jrmoynihan/flow/commit/1b24dcfc91ae604072a32d451465478eb56669fc))
    - Merge pull request #23 from jrmoynihan/release/clustering-autospectral-dry ([`b26691b`](https://github.com/jrmoynihan/flow/commit/b26691b458ca8b0e99e6d39b34e3b4cd700500df))
</details>

## v0.1.0 (2026-11-03)

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

 - <csr-id-90d645a8d2a8ca920d5f86730a3242612443684e/> add joint unmix, variants, and vs-R harness
   Port AutoSpectral v1.6 joint per-cell unmix and SOM spectral variants,
   reuse per-worker scratch, and keep optional f32 for large tables while
   defaulting to f64 for R agreement.
 - <csr-id-560f8fca8b1e67ed0c6ad0df057054c1ad8794fe/> factor-once OLS match and n×K×d benches
   Reuse per-AF Gram/QR factors and event-level Rayon so residual matching
   scales with K; add env-gated Criterion axes and GPU query-vs-library
   AnnIndex (Exact/IVF) so keep/skip is measured rather than guessed.
 - <csr-id-ba6f951028700a3284081848ae09f7ef80b3788f/> finish Phase 1 discovery, TRU-OLS handoff, and benches
   Add scatter-match/PCA cleaning, HNSW-medoid and FlowSOM library builders,
   and TruOls::from_preprocessed wiring after AF match. Include synthetic FCS
   and quality-comparison examples plus Criterion discover/match groups.
 - <csr-id-f6bccfbfccca0c76ddf9bb39c1a5091c56875967/> add AF library discovery and matching crate
   Introduce flow-autospectral for multi-AF spectral library discovery from
   unstained controls and per-event matching against stained data. Extend
   flow-knn with a reusable AnnIndex API (Exact, usearch, ann-search-rs)
   for query-vs-database search used by spectral AF library matching.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-autospectral v0.1.0 ([`2ac4911`](https://github.com/jrmoynihan/flow/commit/2ac4911ea03a9f928b6ee4ce45cfed2105432cd0))
    - Polish changelogs for knn 0.1.2, clustering 0.2.0, autospectral 0.1.0 ([`964879b`](https://github.com/jrmoynihan/flow/commit/964879b6c4d52736bd5dd54e0d80572f655a9557))
    - Merge pull request #22 from jrmoynihan/feature/flow-autospectral ([`35007ba`](https://github.com/jrmoynihan/flow/commit/35007ba7841c164cf8e4ffae220a782dcb87d7ce))
    - Add joint unmix, variants, and vs-R harness ([`90d645a`](https://github.com/jrmoynihan/flow/commit/90d645a8d2a8ca920d5f86730a3242612443684e))
    - Add performance-guided development map and host calibrator ([`e33bbe7`](https://github.com/jrmoynihan/flow/commit/e33bbe75d3777e0b8f7c8f5ebe0d71259ab9b4ef))
    - Factor-once OLS match and n×K×d benches ([`560f8fc`](https://github.com/jrmoynihan/flow/commit/560f8fca8b1e67ed0c6ad0df057054c1ad8794fe))
    - Finish Phase 1 discovery, TRU-OLS handoff, and benches ([`ba6f951`](https://github.com/jrmoynihan/flow/commit/ba6f951028700a3284081848ae09f7ef80b3788f))
    - Add AF library discovery and matching crate ([`f6bccfb`](https://github.com/jrmoynihan/flow/commit/f6bccfbfccca0c76ddf9bb39c1a5091c56875967))
</details>

