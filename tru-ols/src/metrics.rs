//! Quantitative metrics for comparing OLS and TRU-OLS unmixing output.
//!
//! Provides spread metrics (CV, robust SD, USE), spillover metrics (SSE, SSM),
//! goodness-of-fit metrics (R², residual summaries), and dimensionality tracking.

use faer::{Col, Mat, MatRef};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Spread / variance metrics
// ---------------------------------------------------------------------------

/// Spread statistics for a single channel or population.
#[derive(Debug, Clone, Serialize)]
pub struct SpreadMetrics {
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    /// Robust SD via MAD × 1.4826 (comparable to SD for normal data).
    pub robust_sd: f64,
    /// Coefficient of variation: (std_dev / |mean|) × 100.
    pub cv: f64,
}

/// Compute [`SpreadMetrics`] from a 1-D slice.
pub fn spread_metrics(values: &[f64]) -> SpreadMetrics {
    let n = values.len() as f64;
    if values.is_empty() {
        return SpreadMetrics {
            mean: 0.0,
            median: 0.0,
            std_dev: 0.0,
            robust_sd: 0.0,
            cv: 0.0,
        };
    }

    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };

    let abs_devs: Vec<f64> = sorted.iter().map(|&x| (x - median).abs()).collect();
    let mut abs_devs_sorted = abs_devs;
    abs_devs_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad = if abs_devs_sorted.len().is_multiple_of(2) {
        (abs_devs_sorted[abs_devs_sorted.len() / 2 - 1]
            + abs_devs_sorted[abs_devs_sorted.len() / 2])
            / 2.0
    } else {
        abs_devs_sorted[abs_devs_sorted.len() / 2]
    };
    let robust_sd = mad * 1.4826;

    let cv = if mean.abs() > 1e-15 {
        (std_dev / mean.abs()) * 100.0
    } else {
        0.0
    };

    SpreadMetrics {
        mean,
        median,
        std_dev,
        robust_sd,
        cv,
    }
}

// ---------------------------------------------------------------------------
// Unmixing Spreading Error (USE)
// ---------------------------------------------------------------------------

/// USE = rSD_full / rSD_single for a given endmember on the unstained control.
///
/// * `rsd_full_matrix` – robust SD when unmixed with the full panel matrix.
/// * `rsd_single_matrix` – robust SD when unmixed with a single-endmember matrix.
///
/// Values > 1 indicate that adding other endmembers inflates variance.
#[derive(Debug, Clone, Serialize)]
pub struct UnmixingSpreadingError {
    pub endmember_idx: usize,
    pub endmember_name: String,
    pub rsd_full: f64,
    pub rsd_single: f64,
    pub use_value: f64,
}

