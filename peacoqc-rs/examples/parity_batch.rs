//! Batch PeacoQC vs R report % on FlowJo ExtNode CSVs.
//!
//! Pass a directory that contains the CSVs (and optionally set labels via a
//! manifest). No machine-specific paths are baked into this example.
//!
//! ```bash
//! # Directory with ExtNode CSVs named like "D10 Well_013..ExtNode.csv"
//! cargo run --example parity_batch --features flow-fcs --no-default-features --release -- \
//!   ./testdata/peacoqc-parity
//!
//! # Or: PEACOQC_PARITY_DIR=./testdata/peacoqc-parity cargo run --example parity_batch ...
//! ```
//!
//! Manifest (optional) `parity_manifest.tsv` in that directory:
//! ```text
//! label	r_pct	csv_filename
//! D10	10.8493624378647	D10 Well_013..ExtNode.csv
//! ```
//! If no manifest is present, the built-in label→filename map is used and each
//! CSV must exist under the provided directory.

use peacoqc_rs::fcs::{ParameterMetadata, SimpleFcs};
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};
use polars::prelude::*;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const R_CHANNELS: &[&str] = &[
    "Comp-Spark UV 387-A",
    "Comp-LIVE DEAD Blue-A",
    "Comp-BUV496-A",
    "Comp-BUV615-A",
    "Comp-BUV737-A",
    "Comp-BUV805-A",
    "Comp-BV421-A",
    "Comp-BV480-A",
    "Comp-BV570-A",
    "Comp-BV605-A",
    "Comp-BV650-A",
    "Comp-BV711-A",
    "Comp-BV750-A",
    "Comp-Alexa Fluor 488-A",
    "Comp-RB705-A",
    "Comp-PE-A",
    "Comp-PE-CF594-A",
    "Comp-PE-Cy5.5-A",
    "Comp-PE-Cy7-A",
    "Comp-Alexa Fluor 647-A",
    "Comp-R718-A",
    "Comp-APC-Fire 750-A",
    "Comp-AF-A",
];

/// Default cases when no manifest is present (filenames only — resolve under CLI dir).
const DEFAULT_CASES: &[(&str, f64, &str)] = &[
    ("D10", 10.8493624378647, "D10 Well_013..ExtNode.csv"),
    ("D11", 0.851445216222272, "D11 Well_014..ExtNode.csv"),
    ("D12", 6.69745093422939, "D12 Well_015..ExtNode.csv"),
    ("E10", 8.47794163271215, "E10 Well_016..ExtNode.csv"),
    ("E11", 11.5788750772612, "E11 Well_017..ExtNode.csv"),
    ("E12", 10.5941165160546, "E12 Well_018..ExtNode.csv"),
    ("F1", 1.2918643715582, "F1 Well_010..ExtNode.csv"),
    ("F2", 10.1876158122298, "F2 Well_011..ExtNode.csv"),
];

struct Case {
    label: String,
    r_pct: f64,
    csv: PathBuf,
}

fn resolve_parity_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(arg) = env::args().nth(1) {
        return Ok(PathBuf::from(arg));
    }
    if let Ok(dir) = env::var("PEACOQC_PARITY_DIR") {
        return Ok(PathBuf::from(dir));
    }
    Err(
        "usage: parity_batch <dir>  (or set PEACOQC_PARITY_DIR)\n\
         dir must contain ExtNode CSVs; optional parity_manifest.tsv"
            .into(),
    )
}

fn load_cases(dir: &Path) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let manifest = dir.join("parity_manifest.tsv");
    if manifest.is_file() {
        let text = fs::read_to_string(&manifest)?;
        let mut cases = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if i == 0 && line.to_lowercase().contains("label") {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                return Err(format!("manifest line {}: expected label\\tr_pct\\tcsv", i + 1).into());
            }
            let r_pct: f64 = cols[1]
                .parse()
                .map_err(|e| format!("manifest line {}: bad r_pct: {e}", i + 1))?;
            cases.push(Case {
                label: cols[0].to_string(),
                r_pct,
                csv: dir.join(cols[2]),
            });
        }
        return Ok(cases);
    }

    Ok(DEFAULT_CASES
        .iter()
        .map(|(label, r_pct, file)| Case {
            label: (*label).to_string(),
            r_pct: *r_pct,
            csv: dir.join(file),
        })
        .collect())
}

fn load_csv(path: &Path) -> Result<(SimpleFcs, Vec<String>), Box<dyn std::error::Error>> {
    let mut df = CsvReadOptions::default()
        .with_has_header(true)
        .try_into_reader_with_file_path(Some(path.to_path_buf()))?
        .finish()?;

    let rename: Vec<(String, String)> = df
        .get_column_names()
        .iter()
        .map(|c| {
            let s = c.as_str();
            let base = s.split(" :: ").next().unwrap_or(s).to_string();
            (s.to_string(), base)
        })
        .collect();
    for (from, to) in &rename {
        if from != to {
            df.rename(from.as_str(), to.into())?;
        }
    }

    let cast_names: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|c| c.as_str().to_string())
        .collect();
    for name in &cast_names {
        let dtype = df.column(name)?.dtype().clone();
        if matches!(
            dtype,
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
        ) {
            let casted = df.column(name)?.cast(&DataType::Float64)?;
            df.replace(name.as_str(), casted)?;
        }
    }

    let channels: Vec<String> = R_CHANNELS.iter().map(|s| (*s).to_string()).collect();
    for ch in &channels {
        if df.column(ch).is_err() {
            return Err(format!("{}: missing channel {ch}", path.display()).into());
        }
    }

    let mut metadata = HashMap::new();
    for ch in &channels {
        metadata.insert(
            ch.clone(),
            ParameterMetadata {
                min_range: 0.0,
                max_range: 262144.0,
                name: ch.clone(),
            },
        );
    }

    Ok((
        SimpleFcs {
            data_frame: Arc::new(df),
            parameter_metadata: metadata,
        },
        channels,
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = resolve_parity_dir()?;
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()).into());
    }
    let cases = load_cases(&dir)?;

    println!("parity dir: {}", dir.display());
    println!(
        "{:<6} {:>10} {:>10} {:>10} {:>8} {:>8}",
        "well", "rust_%", "r_%", "delta_pp", "n", "ms"
    );

    let mut deltas = Vec::new();
    for case in &cases {
        if !case.csv.exists() {
            println!("{:<6} SKIP (missing {})", case.label, case.csv.display());
            continue;
        }
        let (fcs, channels) = load_csv(&case.csv)?;
        let config = PeacoQCConfig {
            channels,
            determine_good_cells: QCMode::All,
            mad: 6.0,
            it_limit: 0.6,
            consecutive_bins: 5,
            events_per_bin: Some(2000),
            remove_zeros: false,
            apply_compensation: false,
            apply_transformation: false,
            ..Default::default()
        };
        let t0 = Instant::now();
        let result = peacoqc(&fcs, &config)?;
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let delta = result.percentage_removed - case.r_pct;
        deltas.push(delta.abs());
        println!(
            "{:<6} {:10.4} {:10.4} {:+10.4} {:8} {:8.0}",
            case.label,
            result.percentage_removed,
            case.r_pct,
            delta,
            fcs.n_events(),
            ms
        );
    }

    if !deltas.is_empty() {
        let mean = deltas.iter().sum::<f64>() / deltas.len() as f64;
        let max = deltas.iter().cloned().fold(0.0_f64, f64::max);
        println!(
            "\n|Δ| mean={mean:.4} pp  max={max:.4} pp  n={}",
            deltas.len()
        );
    }
    Ok(())
}
