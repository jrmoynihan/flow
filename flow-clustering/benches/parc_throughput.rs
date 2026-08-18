//! PARC throughput and prune / k-NN backend Criterion benches.
//!
//! ```text
//! cargo bench -p flow-clustering --bench parc_throughput --features parc
//! cargo bench -p flow-clustering --bench parc_throughput --features parc -- parc_e2e_knn_ab
//! ```
//!
//! Interleaves A/B pairs (Exact↔HNSW, seq↔Rayon) to limit session drift.
//! Peak RSS is measured by the `parc_rss` example, not Criterion.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_clustering::{KeepLocalDist, Parc, ParcConfig};
use flow_knn::{compute_knn, DistanceMetric, HnswParams, KnnGraph, KnnMethod};
use ndarray::Array2;
use std::hint::black_box;
use std::time::Duration;

fn synth_clouds(n: usize, d: usize) -> Array2<f64> {
    // Two equal clouds separated on axis 0 (isotropic jitter).
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

fn hnsw_method() -> KnnMethod {
    KnnMethod::Hnsw(HnswParams {
        m: 24,
        ef_construction: 150,
        ef_search: 100,
        quantization: flow_knn::Quantization::F32,
    })
}

fn bench_config(knn: usize, parallel_prune: bool, method: KnnMethod) -> ParcConfig {
    ParcConfig {
        knn,
        knn_method: Some(method),
        keep_all_local_dist: KeepLocalDist::Never,
        parallel_prune,
        too_big_factor: 0.9,
        ..ParcConfig::default()
    }
}

fn precompute_knn(data: &Array2<f64>, k: usize) -> KnnGraph {
    let n = data.nrows();
    let d = data.ncols();
    let flat: Vec<f32> = data.iter().map(|&x| x as f32).collect();
    compute_knn(
        &flat,
        n,
        d,
        k.min(n - 1),
        &KnnMethod::Exact,
        DistanceMetric::Euclidean,
    )
    .expect("knn")
}

/// End-to-end PARC with Exact k-NN (baseline / prune-isolation companion).
fn bench_parc_e2e_exact(c: &mut Criterion) {
    let cases: &[(usize, usize, usize)] = &[
        (2_000, 10, 15),
        (5_000, 10, 20),
        (5_000, 30, 20),
        (20_000, 10, 30),
        (20_000, 30, 30),
        (50_000, 20, 30),
    ];

    let mut group = c.benchmark_group("parc_e2e_exact");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(15);

    for &(n, d, knn) in cases {
        let data = synth_clouds(n, d);
        let config = bench_config(knn, true, KnnMethod::Exact);
        let id = format!("n{n}_d{d}_k{knn}");
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(id), &data, |b, data| {
            b.iter(|| {
                black_box(Parc::fit(black_box(data), black_box(&config)).expect("parc"))
            });
        });
    }
    group.finish();
}

/// Interleaved Exact vs HNSW end-to-end (Rayon prune on). Primary publishable A/B.
fn bench_parc_e2e_knn_ab(c: &mut Criterion) {
    let cases: &[(usize, usize, usize)] = &[
        (5_000, 20, 20),
        (20_000, 20, 30),
        (50_000, 20, 30),
        (100_000, 20, 30),
    ];

    let mut group = c.benchmark_group("parc_e2e_knn_ab");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(12));
    group.sample_size(12);

    for &(n, d, knn) in cases {
        let data = synth_clouds(n, d);
        group.throughput(Throughput::Elements(n as u64));

        // Interleave Exact then HNSW for the same size.
        for (label, method) in [
            ("exact", KnnMethod::Exact),
            ("hnsw", hnsw_method()),
        ] {
            let config = bench_config(knn, true, method);
            let id = format!("n{n}_d{d}_k{knn}_{label}");
            group.bench_with_input(
                BenchmarkId::from_parameter(id),
                &(data.clone(), config),
                |b, (data, config)| {
                    b.iter(|| {
                        black_box(Parc::fit(black_box(data), black_box(config)).expect("parc"))
                    });
                },
            );
        }
    }
    group.finish();
}

/// Prune + Leiden only (precomputed Exact k-NN), Rayon vs sequential prune interleaved.
fn bench_parc_prune_ab(c: &mut Criterion) {
    let cases: &[(usize, usize, usize)] = &[
        (5_000, 20, 20),
        (20_000, 20, 30),
        (50_000, 20, 30),
    ];

    let mut group = c.benchmark_group("parc_prune_rayon_ab");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    for &(n, d, knn) in cases {
        let data = synth_clouds(n, d);
        let knn_graph = precompute_knn(&data, knn);
        group.throughput(Throughput::Elements(n as u64));

        for parallel in [false, true] {
            let config = bench_config(knn, parallel, KnnMethod::Exact);
            let label = if parallel { "rayon" } else { "seq" };
            let id = format!("n{n}_d{d}_k{knn}_{label}");
            group.bench_with_input(
                BenchmarkId::from_parameter(id),
                &(data.clone(), knn_graph.clone(), config),
                |b, (data, graph, config)| {
                    b.iter(|| {
                        black_box(
                            Parc::fit_with_knn(
                                black_box(data),
                                black_box(config),
                                Some(black_box(graph)),
                            )
                            .expect("parc"),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parc_e2e_knn_ab,
    bench_parc_e2e_exact,
    bench_parc_prune_ab
);
criterion_main!(benches);
