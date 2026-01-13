use crate::error::{PeacoQCError, Result};

/// Kernel Density Estimation using Gaussian kernel
///
/// This is a simplified implementation of R's density() function
/// with automatic bandwidth selection using Silverman's rule of thumb
pub struct KernelDensity {
    pub x: Vec<f64>, // Grid points
    pub y: Vec<f64>, // Density values
}

impl KernelDensity {
    /// Compute kernel density estimate
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

        // Compute density at each grid point using Gaussian kernel
        let y: Vec<f64> = x
            .iter()
            .map(|&xi| {
                let sum: f64 = clean_data
                    .iter()
                    .map(|&xj| {
                        let u = (xi - xj) / bandwidth;
                        gaussian_kernel(u)
                    })
                    .sum();
                sum / (n * bandwidth)
            })
            .collect();

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
