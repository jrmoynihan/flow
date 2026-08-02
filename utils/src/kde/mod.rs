//! Kernel Density Estimation (KDE) module
//!
//! Provides FFT-accelerated KDE with optional GPU support.

mod fft;
#[cfg(feature = "gpu")]
mod gpu;
mod kde2d;

use crate::common::{interquartile_range, standard_deviation};
use thiserror::Error;

pub use fft::kde_fft;
#[cfg(feature = "gpu")]
pub use gpu::kde_fft_gpu;
pub use kde2d::KernelDensity2D;

/// Error type for KDE operations
#[derive(Error, Debug)]
pub enum KdeError {
    #[error("Empty data for KDE")]
    EmptyData,
    #[error("Insufficient data: need at least {min} points, got {actual}")]
    InsufficientData { min: usize, actual: usize },
    #[error("Statistics error: {0}")]
    StatsError(String),
    #[error("FFT error: {0}")]
    FftError(String),
}

pub type KdeResult<T> = Result<T, KdeError>;

/// Kernel Density Estimation using Gaussian kernel with FFT acceleration
///
/// This is a simplified implementation of R's density() function
/// with automatic bandwidth selection using Silverman's rule of thumb.
/// Uses FFT-based convolution for O(n log n) performance instead of O(n*m).
pub struct KernelDensity {
    /// Grid points
    pub x: Vec<f64>,
    /// Density values
    pub y: Vec<f64>,
}

impl KernelDensity {
    /// Compute kernel density estimate using FFT-based convolution
    ///
    /// # Arguments
    /// * `data` - Input data
    /// * `adjust` - Bandwidth adjustment factor (default: 1.0)
    /// * `n_points` - Number of grid points (default: 512)
    pub fn estimate(data: &[f64], adjust: f64, n_points: usize) -> KdeResult<Self> {
        if data.is_empty() {
            return Err(KdeError::EmptyData);
        }

        // Remove NaN values
        let clean_data: Vec<f64> = data.iter().filter(|x| x.is_finite()).copied().collect();

        if clean_data.len() < 3 {
            return Err(KdeError::InsufficientData {
                min: 3,
                actual: clean_data.len(),
            });
        }

        // Calculate bandwidth using Silverman's rule of thumb
        let n = clean_data.len() as f64;
        let std_dev = standard_deviation(&clean_data).map_err(|e| KdeError::StatsError(e))?;
        let iqr = interquartile_range(&clean_data).map_err(|e| KdeError::StatsError(e))?;

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
        // Use GPU if available (batched operations provide speedup even for smaller datasets)
        #[cfg(feature = "gpu")]
        let y = if gpu::is_gpu_available() {
            kde_fft_gpu(&clean_data, &x, bandwidth, n)?
        } else {
            kde_fft(&clean_data, &x, bandwidth, n)?
        };

        #[cfg(not(feature = "gpu"))]
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

    /// Get density value at a specific point using linear interpolation
    ///
    /// # Arguments
    /// * `x` - The point at which to evaluate the density
    ///
    /// # Returns
    /// The interpolated density value, or 0.0 if x is outside the grid range
    pub fn density_at(&self, x: f64) -> f64 {
        if self.x.is_empty() || self.y.is_empty() {
            return 0.0;
        }

        // Handle out-of-bounds
        if x <= self.x[0] {
            return self.y[0];
        }
        if x >= self.x[self.x.len() - 1] {
            return self.y[self.y.len() - 1];
        }

        // Find the two grid points to interpolate between
        let mut left_idx = 0;
        let mut right_idx = self.x.len() - 1;

        // Binary search for the interval
        while right_idx - left_idx > 1 {
            let mid = (left_idx + right_idx) / 2;
            if self.x[mid] <= x {
                left_idx = mid;
            } else {
                right_idx = mid;
            }
        }

        // Linear interpolation
        let x0 = self.x[left_idx];
        let x1 = self.x[right_idx];
        let y0 = self.y[left_idx];
        let y1 = self.y[right_idx];

        if (x1 - x0).abs() < 1e-10 {
            y0
        } else {
            y0 + (y1 - y0) * (x - x0) / (x1 - x0)
        }
    }
}
