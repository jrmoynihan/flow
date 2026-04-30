//! Dual-run benchmark harness that executes both OLS and TRU-OLS on identical data.
//!
//! This module runs both algorithms on the same observations and mixing matrix,
//! then computes comparative metrics stored in a [`ComparisonReport`].

use crate::error::TruOlsError;
use crate::metrics::{
    ComparisonReport, SpreadMetrics, compute_fit_metrics, compute_use, dimensionality_metrics,
    spread_metrics,
};
use crate::preprocessing::solve_linear_system;
use crate::unmixing::TruOls;
use faer::{Col, Mat, MatRef};
use std::fmt::Write as _;

/// Configuration for a benchmark run.
pub struct BenchmarkConfig {
    /// Human-readable label for this dataset.
    pub dataset_label: String,
    /// Cutoff percentile for TRU-OLS (default 0.995).
    pub cutoff_percentile: f64,
    /// Index of autofluorescence endmember in the mixing matrix.
    pub autofluorescence_idx: usize,
    /// Endmember names.
    pub endmember_names: Vec<String>,
}

/// Run OLS unmixing (no truncation, no nonspecific subtraction) on the full matrix.
///
/// Returns (events × endmembers) abundance matrix.
pub fn run_ols(
    observations: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
) -> Result<Mat<f64>, TruOlsError> {
    let n_events = observations.nrows();
    let n_det = observations.ncols();
    let n_em = mixing_matrix.ncols();

    if mixing_matrix.nrows() != n_det {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_det,
            actual: mixing_matrix.nrows(),
        });
    }

    let mut result = Mat::zeros(n_events, n_em);

    if crate::use_parallel_independent_events(n_events) {
        use rayon::prelude::*;
        let mut rows: Vec<(usize, Vec<f64>)> = (0..n_events)
            .into_par_iter()
            .map(|ev| {
                let obs = Col::from_fn(n_det, |i| observations[(ev, i)]);
                let ab = solve_linear_system(mixing_matrix, obs.as_ref())?;
                let row: Vec<f64> = (0..n_em).map(|j| ab[j]).collect();
                Ok((ev, row))
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.sort_by_key(|(ev, _)| *ev);
        for (ev, row) in rows {
            for em in 0..n_em {
                result[(ev, em)] = row[em];
            }
        }
    } else {
        for ev in 0..n_events {
            let obs = Col::from_fn(n_det, |i| observations[(ev, i)]);
            let ab = solve_linear_system(mixing_matrix, obs.as_ref())?;
            for em in 0..n_em {
                result[(ev, em)] = ab[em];
            }
        }
    }

    Ok(result)
}

/// Execute both OLS and TRU-OLS on the same data and produce a [`ComparisonReport`].
///
/// # Arguments
/// * `observations` – (events × detectors) stained sample data.
/// * `unstained` – (events × detectors) unstained control data.
/// * `mixing_matrix` – (detectors × endmembers).
/// * `config` – benchmark configuration.
pub fn run_comparison(
    observations: MatRef<'_, f64>,
    unstained: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
    config: &BenchmarkConfig,
) -> Result<ComparisonReport, TruOlsError> {
    let n_events = observations.nrows();
    let n_em = mixing_matrix.ncols();
    let n_det = mixing_matrix.nrows();

    // --- OLS ---
    let ols_ab = run_ols(observations, mixing_matrix)?;

    // --- TRU-OLS ---
    let mut tru_ols = TruOls::new(
        Mat::from_fn(n_det, n_em, |i, j| mixing_matrix[(i, j)]),
        Mat::from_fn(unstained.nrows(), n_det, |i, j| unstained[(i, j)]),
        config.autofluorescence_idx,
    )?;

    if (config.cutoff_percentile - 0.995).abs() > 1e-6 {
        tru_ols.set_cutoff_percentile(config.cutoff_percentile, unstained)?;
    }

    let tru_ab = tru_ols.unmix(observations)?;

    // --- Per-endmember spread ---
    let ols_spread = per_endmember_spread(ols_ab.as_ref(), n_em);
    let tru_spread = per_endmember_spread(tru_ab.as_ref(), n_em);

    // --- Fit metrics ---
    let ols_fit = compute_fit_metrics(observations, ols_ab.as_ref(), mixing_matrix);
    let tru_fit = compute_fit_metrics(observations, tru_ab.as_ref(), mixing_matrix);

    // --- Dimensionality ---
    let dim = dimensionality_metrics(tru_ab.as_ref(), 1e-10);

    // --- USE ---
    let use_vals = compute_use(unstained, mixing_matrix, &config.endmember_names);

    Ok(ComparisonReport {
        dataset_label: config.dataset_label.clone(),
        n_events,
        n_endmembers: n_em,
        n_detectors: n_det,
        ols_spread,
        tru_ols_spread: tru_spread,
        ols_fit,
        tru_ols_fit: tru_fit,
        dimensionality: dim,
        use_values: use_vals,
        endmember_names: config.endmember_names.clone(),
    })
}

/// Human-readable **quality** comparison (spread, fit, dimensionality, USE) for [`ComparisonReport`](crate::metrics::ComparisonReport).
///
/// Wall-clock timing is not included; use Criterion benches and `docs/PROFILING.md` for performance.
pub fn comparison_report_markdown(report: &ComparisonReport) -> String {
    let mut out = String::new();

    writeln!(out, "# TRU-OLS vs OLS — {}\n", report.dataset_label).unwrap();
    writeln!(
        out,
        "Events: **{}** · Detectors: **{}** · Endmembers: **{}**\n",
        report.n_events, report.n_detectors, report.n_endmembers
    )
    .unwrap();

    let mut tru_tighter = 0usize;
    for i in 0..report.n_endmembers {
        if report.tru_ols_spread[i].robust_sd < report.ols_spread[i].robust_sd {
            tru_tighter += 1;
        }
    }
    writeln!(out, "## Summary\n").unwrap();
    writeln!(
        out,
        "- **Robust SD (rSD):** TRU-OLS shows **lower** per-endmember rSD than OLS on **{} / {}** endmembers (tighter abundance spread where lower is better).\n",
        tru_tighter, report.n_endmembers
    )
    .unwrap();
    writeln!(
        out,
        "- **R² (mean / median):** OLS **{:.4} / {:.4}** vs TRU-OLS **{:.4} / {:.4}** (fit to observations using full **M**·abundances; TRU-OLS may trade some global fit for sparsity).\n",
        report.ols_fit.r_squared_mean,
        report.ols_fit.r_squared_median,
        report.tru_ols_fit.r_squared_mean,
        report.tru_ols_fit.r_squared_median
    )
    .unwrap();
    writeln!(
        out,
        "- **Residuals (|·| mean / max):** OLS **{:.4} / {:.4}** vs TRU-OLS **{:.4} / {:.4}**.\n",
        report.ols_fit.residual_abs_mean,
        report.ols_fit.residual_abs_max,
        report.tru_ols_fit.residual_abs_mean,
        report.tru_ols_fit.residual_abs_max
    )
    .unwrap();
    writeln!(
        out,
        "- **TRU-OLS active endmembers / event (median):** **{:.2}** of {} (non-zero threshold in `dimensionality_metrics`).\n",
        report.dimensionality.median_relevant,
        report.dimensionality.total_endmembers
    )
    .unwrap();

    writeln!(out, "## Per-endmember spread\n").unwrap();
    writeln!(
        out,
        "| Endmember | OLS rSD | TRU rSD | OLS CV % | TRU CV % | OLS mean | TRU mean |\n|---|---:|---:|---:|---:|---:|---:|"
    )
    .unwrap();
    for i in 0..report.n_endmembers {
        let name = report
            .endmember_names
            .get(i)
            .map(String::as_str)
            .unwrap_or("?");
        let o = &report.ols_spread[i];
        let t = &report.tru_ols_spread[i];
        writeln!(
            out,
            "| {} | {:.6} | {:.6} | {:.4} | {:.4} | {:.6} | {:.6} |",
            name, o.robust_sd, t.robust_sd, o.cv, t.cv, o.mean, t.mean
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    writeln!(
        out,
        "## Unmixing spreading error (USE, unstained control)\n"
    )
    .unwrap();
    writeln!(
        out,
        "| Endmember | rSD full panel | rSD single-dye | USE |\n|---|---:|---:|---:|"
    )
    .unwrap();
    for u in &report.use_values {
        writeln!(
            out,
            "| {} | {:.6} | {:.6} | {:.4} |",
            u.endmember_name, u.rsd_full, u.rsd_single, u.use_value
        )
        .unwrap();
    }
    writeln!(out).unwrap();

    writeln!(out, "## Goodness-of-fit (full mixing matrix)\n").unwrap();
    writeln!(
        out,
        "| Method | R² mean | R² median | |residual| mean | |residual| median | |residual| max |\n|---|---:|---:|---:|---:|---:|"
    )
    .unwrap();
    writeln!(
        out,
        "| OLS | {:.6} | {:.6} | {:.6} | {:.6} | {:.6} |",
        report.ols_fit.r_squared_mean,
        report.ols_fit.r_squared_median,
        report.ols_fit.residual_abs_mean,
        report.ols_fit.residual_abs_median,
        report.ols_fit.residual_abs_max
    )
    .unwrap();
    writeln!(
        out,
        "| TRU-OLS | {:.6} | {:.6} | {:.6} | {:.6} | {:.6} |",
        report.tru_ols_fit.r_squared_mean,
        report.tru_ols_fit.r_squared_median,
        report.tru_ols_fit.residual_abs_mean,
        report.tru_ols_fit.residual_abs_median,
        report.tru_ols_fit.residual_abs_max
    )
    .unwrap();

    out
}

fn per_endmember_spread(abundances: MatRef<'_, f64>, n_em: usize) -> Vec<SpreadMetrics> {
    let n_events = abundances.nrows();
    if crate::use_parallel_independent_events(n_events) && n_em > 1 {
        use rayon::prelude::*;
        let mut cols: Vec<(usize, SpreadMetrics)> = (0..n_em)
            .into_par_iter()
            .map(|em| {
                let vals: Vec<f64> = (0..n_events).map(|ev| abundances[(ev, em)]).collect();
                (em, spread_metrics(&vals))
            })
            .collect();
        cols.sort_by_key(|(em, _)| *em);
        cols.into_iter().map(|(_, m)| m).collect()
    } else {
        (0..n_em)
            .map(|em| {
                let vals: Vec<f64> = (0..n_events).map(|ev| abundances[(ev, em)]).collect();
                spread_metrics(&vals)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    fn simple_fixture() -> (Mat<f64>, Mat<f64>, Mat<f64>) {
        let mixing = mat![[1.0, 0.2, 0.0], [0.0, 1.0, 0.2], [0.0, 0.0, 1.0]];
        let unstained = Mat::from_fn(50, 3, |_, _| 0.01);
        let observations = Mat::from_fn(100, 3, |ev, det| {
            if det == 0 {
                10.0 + (ev as f64) * 0.1
            } else if det == 1 {
                5.0 + (ev as f64) * 0.05
            } else {
                2.0
            }
        });
        (mixing, unstained, observations)
    }

    #[test]
    fn ols_produces_correct_shape() {
        let (mixing, _unstained, observations) = simple_fixture();
        let ab = run_ols(observations.as_ref(), mixing.as_ref()).unwrap();
        assert_eq!(ab.nrows(), 100);
        assert_eq!(ab.ncols(), 3);
    }

    #[test]
    fn comparison_report_basic() {
        let (mixing, unstained, observations) = simple_fixture();
        let config = BenchmarkConfig {
            dataset_label: "test".into(),
            cutoff_percentile: 0.995,
            autofluorescence_idx: 0,
            endmember_names: vec!["A".into(), "B".into(), "C".into()],
        };
        let report = run_comparison(
            observations.as_ref(),
            unstained.as_ref(),
            mixing.as_ref(),
            &config,
        )
        .unwrap();
        assert_eq!(report.n_events, 100);
        assert_eq!(report.ols_spread.len(), 3);
        assert_eq!(report.tru_ols_spread.len(), 3);
        assert!(report.ols_fit.r_squared_mean > 0.0);
    }

    #[test]
    fn comparison_report_markdown_contains_sections() {
        let (mixing, unstained, observations) = simple_fixture();
        let config = BenchmarkConfig {
            dataset_label: "fixture".into(),
            cutoff_percentile: 0.995,
            autofluorescence_idx: 0,
            endmember_names: vec!["A".into(), "B".into(), "C".into()],
        };
        let report = run_comparison(
            observations.as_ref(),
            unstained.as_ref(),
            mixing.as_ref(),
            &config,
        )
        .unwrap();
        let md = comparison_report_markdown(&report);
        assert!(md.contains("## Summary"));
        assert!(md.contains("Per-endmember spread"));
        assert!(md.contains("Goodness-of-fit"));
    }
}
