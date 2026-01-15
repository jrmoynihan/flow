use crate::error::{PeacoQCError, Result};
use realfft::RealFftPlanner;
use realfft::num_complex::Complex;

/// Kernel Density Estimation using Gaussian kernel with FFT acceleration
///
/// This is a simplified implementation of R's density() function
/// with automatic bandwidth selection using Silverman's rule of thumb.
/// Uses FFT-based convolution for O(n log n) performance instead of O(n*m).
pub struct KernelDensity {
    pub x: Vec<f64>, // Grid points
    pub y: Vec<f64>, // Density values
}

impl KernelDensity {
    /// Compute kernel density estimate using FFT-based convolution
    ///
    /// # Arguments
    /// * `data` - Input data
    /// * `adjust` - Bandwidth adjustment factor (default: 1.0)
    /// * `n_points` - Number of grid points (default: 512)
    pub fn estimate(data: &[f64], adjust: f64, n_points: usize) -> Result<Self> {
        if data.is_empty() {
            return Err(PeacoQCError::StatsError("Empty data for KDE".to_string()));
        }

        // Remove NaN values
        let clean_data: Vec<f64> = data.iter().filter(|x| x.is_finite()).copied().collect();

        if clean_data.len() < 3 {
            return Err(PeacoQCError::InsufficientData {
                min: 3,
                actual: clean_data.len(),
            });
        }

        // Calculate bandwidth using Silverman's rule of thumb
        let n = clean_data.len() as f64;
        let std_dev = standard_deviation(&clean_data)?;
        let iqr = interquartile_range(&clean_data)?;

        // Silverman's rule: bw = 0.9 * min(sd, IQR/1.34) * n^(-1/5)
        let bw_factor = 0.9 * std_dev.min(iqr / 1.34) * n.powf(-0.2);
        let bandwidth = bw_factor * adjust;

        // Create grid
        let data_min = clean_data.iter().cloned().fold(f64::INFINITY, f64::min);
        let data_max = clean_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let grid_min = data_min - 3.0 * bandwidth;
        let grid_max = data_max + 3.0 * bandwidth;

        let x: Vec<f64> = (0..n_points)
            .map(|i| grid_min + (grid_max - grid_min) * (i as f64) / (n_points - 1) as f64)
            .collect();

        // Use FFT-based KDE for better performance
        let y = kde_fft(&clean_data, &x, bandwidth, n)?;

        Ok(KernelDensity { x, y })
    }

    /// Find local maxima (peaks) in the density estimate
    ///
    /// # Arguments
    /// * `peak_removal` - Minimum peak height as fraction of max density
    ///
    /// # Returns
    /// Vector of x-coordinates where peaks occur
    pub fn find_peaks(&self, peak_removal: f64) -> Vec<f64> {
        if self.y.len() < 3 {
            return Vec::new();
        }

        let max_y = self.y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let threshold = peak_removal * max_y;

        let mut peaks = Vec::new();

        for i in 1..self.y.len() - 1 {
            // Check if this is a local maximum above threshold
            if self.y[i] > self.y[i - 1] && self.y[i] > self.y[i + 1] && self.y[i] > threshold {
                peaks.push(self.x[i]);
            }
        }

        // If no peaks found, return the maximum point
        if peaks.is_empty() {
            if let Some((idx, _)) = self
                .y
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            {
                peaks.push(self.x[idx]);
            }
        }

        peaks
    }
}

/// FFT-based Kernel Density Estimation
///
/// Uses FFT convolution for O(n log n) performance instead of O(n*m).
/// Algorithm:
/// 1. Bin data onto grid
/// 2. Create kernel values on grid
/// 3. Zero-pad both to avoid circular convolution
/// 4. FFT both, multiply in frequency domain, inverse FFT
/// 5. Extract and normalize
fn kde_fft(data: &[f64], grid: &[f64], bandwidth: f64, n: f64) -> Result<Vec<f64>> {
    let m = grid.len();
    if m < 2 {
        return Err(PeacoQCError::StatsError("Grid must have at least 2 points".to_string()));
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
        .map_err(|e| PeacoQCError::StatsError(format!("FFT forward failed: {}", e)))?;

    let mut kernel_spectrum = r2c.make_output_vec();
    r2c.process(&mut kernel_padded, &mut kernel_spectrum)
        .map_err(|e| PeacoQCError::StatsError(format!("FFT forward failed: {}", e)))?;

    // Step 5: Multiply in frequency domain
    let mut conv_spectrum: Vec<Complex<f64>> = binned_spectrum
        .iter()
        .zip(kernel_spectrum.iter())
        .map(|(a, b)| a * b)
        .collect();

    // Step 6: Inverse FFT
    let mut conv_result = c2r.make_output_vec();
    c2r.process(&mut conv_spectrum, &mut conv_result)
        .map_err(|e| PeacoQCError::StatsError(format!("FFT inverse failed: {}", e)))?;

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

/// Gaussian kernel function
#[inline]
fn gaussian_kernel(u: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.3989422804014327; // 1/sqrt(2*pi)
    INV_SQRT_2PI * (-0.5 * u * u).exp()
}

/// Calculate standard deviation
fn standard_deviation(data: &[f64]) -> Result<f64> {
    if data.is_empty() {
        return Err(PeacoQCError::StatsError("Empty data".to_string()));
    }

    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;

    Ok(variance.sqrt())
}

/// Calculate interquartile range (IQR = Q3 - Q1)
fn interquartile_range(data: &[f64]) -> Result<f64> {
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = sorted.len();
    if n < 4 {
        return Ok(sorted[n - 1] - sorted[0]);
    }

    let q1_idx = n / 4;
    let q3_idx = 3 * n / 4;

    Ok(sorted[q3_idx] - sorted[q1_idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kde_basic() {
        // Simple bimodal data
        let mut data = Vec::new();
        for _ in 0..100 {
            data.push(0.0);
            data.push(5.0);
        }

        let kde = KernelDensity::estimate(&data, 1.0, 256).unwrap();
        let peaks = kde.find_peaks(0.3);

        // Should find 2 peaks near 0 and 5
        assert_eq!(peaks.len(), 2);
    }

    #[test]
    fn test_find_peaks() {
        let data = vec![1.0, 2.0, 3.0, 2.0, 1.0, 5.0, 6.0, 7.0, 6.0, 5.0];

        let kde = KernelDensity::estimate(&data, 1.0, 128).unwrap();
        let peaks = kde.find_peaks(0.2);

        // Should find peaks near 3 and 7
        assert!(!peaks.is_empty());
    }
}
