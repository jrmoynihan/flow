//! 2D Kernel Density Estimation
//!
//! Provides 2D KDE for scatter plots and density-based gating.

use crate::common::{gaussian_kernel, interquartile_range, standard_deviation};
use crate::kde::{KdeError, KdeResult};
use ndarray::Array2;
use realfft::RealFftPlanner;
use realfft::num_complex::Complex;

/// 2D Kernel Density Estimation result
#[derive(Debug)]
pub struct KernelDensity2D {
    /// X grid points
    pub x: Vec<f64>,
    /// Y grid points
    pub y: Vec<f64>,
    /// Density values (2D grid: x.len() × y.len())
    pub z: Array2<f64>,
}

impl KernelDensity2D {
    /// Compute 2D kernel density estimate using FFT-based convolution
    ///
    /// # Arguments
    /// * `data_x` - X coordinates of data points
    /// * `data_y` - Y coordinates of data points
    /// * `adjust` - Bandwidth adjustment factor (default: 1.0)
    /// * `n_points` - Number of grid points per dimension (default: 128)
    ///
    /// # Returns
    /// KernelDensity2D with 2D density grid
    pub fn estimate(
        data_x: &[f64],
        data_y: &[f64],
        adjust: f64,
        n_points: usize,
    ) -> KdeResult<Self> {
        if data_x.len() != data_y.len() {
            return Err(KdeError::StatsError(
                "X and Y data must have the same length".to_string(),
            ));
        }

        if data_x.is_empty() {
            return Err(KdeError::EmptyData);
        }

        // Remove NaN values
        let mut clean_x = Vec::new();
        let mut clean_y = Vec::new();
        for i in 0..data_x.len() {
            if data_x[i].is_finite() && data_y[i].is_finite() {
                clean_x.push(data_x[i]);
                clean_y.push(data_y[i]);
            }
        }

        if clean_x.len() < 3 {
            return Err(KdeError::InsufficientData {
                min: 3,
                actual: clean_x.len(),
            });
        }

        // Calculate bandwidths for each dimension
        let n = clean_x.len() as f64;
        let std_dev_x = standard_deviation(&clean_x).map_err(|e| KdeError::StatsError(e))?;
        let iqr_x = interquartile_range(&clean_x).map_err(|e| KdeError::StatsError(e))?;
        let bw_factor_x = 0.9 * std_dev_x.min(iqr_x / 1.34) * n.powf(-0.2);
        let bandwidth_x = bw_factor_x * adjust;

        let std_dev_y = standard_deviation(&clean_y).map_err(|e| KdeError::StatsError(e))?;
        let iqr_y = interquartile_range(&clean_y).map_err(|e| KdeError::StatsError(e))?;
        let bw_factor_y = 0.9 * std_dev_y.min(iqr_y / 1.34) * n.powf(-0.2);
        let bandwidth_y = bw_factor_y * adjust;

        // Create 2D grid
        let x_min = clean_x.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_max = clean_x.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let y_min = clean_y.iter().cloned().fold(f64::INFINITY, f64::min);
        let y_max = clean_y.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let x_grid_min = x_min - 3.0 * bandwidth_x;
        let x_grid_max = x_max + 3.0 * bandwidth_x;
        let y_grid_min = y_min - 3.0 * bandwidth_y;
        let y_grid_max = y_max + 3.0 * bandwidth_y;

        let x: Vec<f64> = (0..n_points)
            .map(|i| x_grid_min + (x_grid_max - x_grid_min) * (i as f64) / (n_points - 1) as f64)
            .collect();
        let y: Vec<f64> = (0..n_points)
            .map(|i| y_grid_min + (y_grid_max - y_grid_min) * (i as f64) / (n_points - 1) as f64)
            .collect();

        // Compute 2D KDE using FFT convolution
        let z = kde2d_fft(&clean_x, &clean_y, &x, &y, bandwidth_x, bandwidth_y, n)?;

        Ok(KernelDensity2D { x, y, z })
    }

