//! FlowSOM synthetic throughput Criterion benches.
//!
//! ```text
//! cargo bench -p flow-clustering --bench flowsom_throughput
//! ```
//!
//! Paper-dataset wall times use `compare_cytometry` (Weber FR-FCM-ZZPH), not
//! this synthetic grid. Van Gassen et al. 2015 describes ~linear scaling in
//! cells; publishable CyTOF yardsticks are Weber/Stassen tables.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_clustering::{FlowSom, FlowSomConfig, SomConfig};
use ndarray::Array2;
use std::hint::black_box;
use std::time::Duration;

fn synth_clouds(n: usize, d: usize) -> Array2<f64> {
    let n_half = n / 2;
    let mut rows = Vec::with_capacity(n * d);
    let mut push = |center0: f64, seed: u64, count: usize| {
        for i in 0..count {
            for j in 0..d {
                let s = seed
                    .wrapping_mul(0x9E3779B97F4A7C15)
                    .wrapping_add((i as u64).wrapping_mul(0xBF58476D1CE4E5B9))
                    .wrapping_add((j as u64).wrapping_mul(0x94D049BB133111EB))
                    .wrapping_mul(6364136223846793005);
                let u = ((s >> 33) as f64) / (u32::MAX as f64) - 0.5;
                rows.push(if j == 0 {
                    center0 + u * 0.5
                } else {
                    u * 0.5
                });
            }
        }
    };
    push(0.0, 0xC0FFEE, n_half);
    push(40.0, 0xBADC0DE, n - n_half);
    Array2::from_shape_vec((n, d), rows).expect("shape")
}

fn default_flowsom_config() -> FlowSomConfig {
    FlowSomConfig {
        som: SomConfig {
            width: 10,
            height: 10,
            n_epochs: 10,
            radius: None,
            seed: Some(42),
        },
        n_metaclusters: 10,
        meta_max_iterations: 100,
        meta_seed: Some(42),
    }
}

fn bench_flowsom_e2e(c: &mut Criterion) {
    let cases: &[(usize, usize)] = &[
        (20_000, 10),
        (20_000, 30),
        (50_000, 20),
        (100_000, 20),
        (100_000, 30),
    ];

    let mut group = c.benchmark_group("flowsom_e2e");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(12);

    let config = default_flowsom_config();
    for &(n, d) in cases {
        let data = synth_clouds(n, d);
        let id = format!("n{n}_d{d}_10x10_k10");
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(id), &data, |b, data| {
            b.iter(|| {
                black_box(FlowSom::fit(black_box(data), black_box(&config)).expect("flowsom"))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_flowsom_e2e);
criterion_main!(benches);
