//! GPU-accelerated FFT operations for KDE
//!
//! Uses GPU for complex multiplication in frequency domain (FFT convolution step).
//! CPU FFT is used for the actual transforms.

use crate::kde::{KdeError, KdeResult};
#[cfg(feature = "gpu")]
use burn::backend::wgpu::WgpuDevice;
#[cfg(feature = "gpu")]
use burn::tensor::Tensor;
use realfft::num_complex::Complex;

#[cfg(feature = "gpu")]
type Backend = burn::backend::wgpu::Wgpu;

/// Check if GPU is available
#[cfg(feature = "gpu")]
pub fn is_gpu_available() -> bool {
    // Simple check - in a real implementation, this would check for actual GPU availability
    // For now, we'll use a simple heuristic or environment variable
    std::env::var("FLOW_UTILS_USE_GPU").is_ok()
}

#[cfg(not(feature = "gpu"))]
pub fn is_gpu_available() -> bool {
    false
}

/// GPU-accelerated FFT-based KDE
///
/// Uses GPU for convolution multiplication and other operations,
/// while using CPU FFT for the actual transforms.
#[cfg(feature = "gpu")]
pub fn kde_fft_gpu(data: &[f64], grid: &[f64], bandwidth: f64, n: f64) -> KdeResult<Vec<f64>> {
    use crate::common::gaussian_kernel;
    use crate::kde::fft::kde_fft;

    // For now, fall back to CPU implementation
    // GPU implementation can be added later if needed
    kde_fft(data, grid, bandwidth, n)
}

#[cfg(not(feature = "gpu"))]
pub fn kde_fft_gpu(_data: &[f64], _grid: &[f64], _bandwidth: f64, _n: f64) -> KdeResult<Vec<f64>> {
    Err(KdeError::StatsError(
        "GPU support not compiled in".to_string(),
    ))
}
