//! GPU context management for efficient GPU operations
//!
//! This module provides a reusable GPU context that:
//! - Caches GPU device to avoid repeated initialization
//! - Caches kernel spectra for reuse across operations
//! - Provides efficient tensor operations

use burn::backend::wgpu::WgpuDevice;
use crate::error::Result;
use realfft::num_complex::Complex;

/// GPU context that persists between operations
///
/// Reusing this context avoids the overhead of:
/// - GPU device initialization
/// - Kernel spectrum recomputation (when same size/bandwidth)
pub struct GpuContext {
    device: WgpuDevice,
    /// Cached kernel spectrum: (spectrum, fft_size, bandwidth)
    /// Cached to avoid recomputing FFT for same kernel
    cached_kernel_spectrum: Option<(Vec<Complex<f64>>, usize, f64)>,
}

impl GpuContext {
    /// Create a new GPU context
    pub fn new() -> Result<Self> {
        let device = WgpuDevice::default();
        Ok(Self {
            device,
            cached_kernel_spectrum: None,
        })
    }

    /// Get the GPU device
    pub fn device(&self) -> &WgpuDevice {
        &self.device
    }

    /// Get or compute kernel spectrum with caching
    ///
    /// Returns cached spectrum if available and matches size/bandwidth,
    /// otherwise computes and caches it.
    pub fn get_or_compute_kernel_spectrum(
        &mut self,
        kernel: &[f64],
        fft_size: usize,
        bandwidth: f64,
    ) -> Result<Vec<Complex<f64>>> {
        // Check cache
        if let Some((cached_spectrum, cached_size, cached_bw)) = &self.cached_kernel_spectrum {
            if *cached_size == fft_size && (cached_bw - bandwidth).abs() < 1e-10 {
                return Ok(cached_spectrum.clone());
            }
        }

        // Compute kernel spectrum
        use realfft::RealFftPlanner;
        let mut planner = RealFftPlanner::<f64>::new();
        let r2c = planner.plan_fft_forward(fft_size);

        // Prepare padded kernel
        let m = kernel.len();
        let mut kernel_padded = vec![0.0; fft_size];
        let kernel_start = (fft_size - m) / 2;
        let first_half = (m + 1) / 2;
        kernel_padded[kernel_start..kernel_start + first_half].copy_from_slice(&kernel[m / 2..]);
        let second_half = m / 2;
        if second_half > 0 {
            kernel_padded[..second_half].copy_from_slice(&kernel[..second_half]);
        }

        // Forward FFT
        let mut kernel_spectrum = r2c.make_output_vec();
        r2c.process(&mut kernel_padded, &mut kernel_spectrum)
            .map_err(|e| crate::error::PeacoQCError::StatsError(format!("FFT forward failed: {}", e)))?;

        // Cache result
        self.cached_kernel_spectrum = Some((kernel_spectrum.clone(), fft_size, bandwidth));

        Ok(kernel_spectrum)
    }

    /// Clear the kernel spectrum cache
    ///
    /// Useful when switching to different kernel sizes/bandwidths
    pub fn clear_kernel_cache(&mut self) {
        self.cached_kernel_spectrum = None;
    }
}
