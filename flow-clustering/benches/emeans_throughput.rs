//! e-means / Lloyd synthetic throughput Criterion benches.
//!
//! ```text
//! cargo bench -p flow-clustering --bench emeans_throughput --features emeans
//! ```

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_clustering::{EMeans, EMeansConfig, Lloyd, LloydConfig};
use std::hint::black_box;
use std::time::Duration;

fn synth_row_major(n: usize, d: usize, seed: u64) -> Vec<f32> {
    let mut data = vec![0.0f32; n * d];
    let mut s = seed;
    for i in 0..n {
        let pop = if i % 3 == 0 { 0.0 } else { 20.0 };
        for j in 0..d {
            s = s
                .wrapping_mul(0x9E3779B97F4A7C15)
                .wrapping_add((i as u64).wrapping_mul(0xBF58476D1CE4E5B9))
                .wrapping_add(j as u64);
            let u = ((s >> 33) as f32) / (u32::MAX as f32) - 0.5;
            data[i * d + j] = pop + u;
        }
    }
    data
}

fn bench_emeans(c: &mut Criterion) {
    let cases: &[(usize, usize, usize)] = &[
        (10_000, 30, 32),
        (100_000, 30, 256),
        (100_000, 60, 256),
        (1_000_000, 30, 256),
    ];

    let mut group = c.benchmark_group("emeans_fit");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(10);

    for &(n, d, k) in cases {
        let data = synth_row_major(n, d, 42);
        let id = format!("emeans_n{n}_d{d}_k{k}_e3");
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(&id), &data, |b, data| {
            let cfg = EMeansConfig {
                n_clusters: k,
                epochs: 3,
                seed: Some(42),
                ..Default::default()
            };
            b.iter(|| {
                black_box(EMeans::fit(black_box(data), n, d, black_box(&cfg)).expect("emeans"))
            });
        });

        let id_l = format!("lloyd_n{n}_d{d}_k{k}_i25");
        group.bench_with_input(BenchmarkId::from_parameter(&id_l), &data, |b, data| {
            let cfg = LloydConfig {
                n_clusters: k,
                max_iterations: 25,
                seed: Some(42),
                ..Default::default()
            };
            b.iter(|| {
                black_box(Lloyd::fit(black_box(data), n, d, black_box(&cfg)).expect("lloyd"))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_emeans);
criterion_main!(benches);
