# flow-linalg

Pure-Rust linear algebra primitives for flow cytometry, built on [`faer`](https://crates.io/crates/faer).

[![crates.io](https://img.shields.io/crates/v/flow-linalg.svg)](https://crates.io/crates/flow-linalg)
[![docs.rs](https://docs.rs/flow-linalg/badge.svg)](https://docs.rs/flow-linalg)
[![MIT](https://img.shields.io/crates/l/flow-linalg.svg)](LICENSE)

## Overview

`flow-linalg` provides the core matrix operations needed for fluorescence compensation and (future) spectral unmixing in flow cytometry pipelines. It requires no system BLAS/LAPACK — pure Rust via `faer`.

## Features

| Feature | Description |
|---------|-------------|
| `compensation` | Spillover matrix inversion and per-event compensation |
| `unmixing` | *(stub)* Spectral unmixing — future implementation |

## Public API

```rust
use flow_linalg::compensation::{invert_spillover, apply_compensation_inv, compensate_channels};
use faer::MatRef;

// Invert a spillover matrix (partial-pivot LU decomposition)
let inv = invert_spillover(spillover.as_ref())?;

// Apply pre-inverted matrix to raw channel data (rayon-parallelized per channel)
let result = apply_compensation_inv(&raw_channels, inv.as_ref(), &channel_names)?;

// Convenience: invert + apply + filter to requested channels
let compensated = compensate_channels(&raw_channels, spillover.as_ref(), &matrix_names, &needed)?;
```

## Algorithms

- **Spillover inversion**: Partial-pivot LU decomposition via `faer`. Validates matrix is square and non-singular before inversion.
- **Compensation application**: Per-event matrix-vector multiply, parallelized across output channels with `rayon`. Validates event count consistency across input channels.

## Scope

This crate is intentionally narrow — it owns:

- Spillover matrix inversion
- Compensation matrix application to event vectors
- *(Future)* Ordinary least-squares and non-negative least-squares for spectral unmixing
- *(Future)* Condition number and quality metrics for compensation matrices

It does **not** own: FCS file parsing, spillover keyword extraction, transform application, or GUI concerns.

## Tests

```bash
cargo test -p flow-linalg --features compensation
```

4 unit tests covering identity matrices, known spillover removal, channel filtering, and error cases.

## License

MIT
