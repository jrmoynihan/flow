# Changelog

## 0.1.0 (unreleased)

- Initial crate: GMM / k-means / HNSW-medoid / FlowSOM AF library discovery, optional scatter-match + PCA cleaning, residual / NN matching via `flow-knn` `AnnIndex`, standalone OLS helpers, optional `tru-ols` mixing-matrix adapter and `TruOls::from_preprocessed` helper.
- Phase 2: `discover_spectral_variants` (SOM + cosine QC) and `unmix_autospectral_joint` (`JointUnmixConfig`; AutoSpectral v1.6 joint pipeline). Example `compare_with_r` times matched 1-thread and N-thread QC-core vs AutoSpectralRcpp, plus fluor-column cosine / AF-index agreement and collinear-pair MAD.
- Examples: `af_match_tru_ols` (synthetic FCS round-trip), `method_comparison` (quality A/B).
- `compare_with_r` times matched 1-thread and N-thread QC-core and e2e vs AutoSpectralRcpp; skips underdetermined `d < F+1` panels. Sample QC-core: ~2× AutoSpectralRcpp at 1 thread on F=8 and F=42 panels through 1M events (`docs/comparison-with-r.md`).
- Criterion benches `discover_and_match`, `match_matrix`, `scatter_clean`, `joint_unmix`.
- Joint unmix reuses a per-worker workspace of arrays (`EventScratch` in `joint.rs`, thread-local under Rayon), copies mixing matrices only when an event first accepts a variant, writes results into pre-sized tables instead of allocating one output vector per event, and implements matrix–vector products as loops over contiguous emitter columns (`gemv`).
- `JointUnmixConfig::precision`: default `f64` (vs-R); optional internal `f32` faer (`JointUnmixPrecision::F32`) for large event tables (`flow-crates-0ap.1`).
