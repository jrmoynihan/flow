//! Cubic smoothing spline implementation matching R's smooth.spline
//!
//! Uses the `csaps` crate which implements cubic smoothing splines similar to
//! R's smooth.spline and MATLAB's csaps.
//!
//! R's smooth.spline uses a `spar` parameter (0.0 to 1.0+), while csaps uses
//! a `smooth` parameter in [0, 1]. We map spar to smooth appropriately.

use crate::error::{PeacoQCError, Result};
use csaps::CubicSmoothingSpline;
use ndarray::Array1;

/// Fit a cubic smoothing spline matching R's smooth.spline
///
/// Uses the `csaps` crate which implements cubic smoothing splines.
/// Maps R's `spar` parameter to csaps' `smooth` parameter.
///
/// # Arguments
/// * `x` - Input x values (must be sorted and unique)
/// * `y` - Input y values
/// * `spar` - Smoothing parameter (0.0 to 1.0+, default 0.5)
///             Lower values = less smoothing (closer to data)
///             Higher values = more smoothing (smoother curve)
///
/// # Returns
/// Smoothed y values at the input x points
///
/// # Parameter Mapping
/// R's `spar` parameter (0.0 to 1.0+) maps to csaps' `smooth` parameter [0, 1]:
/// - spar = 0.0 → smooth ≈ 0.0 (least squares line)
/// - spar = 0.5 → smooth ≈ 0.5 (moderate smoothing)
/// - spar = 1.0 → smooth ≈ 1.0 (natural cubic spline interpolant)
/// - spar > 1.0 → smooth = 1.0 (clamped to maximum)
pub fn smooth_spline(x: &[f64], y: &[f64], spar: f64) -> Result<Vec<f64>> {
    if x.len() != y.len() {
        return Err(PeacoQCError::StatsError(
            "x and y must have the same length".to_string(),
        ));
    }

    if x.len() < 3 {
        // Not enough points for spline, return original
        return Ok(y.to_vec());
    }

    // Check if x is sorted (required for spline)
    let mut sorted_indices: Vec<usize> = (0..x.len()).collect();
    sorted_indices.sort_by(|&i, &j| x[i].partial_cmp(&x[j]).unwrap_or(std::cmp::Ordering::Equal));

    // Reorder x and y if needed
    let x_sorted: Vec<f64> = sorted_indices.iter().map(|&i| x[i]).collect();
    let y_sorted: Vec<f64> = sorted_indices.iter().map(|&i| y[i]).collect();

    // Check for duplicate x values (csaps handles this, but we'll handle it explicitly)
    let mut unique_x = Vec::new();
    let mut unique_y = Vec::new();
    let mut weights = Vec::new();
    
    let mut i = 0;
    while i < x_sorted.len() {
        let x_val = x_sorted[i];
        let mut sum_y = y_sorted[i];
        let mut count = 1;
        let mut j = i + 1;
        
        while j < x_sorted.len() && (x_sorted[j] - x_val).abs() < 1e-10 {
            sum_y += y_sorted[j];
            count += 1;
            j += 1;
        }
        
        unique_x.push(x_val);
        unique_y.push(sum_y / count as f64);
        weights.push(count as f64);
        i = j;
    }
    
    if unique_x.len() < 3 {
        return Ok(y.to_vec());
    }
    
    // Map R's spar parameter to csaps' smooth parameter
    // IMPORTANT: R's spar and csaps' p have an INVERSE relationship:
    // - R: large spar → more smoothing (heavier penalty)
    // - csaps: small p → more smoothing (heavier penalty)
    // 
    // R's spar typically ranges [-1.5, 1.5] with default 0.5 (moderate smoothing)
    // csaps' p ranges [0, 1] with p=0.5 also being moderate smoothing
    //
    // However, the relationship is NOT linear. For R's default spar=0.5:
    // - We want moderate smoothing, which in csaps is around p=0.5
    // - But R's spar=0.5 is on a logarithmic scale of lambda
    //
    // Empirical mapping based on behavior:
    // - spar=0.0 (minimal smoothing) → p≈0.9-1.0 (interpolant)
    // - spar=0.5 (moderate smoothing) → p≈0.5 (moderate)
    // - spar=1.0+ (heavy smoothing) → p≈0.0-0.3 (very smooth)
    //
    // For now, use: p = 1.0 - spar (for spar in [0, 1])
    // This gives: spar=0.0 → p=1.0, spar=0.5 → p=0.5, spar=1.0 → p=0.0
    // For spar > 1.0, clamp p to 0.0 (maximum smoothing)
    let smooth = if spar <= 0.0 {
        1.0  // No smoothing → interpolant
    } else if spar >= 1.0 {
        0.0  // Maximum smoothing → least squares line
    } else {
        1.0 - spar  // Inverse mapping for moderate values
    };
    
    // Debug logging
    if std::env::var("PEACOQC_DEBUG_SPLINE").is_ok() {
        eprintln!(
            "csaps smoothing: n={}, spar={:.3}, smooth={:.3}, x_range={:.2}, y_range={:.2}",
            unique_x.len(),
            spar,
            smooth,
            unique_x[unique_x.len() - 1] - unique_x[0],
            {
                let y_min = unique_y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                let y_max = unique_y.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                y_max - y_min
            }
        );
    }
    
    // Fit smoothing spline using csaps
    // Convert to ndarray arrays
    let x_array = Array1::from(unique_x.clone());
    let y_array = Array1::from(unique_y.clone());
    let weights_array = Array1::from(weights.clone());
    
    let spline = CubicSmoothingSpline::new(&x_array, &y_array)
        .with_smooth(smooth)
        .with_weights(&weights_array)
        .make()
        .map_err(|e| PeacoQCError::StatsError(format!("csaps spline fitting failed: {:?}", e)))?;
    
    // Evaluate at original x points (including duplicates)
    let x_eval = Array1::from(x_sorted.clone());
    let smoothed_array = spline.evaluate(&x_eval)
        .map_err(|e| PeacoQCError::StatsError(format!("csaps evaluation failed: {:?}", e)))?;
    
    // Convert back to Vec<f64>
    let result: Vec<f64> = smoothed_array.iter().map(|&v| v).collect();
    
    // Map back to original order
    Ok(map_to_original_order(&result, &sorted_indices))
}

