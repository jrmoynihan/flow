# flow-autospectral

Multi-autofluorescence (AF) spectral library discovery, per-event AF matching, and joint per-cell fluorophore-variant unmixing for spectral flow cytometry.

[![MIT](https://img.shields.io/crates/l/flow-autospectral.svg)](LICENSE)

## Purpose

Build a library of AF reference spectra from unstained control(s), match stained events to those spectra (residual OLS or ANN nearest-neighbour), then unmix with plain OLS, TRU-OLS, **or** the joint per-cell pipeline (`unmix_autospectral_joint`) that selects AF and fluorophore variants together.

Phase 1 is AF library discovery + match. Phase 2 adds SOM spectral variants and the AutoSpectral v1.6 joint unmix path.

## Highlights

- Pluggable `AfLibraryBuilder` (GMM default, k-means, HNSW-medoid, FlowSOM)
- Optional scatter-match + PCA-intrusive cleaning of unstained events
- Residual-minimizing AF selection with optional HNSW shortlist via [`flow-knn`](../flow-knn/) `AnnIndex`
- `discover_spectral_variants`: SOM `10×10`, cosine QC, optional `k.neighbors` scatter background
- `unmix_autospectral_joint` with [`JointUnmixConfig`](src/config.rs) (all public knobs: passes, AF refine, cell weights, α, collinear pair resolution, `precision` `f64`/`f32`)
- Rayon over events (`FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1` to disable)
- Joint QC-core ~**2×** AutoSpectralRcpp at 1 thread (F=8 and F=42 panels, 10k–1M); see Performance
- Optional `tru-ols` feature: `MixingMatrix` adapter and `TruOls::from_preprocessed` after AF match

## Install

```toml
[dependencies]
flow-autospectral = { path = "../flow-autospectral", features = ["hnsw"] }
# optional:
# flow-autospectral = { path = "../flow-autospectral", features = ["hnsw", "tru-ols"] }
# flow-autospectral = { path = "../flow-autospectral", features = ["hnsw", "tru-ols", "fcs"] }
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

## Examples

```bash
cargo run -p flow-autospectral --example af_match_tru_ols --features tru-ols,fcs
cargo run -p flow-autospectral --example method_comparison --features tru-ols --release
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- --smoke
```

## Performance

Headline comparison is **QC-core** wall time versus AutoSpectralRcpp `pipeline = "joint"` (events already in RAM; FCS I/O excluded).

Comparison breakdown with metric tables: [`docs/comparison-with-r.md`](docs/comparison-with-r.md).
Rust-only Criterion grids: [`docs/PERF_MATRIX.md`](docs/PERF_MATRIX.md).
[`docs/PERF_AB.md`](docs/PERF_AB.md) explains each speedup as a problem, the change we made, and how the operation differs afterward (for example many heap allocations per event versus one workspace per worker). It names the functions and defines the linear-algebra terms, so a port in another language can make the same changes.

Representative release results (Apple M5 Max, rustc 1.95.0, AutoSpectralRcpp 1.2.1 / AutoSpectral 1.7.1 / R 4.6.0, 18 hardware threads, warmup 1, reps 2). Throughput is events/s. The 10,000–200,000-event rows are from 2026-08-19; 1,000,000-event ratios are from 2026-08-20 (same rust/R pair; absolute events/s that session were lower under endpoint-security CPU load — do not mix those absolute rates into the 19 Aug columns).

| Case | rust 1 | R 1 | rust/R 1 | rust 18 | R 18 | rust/R 18 |
|------|--------|-----|----------|---------|------|-----------|
| 10,000 events × 20 det. × 8 fluors | 1.74M | 0.79M | **2.2×** | 7.26M | 2.55M | 2.8× |
| 200,000 events × 20 det. × 8 fluors | 1.72M | 0.89M | **1.9×** | 13.2M | 3.78M | **3.5×** |
| 50,000 events × 64 det. × 42 fluors | 0.166M | 0.075M | **2.2×** | 1.97M | 0.322M | 6.1× |
| 200,000 events × 64 det. × 42 fluors | 0.157M | 0.073M | **2.2×** | 1.73M | 0.325M | 5.3× |
| 1,000,000 events × 20 det. × 8 fluors | — | — | **~2.1×** | — | — | ~2.0× |
| 1,000,000 events × 64 det. × 42 fluors | — | — | **~2.2×** | — | — | ~3.5× |

Single-thread QC-core is the publishable pair: Rust is about **2×** AutoSpectralRcpp across both panels. Treat the **200,000-event** F=8 multi-thread row as the stabler one (50,000 events / 18 threads was almost a tie on two reps). e2e (FCS open + unmix + write) is secondary and I/O-bound at small event counts — see the comparison doc.

On determined panels (`d ≥ F+1`, at most 50,000 events in the agreement sample) fluor-column cosine and AF-index match were **1.000**. Collinear-pair MAD on events generated as true-A single-positives dropped from ~6.5 (OLS) to ~0.10 (joint) on the F=8 panel.

No GPU path in this crate — do not headline GPU.

Harness:

```bash
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- --smoke
cargo run -p flow-autospectral --example compare_with_r --features fcs --release -- \
  --events 10000,50000,200000 --warmup 1 --reps 2 --e2e --out target/autospectral-r-scale
```

Criterion (Rust-only; interleave baseline/HEAD, keep an untouched control — `docs/PERF_AB.md`):

```bash
cargo bench -p flow-autospectral --bench discover_and_match
cargo bench -p flow-autospectral --bench joint_unmix
```

## Related crates

- [`flow-tru-ols`](../tru-ols/) — truncated re-unmixing after AF selection
- [`flow-knn`](../flow-knn/) — reusable `AnnIndex` for library search
- [`flow-clustering`](../flow-clustering/) — GMM / k-means / SOM / FlowSOM
- [`flow-dimensional-reduction`](../flow-dimensional-reduction/) — PCA used by intrusive cleaning

## Acknowledgments

Burton OT, Bücken L, De Vuyst L, Humblet-Baron S, Lopez Menoz De Leon A, Khan S, Cerveira J, Dooley J, Liston A. AutoSpectral improves spectral flow cytometry accuracy through optimised spectral unmixing and autofluorescence-matching at the cellular level. *bioRxiv* 2025.10.27.684855. <https://doi.org/10.1101/2025.10.27.684855>

Van Gassen S *et al.* (2015). FlowSOM. *Cytometry Part A*, 87(7), 636–645.

TRU-OLS truncates irrelevant fluorophore columns after AF selection. See `tru-ols/docs/synergy-autospectral-tru-ols.md` for the combined pipeline. Credit original papers/authors in publications; this crate describes behavior in neutral technical terms.
