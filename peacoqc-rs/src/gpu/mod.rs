//! GPU acceleration for computationally intensive operations
//!
//! This module provides GPU-accelerated implementations for:
//! - FFT-based Kernel Density Estimation (KDE) — **microbench / batched kernels**
//! - Feature matrix operations
//! - Statistical calculations
//!
//! ## Performance (read carefully)
//!
//! **Full PeacoQC e2e** with `--features gpu` was **slower** than Rayon CPU on the
//! 2026-08-10 Rust-vs-R sample (every size 50k–1M). Prefer CPU for production QC;
//! see `docs/throughput_vs_r_sample.md` and beads `flow-crates-aww`.
//!
//! **Batched KDE microbenches** can still win when upload/kernel costs are amortized
//! (`bench_results/README.md`, ~1–5× on some sizes). Older tables claiming ~20–32×
//! below are kernel-isolation numbers, not QC-core wall time. Long-term home for
//! shared KDE GPU is `flow-density` (beads `flow-crates-g1b`).
//!
//! ### Historical batched-KDE isolation table (not e2e PeacoQC)
//!
//! | Configuration | Batched GPU | Sequential CPU | Speedup |
//! |--------------|-------------|----------------|---------|
//! | 5 channels, 50K events | 250 µs | 4.9 ms | **19.7x** |
//! | 5 channels, 100K events | 421 µs | 10.1 ms | **24.0x** |
//! | 5 channels, 500K events | 1.8 ms | 54.0 ms | **30.3x** |
//! | 10 channels, 500K events | 4.1 ms | 109 ms | **26.6x** |
//! | 10 channels, 1M events | 7.8 ms | 253 ms | **32.3x** |
//!
//! Prefer `bench_results/README.md` for Criterion-dated KDE numbers.
//!
//! ## Implementation Details
//!
//! - **Backend**: WGPU (WebGPU) via burn framework
//! - **Custom Kernels**: cubeCL kernels available for complex multiplication (optional)
//! - **Batching**: GPU context reuse and kernel caching amortize overhead *within* kernels
//! - **Fallback**: Automatic CPU fallback when GPU unavailable
//!
//! ## Usage
//!
//! When `--features gpu` is enabled and a device is available, GPU paths are used
//! automatically (no size gate). That auto path is currently a regression for full
//! PeacoQC wall time — leave `gpu` off unless profiling kernels.

#[cfg(feature = "gpu")]
mod backend;
#[cfg(feature = "gpu")]
mod batched;
#[cfg(feature = "gpu")]
mod context;
#[cfg(feature = "gpu")]
mod fft;
#[cfg(feature = "gpu")]
mod matrix;
#[cfg(feature = "gpu")]
mod stats;

#[cfg(all(feature = "gpu", feature = "cubecl"))]
mod kernels;

#[cfg(feature = "gpu")]
pub use backend::{GpuBackend, is_gpu_available};
#[cfg(feature = "gpu")]
pub use batched::{KdeContext, kde_fft_batched_gpu};
#[cfg(feature = "gpu")]
pub use context::GpuContext;
#[cfg(feature = "gpu")]
pub use fft::kde_fft_gpu;
#[cfg(feature = "gpu")]
pub use matrix::build_feature_matrix_gpu;
#[cfg(feature = "gpu")]
pub use stats::{median_gpu, percentile_gpu, standard_deviation_gpu};

// Threshold constants removed - GPU is now used whenever available
// Batched operations provide speedup even for smaller datasets (50K+ events)