/// Compute USE for every endmember.
///
/// `unstained_observations` – (events × detectors)
/// `mixing_matrix` – (detectors × endmembers)
pub fn compute_use(
    unstained_observations: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
    endmember_names: &[String],
) -> Vec<UnmixingSpreadingError> {
    use crate::preprocessing::solve_linear_system;

    let n_events = unstained_observations.nrows();
    let n_endmembers = mixing_matrix.ncols();
    let n_detectors = mixing_matrix.nrows();

    // Full-matrix unmix → per-endmember rSD
    let mut full_abundances: Vec<Vec<f64>> = vec![Vec::new(); n_endmembers];
    let row_opts: Vec<(usize, Option<Vec<f64>>)> =
        if crate::use_parallel_independent_events(n_events) {
            use rayon::prelude::*;
            (0..n_events)
                .into_par_iter()
                .map(|ev| {
                    let obs = Col::from_fn(n_detectors, |i| unstained_observations[(ev, i)]);
                    let opt = solve_linear_system(mixing_matrix, obs.as_ref())
                        .ok()
                        .map(|ab| (0..n_endmembers).map(|em| ab[em]).collect::<Vec<f64>>());
                    (ev, opt)
                })
                .collect()
        } else {
            let mut v = Vec::with_capacity(n_events);
            for ev in 0..n_events {
                let obs = Col::from_fn(n_detectors, |i| unstained_observations[(ev, i)]);
                let opt = solve_linear_system(mixing_matrix, obs.as_ref())
                    .ok()
                    .map(|ab| (0..n_endmembers).map(|em| ab[em]).collect::<Vec<f64>>());
                v.push((ev, opt));
            }
            v
        };
    let mut sorted_rows = row_opts;
    sorted_rows.sort_by_key(|(ev, _)| *ev);
    for (_, opt) in sorted_rows {
        if let Some(row) = opt {
            for em in 0..n_endmembers {
                full_abundances[em].push(row[em]);
            }
        }
    }
    let full_rsd: Vec<f64> = full_abundances
        .iter()
        .map(|vals| spread_metrics(vals).robust_sd)
        .collect();

    // Single-endmember unmix for each endmember
    let mut results = Vec::with_capacity(n_endmembers);
    for em in 0..n_endmembers {
        let single_col = Mat::from_fn(n_detectors, 1, |i, _| mixing_matrix[(i, em)]);
        let single_vals: Vec<f64> = if crate::use_parallel_independent_events(n_events) {
            use rayon::prelude::*;
            (0..n_events)
                .into_par_iter()
                .filter_map(|ev| {
                    let obs = Col::from_fn(n_detectors, |i| unstained_observations[(ev, i)]);
                    solve_linear_system(single_col.as_ref(), obs.as_ref())
                        .ok()
                        .map(|ab| ab[0])
                })
                .collect()
        } else {
            let mut v = Vec::with_capacity(n_events);
            for ev in 0..n_events {
                let obs = Col::from_fn(n_detectors, |i| unstained_observations[(ev, i)]);
                if let Ok(ab) = solve_linear_system(single_col.as_ref(), obs.as_ref()) {
                    v.push(ab[0]);
                }
            }
            v
        };
        let rsd_single = spread_metrics(&single_vals).robust_sd;
        let use_value = if rsd_single.abs() > 1e-15 {
            full_rsd[em] / rsd_single
        } else {
            0.0
        };

        results.push(UnmixingSpreadingError {
            endmember_idx: em,
            endmember_name: endmember_names
                .get(em)
                .cloned()
                .unwrap_or_else(|| format!("Endmember_{}", em)),
            rsd_full: full_rsd[em],
            rsd_single,
            use_value,
        });
    }
    results
}

// ---------------------------------------------------------------------------
// Spillover Spreading Matrix (SSM) / Spillover Spreading Error (SSE)
// ---------------------------------------------------------------------------

/// Spillover Spreading Matrix (n_endmembers × n_endmembers).
///
/// `ssm[(i, j)]` = rSD of endmember j on a population stained with endmember i only,
/// unmixed with the full matrix. Diagonal entries are self-spread.
#[derive(Debug, Clone, Serialize)]
pub struct SpilloverSpreadingMatrix {
    /// Row-major n×n matrix of SSE values.
    pub matrix: Vec<Vec<f64>>,
    pub endmember_names: Vec<String>,
}

