use crate::error::{PeacoQCError, Result};

/// Calculate median and Median Absolute Deviation (MAD)
///
/// MAD = median(|x - median(x)|)
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
    let mut abs_devs: Vec<f64> = data.iter().map(|&x| (x - med).abs()).collect();

    let mad = calc_median(&abs_devs)?;

    Ok((med, mad))
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
    fn test_median_mad() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0]; // 100 is outlier
        let (med, mad) = median_mad(&data).unwrap();
        assert_relative_eq!(med, 3.5);
        assert!(mad > 0.0);
    }
}
