//! One-shot D10-style PeacoQC parity check vs an R report % (default ~10.85%).
//!
//! ```bash
//! cargo run --example d10_check --features flow-fcs --no-default-features --release -- \
//!   path/to/D10\ Well_013..ExtNode.csv
//!
//! # Optional expected R full-% (defaults to D10 Well_013 reference):
//! R_PCT=10.8493624378647 cargo run --example d10_check ... -- path/to/file.csv
//! ```

use peacoqc_rs::fcs::{ParameterMetadata, SimpleFcs};
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};
use polars::prelude::*;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Same channel list as FlowJo RScript after FJComp- → Comp- rewrite.
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let csv_path = env::args().nth(1).map(PathBuf::from).ok_or(
        "usage: d10_check <path-to-ExtNode.csv>\n\
         optional env: R_PCT (expected full-analysis % removed)",
    )?;
    if !csv_path.is_file() {
        return Err(format!("not a file: {}", csv_path.display()).into());
    }

    let r_pct: f64 = env::var("R_PCT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10.8493624378647);
    let r_mad = 10.112580309252;
    let r_it = 1.22797021435448;

    println!("Loading {}", csv_path.display());
    let mut df = CsvReadOptions::default()
        .with_has_header(true)
        .try_into_reader_with_file_path(Some(csv_path))?
        .finish()?;

    // Strip FlowJo " :: marker" display suffixes (same as RScript).
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

    // FlowJo ExtNode CSVs often infer as integer; PeacoQCData::get_channel_f64
    // only accepts f32/f64 (errors are swallowed in peak detection).
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
            return Err(format!("missing channel column after rename: {ch}").into());
        }
    }

    println!(
        "Events: {}  Channels ({}): {:?}",
        df.height(),
        channels.len(),
        channels
    );

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

    let fcs = SimpleFcs {
        data_frame: Arc::new(df),
        parameter_metadata: metadata,
    };

    let config = PeacoQCConfig {
        channels: channels.clone(),
        determine_good_cells: QCMode::All,
        mad: 6.0,
        it_limit: 0.6,
        consecutive_bins: 5,
        events_per_bin: Some(2000),
        remove_zeros: false,
        // CSV is already compensated + transformed (FlowJo ExtNode export).
        apply_compensation: false,
        apply_transformation: false,
        ..Default::default()
    };

    let t0 = Instant::now();
    let result = peacoqc(&fcs, &config)?;
    let elapsed = t0.elapsed();

    let n = fcs.n_events();
    let n_good = result.good_cells.iter().filter(|&&g| g).count();
    let n_bad = n - n_good;

    println!("\n=== Rust PeacoQC ===");
    println!("  Events before: {n}");
    println!("  Events after:  {n_good}");
    println!("  Removed:       {n_bad} ({:.6}%)", result.percentage_removed);
    println!(
        "  % IT:          {}",
        result
            .it_percentage
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "n/a".into())
    );
    println!(
        "  % MAD:         {}",
        result
            .mad_percentage
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "n/a".into())
    );
    println!("  % Consecutive: {:.6}", result.consecutive_percentage);
    println!("  n_bins:        {}", result.n_bins);
    println!("  events/bin:    {}", result.events_per_bin);
    println!("  elapsed:       {elapsed:?}");

    println!("\n=== R reference (R_PCT={r_pct}) ===");
    println!("  Removed:       {r_pct}%");
    println!("  % IT (D10 ref):  {r_it}");
    println!("  % MAD (D10 ref): {r_mad}");

    let delta = result.percentage_removed - r_pct;
    println!("\n=== Delta (Rust − R) ===");
    println!("  Full %: {delta:+.6} pp");
    if let Some(mad) = result.mad_percentage {
        println!("  MAD %:  {:+.6} pp (vs D10 MAD ref; ignore if not D10)", mad - r_mad);
    }
    if let Some(it) = result.it_percentage {
        println!("  IT %:   {:+.6} pp (vs D10 IT ref; ignore if not D10)", it - r_it);
    }

    Ok(())
}
