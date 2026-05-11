# flow-density

FFT-accelerated kernel density estimation for flow cytometry.

[![crates.io](https://img.shields.io/crates/v/flow-density.svg)](https://crates.io/crates/flow-density)
[![docs.rs](https://docs.rs/flow-density/badge.svg)](https://docs.rs/flow-density)
[![MIT](https://img.shields.io/crates/l/flow-density.svg)](LICENSE)

## Overview

`flow-density` provides 1D and 2D kernel density estimation optimized for the event counts typical in flow cytometry (10K–10M events). The FFT-based algorithm avoids the O(n²) cost of naive KDE, making real-time density plots feasible even for large files.

## Features

| Feature | Description |
|---------|-------------|
| `kde` *(default)* | 1D and 2D kernel density estimation |
| `gpu` | WebGPU-accelerated KDE via `burn` + `cubecl` (experimental) |

## Public API

### 1D Density Estimation

```rust
use flow_density::KernelDensity;

let kde = KernelDensity::estimate(&data, 1.0, 512)?;
let peaks = kde.find_peaks(0.1);       // locate density peaks
let d = kde.density_at(42.0);          // query density at a point
```

### 2D Density Estimation

```rust
use flow_density::KernelDensity2D;

let kde2d = KernelDensity2D::estimate(&x_data, &y_data, 1.0, 256)?;
let contour = kde2d.find_contour(0.5); // extract contour at threshold
let d = kde2d.density_at(100.0, 200.0);
```

### Utilities

```rust
use flow_density::common::{standard_deviation, interquartile_range, gaussian_kernel};
```

## Algorithms

- **1D KDE**: Gaussian kernel with Silverman bandwidth selection, evaluated via FFT convolution (`realfft`). Supports bandwidth adjustment factor and configurable grid resolution.
- **2D KDE**: Separable Gaussian kernel on a regular grid, FFT-accelerated along each axis. Contour extraction via threshold-based boundary tracing.
- **Peak finding**: Local maxima detection with configurable minimum prominence (peak removal fraction).

## Scope

This crate owns:

- 1D and 2D kernel density estimation
- Bandwidth selection heuristics (Silverman, Scott)
- Peak/mode detection in density estimates
- Contour extraction from 2D density fields
- *(Future)* Adaptive bandwidth KDE
- *(Future)* GPU-accelerated KDE for interactive use

It does **not** own: plotting/rendering, clustering, gating logic, or FCS file I/O.

## Tests

```bash
cargo test -p flow-density
```

## License

MIT
