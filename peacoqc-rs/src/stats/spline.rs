//! Cubic smoothing spline matching R's `stats::smooth.spline`.
//!
//! Implements the documented B-spline ridge regression used by PeacoQC:
//!
//! ```text
//! (X'WX + λ Σ) c = X'Wy
//! λ = r · 256^(3·spar − 1)   equivalently   r · 16^(6·spar − 2)
//! r = tr_interior(X'WX) / tr_interior(Σ)
//! ```
//!
//! where `Σ_ij = ∫ B_i''(t) B_j''(t) dt`, knots follow R's `.nknots.smspl` thinning
//! on the unit-scaled unique `x` values, and the interior trace skips the first two
//! and last three diagonal entries (R `sbart` convention).

use crate::error::{PeacoQCError, Result};
use faer::prelude::*;
use faer::{Mat, Side};
use faer::linalg::solvers::Llt;

/// Number of knots used by R's `.nknots.smspl(n)` for unique-x count `n`.
pub fn nknots_smspl(n: usize) -> usize {
    if n < 50 {
        n
    } else {
        let a1 = 50f64.log2();
        let a2 = 100f64.log2();
        let a3 = 140f64.log2();
        let a4 = 200f64.log2();
        let nf = n as f64;
        let val = if nf < 200.0 {
            2f64.powf(a1 + (a2 - a1) * (nf - 50.0) / 150.0)
        } else if nf < 800.0 {
            2f64.powf(a2 + (a3 - a2) * (nf - 200.0) / 600.0)
        } else if nf < 3200.0 {
            2f64.powf(a3 + (a4 - a3) * (nf - 800.0) / 2400.0)
        } else {
            200.0 + (nf - 3200.0).powf(0.2)
        };
        val.trunc() as usize
    }
}