/// Compute the SSM from single-stain observations.
///
/// `single_stain_data` – one `(events × detectors)` matrix per endmember.
pub fn compute_ssm(
    single_stain_data: &[MatRef<'_, f64>],
    mixing_matrix: MatRef<'_, f64>,
    endmember_names: &[String],
) -> SpilloverSpreadingMatrix {
    use crate::preprocessing::solve_linear_system;

    let n_em = mixing_matrix.ncols();
    let n_det = mixing_matrix.nrows();
    let mut matrix = vec![vec![0.0; n_em]; n_em];

    for (stain_idx, obs_mat) in single_stain_data.iter().enumerate() {
        let n_events = obs_mat.nrows();
        let mut abundances: Vec<Vec<f64>> = vec![Vec::new(); n_em];
        let row_opts: Vec<(usize, Option<Vec<f64>>)> =
            if crate::use_parallel_independent_events(n_events) {
                use rayon::prelude::*;
                (0..n_events)
                    .into_par_iter()
                    .map(|ev| {
                        let obs = Col::from_fn(n_det, |i| obs_mat[(ev, i)]);
                        let opt = solve_linear_system(mixing_matrix, obs.as_ref())
                            .ok()
                            .map(|ab| (0..n_em).map(|em| ab[em]).collect::<Vec<f64>>());
                        (ev, opt)
                    })
                    .collect()
            } else {
                let mut v = Vec::with_capacity(n_events);
                for ev in 0..n_events {
                    let obs = Col::from_fn(n_det, |i| obs_mat[(ev, i)]);
                    let opt = solve_linear_system(mixing_matrix, obs.as_ref())
                        .ok()
                        .map(|ab| (0..n_em).map(|em| ab[em]).collect::<Vec<f64>>());
                    v.push((ev, opt));
                }
                v
            };
        let mut sorted_rows = row_opts;
        sorted_rows.sort_by_key(|(ev, _)| *ev);
        for (_, opt) in sorted_rows {
            if let Some(row) = opt {
                for em in 0..n_em {
                    abundances[em].push(row[em]);
                }
            }
        }
        for em in 0..n_em {
            matrix[stain_idx][em] = spread_metrics(&abundances[em]).robust_sd;
        }
    }

    SpilloverSpreadingMatrix {
        matrix,
        endmember_names: endmember_names.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Goodness-of-fit metrics
// ---------------------------------------------------------------------------

/// Goodness-of-fit statistics for an unmixing run.
#[derive(Debug, Clone, Serialize)]
pub struct FitMetrics {
    /// R² per event.
    pub r_squared_per_event: Vec<f64>,
    /// Mean R² across all events.
    pub r_squared_mean: f64,
    /// Median R².
    pub r_squared_median: f64,
    /// Mean absolute residual across all events and detectors.
    pub residual_abs_mean: f64,
    /// Median absolute residual.
    pub residual_abs_median: f64,
    /// Maximum absolute residual.
    pub residual_abs_max: f64,
}

/// Compute fit metrics: R² and residual summaries.
///
/// * `observations` – (events × detectors) raw observations used for unmixing.
/// * `abundances` – (events × endmembers) unmixed abundances.
/// * `mixing_matrix` – (detectors × endmembers).
pub fn compute_fit_metrics(
    observations: MatRef<'_, f64>,
    abundances: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
) -> FitMetrics {
    let n_events = observations.nrows();
    let n_det = observations.ncols();
    let n_ab = abundances.ncols();

    let mut per_event: Vec<(usize, f64, Vec<f64>)> =
        if crate::use_parallel_independent_events(n_events) {
            use rayon::prelude::*;
            (0..n_events)
                .into_par_iter()
                .map(|ev| {
                    let ab_col = Col::from_fn(n_ab, |j| abundances[(ev, j)]);
                    let predicted = mixing_matrix * &ab_col;

                    let mut ss_res = 0.0;
                    let mut ss_tot = 0.0;
                    let obs_mean: f64 =
                        (0..n_det).map(|d| observations[(ev, d)]).sum::<f64>() / n_det as f64;

                    let mut abs_res = Vec::with_capacity(n_det);
                    for d in 0..n_det {
                        let obs_d = observations[(ev, d)];
                        let pred_d = predicted[d];
                        let residual = obs_d - pred_d;
                        ss_res += residual * residual;
                        ss_tot += (obs_d - obs_mean).powi(2);
                        abs_res.push(residual.abs());
                    }

                    let r2 = if ss_tot.abs() > 1e-30 {
                        1.0 - ss_res / ss_tot
                    } else {
                        1.0
                    };
                    (ev, r2, abs_res)
                })
                .collect()
        } else {
            let mut v = Vec::with_capacity(n_events);
            for ev in 0..n_events {
                let ab_col = Col::from_fn(n_ab, |j| abundances[(ev, j)]);
                let predicted = mixing_matrix * &ab_col;

                let mut ss_res = 0.0;
                let mut ss_tot = 0.0;
                let obs_mean: f64 =
                    (0..n_det).map(|d| observations[(ev, d)]).sum::<f64>() / n_det as f64;

                let mut abs_res = Vec::with_capacity(n_det);
                for d in 0..n_det {
                    let obs_d = observations[(ev, d)];
                    let pred_d = predicted[d];
                    let residual = obs_d - pred_d;
                    ss_res += residual * residual;
                    ss_tot += (obs_d - obs_mean).powi(2);
                    abs_res.push(residual.abs());
                }

                let r2 = if ss_tot.abs() > 1e-30 {
                    1.0 - ss_res / ss_tot
                } else {
                    1.0
                };
                v.push((ev, r2, abs_res));
            }
            v
        };

    per_event.sort_by_key(|(ev, _, _)| *ev);
    let mut r_sq_per_event = Vec::with_capacity(n_events);
    let mut all_abs_residuals = Vec::with_capacity(n_events * n_det);
    for (_, r2, abs_res) in per_event {
        r_sq_per_event.push(r2);
        all_abs_residuals.extend(abs_res);
    }

    let r_squared_mean = r_sq_per_event.iter().sum::<f64>() / r_sq_per_event.len().max(1) as f64;

    let mut r_sorted = r_sq_per_event.clone();
    r_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let r_squared_median = if r_sorted.is_empty() {
        0.0
    } else if r_sorted.len() % 2 == 0 {
        (r_sorted[r_sorted.len() / 2 - 1] + r_sorted[r_sorted.len() / 2]) / 2.0
    } else {
        r_sorted[r_sorted.len() / 2]
    };

    all_abs_residuals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let residual_abs_mean =
        all_abs_residuals.iter().sum::<f64>() / all_abs_residuals.len().max(1) as f64;
    let residual_abs_median = if all_abs_residuals.is_empty() {
        0.0
    } else if all_abs_residuals.len() % 2 == 0 {
        (all_abs_residuals[all_abs_residuals.len() / 2 - 1]
            + all_abs_residuals[all_abs_residuals.len() / 2])
            / 2.0
    } else {
        all_abs_residuals[all_abs_residuals.len() / 2]
    };
    let residual_abs_max = all_abs_residuals.last().copied().unwrap_or(0.0);

    FitMetrics {
        r_squared_per_event: r_sq_per_event,
        r_squared_mean,
        r_squared_median,
        residual_abs_mean,
        residual_abs_median,
        residual_abs_max,
    }
}

// ---------------------------------------------------------------------------
// Dimensionality / relevant-dye tracking
// ---------------------------------------------------------------------------

/// Summary of how many endmembers TRU-OLS retained per event.
#[derive(Debug, Clone, Serialize)]
pub struct DimensionalityMetrics {
    /// Number of relevant endmembers per event.
    pub relevant_counts: Vec<usize>,
    /// Mean across events.
    pub mean_relevant: f64,
    /// Median across events.
    pub median_relevant: f64,
    /// Total endmembers in the panel.
    pub total_endmembers: usize,
}

/// Derive dimensionality metrics from a TRU-OLS abundance matrix.
///
/// A non-zero abundance (absolute value > `zero_threshold`) is counted as relevant.
pub fn dimensionality_metrics(
    tru_ols_abundances: MatRef<'_, f64>,
    zero_threshold: f64,
) -> DimensionalityMetrics {
    let n_events = tru_ols_abundances.nrows();
    let n_em = tru_ols_abundances.ncols();

    let mut indexed: Vec<(usize, usize)> = if crate::use_parallel_independent_events(n_events) {
        use rayon::prelude::*;
        (0..n_events)
            .into_par_iter()
            .map(|ev| {
                let count = (0..n_em)
                    .filter(|&em| tru_ols_abundances[(ev, em)].abs() > zero_threshold)
                    .count();
                (ev, count)
            })
            .collect()
    } else {
        let mut v = Vec::with_capacity(n_events);
        for ev in 0..n_events {
            let count = (0..n_em)
                .filter(|&em| tru_ols_abundances[(ev, em)].abs() > zero_threshold)
                .count();
            v.push((ev, count));
        }
        v
    };
    indexed.sort_by_key(|(ev, _)| *ev);
    let counts: Vec<usize> = indexed.into_iter().map(|(_, c)| c).collect();

    let mean = counts.iter().sum::<usize>() as f64 / counts.len().max(1) as f64;
    let mut sorted_counts = counts.clone();
    sorted_counts.sort_unstable();
    let median = if sorted_counts.is_empty() {
        0.0
    } else if sorted_counts.len().is_multiple_of(2) {
        (sorted_counts[sorted_counts.len() / 2 - 1] + sorted_counts[sorted_counts.len() / 2]) as f64
            / 2.0
    } else {
        sorted_counts[sorted_counts.len() / 2] as f64
    };

    DimensionalityMetrics {
        relevant_counts: counts,
        mean_relevant: mean,
        median_relevant: median,
        total_endmembers: n_em,
    }
}

// ---------------------------------------------------------------------------
// Composite benchmark report
// ---------------------------------------------------------------------------

/// Full comparison report for one dataset processed by both OLS and TRU-OLS.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    pub dataset_label: String,
    pub n_events: usize,
    pub n_endmembers: usize,
    pub n_detectors: usize,
    /// Per-endmember spread metrics for OLS.
    pub ols_spread: Vec<SpreadMetrics>,
    /// Per-endmember spread metrics for TRU-OLS.
    pub tru_ols_spread: Vec<SpreadMetrics>,
    /// Fit quality for OLS.
    pub ols_fit: FitMetrics,
    /// Fit quality for TRU-OLS.
    pub tru_ols_fit: FitMetrics,
    /// Dimensionality summary (TRU-OLS only).
    pub dimensionality: DimensionalityMetrics,
    /// USE values per endmember.
    pub use_values: Vec<UnmixingSpreadingError>,
    /// Endmember names.
    pub endmember_names: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    #[test]
    fn spread_metrics_basic() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let m = spread_metrics(&vals);
        assert!((m.mean - 3.0).abs() < 1e-10);
        assert!((m.median - 3.0).abs() < 1e-10);
        assert!(m.std_dev > 0.0);
        assert!(m.robust_sd > 0.0);
    }

    #[test]
    fn fit_perfect_recovery() {
        let mixing = mat![[1.0, 0.0], [0.0, 1.0]];
        let abundances = mat![[2.0, 3.0], [4.0, 5.0]];
        let observations = mat![[2.0, 3.0], [4.0, 5.0]];
        let fit = compute_fit_metrics(observations.as_ref(), abundances.as_ref(), mixing.as_ref());
        for &r2 in &fit.r_squared_per_event {
            assert!(r2 > 0.999, "expected near-perfect R², got {}", r2);
        }
        assert!(fit.residual_abs_max < 1e-10);
    }

    #[test]
    fn dimensionality_with_zeros() {
        let ab = mat![[1.0, 0.0, 3.0], [0.0, 0.0, 0.0], [1.0, 2.0, 3.0]];
        let dm = dimensionality_metrics(ab.as_ref(), 1e-10);
        assert_eq!(dm.relevant_counts, vec![2, 0, 3]);
        assert!((dm.median_relevant - 2.0).abs() < 1e-10);
    }
}
