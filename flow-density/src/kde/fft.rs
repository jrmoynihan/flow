//! FFT-based Kernel Density Estimation
//!
//! Uses FFT convolution for O(n log n) performance instead of O(n*m).

use crate::common::gaussian_kernel;
use crate::kde::{KdeError, KdeResult};
use realfft::RealFftPlanner;
use realfft::num_complex::Complex;

/// FFT-based Kernel Density Estimation
///
/// Uses FFT convolution for O(n log n) performance instead of O(n*m).
/// Algorithm:
/// 1. Bin data onto grid
/// 2. Create kernel values on grid
/// 3. Zero-pad both to avoid circular convolution
/// 4. FFT both, multiply in frequency domain, inverse FFT
/// 5. Extract and normalize
pub fn kde_fft(data: &[f64], grid: &[f64], bandwidth: f64, n: f64) -> KdeResult<Vec<f64>> {
    let m = grid.len();
    if m < 2 {
        return Err(KdeError::StatsError(
            "Grid must have at least 2 points".to_string(),
        ));
    }

    let grid_min = grid[0];
    let grid_max = grid[m - 1];
    let grid_spacing = (grid_max - grid_min) / (m - 1) as f64;

    // Step 1: Bin data onto grid
    // Count how many data points fall into each grid bin
    let mut binned = vec![0.0; m];
    for &x in data {
        let idx = ((x - grid_min) / grid_spacing).floor() as isize;
        if idx >= 0 && (idx as usize) < m {
            binned[idx as usize] += 1.0;
        }
    }

    // Step 2: Create kernel values on grid
    // Kernel is centered at grid center (index m/2), symmetric
    let kernel_center = (m - 1) as f64 / 2.0;
    let mut kernel = Vec::with_capacity(m);
    for i in 0..m {
        let grid_pos = (i as f64 - kernel_center) * grid_spacing;
        let u = grid_pos / bandwidth;
        kernel.push(gaussian_kernel(u));
    }

    // Step 3: Zero-pad to avoid circular convolution
    // Use next power of 2 >= 2*m for efficient FFT
    let fft_size = (2 * m).next_power_of_two();

    // Step 4: FFT convolution
    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(fft_size);
    let c2r = planner.plan_fft_inverse(fft_size);

    // Prepare padded arrays
    let mut binned_padded = vec![0.0; fft_size];
    binned_padded[..m].copy_from_slice(&binned);

    // For linear convolution with a symmetric kernel, we need to place the kernel
    // such that when convolved, it's centered. Since the kernel is symmetric and
    // centered at index m/2, we place it starting at position (fft_size - m) / 2
    // to center it in the padded array, then wrap around for circular convolution
    // which becomes linear convolution after extraction
    let mut kernel_padded = vec![0.0; fft_size];
    let kernel_start = (fft_size - m) / 2;
    // Place first half of kernel at the end
    let first_half = (m + 1) / 2;
    kernel_padded[kernel_start..kernel_start + first_half].copy_from_slice(&kernel[m / 2..]);
    // Place second half at the beginning (wrapped)
    let second_half = m / 2;
    if second_half > 0 {
        kernel_padded[..second_half].copy_from_slice(&kernel[..second_half]);
    }

    // Forward FFT
    let mut binned_spectrum = r2c.make_output_vec();
    r2c.process(&mut binned_padded, &mut binned_spectrum)
        .map_err(|e| KdeError::FftError(format!("FFT forward failed: {}", e)))?;

    let mut kernel_spectrum = r2c.make_output_vec();
    r2c.process(&mut kernel_padded, &mut kernel_spectrum)
        .map_err(|e| KdeError::FftError(format!("FFT forward failed: {}", e)))?;

    // Step 5: Multiply in frequency domain
    let mut conv_spectrum: Vec<Complex<f64>> = binned_spectrum
        .iter()
        .zip(kernel_spectrum.iter())
        .map(|(a, b)| a * b)
        .collect();

    // Step 6: Inverse FFT
    let mut conv_result = c2r.make_output_vec();
    c2r.process(&mut conv_spectrum, &mut conv_result)
        .map_err(|e| KdeError::FftError(format!("FFT inverse failed: {}", e)))?;

    // Step 7: Extract relevant portion and normalize
    // With the kernel centered, the valid convolution result starts at kernel_start
    // Extract m points starting from there (wrapping if needed)
    let kernel_start = (fft_size - m) / 2;
    let mut density = Vec::with_capacity(m);
    for i in 0..m {
        let idx = (kernel_start + i) % fft_size;
        density.push(conv_result[idx]);
    }

    // Normalize by fft_size (FFT doesn't normalize automatically), n, and bandwidth
    // This matches the naive implementation: sum(kernel) / (n * bandwidth)
    let density: Vec<f64> = density
        .iter()
        .map(|&val| val / (fft_size as f64 * n * bandwidth))
        .collect();

    Ok(density)
}
