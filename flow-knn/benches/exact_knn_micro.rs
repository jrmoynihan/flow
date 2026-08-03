//! Criterion microbench for exact (brute-force) k-NN.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_knn::{DistanceMetric, exact_knn};
use std::hint::black_box;
use std::time::Duration;

fn synth(n: usize, d: usize) -> Vec<f32> {
    (0..n * d)
        .map(|i| (i as f32 * 0.001).sin())
        .collect()
}

fn bench_exact(c: &mut Criterion) {
    let cases = [(5_000usize, 20usize, 30usize), (10_000, 20, 30)];

    let mut group = c.benchmark_group("exact_knn_micro");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20);

    for &(n, d, k) in &cases {
        let data = synth(n, d);
        let id = format!("{n}x{d}_k{k}");
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(id), &data, |b, data| {
            b.iter(|| {
                black_box(
                    exact_knn(
                        black_box(data),
                        n,
                        d,
                        k,
                        DistanceMetric::EuclideanSq,
                    )
                    .unwrap(),
                )
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_exact);
criterion_main!(benches);
