//! Demo: generate a QC plot with good (grey) and bad (red) events visible.
//!
//! Run with:
//!   cargo run -p peacoqc-rs --no-default-features --features flow-fcs --example demo_qc_plot
//! (Omit `--no-default-features` if you have GPU support; use it to avoid GPU backend issues.)
//!
//! Creates synthetic FCS-like data with a Time channel and two FL channels,
//! injects an unstable region so PeacoQC marks some events bad, then generates
//! the QC plot and opens it (macOS: `open`, Linux: `xdg-open`).

use peacoqc_rs::{
    PeacoQCConfig, PeacoQCData, QCMode, QCPlotConfig, create_qc_plots, fcs::SimpleFcs,
};
use polars::prelude::*;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("PeacoQC demo: QC plot with good/bad events\n");

    let fcs = create_synthetic_data_with_unstable_region()?;
    println!(
        "Synthetic data: {} events, channels: {:?}",
        fcs.n_events(),
        fcs.channel_names()
    );

    let config = PeacoQCConfig {
        channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
        determine_good_cells: QCMode::All,
        ..Default::default()
    };

    let qc_result = peacoqc_rs::peacoqc(&fcs, &config)?;
    let n_bad = qc_result.good_cells.iter().filter(|&&b| !b).count();
    let n_good = qc_result.good_cells.iter().filter(|&&b| b).count();
    println!(
        "QC result: {} good, {} bad ({:.1}% removed)\n",
        n_good, n_bad, qc_result.percentage_removed
    );

    let out_path: PathBuf = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"))
        .join("demo_qc_plot.png");

    std::fs::create_dir_all(out_path.parent().unwrap())?;
    create_qc_plots(&fcs, &qc_result, &out_path, QCPlotConfig::default(), None)?;
    println!("Saved: {}", out_path.display());

    open_image(&out_path);
    Ok(())
}

fn create_synthetic_data_with_unstable_region() -> Result<SimpleFcs, Box<dyn std::error::Error>> {
    use rand::RngExt;
    let mut rng = rand::rng();

    let n_events = 25_000usize;
    let unstable_start = 8_000;
    let unstable_end = 14_000;

    // Time: monotonic, so events/sec is defined
    let time: Vec<f64> = (0..n_events).map(|i| i as f64 * 0.01).collect();

    // FL1-A: stable baseline with a clear "bad" block (spike down then up)
    let mut fl1_a = Vec::with_capacity(n_events);
    for i in 0..n_events {
        let base = 2000.0 + rng.random::<f64>() * 500.0;
        let val = if i >= unstable_start && i < unstable_end {
            base * (0.3
                + 0.4 * ((i - unstable_start) as f64 / (unstable_end - unstable_start) as f64))
        } else {
            base
        };
        fl1_a.push(val);
    }

    // FL2-A: similar, slight dip in unstable region
    let mut fl2_a = Vec::with_capacity(n_events);
    for i in 0..n_events {
        let base = 1500.0 + rng.random::<f64>() * 400.0;
        let val = if i >= unstable_start && i < unstable_end {
            base * 0.6
        } else {
            base
        };
        fl2_a.push(val);
    }

    let df = DataFrame::new(
        n_events,
        vec![
            Column::new("Time".into(), time),
            Column::new("FL1-A".into(), fl1_a),
            Column::new("FL2-A".into(), fl2_a),
        ],
    )?;

    let mut metadata = HashMap::new();
    for ch in ["Time", "FL1-A", "FL2-A"] {
        metadata.insert(
            ch.to_string(),
            peacoqc_rs::fcs::ParameterMetadata {
                min_range: 0.0,
                max_range: 262144.0,
                name: ch.to_string(),
            },
        );
    }

    Ok(SimpleFcs {
        data_frame: Arc::new(df),
        parameter_metadata: metadata,
    })
}

fn open_image(path: &std::path::Path) {
    let open_cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "linux") {
        "xdg-open"
    } else {
        return;
    };
    let status = Command::new(open_cmd).arg(path).status();
    if status.as_ref().map(|s| s.success()).unwrap_or(false) {
        println!("Opened image with {}", open_cmd);
    } else if let Err(e) = status {
        println!(
            "Could not open image: {}. View manually: {}",
            e,
            path.display()
        );
    }
}
