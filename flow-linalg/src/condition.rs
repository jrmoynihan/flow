//! Matrix condition number (κ₂) and complexity index.

use faer::MatRef;

/// Condition number (κ₂ = σ_max / σ_min) and complexity index (log₁₀ κ).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConditionMetrics {
    pub condition_number: f64,
    pub complexity_index: f64,
    pub singular: bool,
}

/// Compute κ₂ and complexity for a (possibly rectangular) `f64` matrix via SVD.
///
/// For an m×n mixing matrix, κ₂ = σ_max / σ_min over the nonzero singular values
/// (same 2-norm condition used for square spillover matrices).
pub fn condition_metrics(matrix: MatRef<'_, f64>) -> Result<ConditionMetrics, String> {
    let m = matrix.nrows();
    let n = matrix.ncols();
    if m == 0 || n == 0 {
        return Err("condition metrics require a non-empty matrix".into());
    }
    let sigma = matrix
        .singular_values()
        .map_err(|e| format!("SVD failed while assessing matrix condition: {e:?}"))?;
    if sigma.is_empty() {
        return Err("SVD returned no singular values".into());
    }
    let sigma_max = sigma[0];
    let sigma_min = *sigma.last().unwrap_or(&0.0);
    let singular = !(sigma_min.is_finite() && sigma_min > f64::EPSILON);
    let condition_number = if singular {
        f64::INFINITY
    } else {
        sigma_max / sigma_min
    };
    let complexity_index = if !condition_number.is_finite() {
        f64::INFINITY
    } else {
        condition_number.max(1.0).log10()
    };
    Ok(ConditionMetrics {
        condition_number,
        complexity_index,
        singular,
    })
}

/// Compute κ₂ and complexity for a (possibly rectangular) `f32` matrix (promotes to f64 for SVD).
pub fn condition_metrics_f32(matrix: MatRef<'_, f32>) -> Result<ConditionMetrics, String> {
    let m = matrix.nrows();
    let n = matrix.ncols();
    if m == 0 || n == 0 {
        return Err("condition metrics require a non-empty matrix".into());
    }
    let owned = faer::Mat::<f64>::from_fn(m, n, |i, j| f64::from(matrix[(i, j)]));
    condition_metrics(owned.as_ref())
}

/// 2-norm condition number only (∞ when singular / empty SVD).
pub fn condition_number_2(matrix: MatRef<'_, f64>) -> f64 {
    condition_metrics(matrix)
        .map(|m| m.condition_number)
        .unwrap_or(f64::INFINITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::Mat;

    #[test]
    fn identity_has_kappa_one() {
        let m = Mat::<f64>::from_fn(2, 2, |i, j| if i == j { 1.0 } else { 0.0 });
        let metrics = condition_metrics(m.as_ref()).expect("metrics");
        assert!(!metrics.singular);
        assert!((metrics.condition_number - 1.0).abs() < 1e-9);
        assert!(metrics.complexity_index.abs() < 1e-9);
    }

    #[test]
    fn f32_identity_matches() {
        let m = Mat::<f32>::from_fn(2, 2, |i, j| if i == j { 1.0 } else { 0.0 });
        let metrics = condition_metrics_f32(m.as_ref()).expect("metrics");
        assert!((metrics.condition_number - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tall_rectangular_is_finite() {
        // 3 detectors × 2 endmembers (normalized-ish columns).
        let m = Mat::<f64>::from_fn(3, 2, |i, j| if i == j { 1.0 } else { 0.1 });
        let metrics = condition_metrics(m.as_ref()).expect("metrics");
        assert!(!metrics.singular);
        assert!(metrics.condition_number.is_finite());
        assert!(metrics.condition_number >= 1.0);
    }
}