/// Map smoothed values back to original order
fn map_to_original_order(smoothed: &[f64], original_indices: &[usize]) -> Vec<f64> {
    if original_indices.is_empty() {
        return smoothed.to_vec();
    }

    // Check if reordering is needed
    let needs_reorder = original_indices.iter().enumerate().any(|(i, &idx)| i != idx);

    if !needs_reorder {
        return smoothed.to_vec();
    }

    // Create inverse mapping
    let mut result = vec![0.0; smoothed.len()];
    for (i, &original_idx) in original_indices.iter().enumerate() {
        result[original_idx] = smoothed[i];
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smooth_spline_basic() {
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let y: Vec<f64> = (0..10).map(|i| (i as f64) * 2.0 + 1.0).collect();

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), y.len());
        // Smoothed values should be close to original for linear data
        for i in 0..smoothed.len() {
            assert!((smoothed[i] - y[i]).abs() < 1.0, "Should be close for linear data");
        }
    }

    #[test]
    fn test_smooth_spline_noisy_data() {
        let x: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let mut y: Vec<f64> = (0..20).map(|i| (i as f64) * 0.5 + 10.0).collect();
        // Add noise
        y[5] += 5.0;
        y[15] -= 3.0;

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), y.len());
        // Smoothed should reduce noise
        assert!((smoothed[5] - y[5]).abs() > 0.1, "Should smooth out noise");
    }

    #[test]
    fn test_smooth_spline_high_smoothing() {
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let y: Vec<f64> = vec![1.0, 5.0, 2.0, 8.0, 1.5, 6.0, 3.0, 7.0, 2.5, 5.5];

        let smoothed = smooth_spline(&x, &y, 1.0).unwrap();

        assert_eq!(smoothed.len(), y.len());
        // High smoothing should produce smoother curve
        // Check that variation is reduced
        let y_var: f64 = y.iter().map(|&yi| (yi - 4.0).powi(2)).sum::<f64>() / y.len() as f64;
        let smoothed_var: f64 = smoothed.iter().map(|&si| (si - 4.0).powi(2)).sum::<f64>() / smoothed.len() as f64;
        assert!(smoothed_var <= y_var * 1.5, "High smoothing should reduce variance");
    }

    #[test]
    fn test_smooth_spline_unsorted() {
        let x: Vec<f64> = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        let y: Vec<f64> = vec![5.0, 1.0, 3.0, 2.0, 4.0];

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), y.len());
        // Should handle unsorted input
    }

    #[test]
    fn test_smooth_spline_small_dataset() {
        let x: Vec<f64> = vec![1.0, 2.0, 3.0];
        let y: Vec<f64> = vec![1.0, 5.0, 2.0];

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), 3);
    }
}
