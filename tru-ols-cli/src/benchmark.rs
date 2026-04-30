//! CLI benchmark runner: orchestrates synthetic data generation, dual OLS/TRU-OLS
//! execution, metric extraction, and report output.

use anyhow::{Context, Result};
use faer::Mat;
use flow_tru_ols::{BenchmarkConfig, ComparisonReport, SpilloverSpreadingMatrix, run_comparison};
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::synthetic_data::{SyntheticBenchmarkData, generate_benchmark_suite};

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Aggregate benchmark report covering multiple datasets.
#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub datasets: Vec<DatasetReport>,
}

/// Per-dataset report combining comparison metrics and optional SSM.
#[derive(Debug, Serialize)]
pub struct DatasetReport {
    pub comparison: ComparisonReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssm: Option<SpilloverSpreadingMatrix>,
    /// Ground-truth RMSE per endmember (synthetic only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ground_truth_rmse_ols: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ground_truth_rmse_tru_ols: Option<Vec<f64>>,
}

// ---------------------------------------------------------------------------
// Benchmark execution
// ---------------------------------------------------------------------------

/// Run the full synthetic benchmark suite and produce a [`BenchmarkReport`].
pub fn run_synthetic_benchmark(
    n_events: usize,
    n_unstained: usize,
    noise_levels: &[f64],
) -> Result<BenchmarkReport> {
    let datasets = generate_benchmark_suite(n_events, n_unstained, noise_levels)?;

    let mut reports = Vec::with_capacity(datasets.len());
    for data in &datasets {
        let report = benchmark_single_dataset(data)?;
        reports.push(report);
    }

    Ok(BenchmarkReport { datasets: reports })
}

/// Run both algorithms on a single synthetic dataset and compute all metrics.
fn benchmark_single_dataset(data: &SyntheticBenchmarkData) -> Result<DatasetReport> {
    let n_em = data.endmember_names.len();
    let n_events = data.observations.nrows();

    let mixing = ndarray_to_faer(&data.mixing_matrix);
    let obs = ndarray_to_faer(&data.observations);
    let unstained = ndarray_to_faer(&data.unstained_observations);

    let config = BenchmarkConfig {
        dataset_label: data.label.clone(),
        cutoff_percentile: 0.995,
        autofluorescence_idx: 0,
        endmember_names: data.endmember_names.clone(),
    };

    let comparison = run_comparison(obs.as_ref(), unstained.as_ref(), mixing.as_ref(), &config)
        .with_context(|| format!("Comparison failed for dataset '{}'", data.label))?;

    // Ground-truth RMSE (OLS and TRU-OLS vs true abundances)
    let ols_ab =
        flow_tru_ols::run_ols(obs.as_ref(), mixing.as_ref()).with_context(|| "OLS failed")?;

    let tru_ols_engine = flow_tru_ols::TruOls::new(
        mixing.clone(),
        unstained.clone(),
        config.autofluorescence_idx,
    )?;
    let tru_ab = tru_ols_engine.unmix(obs.as_ref())?;

    let ols_rmse = per_endmember_rmse(&data.true_abundances, &ols_ab, n_em, n_events);
    let tru_rmse = per_endmember_rmse(&data.true_abundances, &tru_ab, n_em, n_events);

    Ok(DatasetReport {
        comparison,
        ssm: None, // SSM requires single-stain data; skipped for synthetic
        ground_truth_rmse_ols: Some(ols_rmse),
        ground_truth_rmse_tru_ols: Some(tru_rmse),
    })
}

fn per_endmember_rmse(
    truth: &ndarray::Array2<f64>,
    estimated: &Mat<f64>,
    n_em: usize,
    n_events: usize,
) -> Vec<f64> {
    (0..n_em)
        .map(|em| {
            let mse: f64 = (0..n_events)
                .map(|ev| {
                    let diff = truth[(ev, em)] - estimated[(ev, em)];
                    diff * diff
                })
                .sum::<f64>()
                / n_events as f64;
            mse.sqrt()
        })
        .collect()
}

fn ndarray_to_faer(arr: &ndarray::Array2<f64>) -> Mat<f64> {
    Mat::from_fn(arr.nrows(), arr.ncols(), |i, j| arr[(i, j)])
}

// ---------------------------------------------------------------------------
// Report output
// ---------------------------------------------------------------------------

/// Write the benchmark report as JSON.
pub fn write_json_report(report: &BenchmarkReport, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report)
        .context("Failed to serialize benchmark report to JSON")?;
    fs::write(path, json)
        .with_context(|| format!("Failed to write JSON report to {}", path.display()))?;
    Ok(())
}

