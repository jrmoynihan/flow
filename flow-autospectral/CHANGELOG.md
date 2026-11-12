# Changelog

## Unreleased

## [0.1.1] — 2026-11-12

### Changed

- `flow-knn` ^0.1.3, `flow-clustering` ^0.3.0, optional `flow-tru-ols` ^0.3.1.
  Discovery, match, and joint unmix remain CPU-only (faer / Rayon); ann-search-rs
  **0.12.2** may shift `AnnIndex` latency via `flow-knn` only.

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

