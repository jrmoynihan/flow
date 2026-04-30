//! End-to-end TRU-OLS `unmix` throughput (not Criterion micro-benches).
//!
//! ```text
//! cargo build -p flow-tru-ols --release --no-default-features --example unmix_throughput
//! FLOW_TRU_OLS_FORCE_SEQUENTIAL=1 ./target/release/examples/unmix_throughput --n-events 100000 --iter 40
//! ```

use faer::Mat;
use flow_tru_ols::TruOls;
use rand::RngExt;
use std::hint::black_box;
use std::time::Instant;

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
    let mut n_events = 100_000usize;
    let mut n_det = 10usize;
    let mut n_em = 10usize;
    let mut n_iter = 40usize;

    let mut args = std::env::args().skip(1);
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
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(1);
            }
        }
    }

    let (mix, unstained, obs) = fixture_tru_ols(n_events, n_det, n_em);
    eprintln!("unmix_throughput: n_events={n_events} panel={n_det}x{n_em} iterations={n_iter}");

    let t0 = Instant::now();
    let tru_ols = TruOls::new(mix, unstained, 0).expect("TruOls::new");
    for _ in 0..n_iter {
        black_box(tru_ols.unmix(obs.as_ref()).expect("unmix"));
    }
    let elapsed = t0.elapsed();
    let secs = elapsed.as_secs_f64();
    let total_events = (n_events as f64) * (n_iter as f64);
    eprintln!("elapsed: {:?}", elapsed);
    eprintln!("throughput_events_per_sec: {:.0}", total_events / secs);
}
