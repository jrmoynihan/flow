# flow-dimensional-reduction

Dimensionality reduction primitives for flow cytometry, built on [`faer`](https://crates.io/crates/faer).

[![crates.io](https://img.shields.io/crates/v/flow-dimensional-reduction.svg)](https://crates.io/crates/flow-dimensional-reduction)
[![docs.rs](https://docs.rs/flow-dimensional-reduction/badge.svg)](https://docs.rs/flow-dimensional-reduction)
[![MIT](https://img.shields.io/crates/l/flow-dimensional-reduction.svg)](LICENSE)

## Overview

Currently provides [`Pca`], a faer-based Principal Component Analysis using the
covariance method: it decomposes the `d × d` covariance matrix rather than the
`n × d` data matrix. For flow cytometry workloads (`n` ≈ 10⁶–10⁷ events,
`d` ≈ 10–50 channels), this is dramatically cheaper than a data-matrix SVD.

`Pca` is state-aware via a typestate: `Pca<UnfittedPcaResult>` only exposes
`fit`, and `Pca<FittedPcaResult>` (returned by `fit`) is the only state that
exposes `transform` and the basis accessors. Projecting before fitting is a
compile error, not a runtime one.

## Numerics

Data is `f32` at the API boundary — column means and the covariance matrix are
accumulated in `f64` and downcast only once, when the final basis is stored.
This keeps `n` from degrading precision even when it is large enough that
naive `f32` accumulation would lose significant bits.

## Usage

```rust
use flow_dimensional_reduction::Pca;

// Row-major `n x d` data.
let data: Vec<f32> = vec![
    1.0, 2.0,
    2.0, 4.1,
    3.0, 5.9,
    4.0, 8.2,
    5.0, 9.8,
];
let (n, d) = (5, 2);

let pca = Pca::new(2).fit(&data, n, d)?;
let projected = pca.transform(&data, n, d)?; // n x n_components(), row-major
let ratios = pca.explained_variance_ratio(); // descending, sums to 1.0
# Ok::<(), flow_dimensional_reduction::PcaError>(())
```

## Scope

This crate owns covariance-method PCA fit/transform. It does not own
downstream consumer-specific wiring — see, for example, `flow-pacmap`'s
`pca_init`, which specializes `Pca` to two components for embedding
initialization.

## License

MIT
