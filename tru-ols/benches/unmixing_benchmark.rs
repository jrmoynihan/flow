//! Performance benchmarks for TRU-OLS unmixing (production-scale event counts by default).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use faer::Mat;
use flow_tru_ols::TruOls;
use flow_tru_ols::benchmark::run_ols;
use flow_tru_ols::run_ols_normal_equations;
use rand::RngExt;
use std::hint::black_box;

fn generate_test_data(
    n_events: usize,
    n_detectors: usize,
    n_endmembers: usize,
) -> (Mat<f64>, Mat<f64>, Mat<f64>) {
    let mut rng = rand::rng();

    let mixing_matrix = Mat::from_fn(n_detectors, n_endmembers, |i, j| {
        if i == j {
            0.8 + rng.random_range(0.0..0.2)
        } else {
            rng.random_range(0.0..0.1)
        }
    });

    let unstained = Mat::from_fn(1000, n_detectors, |_, _| rng.random_range(-0.1..0.1));

    let observations = Mat::from_fn(n_events, n_detectors, |_, _| rng.random_range(0.0..100.0));

    (mixing_matrix, unstained, observations)
}

fn benchmark_unmixing(c: &mut Criterion) {
    let mut group = c.benchmark_group("unmixing");
    group.sample_size(12);

    for n_events in [2_000usize, 50_000, 100_000] {
        let (mixing_matrix, unstained, observations) = generate_test_data(n_events, 10, 10);
        let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(n_events),
            &observations,
            |b, obs| {
                b.iter(|| tru_ols.unmix(black_box(obs.as_ref())).unwrap());
            },
        );
    }

    group.finish();
}

fn benchmark_unmixing_250k(c: &mut Criterion) {
    if std::env::var("FLOW_TRU_OLS_BENCH_1M").ok().as_deref() != Some("1") {
        return;
    }
    let mut group = c.benchmark_group("unmixing_250k");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(15));

    let n_events = 250_000usize;
    let (mixing_matrix, unstained, observations) = generate_test_data(n_events, 10, 10);
    let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();
    group.bench_with_input(
        BenchmarkId::from_parameter(n_events),
        &observations,
        |b, obs| {
            b.iter(|| tru_ols.unmix(black_box(obs.as_ref())).unwrap());
        },
    );

    group.finish();
}

fn benchmark_f32_to_f64_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_to_f64_conversion");

    for size in [10_000, 100_000, 1_000_000].iter() {
        let f32_data: Vec<f32> = (0..*size).map(|i| i as f32 * 0.1).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &f32_data, |b, data| {
            b.iter(|| {
                let f64_data: Vec<f64> = data.iter().map(|&x| x as f64).collect();
                black_box(f64_data)
            });
        });
    }

    group.finish();
}

fn benchmark_parallel_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_vs_sequential");
    group.sample_size(12);

    // Production-scale unmix; compare with vs without `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` (see PROFILING.md).
    let (mixing_matrix, unstained, observations) = generate_test_data(200_000, 10, 10);
    let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();
    group.bench_function("unmix_200k_events", |b| {
        b.iter(|| tru_ols.unmix(black_box(observations.as_ref())).unwrap());
    });

    group.finish();
}

fn benchmark_ols_vs_normal_equations(c: &mut Criterion) {
    let mut group = c.benchmark_group("ols_vs_normal_equations");
    group.sample_size(12);

    let n_events = 100_000usize;
    let (mixing_matrix, _unstained, observations) = generate_test_data(n_events, 10, 10);

    group.bench_function("run_ols_qr_per_event", |b| {
        b.iter(|| {
            run_ols(
                black_box(observations.as_ref()),
                black_box(mixing_matrix.as_ref()),
            )
            .unwrap()
        });
    });

    group.bench_function("run_ols_normal_equations_gram", |b| {
        b.iter(|| {
            run_ols_normal_equations(
                black_box(observations.as_ref()),
                black_box(mixing_matrix.as_ref()),
            )
            .unwrap()
        });
    });

    group.finish();
}

fn benchmark_parameter_sweep(c: &mut Criterion) {
    let mut group = c.benchmark_group("parameter_sweep_unmix");
    group.sample_size(12);

    for n_events in [50_000usize, 100_000] {
        let (mixing_matrix, unstained, observations) = generate_test_data(n_events, 10, 10);
        let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();
        group.bench_with_input(
            BenchmarkId::from_parameter(n_events),
            &observations,
            |b, obs| {
                b.iter(|| tru_ols.unmix(black_box(obs.as_ref())).unwrap());
            },
        );
    }
    group.finish();
}

fn benchmark_parameter_sweep_250k(c: &mut Criterion) {
    if std::env::var("FLOW_TRU_OLS_BENCH_1M").ok().as_deref() != Some("1") {
        return;
    }
    let mut group = c.benchmark_group("parameter_sweep_unmix_250k");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(15));

    let n_events = 250_000usize;
    let (mixing_matrix, unstained, observations) = generate_test_data(n_events, 10, 10);
    let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();
    group.bench_with_input(
        BenchmarkId::from_parameter(n_events),
        &observations,
        |b, obs| {
            b.iter(|| tru_ols.unmix(black_box(obs.as_ref())).unwrap());
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    benchmark_unmixing,
    benchmark_unmixing_250k,
    benchmark_f32_to_f64_conversion,
    benchmark_parallel_vs_sequential,
    benchmark_ols_vs_normal_equations,
    benchmark_parameter_sweep,
    benchmark_parameter_sweep_250k,
);
criterion_main!(benches);
