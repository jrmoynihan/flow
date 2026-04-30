//! Generate a **TRU-OLS vs OLS quality** report (spread, R², residuals, USE, dimensionality).
//!
//! Synthetic, **seeded** data for reproducibility — replace with real FCS-derived matrices for production.
//!
//! ```text
//! cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report
//! cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report -- --output report.md
//! cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report -- --json
//! cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report -- --n-events 12000
//! ```

use faer::Mat;
use flow_tru_ols::benchmark::{BenchmarkConfig, comparison_report_markdown, run_comparison};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

fn synthetic_fixture(
    seed: u64,
    n_events: usize,
    n_det: usize,
    n_em: usize,
) -> (Mat<f64>, Mat<f64>, Mat<f64>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mixing_matrix = Mat::from_fn(n_det, n_em, |i, j| {
        if i == j {
            0.8 + rng.random_range(0.0..0.2)
        } else {
            rng.random_range(0.0..0.1)
        }
    });
    let unstained = Mat::from_fn(1000, n_det, |_, _| rng.random_range(-0.1..0.1));
    let observations = Mat::from_fn(n_events, n_det, |_, _| rng.random_range(0.0..100.0));
    (mixing_matrix, unstained, observations)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut n_events = 8_000usize;
    let n_det = 10usize;
    let n_em = 10usize;
    let mut seed = 42u64;
    let mut json = false;
    let mut output: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json = true,
            "--output" => {
                output = Some(PathBuf::from(args.next().ok_or("--output needs a path")?));
            }
            "--n-events" => {
                n_events = args.next().ok_or("--n-events needs a value")?.parse()?;
            }
            "--seed" => {
                seed = args.next().ok_or("--seed needs a value")?.parse()?;
            }
            _ => eprintln!("ignoring unknown arg: {a}"),
        }
    }

    let names: Vec<String> = (0..n_em).map(|i| format!("EM{i}")).collect();
    let (mixing, unstained, observations) = synthetic_fixture(seed, n_events, n_det, n_em);

    let config = BenchmarkConfig {
        dataset_label: format!("synthetic_seed{seed}_n{n_events}"),
        cutoff_percentile: 0.995,
        autofluorescence_idx: 0,
        endmember_names: names,
    };

    let report = run_comparison(
        observations.as_ref(),
        unstained.as_ref(),
        mixing.as_ref(),
        &config,
    )?;

    if json {
        let s = serde_json::to_string_pretty(&report)?;
        if let Some(path) = &output {
            std::fs::write(path, s)?;
        } else {
            io::stdout().write_all(s.as_bytes())?;
            writeln!(io::stdout())?;
        }
    } else {
        let mut buf = format!(
            "> **Note:** Synthetic mixing + observations (detectors × endmembers = {}×{}), **{}** events, RNG seed **{}**. For science conclusions, call `run_comparison` on real FCS-derived matrices.\n\n",
            n_det, n_em, n_events, seed
        );
        buf.push_str(&comparison_report_markdown(&report));
        if let Some(path) = output {
            let mut f = File::create(path)?;
            f.write_all(buf.as_bytes())?;
        } else {
            print!("{buf}");
        }
    }

    Ok(())
}
