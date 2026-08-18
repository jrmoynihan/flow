# flow-linalg

Pure-Rust linear algebra primitives for flow cytometry, built on [`faer`](https://crates.io/crates/faer).

[![crates.io](https://img.shields.io/crates/v/flow-linalg.svg)](https://crates.io/crates/flow-linalg)
[![docs.rs](https://docs.rs/flow-linalg/badge.svg)](https://docs.rs/flow-linalg)
[![MIT](https://img.shields.io/crates/l/flow-linalg.svg)](LICENSE)

## Overview

`flow-linalg` provides the core matrix operations needed for:

## Features

- **Spillover estimation**: Per-control column from median(positive) − median(negative), diagonal-normalized so `S[j][j] = 1`.
- **Spillover inversion**: Partial-pivot LU decomposition via `faer`. Validates matrix is square and non-singular before inversion.
- **Compensation application**: Per-event matrix-vector multiply, parallelized across output channels with `rayon`. Validates event count consistency across input channels
- Matrix condition number / complexity metrics
- Hotspot (similarity / mixing-matrix) diagnostics for spectral panels

| Feature Flag | Description |
| ------------ | ----------- |
| `compensation` | Spillover matrix inversion and per-event compensation |

## How it Works

Compensation uses the `faer` crate's LU for spillover inversion and Rayon-parallel per-channel application when the `compensation` feature is on.

Condition number metrics use SVD (\(κ₂\)).

Hotspot matrix helpers operate on cosine-similarity or unit-normalized mixing-matrix Gram forms. No system BLAS is required.

## Installation

```bash
cargo add flow-linalg
```

Or add it directly to your `Cargo.toml`:

```toml
[dependencies]
flow-linalg = { version = "0.1.3", features = ["compensation"] }
```

## API Usage

```rust
use flow_linalg::compensation::{
    apply_compensation_inv, estimate_spillover, invert_spillover, SingleStainControl,
};
use flow_linalg::{
    condition_metrics_f32, hotspot_from_mixing_matrix, ConditionMetrics, HotspotMatrix,
};
use anyhow::Result;
use faer::Mat;
use std::collections::HashMap;

fn example(
    controls: &[SingleStainControl<'_>],
    n_detectors: usize,
    raw_channels: &[(&str, &[f32])],
    channel_names: &[&str],
    mixing: &Mat<f64>,
) -> Result<()> {
    // Estimate spillover from single-stain positive/negative populations
    let spillover: Mat<f32> = estimate_spillover(controls, n_detectors)?;
    // Invert a spillover matrix (partial-pivot LU decomposition)
    let inv: Mat<f32> = invert_spillover(spillover.as_ref())?;
    // Apply pre-inverted matrix to raw channel data (rayon-parallelized per channel)
    let compensated: HashMap<String, Vec<f32>> =
        apply_compensation_inv(raw_channels, inv.as_ref(), channel_names)?;

    // Spillover is f32 — use condition_metrics_f32 (or convert to f64 for condition_metrics).
    let metrics: ConditionMetrics = condition_metrics_f32(spillover.as_ref())?;
    let kappa: f64 = metrics.condition_number;
    let complexity: f64 = metrics.complexity_index;

// Calculate a spectral-unmixing-dependent error "hotspot" matrix (Mage, et al.)
    let hotspot: HotspotMatrix = hotspot_from_mixing_matrix(mixing.as_ref())?;
    let sifs: Vec<f64> = hotspot.sifs();
    Ok(())
}
```

## Performance

No published numbers yet.  `compensation` parallelizes across output channels.

Prefer measuring on your panel size rather than just micro-benching.

## Tests

```bash
cargo test -p flow-linalg --features compensation
```

## License

MIT

## Related Crates

- **Plotting or gating** → [`flow-plots`](../plots/), [`flow-gates`](../gates/)
- **FCS file parsing** → [`flow-fcs`](../fcs/) — FCS I/O and optional compensation integration
- **Spectral unmixing** → [`flow-tru-ols`](../tru-ols/) — Truncated Re-Unmixing OLS
- **Unmixing via CLI** → [`tru-ols-cli`](../tru-ols-cli/) CLI for Unmix + QC pipeline