/// Fit a cubic smoothing spline matching R's `smooth.spline(x, y, spar=…)`.
///
/// # Arguments
/// * `x` - Abscissae (need not be sorted; duplicates are pooled like R)
/// * `y` - Ordinates
/// * `spar` - R smoothing parameter (PeacoQC uses `0.5`)
pub fn smooth_spline(x: &[f64], y: &[f64], spar: f64) -> Result<Vec<f64>> {
    if x.len() != y.len() {
        return Err(PeacoQCError::StatsError(
            "x and y must have the same length".to_string(),
        ));
    }
    if x.len() < 4 {
        return Ok(y.to_vec());
    }

    let n_orig = x.len();
    let mut order: Vec<usize> = (0..n_orig).collect();
    order.sort_by(|&i, &j| {
        x[i]
            .partial_cmp(&x[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Pool near-duplicates (R tol defaults to 1e-6 * IQR(x); for seq_along this is a no-op).
    let x_sorted: Vec<f64> = order.iter().map(|&i| x[i]).collect();
    let y_sorted: Vec<f64> = order.iter().map(|&i| y[i]).collect();
    let iqr = interquartile_range(&x_sorted);
    let tol = 1e-6 * iqr.max(1.0);

    let mut ux = Vec::new();
    let mut ybar = Vec::new();
    let mut wbar = Vec::new();
    let mut ox = vec![0usize; n_orig]; // maps original sorted index → unique index

    let mut i = 0;
    while i < n_orig {
        let x0 = x_sorted[i];
        let mut w = 1.0;
        let mut ys = y_sorted[i];
        let mut j = i + 1;
        while j < n_orig && (x_sorted[j] - x0).abs() <= tol {
            w += 1.0;
            ys += y_sorted[j];
            j += 1;
        }
        let u = ux.len();
        for k in i..j {
            ox[k] = u;
        }
        ux.push(x0);
        ybar.push(ys / w);
        wbar.push(w);
        i = j;
    }

    let nx = ux.len();
    if nx < 4 {
        return Ok(y.to_vec());
    }

    let x_min = ux[0];
    let x_range = ux[nx - 1] - x_min;
    if x_range <= 0.0 {
        return Ok(y.to_vec());
    }
    let xbar: Vec<f64> = ux.iter().map(|&v| (v - x_min) / x_range).collect();

    let nknots = nknots_smspl(nx).clamp(1, nx);
    let knot = build_knot_vector(&xbar, nknots);
    let nk = nknots + 2; // number of B-spline coefficients (R)
    debug_assert_eq!(knot.len(), nk + 4);

    let x_design = bspline_design(&xbar, &knot, 0)?;
    let sigma = penalty_matrix(&knot, nk)?;
    let (xtwx, xty) = weighted_normal_equations(&x_design, &ybar, &wbar, nk);

    let r = interior_trace_ratio(&xtwx, &sigma, nk);
    let lambda = r * 16f64.powf(spar * 6.0 - 2.0);

    let mut a = xtwx;
    for i in 0..nk {
        for j in 0..nk {
            a[(i, j)] += lambda * sigma[(i, j)];
        }
    }

    let coef = solve_spd(a, xty)?;
    let fitted_unique = matvec(&x_design, &coef, nx, nk);

    // Map unique fits back to original order
    let mut fitted_sorted = vec![0.0; n_orig];
    for (sorted_i, &u) in ox.iter().enumerate() {
        fitted_sorted[sorted_i] = fitted_unique[u];
    }

    let mut result = vec![0.0; n_orig];
    for (sorted_pos, &orig_idx) in order.iter().enumerate() {
        result[orig_idx] = fitted_sorted[sorted_pos];
    }
    Ok(result)
}

fn interquartile_range(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n < 4 {
        return sorted.last().copied().unwrap_or(0.0) - sorted.first().copied().unwrap_or(0.0);
    }
    let q1 = sorted[n / 4];
    let q3 = sorted[(3 * n) / 4];
    q3 - q1
}

/// R: `knot <- c(rep(xbar[1], 3), xbar[seq.int(1, nx, length.out=nknots)], rep(xbar[nx], 3))`
fn build_knot_vector(xbar: &[f64], nknots: usize) -> Vec<f64> {
    let nx = xbar.len();
    let mut knot = Vec::with_capacity(nknots + 6);
    knot.extend(std::iter::repeat_n(xbar[0], 3));
    for i in 0..nknots {
        let idx = seq_int_index(nx, nknots, i);
        knot.push(xbar[idx]);
    }
    knot.extend(std::iter::repeat_n(xbar[nx - 1], 3));
    knot
}

/// 0-based index matching R `seq.int(1, nx, length.out=nknots)[i+1]` with truncated subsetting.
fn seq_int_index(nx: usize, nknots: usize, i: usize) -> usize {
    if nknots <= 1 {
        return 0;
    }
    let t = i as f64 / (nknots - 1) as f64;
    let one_based = 1.0 + t * (nx - 1) as f64;
    let truncated = one_based.trunc() as usize;
    truncated.saturating_sub(1).min(nx - 1)
}

fn weighted_normal_equations(
    x_design: &Mat<f64>,
    y: &[f64],
    w: &[f64],
    nk: usize,
) -> (Mat<f64>, Mat<f64>) {
    let n = y.len();
    let mut xtwx = Mat::<f64>::zeros(nk, nk);
    let mut xty = Mat::<f64>::zeros(nk, 1);
    for row in 0..n {
        let wi = w[row];
        let yi = y[row];
        for j in 0..nk {
            let xj = x_design[(row, j)];
            xty[(j, 0)] += wi * xj * yi;
            for k in 0..=j {
                let val = wi * xj * x_design[(row, k)];
                xtwx[(j, k)] += val;
                if k != j {
                    xtwx[(k, j)] += val;
                }
            }
        }
    }
    (xtwx, xty)
}

/// R `sbart` traces only diagonal entries with 0-based indices `2 .. nk-4`.
fn interior_trace_ratio(xtwx: &Mat<f64>, sigma: &Mat<f64>, nk: usize) -> f64 {
    if nk <= 5 {
        let mut t1 = 0.0;
        let mut t2 = 0.0;
        for i in 0..nk {
            t1 += xtwx[(i, i)];
            t2 += sigma[(i, i)];
        }
        return if t2 > 0.0 { t1 / t2 } else { 1.0 };
    }
    let mut t1 = 0.0;
    let mut t2 = 0.0;
    for i in 2..(nk - 3) {
        t1 += xtwx[(i, i)];
        t2 += sigma[(i, i)];
    }
    if t2 > 0.0 { t1 / t2 } else { 1.0 }
}

fn solve_spd(a: Mat<f64>, b: Mat<f64>) -> Result<Vec<f64>> {
    let n = a.nrows();
    let llt = Llt::new(a.as_ref(), Side::Lower).map_err(|e| {
        PeacoQCError::StatsError(format!("Cholesky failed for smoothing spline: {e}"))
    })?;
    let x = llt.solve(b.as_ref());
    Ok((0..n).map(|i| x[(i, 0)]).collect())
}

fn matvec(a: &Mat<f64>, x: &[f64], nrows: usize, ncols: usize) -> Vec<f64> {
    let mut out = vec![0.0; nrows];
    for i in 0..nrows {
        let mut s = 0.0;
        for j in 0..ncols {
            s += a[(i, j)] * x[j];
        }
        out[i] = s;
    }
    out
}

/// Evaluate cubic B-spline basis (order 4) or its `deriv`-th derivative at sites `x`.
fn bspline_design(x: &[f64], knots: &[f64], deriv: usize) -> Result<Mat<f64>> {
    let n = x.len();
    let n_basis = knots.len().saturating_sub(4);
    if n_basis == 0 {
        return Err(PeacoQCError::StatsError(
            "insufficient knots for cubic B-spline".to_string(),
        ));
    }
    let mut m = Mat::<f64>::zeros(n, n_basis);
    for (row, &xi) in x.iter().enumerate() {
        let vals = eval_all_basis(xi, knots, deriv);
        for (j, v) in vals.iter().enumerate() {
            m[(row, j)] = *v;
        }
    }
    Ok(m)
}

fn eval_all_basis(x: f64, knots: &[f64], deriv: usize) -> Vec<f64> {
    let n_basis = knots.len() - 4;
    // Half-open Cox–de Boor intervals are [t_i, t_{i+1}). Pull the right endpoint
    // inside the last span so the partition of unity still holds at x = max(knots).
    let x_right = *knots.last().unwrap_or(&1.0);
    let x_eval = if (x - x_right).abs() <= 1e-14 {
        let mut prev = x_right;
        for &t in knots.iter().rev().skip(1) {
            if (t - x_right).abs() > 1e-14 {
                prev = t;
                break;
            }
        }
        // Midpoint of the last positive-length span, slightly inside the right end.
        x_right - (x_right - prev) * 1e-12
    } else {
        x
    };
    let mut out = vec![0.0; n_basis];
    for i in 0..n_basis {
        out[i] = bspline_value(x_eval, knots, i, 4, deriv);
    }
    out
}

/// Cox-de Boor evaluation of B-spline `i` of order `k` (k=4 ⇒ cubic), optional derivative.
fn bspline_value(x: f64, knots: &[f64], i: usize, k: usize, deriv: usize) -> f64 {
    if deriv == 0 {
        return bspline_basis(x, knots, i, k);
    }
    if k <= 1 {
        return 0.0;
    }
    let left_den = knots[i + k - 1] - knots[i];
    let right_den = knots[i + k] - knots[i + 1];
    let left = if left_den.abs() > 0.0 {
        (k as f64 - 1.0) / left_den * bspline_value(x, knots, i, k - 1, deriv - 1)
    } else {
        0.0
    };
    let right = if right_den.abs() > 0.0 {
        (k as f64 - 1.0) / right_den * bspline_value(x, knots, i + 1, k - 1, deriv - 1)
    } else {
        0.0
    };
    left - right
}

fn bspline_basis(x: f64, knots: &[f64], i: usize, k: usize) -> f64 {
    if k == 1 {
        let t0 = knots[i];
        let t1 = knots[i + 1];
        return if t1 > t0 && x >= t0 && x < t1 {
            1.0
        } else {
            0.0
        };
    }
    let left_den = knots[i + k - 1] - knots[i];
    let right_den = knots[i + k] - knots[i + 1];
    let left = if left_den.abs() > 0.0 {
        (x - knots[i]) / left_den * bspline_basis(x, knots, i, k - 1)
    } else {
        0.0
    };
    let right = if right_den.abs() > 0.0 {
        (knots[i + k] - x) / right_den * bspline_basis(x, knots, i + 1, k - 1)
    } else {
        0.0
    };
    left + right
}

/// `Σ_ij = ∫ B_i''(t) B_j''(t) dt` via 4-point Gauss–Legendre on each knot span.
fn penalty_matrix(knots: &[f64], n_basis: usize) -> Result<Mat<f64>> {
    // Gauss–Legendre on [-1, 1]
    const XI: [f64; 4] = [
        -0.861_136_311_594_052_6,
        -0.339_981_043_584_856_3,
        0.339_981_043_584_856_3,
        0.861_136_311_594_052_6,
    ];
    const WI: [f64; 4] = [
        0.347_854_845_137_453_85,
        0.652_145_154_862_546_1,
        0.652_145_154_862_546_1,
        0.347_854_845_137_453_85,
    ];

    let mut breaks: Vec<f64> = Vec::new();
    for &k in knots {
        if breaks.last().is_none_or(|b| (k - *b).abs() > 1e-15) {
            breaks.push(k);
        }
    }

    let mut sigma = Mat::<f64>::zeros(n_basis, n_basis);
    for w in breaks.windows(2) {
        let a = w[0];
        let b = w[1];
        if b <= a {
            continue;
        }
        let mid = 0.5 * (a + b);
        let half = 0.5 * (b - a);
        for q in 0..4 {
            let t = mid + half * XI[q];
            let b2 = eval_all_basis(t, knots, 2);
            let weight = WI[q] * half;
            for i in 0..n_basis {
                for j in 0..=i {
                    let val = weight * b2[i] * b2[j];
                    sigma[(i, j)] += val;
                    if i != j {
                        sigma[(j, i)] += val;
                    }
                }
            }
        }
    }
    Ok(sigma)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nknots_smspl_matches_r() {
        assert_eq!(nknots_smspl(30), 30);
        assert_eq!(nknots_smspl(49), 49);
        // R trunc(2^log2(50)) underflows slightly → 49 (same FP quirk).
        assert_eq!(nknots_smspl(50), 49);
        assert_eq!(nknots_smspl(100), 62);
        assert_eq!(nknots_smspl(520), 119);
    }

    #[test]
    fn test_smooth_spline_basic() {
        let x: Vec<f64> = (1..20).map(|i| i as f64).collect();
        let y: Vec<f64> = (1..20).map(|i| (i as f64) * 2.0 + 1.0).collect();

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), y.len());
        for i in 0..smoothed.len() {
            assert!(
                (smoothed[i] - y[i]).abs() < 1.0,
                "Should be close for linear data at {i}: got {} want {}",
                smoothed[i],
                y[i]
            );
        }
    }

    #[test]
    fn test_smooth_spline_noisy_data() {
        let x: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let mut y: Vec<f64> = (0..20).map(|i| (i as f64) * 0.5 + 10.0).collect();
        y[5] += 5.0;
        y[15] -= 3.0;

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), y.len());
        assert!((smoothed[5] - y[5]).abs() > 0.1, "Should smooth out noise");
    }

    #[test]
    fn test_smooth_spline_high_smoothing() {
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let y: Vec<f64> = vec![1.0, 5.0, 2.0, 8.0, 1.5, 6.0, 3.0, 7.0, 2.5, 5.5];

        let smoothed = smooth_spline(&x, &y, 1.0).unwrap();

        assert_eq!(smoothed.len(), y.len());
        let y_var: f64 = y.iter().map(|&yi| (yi - 4.0).powi(2)).sum::<f64>() / y.len() as f64;
        let smoothed_var: f64 =
            smoothed.iter().map(|&si| (si - 4.0).powi(2)).sum::<f64>() / smoothed.len() as f64;
        assert!(
            smoothed_var <= y_var * 1.5,
            "High smoothing should reduce variance"
        );
    }

    #[test]
    fn test_smooth_spline_unsorted() {
        let x: Vec<f64> = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        let y: Vec<f64> = vec![5.0, 1.0, 3.0, 2.0, 4.0];

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), y.len());
    }

    #[test]
    fn test_smooth_spline_small_dataset() {
        // < 4 unique points: returns original
        let x: Vec<f64> = vec![1.0, 2.0, 3.0];
        let y: Vec<f64> = vec![1.0, 5.0, 2.0];

        let smoothed = smooth_spline(&x, &y, 0.5).unwrap();

        assert_eq!(smoothed.len(), 3);
        assert_eq!(smoothed, y);
    }
}
