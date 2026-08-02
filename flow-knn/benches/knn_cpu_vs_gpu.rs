//! CPU vs GPU k-NN (exact / HNSW / exhaustive GPU / IVF GPU / NN-Descent GPU).
//!
//! ```bash
//! cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu -- \
//!   --warm-up-time 1 --measurement-time 20 --sample-size 10
//!
//! # Smoke
//! FLOW_KNN_BENCH_SMOKE=1 cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu
//!
//! # Pressure (adds 250k / 500k IVF + HNSW; skips exact_gpu above 50k)
//! FLOW_KNN_BENCH_PRESSURE=1 cargo bench -p flow-knn --features "hnsw,ann-search,gpu" --bench knn_cpu_vs_gpu
//! ```

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_knn::{
    DistanceMetric, HnswParams, IvfGpuParams, KnnMethod, NnDescentGpuParams, compute_knn,
    gpu_adapter_available,
};
use rand::{RngExt, SeedableRng, rngs::SmallRng};
use std::hint::black_box;
use std::time::Duration;

const K: usize = 60;

fn smoke() -> bool {
    std::env::var("FLOW_KNN_BENCH_SMOKE").ok().as_deref() == Some("1")
}

fn pressure() -> bool {
    std::env::var("FLOW_KNN_BENCH_PRESSURE").ok().as_deref() == Some("1")
}

fn sizes() -> Vec<(usize, usize)> {
    if smoke() {
        return vec![(2_000, 10)];
    }
    let mut cells = vec![
        (10_000usize, 10),
        (50_000, 10),
        (50_000, 20),
        (100_000, 10),
        (100_000, 20),
    ];
    if pressure() {
        cells.extend([(250_000, 20), (500_000, 20)]);
    }
    cells
}

fn synthetic(n: usize, d: usize) -> Vec<f32> {
    let mut rng = SmallRng::seed_from_u64(42);
    let mut data = Vec::with_capacity(n * d);
    for i in 0..n {
        let c = if i % 2 == 0 { 0.0f32 } else { 5.0 };
        for j in 0..d {
            data.push(c + rng.random::<f32>() + j as f32 * 0.01);
        }
    }
    data
}

fn knn_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("knn_cpu_vs_gpu");
    group.sample_size(if smoke() { 10 } else { 10 });
    group.measurement_time(Duration::from_secs(if smoke() {
        3
    } else if pressure() {
        40
    } else {
        25
    }));
    group.warm_up_time(Duration::from_secs(1));

    let gpu_ok = gpu_adapter_available();
    if !gpu_ok {
        eprintln!("WGPU adapter unavailable — GPU arms will be skipped");
    }

    for &(n, d) in &sizes() {
        let data = synthetic(n, d);
        group.throughput(Throughput::Elements(n as u64));
        let label = format!("n{n}_d{d}");

        // CPU exact (skip at very large n)
        if n <= 100_000 {
            group.bench_function(BenchmarkId::new("exact_cpu", &label), |b| {
                b.iter(|| {
                    black_box(
                        compute_knn(
                            black_box(&data),
                            black_box(n),
                            black_box(d),
                            black_box(K),
                            black_box(&KnnMethod::Exact),
                            black_box(DistanceMetric::Euclidean),
                        )
                        .expect("exact"),
                    )
                });
            });
        }

        group.bench_function(BenchmarkId::new("hnsw_ann_cpu", &label), |b| {
            let method = KnnMethod::AnnSearchHnsw(HnswParams::default());
            b.iter(|| {
                black_box(
                    compute_knn(
                        black_box(&data),
                        black_box(n),
                        black_box(d),
                        black_box(K),
                        black_box(&method),
                        black_box(DistanceMetric::Euclidean),
                    )
                    .expect("ann hnsw"),
                )
            });
        });

        if gpu_ok {
            if n <= 50_000 {
                group.bench_function(BenchmarkId::new("exact_gpu", &label), |b| {
                    b.iter(|| {
                        black_box(
                            compute_knn(
                                black_box(&data),
                                black_box(n),
                                black_box(d),
                                black_box(K),
                                black_box(&KnnMethod::GpuExact),
                                black_box(DistanceMetric::Euclidean),
                            )
                            .expect("exact gpu"),
                        )
                    });
                });
            }

            group.bench_function(BenchmarkId::new("ivf_gpu", &label), |b| {
                let method = KnnMethod::GpuIvf(IvfGpuParams::default());
                b.iter(|| {
                    black_box(
                        compute_knn(
                            black_box(&data),
                            black_box(n),
                            black_box(d),
                            black_box(K),
                            black_box(&method),
                            black_box(DistanceMetric::Euclidean),
                        )
                        .expect("ivf gpu"),
                    )
                });
            });

            if n >= 50_000 {
                group.bench_function(BenchmarkId::new("nndescent_gpu", &label), |b| {
                    let method = KnnMethod::GpuNnDescent(NnDescentGpuParams::default());
                    b.iter(|| {
                        black_box(
                            compute_knn(
                                black_box(&data),
                                black_box(n),
                                black_box(d),
                                black_box(K),
                                black_box(&method),
                                black_box(DistanceMetric::Euclidean),
                            )
                            .expect("nndescent gpu"),
                        )
                    });
                });
            }
        }
    }

    group.finish();
}

criterion_group!(benches, knn_cpu_vs_gpu);
criterion_main!(benches);
