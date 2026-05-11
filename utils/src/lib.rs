//! **Deprecated**: `flow-utils` has been split into focused crates.
//! Use `flow-density` (KDE), `flow-clustering` (clustering) instead.

//! # flow-utils
//!
//! Shared algorithms and utilities for flow cytometry crates.
//!
//! This crate provides high-performance implementations of common algorithms used across
//! multiple flow cytometry crates, including:
//!
//! - **Kernel Density Estimation (KDE)**: FFT-accelerated KDE with GPU support
//! - **Clustering**: K-means, DBSCAN, Gaussian Mixture Model
//! - **PCA**: Principal Component Analysis for dimensionality reduction
//!
//! ## Features
//!
//! - `gpu`: Enable GPU acceleration for KDE (requires burn and cubecl)

pub mod clustering;
pub mod common;
pub mod kde;
pub mod pca;

pub use clustering::{
    ClusteringError, ClusteringResult, Dbscan, DbscanConfig, DbscanResult, Gmm, GmmConfig,
    GmmResult, KMeans, KMeansConfig, KMeansResult, SilhouetteResult, silhouette_scores,
    silhouette_scores_sampled,
};
pub use kde::{KdeError, KdeResult, KernelDensity, KernelDensity2D};
pub use pca::{Pca, PcaError, PcaResult};
