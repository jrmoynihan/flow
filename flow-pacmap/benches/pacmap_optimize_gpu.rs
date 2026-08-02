//! CPU vs GPU PaCMAP optimize (Adam + pair gradients).
//!
//! ```bash
//! cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu -- \
//!   --warm-up-time 1 --measurement-time 20 --sample-size 10
//!
//! FLOW_PACMAP_BENCH_SMOKE=1 cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu
//! FLOW_PACMAP_BENCH_PRESSURE=1 cargo bench -p flow-pacmap --features "cubecl,ann-search" --bench pacmap_optimize_gpu
//! ```

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_pacmap::config::{Init, KnnMethod, OptimizeBackend, PaCMAPConfig};
use flow_pacmap::{compute_knn, fit_transform};
use rand::{RngExt, SeedableRng, rngs::SmallRng};
use std::hint::black_box;
use std::time::Duration;

fn smoke() -> bool {
    std::env::var("FLOW_PACMAP_BENCH_SMOKE").ok().as_deref() == Some("1")
}

fn pressure() -> bool {
    std::env::var("FLOW_PACMAP_BENCH_PRESSURE").ok().as_deref() == Some("1")
}

fn sizes() -> Vec<(usize, usize)> {
    if smoke() {
        vec![(2_000, 10)]
    } else if pressure() {
        vec![(10_000, 10), (50_000, 10), (100_000, 15), (250_000, 20)]
    } else {
        vec![(10_000, 10), (50_000, 10), (100_000, 15)]
    }
}

fn phase_iters() -> [usize; 3] {
    if smoke() {
        [5, 5, 10]
    } else {
        [100, 100, 250]
    }
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

fn config(backend: OptimizeBackend) -> PaCMAPConfig {
    PaCMAPConfig {
        n_neighbors: 10,
        phase_iters: phase_iters(),
        knn_method: KnnMethod::default(),
        init: Init::Random(Some(7)),
        seed: Some(11),
        optimize_backend: backend,
        ..Default::default()
    }
}

fn pacmap_optimize_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("pacmap_optimize_cpu_vs_gpu");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(if smoke() {
        3
    } else if pressure() {
        45
    } else {
        30
    }));
    group.warm_up_time(Duration::from_secs(1));

    for &(n, d) in &sizes() {
        let data = synthetic(n, d);
        group.throughput(Throughput::Elements(n as u64));

        // Precompute KNN once so both arms time optimize (+pair build once inside fit).
        let k = flow_pacmap::KnnGraph::required_k_for_pacmap(n, 10);
        let knn = compute_knn(
            &data,
            n,
            d,
            k,
            &KnnMethod::default(),
            Default::default(),
        )
        .expect("knn");

        group.bench_function(BenchmarkId::new("cpu", format!("n{n}_d{d}")), |b| {
            let cfg = config(OptimizeBackend::Cpu);
            b.iter(|| {
                black_box(
                    fit_transform(
                        black_box(&data),
                        black_box(n),
                        black_box(d),
                        black_box(cfg.clone()),
                        black_box(Some(&knn)),
                        None,
                        None,
                    )
                    .expect("cpu fit"),
                )
            });
        });

        match flow_pacmap::gpu::try_shared_gpu_context() {
            Ok(_) => {
                group.bench_function(BenchmarkId::new("gpu", format!("n{n}_d{d}")), |b| {
                    let cfg = config(OptimizeBackend::Gpu);
                    b.iter(|| {
                        black_box(
                            fit_transform(
                                black_box(&data),
                                black_box(n),
                                black_box(d),
                                black_box(cfg.clone()),
                                black_box(Some(&knn)),
                                None,
                                None,
                            )
                            .expect("gpu fit"),
                        )
                    });
                });
            }
            Err(e) => {
                eprintln!("skipping GPU arm for n={n}: {e}");
            }
        }
    }

    group.finish();
}

criterion_group!(benches, pacmap_optimize_cpu_vs_gpu);
criterion_main!(benches);