/// Write a human-readable markdown summary.
pub fn write_markdown_report(report: &BenchmarkReport, path: &Path) -> Result<()> {
    let mut md = String::new();
    md.push_str("# TRU-OLS vs OLS Benchmark Report\n\n");

    for ds in &report.datasets {
        let c = &ds.comparison;
        md.push_str(&format!("## {}\n\n", c.dataset_label));
        md.push_str(&format!(
            "- Events: {}, Detectors: {}, Endmembers: {}\n",
            c.n_events, c.n_detectors, c.n_endmembers
        ));
        md.push_str(&format!(
            "- OLS  R² mean: {:.4}, median: {:.4}\n",
            c.ols_fit.r_squared_mean, c.ols_fit.r_squared_median
        ));
        md.push_str(&format!(
            "- TRU  R² mean: {:.4}, median: {:.4}\n",
            c.tru_ols_fit.r_squared_mean, c.tru_ols_fit.r_squared_median
        ));
        md.push_str(&format!(
            "- OLS  |residual| mean: {:.6}\n",
            c.ols_fit.residual_abs_mean
        ));
        md.push_str(&format!(
            "- TRU  |residual| mean: {:.6}\n",
            c.tru_ols_fit.residual_abs_mean
        ));
        md.push_str(&format!(
            "- Median relevant dyes: {:.1} / {}\n\n",
            c.dimensionality.median_relevant, c.dimensionality.total_endmembers
        ));

        // Spread comparison table
        md.push_str("| Endmember | OLS CV | TRU CV | OLS rSD | TRU rSD | USE |\n");
        md.push_str("|-----------|--------|--------|---------|---------|-----|\n");
        for (i, name) in c.endmember_names.iter().enumerate() {
            let ols_cv = c.ols_spread.get(i).map(|s| s.cv).unwrap_or(0.0);
            let tru_cv = c.tru_ols_spread.get(i).map(|s| s.cv).unwrap_or(0.0);
            let ols_rsd = c.ols_spread.get(i).map(|s| s.robust_sd).unwrap_or(0.0);
            let tru_rsd = c.tru_ols_spread.get(i).map(|s| s.robust_sd).unwrap_or(0.0);
            let use_val = c.use_values.get(i).map(|u| u.use_value).unwrap_or(0.0);
            md.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:.4} | {:.4} | {:.2} |\n",
                name, ols_cv, tru_cv, ols_rsd, tru_rsd, use_val
            ));
        }
        md.push('\n');

        // Ground-truth RMSE if present
        if let (Some(ols_rmse), Some(tru_rmse)) =
            (&ds.ground_truth_rmse_ols, &ds.ground_truth_rmse_tru_ols)
        {
            md.push_str("| Endmember | OLS RMSE | TRU RMSE |\n");
            md.push_str("|-----------|----------|----------|\n");
            for (i, name) in c.endmember_names.iter().enumerate() {
                md.push_str(&format!(
                    "| {} | {:.6} | {:.6} |\n",
                    name,
                    ols_rmse.get(i).unwrap_or(&0.0),
                    tru_rmse.get(i).unwrap_or(&0.0)
                ));
            }
            md.push('\n');
        }
    }

    fs::write(path, md)
        .with_context(|| format!("Failed to write markdown report to {}", path.display()))?;
    Ok(())
}

/// Write per-event CSV for a single dataset comparison.
pub fn write_csv_report(report: &BenchmarkReport, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create CSV output directory: {}", dir.display()))?;

    for ds in &report.datasets {
        let c = &ds.comparison;
        let filename = format!("{}_metrics.csv", c.dataset_label.replace(' ', "_"));
        let path = dir.join(&filename);
        let mut wtr = csv::Writer::from_path(&path)
            .with_context(|| format!("Failed to create CSV writer for {}", path.display()))?;

        // Header
        let mut header = vec![
            "endmember".to_string(),
            "ols_cv".to_string(),
            "tru_cv".to_string(),
            "ols_rsd".to_string(),
            "tru_rsd".to_string(),
            "ols_mean".to_string(),
            "tru_mean".to_string(),
            "use_value".to_string(),
        ];
        if ds.ground_truth_rmse_ols.is_some() {
            header.push("ols_rmse".to_string());
            header.push("tru_rmse".to_string());
        }
        wtr.write_record(&header)?;

        for (i, name) in c.endmember_names.iter().enumerate() {
            let mut row: Vec<String> = vec![
                name.to_owned(),
                format!("{:.6}", c.ols_spread.get(i).map(|s| s.cv).unwrap_or(0.0)),
                format!(
                    "{:.6}",
                    c.tru_ols_spread.get(i).map(|s| s.cv).unwrap_or(0.0)
                ),
                format!(
                    "{:.6}",
                    c.ols_spread.get(i).map(|s| s.robust_sd).unwrap_or(0.0)
                ),
                format!(
                    "{:.6}",
                    c.tru_ols_spread.get(i).map(|s| s.robust_sd).unwrap_or(0.0)
                ),
                format!("{:.6}", c.ols_spread.get(i).map(|s| s.mean).unwrap_or(0.0)),
                format!(
                    "{:.6}",
                    c.tru_ols_spread.get(i).map(|s| s.mean).unwrap_or(0.0)
                ),
                format!(
                    "{:.6}",
                    c.use_values.get(i).map(|u| u.use_value).unwrap_or(0.0)
                ),
            ];
            if let (Some(ols_rmse), Some(tru_rmse)) =
                (&ds.ground_truth_rmse_ols, &ds.ground_truth_rmse_tru_ols)
            {
                row.push(format!("{:.6}", ols_rmse.get(i).unwrap_or(&0.0)));
                row.push(format!("{:.6}", tru_rmse.get(i).unwrap_or(&0.0)));
            }
            wtr.write_record(&row)?;
        }
        wtr.flush()?;
    }
    Ok(())
}
