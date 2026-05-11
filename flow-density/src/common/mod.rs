//! Common utilities for flow-utils

/// Calculate standard deviation
pub fn standard_deviation(data: &[f64]) -> Result<f64, String> {
    if data.is_empty() {
        return Err("Empty data".to_string());
    }

    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;

    Ok(variance.sqrt())
}

/// Calculate interquartile range (IQR = Q3 - Q1)
pub fn interquartile_range(data: &[f64]) -> Result<f64, String> {
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

/// Gaussian kernel function
#[inline]
pub fn gaussian_kernel(u: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.3989422804014327; // 1/sqrt(2*pi)
    INV_SQRT_2PI * (-0.5 * u * u).exp()
}