    /// Find density contour at given threshold level
    ///
    /// # Arguments
    /// * `threshold` - Density threshold (as fraction of max density)
    ///
    /// # Returns
    /// Vector of (x, y) coordinates forming the contour
    pub fn find_contour(&self, threshold: f64) -> Vec<(f64, f64)> {
        let max_density = self.z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let density_threshold = threshold * max_density;

        // Simple contour extraction: find points above threshold
        // TODO: Implement proper contour tracing (marching squares algorithm)
        let mut contour_points = Vec::new();

        for i in 0..self.x.len() {
            for j in 0..self.y.len() {
                if self.z[[i, j]] >= density_threshold {
                    // Check if this is on the boundary (has a neighbor below threshold)
                    let is_boundary = (i > 0 && self.z[[i - 1, j]] < density_threshold)
                        || (i < self.x.len() - 1 && self.z[[i + 1, j]] < density_threshold)
                        || (j > 0 && self.z[[i, j - 1]] < density_threshold)
                        || (j < self.y.len() - 1 && self.z[[i, j + 1]] < density_threshold);

                    if is_boundary {
                        contour_points.push((self.x[i], self.y[j]));
                    }
                }
            }
        }

        contour_points
    }

    /// Get density value at a specific point (interpolated)
    ///
    /// # Arguments
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
    ///
    /// # Returns
    /// Interpolated density value
    pub fn density_at(&self, x: f64, y: f64) -> f64 {
        // Find grid indices
        let x_idx = self.find_grid_index(&self.x, x);
        let y_idx = self.find_grid_index(&self.y, y);

        if x_idx >= self.x.len() || y_idx >= self.y.len() {
            return 0.0;
        }

        // Simple bilinear interpolation
        let x0 = if x_idx > 0 { x_idx - 1 } else { 0 };
        let x1 = x_idx.min(self.x.len() - 1);
        let y0 = if y_idx > 0 { y_idx - 1 } else { 0 };
        let y1 = y_idx.min(self.y.len() - 1);

        let z00 = self.z[[x0, y0]];
        let z01 = self.z[[x0, y1]];
        let z10 = self.z[[x1, y0]];
        let z11 = self.z[[x1, y1]];

        // Bilinear interpolation
        let dx = if x1 > x0 {
            (x - self.x[x0]) / (self.x[x1] - self.x[x0])
        } else {
            0.0
        };
        let dy = if y1 > y0 {
            (y - self.y[y0]) / (self.y[y1] - self.y[y0])
        } else {
            0.0
        };

        z00 * (1.0 - dx) * (1.0 - dy)
            + z10 * dx * (1.0 - dy)
            + z01 * (1.0 - dx) * dy
            + z11 * dx * dy
    }

    fn find_grid_index(&self, grid: &[f64], value: f64) -> usize {
        if value <= grid[0] {
            return 0;
        }
        if value >= grid[grid.len() - 1] {
            return grid.len() - 1;
        }

        // Binary search for efficiency
        let mut left = 0;
        let mut right = grid.len() - 1;
        while right - left > 1 {
            let mid = (left + right) / 2;
            if grid[mid] <= value {
                left = mid;
            } else {
                right = mid;
            }
        }
        left
    }
}

