//! Dimensionality reduction primitives for flow cytometry.
//!
//! Currently provides [`Pca`], a faer-based principal component analysis using
//! the covariance method: it decomposes the `d × d` covariance matrix rather
//! than the `n × d` data matrix. For flow cytometry workloads (n ≈ 10⁶–10⁷,
//! d ≈ 10–50) this is dramatically cheaper.
//!
//! Data is `f32` on the boundary; means and covariance are accumulated in `f64`
//! and downcast only when the final basis is stored.
//!
//! Note: `flow-pacmap` previously carried its own `pca_init`. It is now a
//! two-component specialization of this crate's [`Pca`].

pub mod pca;

pub use pca::{FittedPcaResult, Pca, PcaComponent, PcaError, PcaResult, UnfittedPcaResult};
