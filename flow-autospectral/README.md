# flow-autospectral

Multi-autofluorescence (AF) spectral library discovery and per-event matching for spectral flow cytometry.

[![MIT](https://img.shields.io/crates/l/flow-autospectral.svg)](LICENSE)

## Purpose

Build a library of AF reference spectra from unstained control(s), match stained events to those spectra (residual OLS or ANN nearest-neighbour), then unmix with plain OLS **or** hand a selected AF column to TRU-OLS.

This is a phased AutoSpectral-style pipeline. Phase 1 is AF-focused (GMM / k-means discovery). FlowSOM and fluorophore spectral variants are follow-on work — not gating dependencies.

## Highlights

- Pluggable `AfLibraryBuilder` (GMM default, k-means; FlowSOM later)
- Residual-minimizing AF selection with optional HNSW shortlist via [`flow-knn`](../flow-knn/) `AnnIndex`
- Rayon over events (`FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1` to disable)
- Optional `tru-ols` feature for mixing-matrix adapters

## Install

```toml
[dependencies]
flow-autospectral = { path = "../flow-autospectral", features = ["hnsw"] }
# optional:
# flow-autospectral = { path = "../flow-autospectral", features = ["hnsw", "tru-ols"] }
```

## Quick start

```rust
use flow_autospectral::{
    discover_af_library, match_events, DiscoverConfig, DiscoveryBackend, MatchConfig,
};
use faer::Mat;

fn example(unstained: &[f64], n_u: usize, stained: &[f64], n_s: usize, d: usize) {
    let detectors = (0..d).map(|i| format!("D{i}")).collect::<Vec<_>>();
    let cfg = DiscoverConfig {
        backend: DiscoveryBackend::Gmm,
        fixed_k: Some(4),
        ..DiscoverConfig::default()
    };
    let library = discover_af_library(unstained, n_u, d, &detectors, &cfg).unwrap();
    let fluor = Mat::<f64>::zeros(d, 0); // or detectors × fluorophores
    let matched = match_events(stained, n_s, &library, fluor.as_ref(), &MatchConfig::default()).unwrap();
    let _ = matched.af_indices;
}
```

## Related crates

- [`flow-tru-ols`](../tru-ols/) — truncated re-unmixing after AF selection
- [`flow-knn`](../flow-knn/) — reusable `AnnIndex` for library search
- [`flow-clustering`](../flow-clustering/) — GMM / k-means (FlowSOM planned)

## Acknowledgments

AutoSpectral (academic method) refines mixing-matrix values via multi-AF / spectral-variant residual minimization; TRU-OLS truncates irrelevant fluorophore columns. See `tru-ols/docs/synergy-autospectral-tru-ols.md` for the combined pipeline rationale. Credit original papers/authors in publications; this crate describes behavior in neutral technical terms.
