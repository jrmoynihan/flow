//! Criterion microbench for PaCMAP `compute_gradient`.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use flow_pacmap::gradient::compute_gradient;
use flow_pacmap::weights::Weights;
use std::hint::black_box;
use std::time::Duration;

fn synth_pairs(n: usize, n_pairs: usize, seed: u32) -> Vec<[u32; 2]> {
    let mut pairs = Vec::with_capacity(n_pairs);
    let mut state = seed;
    for _ in 0..n_pairs {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let i = state % n as u32;
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        let mut j = state % n as u32;
        if j == i {
            j = (j + 1) % n as u32;
        }
        pairs.push([i, j]);
    }
    pairs
}

fn bench_gradient(c: &mut Criterion) {
    let n = 50_000usize;
    let embedding: Vec<[f32; 2]> = (0..n)
        .map(|i| [(i as f32 * 0.001).sin(), (i as f32 * 0.001).cos()])
        .collect();
    // Rough pair counts at FCS scale for n_neighbors≈10.
    let near = synth_pairs(n, n * 10, 1);
    let mid_near = synth_pairs(n, n * 5, 2);
    let further = synth_pairs(n, n * 20, 3);
    let weights = Weights {
        w_nb: 3.0,
        w_mn: 3.0,
        w_fp: 1.0,
    };

    let mut group = c.benchmark_group("gradient_micro");
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(25);
    group.throughput(Throughput::Elements(
        (near.len() + mid_near.len() + further.len()) as u64,
    ));
    group.bench_function("50k", |b| {
        b.iter(|| {
            black_box(compute_gradient(
                black_box(&embedding),
                black_box(&near),
                black_box(&mid_near),
                black_box(&further),
                &weights,
                n,
            ))
        });
    });
    group.finish();
}

criterion_group!(benches, bench_gradient);
criterion_main!(benches);
