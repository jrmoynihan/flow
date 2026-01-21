//! Matrix operations benchmarks
//!
//! Benchmarks CPU matrix operations for flow cytometry compensation.
//! GPU implementations were previously benchmarked but found to be slower
//! than CPU for typical workloads due to transfer overhead. See GPU_BENCHMARKING.md for details.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_fcs::MatrixOps;
use ndarray::Array2;
use std::hint::black_box;

/// Generate a random compensation matrix for testing
fn generate_compensation_matrix(n: usize) -> Array2<f32> {
    use rand::Rng;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    // Create a diagonally dominant matrix to ensure invertibility
    let mut rng = StdRng::seed_from_u64(42);
    let mut matrix = Array2::<f32>::zeros((n, n));

    for i in 0..n {
        for j in 0..n {
            if i == j {
                // Diagonal: make it dominant
                matrix[[i, j]] = 1.0 + rng.gen_range(0.0..0.1);
            } else {
                // Off-diagonal: small values
                matrix[[i, j]] = rng.gen_range(-0.1..0.1);
            }
        }
    }

    matrix
}

/// Generate synthetic channel data for testing
fn generate_channel_data(n_channels: usize, n_events: usize) -> Vec<Vec<f32>> {
    use rand::Rng;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    let mut rng = StdRng::seed_from_u64(123);
    let mut data = Vec::with_capacity(n_channels);

    for _ in 0..n_channels {
        let channel: Vec<f32> = (0..n_events).map(|_| rng.gen_range(0.0..1000.0)).collect();
        data.push(channel);
    }

    data
}

fn bench_matrix_inversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("matrix_inversion");

    // Typical flow cytometry matrix sizes: 5, 10, 15, 20, 30 channels
    let sizes = vec![5, 10, 15, 20, 30];

    for &n in &sizes {
        let matrix = generate_compensation_matrix(n);

        group.throughput(Throughput::Elements((n * n) as u64));
        group.bench_with_input(BenchmarkId::new("CPU_LAPACK", n), &matrix, |b, m| {
            b.iter(|| black_box(MatrixOps::invert_matrix(m)).unwrap());
        });
    }

    group.finish();
}

fn bench_batch_matvec_cpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_matvec_cpu");

    // Test various combinations of channels and events
    let test_cases = vec![
        (5, 10_000),
        (10, 50_000),
        (15, 100_000),
        (20, 250_000),
        (30, 500_000),
        (30, 1_000_000),
    ];

    for &(n_channels, n_events) in &test_cases {
        let matrix = generate_compensation_matrix(n_channels);
        let channel_data = generate_channel_data(n_channels, n_events);

        group.throughput(Throughput::Elements((n_channels * n_events) as u64));
        group.bench_with_input(
            BenchmarkId::new("CPU", format!("{}ch_{}ev", n_channels, n_events)),
            &(&matrix, &channel_data),
            |b, (m, d)| {
                b.iter(|| black_box(MatrixOps::batch_matvec(*m, *d)).unwrap());
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_matrix_inversion, bench_batch_matvec_cpu);

criterion_main!(benches);
