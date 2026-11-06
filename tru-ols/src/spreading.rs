//! Robust spread (rSD) in asinh space and Mage empirical USE helpers.

use crate::batched_ols::run_ols_normal_equations;
use crate::error::TruOlsError;
use crate::metrics::spread_metrics;
use faer::{Mat, MatRef};

/// Default asinh cofactor when live channel scaling is unavailable.
pub const DEFAULT_ASINH_COFACTOR: f64 = 200.0;

/// Minimum unstained events before empirical USE is attempted.
pub const MIN_EMPIRICAL_USE_EVENTS: usize = 500;

/// Minimum events in a putative-negative mask before rSD is reported.
pub const MIN_SPREAD_EVENTS: usize = 30;

#[derive(Debug, Clone)]
pub struct EmpiricalUseComputed {
    pub endmember_idx: usize,
    pub endmember_name: String,
    pub rsd_full: Option<f64>,
    pub rsd_single: Option<f64>,
    pub use_value: Option<f64>,
    pub skip_reason: Option<String>,
}

pub fn asinh_transform(values: &[f64], cofactor: f64) -> Vec<f64> {
    let c = if cofactor.is_finite() && cofactor > 0.0 {
        cofactor
    } else {
        DEFAULT_ASINH_COFACTOR
    };
    values
        .iter()
        .map(|&x| {
            if x.is_finite() {
                (x / c).asinh()
            } else {
                f64::NAN
            }
        })
        .collect()
}

pub fn robust_rsd_asinh(values: &[f64], cofactor: f64) -> Option<f64> {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.len() < 2 {
        return None;
    }
    let asinh_vals = asinh_transform(&finite, cofactor);
    let rsd = spread_metrics(&asinh_vals).robust_sd;
    if rsd.is_finite() && rsd > 0.0 {
        Some(rsd)
    } else {
        None
    }
}

pub fn empirical_use_ratio(rsd_full: f64, rsd_single: f64) -> Option<f64> {
    if rsd_full.is_finite() && rsd_single.is_finite() && rsd_single > 0.0 {
        Some(rsd_full / rsd_single)
    } else {
        None
    }
}

fn single_color_column_indices(em: usize, af_col_index: Option<usize>) -> Vec<usize> {
    match af_col_index {
        Some(af_idx) if em == af_idx => vec![af_idx],
        Some(af_idx) => vec![em, af_idx],
        None => vec![em],
    }
}

fn submatrix_columns(matrix: MatRef<'_, f64>, cols: &[usize]) -> Mat<f64> {
    let n_det = matrix.nrows();
    Mat::from_fn(n_det, cols.len(), |row, j| matrix[(row, cols[j])])
}

/// Mage USE with asinh rSD and AF-inclusive single-color matrices when AF is present.
pub fn compute_empirical_use_asinh(
    unstained_observations: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
    endmember_names: &[String],
    af_col_index: Option<usize>,
    cofactor: f64,
) -> Result<Vec<EmpiricalUseComputed>, TruOlsError> {
    let n_events = unstained_observations.nrows();
    let n_endmembers = mixing_matrix.ncols();
    if n_endmembers == 0 || endmember_names.len() != n_endmembers {
        return Err(TruOlsError::LinearAlgebra(
            "endmember_names length must match mixing matrix columns".into(),
        ));
    }
    if n_events < MIN_EMPIRICAL_USE_EVENTS {
        return Err(TruOlsError::LinearAlgebra(format!(
            "need at least {MIN_EMPIRICAL_USE_EVENTS} unstained Size∩Neg events, got {n_events}"
        )));
    }

    let full_ab = run_ols_normal_equations(unstained_observations, mixing_matrix)?;

    let mut results = Vec::with_capacity(n_endmembers);
    for em in 0..n_endmembers {
        let col_vals: Vec<f64> = (0..n_events).map(|ev| full_ab[(ev, em)]).collect();
        let rsd_full = robust_rsd_asinh(&col_vals, cofactor);

        let single_cols = single_color_column_indices(em, af_col_index);
        let single_matrix = submatrix_columns(mixing_matrix, &single_cols);
        let single_ab =
            match run_ols_normal_equations(unstained_observations, single_matrix.as_ref()) {
                Ok(ab) => ab,
                Err(e) => {
                    results.push(EmpiricalUseComputed {
                        endmember_idx: em,
                        endmember_name: endmember_names[em].clone(),
                        rsd_full,
                        rsd_single: None,
                        use_value: None,
                        skip_reason: Some(e.to_string()),
                    });
                    continue;
                }
            };
        let single_vals: Vec<f64> = (0..n_events).map(|ev| single_ab[(ev, 0)]).collect();
        let rsd_single = robust_rsd_asinh(&single_vals, cofactor);

        let use_value = match (rsd_full, rsd_single) {
            (Some(f), Some(s)) => empirical_use_ratio(f, s),
            _ => None,
        };
        let skip_reason = if use_value.is_none() {
            Some("insufficient spread or singular single-color solve".into())
        } else {
            None
        };

        results.push(EmpiricalUseComputed {
            endmember_idx: em,
            endmember_name: endmember_names[em].clone(),
            rsd_full,
            rsd_single,
            use_value,
            skip_reason,
        });
    }
    Ok(results)
}