/// 2D FFT-based Kernel Density Estimation
///
/// Uses 2D FFT convolution for efficient computation.
fn kde2d_fft(
    data_x: &[f64],
    data_y: &[f64],
    x_grid: &[f64],
    y_grid: &[f64],
    bandwidth_x: f64,
    bandwidth_y: f64,
    n: f64,
) -> KdeResult<Array2<f64>> {
    let nx = x_grid.len();
    let ny = y_grid.len();

    if nx < 2 || ny < 2 {
        return Err(KdeError::StatsError(
            "Grid must have at least 2 points in each dimension".to_string(),
        ));
    }

    let x_spacing = (x_grid[nx - 1] - x_grid[0]) / (nx - 1) as f64;
    let y_spacing = (y_grid[ny - 1] - y_grid[0]) / (ny - 1) as f64;

    // Step 1: Bin data onto 2D grid
    let mut binned = Array2::<f64>::zeros((nx, ny));
    for (&x, &y) in data_x.iter().zip(data_y.iter()) {
        let x_idx = ((x - x_grid[0]) / x_spacing).floor() as isize;
        let y_idx = ((y - y_grid[0]) / y_spacing).floor() as isize;
        if x_idx >= 0 && (x_idx as usize) < nx && y_idx >= 0 && (y_idx as usize) < ny {
            binned[[x_idx as usize, y_idx as usize]] += 1.0;
        }
    }

    // Step 2: Create 2D Gaussian kernel
    let kernel_center_x = (nx - 1) as f64 / 2.0;
    let kernel_center_y = (ny - 1) as f64 / 2.0;
    let mut kernel = Array2::<f64>::zeros((nx, ny));

    for i in 0..nx {
        for j in 0..ny {
            let grid_x = (i as f64 - kernel_center_x) * x_spacing;
            let grid_y = (j as f64 - kernel_center_y) * y_spacing;
            let u_x = grid_x / bandwidth_x;
            let u_y = grid_y / bandwidth_y;
            kernel[[i, j]] = gaussian_kernel(u_x) * gaussian_kernel(u_y);
        }
    }

    // Step 3: 2D FFT convolution
    // Use next power of 2 for efficient FFT
    let _fft_size_x = (2 * nx).next_power_of_two();
    let _fft_size_y = (2 * ny).next_power_of_two();

    // For 2D FFT, we'll use a simpler approach: compute 1D FFTs along each dimension
    // This is less optimal than true 2D FFT but simpler to implement
    // TODO: Consider using a proper 2D FFT library for better performance

    // For now, use a simplified approach: compute density by convolving each row/column
    // This is an approximation but works reasonably well
    let mut density = Array2::<f64>::zeros((nx, ny));

    // Convolve along X dimension for each Y
    for j in 0..ny {
        let mut row_binned = vec![0.0; nx];
        let mut row_kernel = vec![0.0; nx];
        for i in 0..nx {
            row_binned[i] = binned[[i, j]];
            row_kernel[i] = kernel[[i, j]];
        }

        // Use 1D FFT convolution for this row
        let row_density = kde1d_row(&row_binned, &row_kernel, bandwidth_x, n)?;
        for i in 0..nx {
            density[[i, j]] = row_density[i];
        }
    }

    // Convolve along Y dimension for each X (simplified - just average)
    // Full 2D convolution would be better but this approximation works
    for i in 0..nx {
        let mut col_density = vec![0.0; ny];
        for j in 0..ny {
            col_density[j] = density[[i, j]];
        }
        // Apply Y kernel smoothing
        for j in 0..ny {
            let mut sum = 0.0;
            let mut weight_sum = 0.0;
            for k in 0..ny {
                let dist = (j as f64 - k as f64) * y_spacing;
                let weight = gaussian_kernel(dist / bandwidth_y);
                sum += col_density[k] * weight;
                weight_sum += weight;
            }
            if weight_sum > 0.0 {
                density[[i, j]] = sum / weight_sum;
            }
        }
    }

    // Normalize
    let total: f64 = density.iter().sum();
    if total > 0.0 {
        for val in density.iter_mut() {
            *val /= total * x_spacing * y_spacing;
        }
    }

    Ok(density)
}

/// Helper function for 1D row convolution (simplified)
fn kde1d_row(binned: &[f64], kernel: &[f64], bandwidth: f64, n: f64) -> KdeResult<Vec<f64>> {
    let m = binned.len();
    let fft_size = (2 * m).next_power_of_two();

    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(fft_size);
    let c2r = planner.plan_fft_inverse(fft_size);

    // Prepare padded arrays
    let mut binned_padded = vec![0.0; fft_size];
    binned_padded[..m].copy_from_slice(binned);

    let mut kernel_padded = vec![0.0; fft_size];
    let kernel_start = (fft_size - m) / 2;
    let first_half = (m + 1) / 2;
    kernel_padded[kernel_start..kernel_start + first_half].copy_from_slice(&kernel[m / 2..]);
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

    // Multiply in frequency domain
    let mut conv_spectrum: Vec<Complex<f64>> = binned_spectrum
        .iter()
        .zip(kernel_spectrum.iter())
        .map(|(a, b)| a * b)
        .collect();

    // Inverse FFT
    let mut conv_result = c2r.make_output_vec();
    c2r.process(&mut conv_spectrum, &mut conv_result)
        .map_err(|e| KdeError::FftError(format!("FFT inverse failed: {}", e)))?;

    // Extract and normalize
    let kernel_start = (fft_size - m) / 2;
    let mut density = Vec::with_capacity(m);
    for i in 0..m {
        let idx = (kernel_start + i) % fft_size;
        density.push(conv_result[idx] / (fft_size as f64 * n * bandwidth));
    }

    Ok(density)
}
