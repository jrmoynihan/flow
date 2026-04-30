//! Load TRU-OLS inputs from an `e2e_plate_throughput` / `compare_with_julia` export directory and time
//! **`unmix` only** (no FCS I/O, no mixing-matrix generation).
//!
//! Expects these files in the directory:
//! - `mixing_matrix.csv`, `unstained_data.csv`, `stained_data.csv`
//! - `rust_cutoffs.csv`, `rust_nonspecific.csv`
//! - `endmember_names.csv` (to resolve autofluorescence index unless `--af-index` is set)
//!
//! ```text
//! cargo build -p flow-tru-ols --release --no-default-features --example unmix_profile_csv
//! FLOW_TRU_OLS_FORCE_SEQUENTIAL=1 samply record -s -n -o unmix_plate.json \
//!   ./target/release/examples/unmix_profile_csv /path/to/e2e_bench_after_perf --iter 1
//! ```

use faer::{Col, Mat};
use flow_tru_ols::TruOls;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn load_mixing_matrix_csv(path: &Path) -> Result<Mat<f64>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines().map_while(Result::ok);
    let _header = lines
        .next()
        .ok_or_else(|| "empty mixing_matrix.csv".to_string())?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in lines {
        let values: Vec<f64> = line
            .split(',')
            .skip(1)
            .map(|s| s.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("mixing_matrix parse: {e}"))?;
        if !values.is_empty() {
            rows.push(values);
        }
    }
    if rows.is_empty() {
        return Err("no mixing rows".into());
    }
    let n = rows.len();
    let m = rows[0].len();
    Ok(Mat::from_fn(n, m, |i, j| rows[i][j]))
}

fn load_event_matrix_csv(path: &Path) -> Result<Mat<f64>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines().map_while(Result::ok);
    let _header = lines.next().ok_or_else(|| "empty data csv".to_string())?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for line in lines {
        let values: Vec<f64> = line
            .split(',')
            .map(|s| s.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("data parse: {e}"))?;
        if !values.is_empty() {
            rows.push(values);
        }
    }
    if rows.is_empty() {
        return Err("no data rows".into());
    }
    let n = rows.len();
    let m = rows[0].len();
    Ok(Mat::from_fn(n, m, |i, j| rows[i][j]))
}

fn load_vector_csv(path: &Path) -> Result<Col<f64>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines().map_while(Result::ok);
    let _label = lines.next().ok_or_else(|| "empty vector csv".to_string())?;
    let mut v = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        v.push(
            line.parse::<f64>()
                .map_err(|e| format!("vector parse: {e}"))?,
        );
    }
    Ok(Col::from_fn(v.len(), |i| v[i]))
}

fn autofluorescence_index(dir: &Path, name: &str) -> Result<usize, String> {
    let path = dir.join("endmember_names.csv");
    let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines().map_while(Result::ok);
    let _header = lines
        .next()
        .ok_or_else(|| "empty endmember_names".to_string())?;
    for (i, line) in lines.enumerate() {
        let line = line.trim();
        if line == name {
            return Ok(i);
        }
    }
    Err(format!(
        "endmember '{name}' not found in {}",
        path.display()
    ))
}

fn main() -> Result<(), String> {
    let mut dir: Option<PathBuf> = None;
    let mut iter = 1usize;
    let mut af_index: Option<usize> = None;
    let mut stats_only = false;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--iter" => {
                iter = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or("--iter needs a number")?;
            }
            "--af-index" => {
                af_index = Some(
                    args.next()
                        .and_then(|s| s.parse().ok())
                        .ok_or("--af-index needs a number")?,
                );
            }
            "--stats" => stats_only = true,
            s if !s.starts_with('-') => dir = Some(PathBuf::from(s)),
            other => return Err(format!("unknown arg: {other}")),
        }
    }
    let dir = dir.ok_or_else(|| {
        "usage: unmix_profile_csv <export_dir> [--iter N] [--af-index I] [--stats]\n\
         --stats: only run summarize_truncation_iterations (no timed unmix loop)"
            .to_string()
    })?;

    let mixing = load_mixing_matrix_csv(&dir.join("mixing_matrix.csv"))?;
    let unstained = load_event_matrix_csv(&dir.join("unstained_data.csv"))?;
    let stained = load_event_matrix_csv(&dir.join("stained_data.csv"))?;
    let cutoffs = load_vector_csv(&dir.join("rust_cutoffs.csv"))?;
    let nonspecific = load_vector_csv(&dir.join("rust_nonspecific.csv"))?;

    let af = af_index.unwrap_or_else(|| {
        autofluorescence_index(&dir, "Autofluorescence").unwrap_or_else(|e| {
            eprintln!("warning: {e}, using index 0");
            0
        })
    });

    let tru_ols = TruOls::from_preprocessed(mixing, unstained, cutoffs, nonspecific, af)
        .map_err(|e| e.to_string())?;

    let n_ev = stained.nrows();
    eprintln!(
        "unmix_profile_csv: dir={} stained_events={} iter={} FLOW_TRU_OLS_FORCE_SEQUENTIAL={:?}",
        dir.display(),
        n_ev,
        iter,
        std::env::var("FLOW_TRU_OLS_FORCE_SEQUENTIAL").ok()
    );

    if stats_only {
        let s = tru_ols
            .summarize_truncation_iterations(stained.as_ref())
            .map_err(|e| e.to_string())?;
        eprintln!(
            "truncation inner_iterations: min={} max={} mean={:.3}",
            s.inner_iterations_min, s.inner_iterations_max, s.inner_iterations_mean
        );
        return Ok(());
    }

    let t0 = Instant::now();
    for _ in 0..iter {
        let u = tru_ols.unmix(stained.as_ref()).map_err(|e| e.to_string())?;
        std::hint::black_box(u);
    }
    let elapsed = t0.elapsed();
    let secs = elapsed.as_secs_f64();
    eprintln!("unmix only: {:?}  ({} iters)", elapsed, iter);
    eprintln!(
        "throughput stained events/s: {:.0}",
        (n_ev as f64) * (iter as f64) / secs
    );

    let s = tru_ols
        .summarize_truncation_iterations(stained.as_ref())
        .map_err(|e| e.to_string())?;
    eprintln!(
        "truncation inner_iterations: min={} max={} mean={:.3}",
        s.inner_iterations_min, s.inner_iterations_max, s.inner_iterations_mean
    );

    Ok(())
}
