# Changelog

## 0.1.0 — 2026-11-03

### Added

- Multi-AF spectral library discovery from unstained controls (GMM default;
  k-means, HNSW-medoid, FlowSOM backends), optional scatter-match + PCA
  cleaning, and residual / NN matching via `flow-knn` `AnnIndex`.
- Standalone OLS helpers and optional `tru-ols` mixing-matrix adapter with
  `TruOls::from_preprocessed` after AF match.
- `discover_spectral_variants` (SOM + cosine QC) and
  `unmix_autospectral_joint` (`JointUnmixConfig`; AutoSpectral v1.6 joint
  pipeline; defaults follow R `unmix.fcs`, including `n_passes = 1`).
- Examples: `af_match_tru_ols`, `method_comparison`, `compare_with_r`
  (1-thread and N-thread QC-core vs AutoSpectralRcpp; fluor-column cosine /
  AF-index agreement; collinear-pair MAD).
- Criterion benches: `discover_and_match`, `match_matrix`, `scatter_clean`,
  `joint_unmix`.

### Performance

- Joint unmix reuses a per-worker workspace (`EventScratch`), copies mixing
  matrices only when an event first accepts a variant, writes into pre-sized
  tables, and uses contiguous column-major `gemv` products.
- `JointUnmixConfig::precision`: default `f64` (vs-R); optional internal
  `f32` faer for large event tables.

### Documentation

- README performance table and `docs/comparison-with-r.md` /
  `docs/PERF_MATRIX.md` / `docs/PERF_AB.md`.
