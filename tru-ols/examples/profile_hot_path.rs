//! Long-running hot paths for sampling profilers (`samply`, `cargo flamegraph`, Instruments, `perf`).
//!
//! **TRU-OLS (primary):** mode `tru_ols_unmix`. **`tru_ols_new`** samples only `TruOls::new` (cutoffs + nonspecific from unstained; same work as `CutoffCalculator` + `NonspecificObservation`). **Plain OLS (secondary):** `ols_qr`, `normal_equations`, optional `normal_equations_gpu`.
//!
//! macOS: if `cargo flamegraph` fails to collapse traces, use **`samply record`** and open the JSON at [profiler.firefox.com](https://profiler.firefox.com/) (see `tru-ols/docs/PROFILING.md`).
//!
//! ```text
//! cargo install samply
//! samply record -s -n -o profile.json ./target/release/examples/profile_hot_path tru_ols_unmix --n-events 50000 --iter 10
//! samply record -s -n -o profile_new.json ./target/release/examples/profile_hot_path tru_ols_new --n-events 50000 --iter 3
//! ```
//!
//! ```text
//! cargo install flamegraph
//! cargo flamegraph -p flow-tru-ols --no-default-features --example profile_hot_path -- ols_qr
//! ```
//!
//! Linux (clearer stacks with `perf`):
//!
//! ```text
//! RUSTFLAGS="-C force-frame-pointers=yes" cargo flamegraph -p flow-tru-ols --no-default-features --example profile_hot_path -- normal_equations
//! ```
//!
//! Optional GPU path (WGPU adapter required):
//!
//! ```text
//! cargo flamegraph -p flow-tru-ols --no-default-features --features cubecl --example profile_hot_path -- normal_equations_gpu
//! ```
//!
//! Flags (after mode): `--n-events`, `--n-det`, `--n-em`, `--iter`.

use faer::Mat;
use flow_tru_ols::TruOls;
use flow_tru_ols::benchmark::run_ols;
use flow_tru_ols::run_ols_normal_equations;
use rand::RngExt;
use std::hint::black_box;
use std::time::Instant;

#[cfg(feature = "cubecl")]
use flow_tru_ols::{run_ols_normal_equations_gpu_rhs, try_shared_gpu_context};

#[derive(Clone, Copy, Debug)]
enum Mode {
    /// Full TRU-OLS `unmix` (primary production path).
    TruOlsUnmix,
    /// `TruOls::new` only (cutoffs + nonspecific from unstained; same cost as preprocessing inside `new`).
    TruOlsNew,
    OlsQr,
    NormalEquations,
    #[cfg(feature = "cubecl")]
    NormalEquationsGpu,
}

fn parse_mode(s: &str) -> Result<Mode, String> {
    match s {
        "tru_ols_unmix" => Ok(Mode::TruOlsUnmix),
        "tru_ols_new" => Ok(Mode::TruOlsNew),
        "ols_qr" => Ok(Mode::OlsQr),
        "normal_equations" => Ok(Mode::NormalEquations),
        #[cfg(feature = "cubecl")]
        "normal_equations_gpu" => Ok(Mode::NormalEquationsGpu),
        #[cfg(not(feature = "cubecl"))]
        "normal_equations_gpu" => {
            Err("normal_equations_gpu requires building with --features cubecl".to_string())
        }
        _ => Err(format!(
            "unknown mode '{s}' (expected tru_ols_unmix, tru_ols_new, ols_qr, normal_equations, or normal_equations_gpu with --features cubecl)",
        )),
    }
}

fn fixture(n_events: usize, n_det: usize, n_em: usize) -> (Mat<f64>, Mat<f64>) {
    let mut rng = rand::rng();
    let mixing_matrix = Mat::from_fn(n_det, n_em, |i, j| {
        if i == j {
            0.8 + rng.random_range(0.0..0.2)
        } else {
            rng.random_range(0.0..0.1)
        }
    });
    let observations = Mat::from_fn(n_events, n_det, |_, _| rng.random_range(0.0..100.0));
    (mixing_matrix, observations)
}

/// Unstained control uses 1000 rows (same shape as `unmixing_benchmark`).
fn fixture_tru_ols(n_events: usize, n_det: usize, n_em: usize) -> (Mat<f64>, Mat<f64>, Mat<f64>) {
    let mut rng = rand::rng();
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

fn main() {
    let mut mode = Mode::OlsQr;
    let mut n_events = 100_000usize;
    let mut n_det = 10usize;
    let mut n_em = 10usize;
    let mut n_iter = 400usize;

    let mut args = std::env::args().skip(1).peekable();
    if let Some(first) = args.peek() {
        if !first.starts_with('-') {
            mode = parse_mode(&args.next().unwrap()).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
        }
    }

    while let Some(a) = args.next() {
        match a.as_str() {
            "--n-events" => {
                n_events = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--n-events requires a number");
            }
            "--n-det" => {
                n_det = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--n-det requires a number");
            }
            "--n-em" => {
                n_em = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--n-em requires a number");
            }
            "--iter" => {
                n_iter = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--iter requires a number");
            }
            other if !other.starts_with('-') => {
                mode = parse_mode(other).unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(1);
                });
            }
            _ => {
                eprintln!("unknown argument: {a}");
                std::process::exit(1);
            }
        }
    }

    let (mixing, observations) = fixture(n_events, n_det, n_em);
    let tru_ols_fixture = if matches!(mode, Mode::TruOlsUnmix | Mode::TruOlsNew) {
        Some(fixture_tru_ols(n_events, n_det, n_em))
    } else {
        None
    };

    #[cfg(feature = "cubecl")]
    let gpu_ctx = if matches!(mode, Mode::NormalEquationsGpu) {
        Some(try_shared_gpu_context().unwrap_or_else(|e| {
            eprintln!("GPU context: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    eprintln!(
        "profile_hot_path: mode={mode:?} n_events={n_events} panel={n_det}x{n_em} iterations={n_iter}"
    );

    let t0 = Instant::now();
    match mode {
        Mode::TruOlsUnmix => {
            let (mix, unstained, obs) = tru_ols_fixture.expect("tru_ols fixture");
            let tru_ols = TruOls::new(mix, unstained, 0).expect("TruOls::new");
            for _ in 0..n_iter {
                black_box(tru_ols.unmix(obs.as_ref()).expect("unmix"));
            }
        }
        Mode::TruOlsNew => {
            let (mix, unstained, _obs) = tru_ols_fixture.expect("tru_ols fixture");
            for _ in 0..n_iter {
                black_box(TruOls::new(mix.clone(), unstained.clone(), 0).expect("TruOls::new"));
            }
        }
        Mode::OlsQr => {
            for _ in 0..n_iter {
                black_box(run_ols(observations.as_ref(), mixing.as_ref()).expect("run_ols"));
            }
        }
        Mode::NormalEquations => {
            for _ in 0..n_iter {
                black_box(
                    run_ols_normal_equations(observations.as_ref(), mixing.as_ref())
                        .expect("run_ols_normal_equations"),
                );
            }
        }
        #[cfg(feature = "cubecl")]
        Mode::NormalEquationsGpu => {
            let gpu = gpu_ctx.expect("gpu");
            for _ in 0..n_iter {
                black_box(
                    run_ols_normal_equations_gpu_rhs(observations.as_ref(), mixing.as_ref(), gpu)
                        .expect("run_ols_normal_equations_gpu_rhs"),
                );
            }
        }
    }
    eprintln!("elapsed: {:?}", t0.elapsed());
}
