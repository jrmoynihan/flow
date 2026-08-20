# flow-density

FFT-accelerated kernel density estimation for flow cytometry.

[![crates.io](https://img.shields.io/crates/v/flow-density.svg)](https://crates.io/crates/flow-density)
[![docs.rs](https://docs.rs/flow-density/badge.svg)](https://docs.rs/flow-density)
[![MIT](https://img.shields.io/crates/l/flow-density.svg)](LICENSE)

## Overview

`flow-density` provides 1D and 2D kernel density estimation (KDE), peak-finding (mode), or contour extraction optimized for the event counts typical in flow cytometry (10K–10M events).

The FFT-based KDE algorithm avoids the O(n²) cost of naive KDE, making real-time density plots feasible even for large files.

## Features

| Feature | Description |
| ------- | ----------- |
| `kde` *(default)* | 1D and 2D kernel density estimation |
| `gpu` | WebGPU-accelerated KDE via `burn` + `cubecl` (experimental) |

- **1D KDE**: Gaussian kernel with Silverman bandwidth selection, evaluated via FFT convolution (`realfft`). Supports bandwidth adjustment factor and configurable grid resolution.
- **2D KDE**: Separable Gaussian kernel on a regular grid, FFT-accelerated along each axis. Contour  extraction via threshold-based boundary tracing.
- **Peak finding**: Local maxima detection with configurable minimum prominence (peak removal fraction).
- **Contour extraction** from 2D density fields
- *(Future)* Adaptive bandwidth KDE

## Related crates

- **Rendered density/scatter plots** → [`flow-plots`](../plots/) (pixel occupancy ≠ FFT KDE)
- **Gates** [`flow-gates`](../gates/), [`flow-plots`](../plots/) — Uses FFT KDE for peak detection
- **QC** [`peacoqc-rs`](../peacoqc-rs/) - Uses FFT KDE for density-based gating
- **Clustering** [`flow-clustering`](../flow-clustering/) — clustering, not density estimation
- **Single-stain peak isolation for unmixing** → [`flow-peak-detection`](../flow-peak-detection/)

## Installation

```bash
cargo add flow-density
```

Or add it directly to your `Cargo.toml`:

```toml
[dependencies]
flow-density = "0.1.2"
```

## API Usage

### 1D Density Estimation

```rust
use flow_density::{KernelDensity, KdeResult};

fn example(data: &[f64]) -> KdeResult<()> {
    let kde: KernelDensity = KernelDensity::estimate(data, 1.0, 512)?;
    let peaks: Vec<f64> = kde.find_peaks(0.1);  // locate density peaks
    let d: f64 = kde.density_at(42.0);  // query density at a point
    Ok(())
}
```

### 2D Density Estimation

```rust
use flow_density::{KernelDensity2D, KdeResult};

fn example(x_data: &[f64], y_data: &[f64]) -> KdeResult<()> {
    let kde2d: KernelDensity2D = KernelDensity2D::estimate(x_data, y_data, 1.0, 256)?;
    let contour: Vec<(f64, f64)> = kde2d.find_contour(0.5);
    let d: f64 = kde2d.density_at(100.0, 200.0);
    Ok(())
}
```

## Performance

FFT KDE targets 10K–10M events where naive O(n²) KDE is impractical.

Cost model (workspace [`docs/dev/PERF_PGD.md`](../docs/dev/PERF_PGD.md)): a 1D FFT on a 512-point grid is microseconds of arithmetic; streaming 1,000,000 `f64` samples is 8 MiB (P-core L2 is 16 MiB on the M5 Max host). Naive O(n²) KDE at that n would be ~10¹² kernel evals — **>100×** vs FFT, which is why the FFT path exists. API is `&[f64]`; switching the **grid** to `f32` would not change cache level. GPU KDE (`--features gpu`) is experimental; PeacoQC e2e often still prefers Rayon CPU (`gpu-after-amortize`).

## Testing

```bash
cargo test -p flow-density
```

## License

MIT
