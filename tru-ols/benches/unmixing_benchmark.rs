//! Performance benchmarks for TRU-OLS unmixing

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use faer::Mat;
use std::hint::black_box;
use flow_tru_ols::TruOls;
use rand::RngExt;

fn generate_test_data(
    n_events: usize,
    n_detectors: usize,
    n_endmembers: usize,
) -> (Mat<f64>, Mat<f64>, Mat<f64>) {
    let mut rng = rand::rng();

    // Generate mixing matrix
    let mixing_matrix = Mat::from_fn(n_detectors, n_endmembers, |i, j| {
        if i == j {
            0.8 + rng.random_range(0.0..0.2)
        } else {
            rng.random_range(0.0..0.1)
        }
    });

    // Generate unstained control
    let unstained = Mat::from_fn(1000, n_detectors, |_, _| rng.random_range(-0.1..0.1));

    // Generate test observations
    let observations =
        Mat::from_fn(n_events, n_detectors, |_, _| rng.random_range(0.0..100.0));

    (mixing_matrix, unstained, observations)
}

fn benchmark_unmixing(c: &mut Criterion) {
    let mut group = c.benchmark_group("unmixing");
    
    // Test different dataset sizes
    for n_events in [100, 1000, 10000, 100000].iter() {
        let (mixing_matrix, unstained, observations) = generate_test_data(*n_events, 10, 10);
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

fn benchmark_f32_to_f64_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("f32_to_f64_conversion");
    
    for size in [1000, 10000, 100000, 1000000].iter() {
        let f32_data: Vec<f32> = (0..*size).map(|i| i as f32 * 0.1).collect();
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &f32_data,
            |b, data| {
                b.iter(|| {
                    let f64_data: Vec<f64> = data.iter().map(|&x| x as f64).collect();
                    black_box(f64_data)
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_parallel_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_vs_sequential");
    // PARALLEL_THRESHOLD in unmixing.rs is 10_000; below = sequential, above = parallel
    const PARALLEL_THRESHOLD: usize = 10_000;

    // Sequential path: 8k events (< threshold)
    let (mixing_matrix, unstained, observations_8k) =
        generate_test_data(PARALLEL_THRESHOLD - 2000, 10, 10);
    let tru_ols_seq = TruOls::new(mixing_matrix, unstained, 0).unwrap();
    group.bench_function("unmix_8k_events_sequential", |b| {
        b.iter(|| tru_ols_seq.unmix(black_box(observations_8k.as_ref())).unwrap());
    });

    // Parallel path: 50k events (> threshold)
    let (mixing_matrix, unstained, observations_50k) = generate_test_data(50000, 10, 10);
    let tru_ols_par = TruOls::new(mixing_matrix, unstained, 0).unwrap();
    group.bench_function("unmix_50k_events_parallel", |b| {
        b.iter(|| tru_ols_par.unmix(black_box(observations_50k.as_ref())).unwrap());
    });

    group.finish();
}

criterion_group!(benches, benchmark_unmixing, benchmark_f32_to_f64_conversion, benchmark_parallel_vs_sequential);
criterion_main!(benches);
