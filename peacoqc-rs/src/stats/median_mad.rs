//! Median and Median Absolute Deviation (MAD) calculations
//!
//! This module provides statistical functions matching R's behavior,
//! including the MAD scale factor for consistency with standard deviation.

use crate::error::{PeacoQCError, Result};

/// Scale factor for MAD to be consistent with standard deviation for normal data.
///
/// R's `stats::mad()` uses this constant by default (`constant = 1.4826`).
/// This makes MAD comparable to standard deviation: for normally distributed data,
/// `MAD * 1.4826 ≈ SD`.
///
/// The constant is derived from: `1 / qnorm(3/4) ≈ 1.4826`
pub const MAD_SCALE_FACTOR: f64 = 1.4826;

/// Calculate median and Median Absolute Deviation (MAD) without scaling
///
/// MAD = median(|x - median(x)|)
///
/// Note: This returns the raw MAD without the scale factor.
/// For R-compatible behavior, use `median_mad_scaled()`.
///
/// # Arguments
/// * `data` - Slice of f64 values
///
/// # Returns
/// * (median, mad)
pub fn median_mad(data: &[f64]) -> Result<(f64, f64)> {
    if data.is_empty() {
        return Err(PeacoQCError::StatsError("Empty data".to_string()));
    }

    let med = calc_median(data)?;

    // Calculate absolute deviations
    let abs_devs: Vec<f64> = data.iter().map(|&x| (x - med).abs()).collect();

    let mad = calc_median(&abs_devs)?;

    Ok((med, mad))
}

/// Calculate median and MAD with R's scale factor (matches `stats::mad()`)
///
/// R equivalent:
/// ```r
/// median <- median(x)
/// mad <- mad(x)  # which is: constant * median(|x - median(x)|) where constant = 1.4826
/// ```
///
/// # Arguments
/// * `data` - Slice of f64 values
///
/// # Returns
/// * (median, scaled_mad) where scaled_mad = raw_mad * 1.4826
pub fn median_mad_scaled(data: &[f64]) -> Result<(f64, f64)> {
    let (med, raw_mad) = median_mad(data)?;
    Ok((med, raw_mad * MAD_SCALE_FACTOR))
}

/// Calculate MAD with scale factor (matches R's `stats::mad()`)
///
/// # Arguments
/// * `data` - Slice of f64 values
///
/// # Returns
/// * scaled MAD = median(|x - median(x)|) * 1.4826
pub fn mad_scaled(data: &[f64]) -> Result<f64> {
    let (_, scaled_mad) = median_mad_scaled(data)?;
    Ok(scaled_mad)
}

/// Calculate median of a slice of f64 values
pub fn calc_median(data: &[f64]) -> Result<f64> {
    if data.is_empty() {
        return Err(PeacoQCError::StatsError("Empty data".to_string()));
    }

    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let len = sorted.len();
    let median_value = if len % 2 == 0 {
        (sorted[len / 2 - 1] + sorted[len / 2]) / 2.0
    } else {
        sorted[len / 2]
    };

    Ok(median_value)
}

/// Convenience wrapper for calc_median
pub fn median(data: &[f64]) -> Result<f64> {
    calc_median(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_median_odd() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = median(&data).unwrap();
        assert_relative_eq!(result, 3.0);
    }

    #[test]
    fn test_median_even() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let result = median(&data).unwrap();
        assert_relative_eq!(result, 2.5);
    }

    #[test]
    fn test_median_mad_raw() {
        // Test raw MAD (without scale factor)
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0]; // 100 is outlier
        let (med, mad) = median_mad(&data).unwrap();
        assert_relative_eq!(med, 3.5);
        assert!(mad > 0.0);
    }

    #[test]
    fn test_median_mad_scaled() {
        // Test scaled MAD (with R's constant)
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (med, raw_mad) = median_mad(&data).unwrap();
        let (med2, scaled_mad) = median_mad_scaled(&data).unwrap();

        assert_relative_eq!(med, med2);
        assert_relative_eq!(scaled_mad, raw_mad * MAD_SCALE_FACTOR);
    }

    #[test]
    fn test_mad_scale_factor() {
        // For normal data, MAD * 1.4826 ≈ SD
        // Generate pseudo-normal data (simple symmetric distribution)
        let data = vec![-2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0];

        let (_, scaled_mad) = median_mad_scaled(&data).unwrap();

        // For this symmetric data centered at 0:
        // median = 0, raw_mad = median(|x|) = 1.0, scaled_mad = 1.4826
        assert!(
            (scaled_mad - 1.4826).abs() < 0.01,
            "scaled_mad = {}",
            scaled_mad
        );
    }

    #[test]
    fn test_mad_scaled_function() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (_, expected) = median_mad_scaled(&data).unwrap();
        let actual = mad_scaled(&data).unwrap();
        assert_relative_eq!(actual, expected);
    }
}
