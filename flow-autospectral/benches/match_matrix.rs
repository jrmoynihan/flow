//! n × K × d throughput matrix for residual vs NN matching.
//!
//! Groups are separate so they can be A/B'd independently:
//! `match_residual_naive`, `match_residual_factored`, `match_residual_seq`,
//! `match_nn_control`, `unmix_ols`.
//!
//! Env:
//! - default: n = [10_000], K = [1, 4, 8, 32], d = [8, 20]
//! - `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1`: also n += [50_000, 100_000], d += [40], K += [16, 64]
//! - `FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1`: also n += [250_000]
//!
//! Synthetic data only (no FCS). Fluor is `Mat::zeros(d, 2)` filled with small
//! non-zero unit-peak-like columns so residual OLS is not an AF-only fit.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use faer::Mat;
use flow_autospectral::{
    AfLibrary, MatchConfig, MatchStrategy, OlsUnmixConfig, match_events, normalize_unit_peak,
    swap_af_column, unmix_events_ols_with,
};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::hint::black_box;
use std::time::Duration;

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn match_grid() -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let mut ns = vec![10_000usize];
    let mut ds = vec![8usize, 20];
    let mut ks = vec![1usize, 4, 8, 32];
    if env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX") {
        ns.extend([50_000, 100_000]);
        ds.push(40);
        ks.extend([16, 64]);
    }
    if env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE") {
        ns.push(250_000);
    }
    ns.sort_unstable();
    ds.sort_unstable();
    ks.sort_unstable();
    (ns, ds, ks)
}

/// Detectors × 2 fluors: `zeros(d, 2)` then small non-zero peaks (not `zeros(d, 0)`).
fn small_fluor(d: usize) -> Mat<f64> {
    let mut m = Mat::<f64>::zeros(d, 2);
    if d == 0 {
        return m;
    }
    m[(0, 0)] = 1.0;
    if d > 1 {
        m[(1, 0)] = 0.12;
        m[(d - 1, 1)] = 1.0;
        if d > 2 {
            m[(d - 2, 1)] = 0.12;
        }
    } else {
        m[(0, 1)] = 0.5;
    }
    m
}

fn unit_peak_library(k: usize, d: usize, seed: u64) -> AfLibrary {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut signatures = Mat::<f64>::zeros(d, k);
    for j in 0..k {
        let peak = j % d;
        let mut col = vec![0.0; d];
        for (i, slot) in col.iter_mut().enumerate() {
            let base = if i == peak { 1.0 } else { 0.03 };
            *slot = base + rng.random_range(-0.005..0.005);
        }
        normalize_unit_peak(&mut col);
        for i in 0..d {
            signatures[(i, j)] = col[i];
        }
    }
    AfLibrary {
        signatures,
        names: (0..k).map(|j| format!("AF_{j}")).collect(),
        detector_names: (0..d).map(|i| format!("D{i}")).collect(),
        provenance: format!("synthetic unit-peak K={k} d={d}"),
    }
}

fn stained_mix(n: usize, library: &AfLibrary, fluor: &Mat<f64>, seed: u64) -> Vec<f64> {
    let d = library.n_detectors();
    let k = library.n_signatures().max(1);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut events = Vec::with_capacity(n * d);
    for i in 0..n {
        let af = i % k;
        let af_scale = rng.random_range(4.0..10.0);
        let f0 = rng.random_range(0.2..1.5);
        let f1 = rng.random_range(0.2..1.5);
        for c in 0..d {
            let mut v = library.signatures[(c, af)] * af_scale;
            v += fluor[(c, 0)] * f0 + fluor[(c, 1)] * f1;
            v += rng.random_range(-0.05..0.05);
            events.push(v);
        }
    }
    events
}

fn residual_cfg(reuse_af_factors: bool, parallel_event_threshold: usize) -> MatchConfig {
    MatchConfig {
        strategy: MatchStrategy::ResidualOls,
        reuse_af_factors,
        parallel_event_threshold,
        // Keep the K sweep exhaustive (default shortlists when K > 32).
        exhaustive_residual_max_k: usize::MAX,
        ..MatchConfig::default()
    }
}

fn run_match_group(c: &mut Criterion, group_name: &str, match_cfg: MatchConfig) {
    let mut group = c.benchmark_group(group_name);
    let (ns, ds, ks) = match_grid();
    for &n in &ns {
        for &d in &ds {
            for &k in &ks {
                let library = unit_peak_library(k, d, 1);
                let fluor = small_fluor(d);
                let stained = stained_mix(n, &library, &fluor, 9);
                group.throughput(Throughput::Elements(n as u64));
                group.bench_with_input(
                    BenchmarkId::from_parameter(format!("n{n}_d{d}_K{k}")),
                    &stained,
                    |b, ev| {
                        b.iter(|| {
                            black_box(
                                match_events(
                                    black_box(ev),
                                    n,
                                    black_box(&library),
                                    fluor.as_ref(),
                                    black_box(&match_cfg),
                                )
                                .unwrap(),
                            )
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

fn bench_match_residual_naive(c: &mut Criterion) {
    run_match_group(c, "match_residual_naive", residual_cfg(false, 256));
}

fn bench_match_residual_factored(c: &mut Criterion) {
    run_match_group(c, "match_residual_factored", residual_cfg(true, 256));
}

fn bench_match_residual_seq(c: &mut Criterion) {
    run_match_group(c, "match_residual_seq", residual_cfg(true, usize::MAX));
}

fn bench_match_nn_control(c: &mut Criterion) {
    // Untouched control: NearestNeighbor only — do not mix residual into this group.
    let match_cfg = MatchConfig {
        strategy: MatchStrategy::NearestNeighbor,
        parallel_event_threshold: 256,
        ..MatchConfig::default()
    };
    run_match_group(c, "match_nn_control", match_cfg);
}

fn bench_unmix_ols(c: &mut Criterion) {
    let mut group = c.benchmark_group("unmix_ols");
    let (ns, ds, _) = match_grid();
    let configs: [(&str, OlsUnmixConfig); 3] = [
        (
            "naive",
            OlsUnmixConfig {
                reuse_factor: false,
                parallel_event_threshold: 256,
            },
        ),
        (
            "factored",
            OlsUnmixConfig {
                reuse_factor: true,
                parallel_event_threshold: 256,
            },
        ),
        (
            "seq",
            OlsUnmixConfig {
                reuse_factor: true,
                parallel_event_threshold: usize::MAX,
            },
        ),
    ];
    for &n in &ns {
        for &d in &ds {
            let library = unit_peak_library(4, d, 1);
            let fluor = small_fluor(d);
            let stained = stained_mix(n, &library, &fluor, 9);
            let mixing = swap_af_column(fluor.as_ref(), &library, 0).unwrap();
            for (label, cfg) in &configs {
                group.throughput(Throughput::Elements(n as u64));
                group.bench_with_input(
                    BenchmarkId::from_parameter(format!("n{n}_d{d}_{label}")),
                    &stained,
                    |b, ev| {
                        b.iter(|| {
                            black_box(
                                unmix_events_ols_with(
                                    mixing.as_ref(),
                                    black_box(ev),
                                    n,
                                    black_box(cfg),
                                )
                                .unwrap(),
                            )
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    targets = bench_match_residual_naive,
        bench_match_residual_factored,
        bench_match_residual_seq,
        bench_match_nn_control,
        bench_unmix_ols
}
criterion_main!(benches);