/// Per-fluor rSD on a boolean mask (same mask for left/right comparisons).
pub fn per_fluor_rsd_masked(
    left_abundances: MatRef<'_, f64>,
    right_abundances: MatRef<'_, f64>,
    masks: &[Vec<bool>],
    cofactor: f64,
) -> Vec<(Option<f64>, Option<f64>)> {
    let n_em = left_abundances
        .ncols()
        .min(right_abundances.ncols())
        .min(masks.len());
    let mut out = Vec::with_capacity(n_em);
    for em in 0..n_em {
        let mask = &masks[em];
        let left_vals: Vec<f64> = (0..left_abundances.nrows())
            .filter(|&ev| mask.get(ev).copied().unwrap_or(false))
            .map(|ev| left_abundances[(ev, em)])
            .collect();
        let right_vals: Vec<f64> = (0..right_abundances.nrows())
            .filter(|&ev| mask.get(ev).copied().unwrap_or(false))
            .map(|ev| right_abundances[(ev, em)])
            .collect();
        let left_rsd = if left_vals.len() >= MIN_SPREAD_EVENTS {
            robust_rsd_asinh(&left_vals, cofactor)
        } else {
            None
        };
        let right_rsd = if right_vals.len() >= MIN_SPREAD_EVENTS {
            robust_rsd_asinh(&right_vals, cofactor)
        } else {
            None
        };
        out.push((left_rsd, right_rsd));
    }
    out
}

pub fn quantile(values: &[f64], q: f64) -> Option<f64> {
    let mut finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return None;
    }
    finite.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q = q.clamp(0.0, 1.0);
    let idx = ((finite.len() - 1) as f64 * q).round() as usize;
    Some(finite[idx.min(finite.len() - 1)])
}

pub fn percentile_low(values: &[f64], pct: f64) -> Option<f64> {
    quantile(values, pct.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    #[test]
    fn constant_abundances_have_zero_mad() {
        assert_eq!(robust_rsd_asinh(&[1.0, 1.0, 1.0], 200.0), None);
    }

    #[test]
    fn spread_inflates_rsd() {
        let tight = robust_rsd_asinh(&[0.0, 0.1, -0.1], 200.0).unwrap();
        let wide = robust_rsd_asinh(&[-2.0, 0.0, 2.0], 200.0).unwrap();
        assert!(wide > tight);
    }

    #[test]
    fn use_ratio_is_full_over_single() {
        assert_eq!(empirical_use_ratio(2.0, 1.0), Some(2.0));
        assert_eq!(empirical_use_ratio(1.0, 0.0), None);
    }

    #[test]
    fn empirical_use_orthogonal_panel_near_one() {
        let mixing = mat![[1.0, 0.0], [0.0, 1.0]];
        let observations = Mat::from_fn(800, 2, |ev, det| {
            let base = (ev as f64 * 0.01).sin() * 0.05;
            if det == 0 { base + 0.1 } else { base + 0.2 }
        });
        let names = vec!["A".into(), "B".into()];
        let rows = compute_empirical_use_asinh(
            observations.as_ref(),
            mixing.as_ref(),
            &names,
            None,
            DEFAULT_ASINH_COFACTOR,
        )
        .expect("use");
        for row in &rows {
            let u = row.use_value.expect("use");
            assert!((u - 1.0).abs() < 0.35, "{} use={u}", row.endmember_name);
        }
    }
}
