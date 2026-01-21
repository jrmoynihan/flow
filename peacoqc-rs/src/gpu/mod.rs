//! GPU acceleration for computationally intensive operations
//!
//! This module provides GPU-accelerated implementations for:
//! - FFT-based Kernel Density Estimation (KDE)
//! - Feature matrix operations
//! - Statistical calculations
//!
//! ## Performance
//!
//! GPU acceleration provides significant benefits for **batched multi-channel operations**:
//!
//! | Configuration | Batched GPU | Sequential CPU | Speedup |
//! |--------------|-------------|----------------|---------|
//! | 5 channels, 50K events | 250 µs | 4.9 ms | **19.7x** |
//! | 5 channels, 100K events | 421 µs | 10.1 ms | **24.0x** |
//! | 5 channels, 500K events | 1.8 ms | 54.0 ms | **30.3x** |
//! | 10 channels, 500K events | 4.1 ms | 109 ms | **26.6x** |
//! | 10 channels, 1M events | 7.8 ms | 253 ms | **32.3x** |
//!
//! Batched operations provide significant speedup even for smaller datasets (50K+ events).
//!
//! ## Implementation Details
//!
//! - **Backend**: WGPU (WebGPU) via burn framework
//! - **Custom Kernels**: cubeCL kernels available for complex multiplication (optional)
//! - **Batching**: GPU context reuse and kernel caching amortize overhead
//! - **Fallback**: Automatic CPU fallback when GPU unavailable
//!
//! ## Usage
//!
//! GPU acceleration is automatic when:
//! - `--features gpu` is enabled
//! - GPU is available
//!
//! For batched operations, use `kde_fft_batched_gpu()` with `GpuContext` for best performance.

#[cfg(feature = "gpu")]
mod backend;
#[cfg(feature = "gpu")]
mod context;
#[cfg(feature = "gpu")]
mod fft;
#[cfg(feature = "gpu")]
mod batched;
#[cfg(feature = "gpu")]
mod matrix;
#[cfg(feature = "gpu")]
mod stats;

#[cfg(all(feature = "gpu", feature = "cubecl"))]
mod kernels;

#[cfg(feature = "gpu")]
pub use backend::{is_gpu_available, GpuBackend};
#[cfg(feature = "gpu")]
pub use context::GpuContext;
#[cfg(feature = "gpu")]
pub use fft::kde_fft_gpu;
#[cfg(feature = "gpu")]
pub use batched::{kde_fft_batched_gpu, KdeContext};
#[cfg(feature = "gpu")]
pub use matrix::build_feature_matrix_gpu;
#[cfg(feature = "gpu")]
pub use stats::{standard_deviation_gpu, median_gpu, percentile_gpu};

// Threshold constants removed - GPU is now used whenever available
// Batched operations provide speedup even for smaller datasets (50K+ events)
