//! CPU vs GPU assignment crossover for e-means.
//!
//! ```text
//! cargo bench -p flow-clustering --bench emeans_assign_cpu_vs_gpu --features emeans,emeans-gpu
//! ```
//!
//! Skips GPU arms when no WGPU adapter is available. Interleaves CPU and GPU
//! with an untouched control (sum of f32) per Beads memory on M5 Max drift.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_clustering::clustering::emeans::gpu::GpuAssign;
use flow_clustering::clustering::emeans::{Assign, CpuLoopAssign};
use std::hint::black_box;
use std::time::Duration;

fn synth(n: usize, d: usize, k: usize) -> (Vec<f32>, Vec<f32>) {
    let mut points = vec![0.0f32; n * d];
    let mut centroids = vec![0.0f32; k * d];
    for i in 0..n * d {
        points[i] = ((i * 17) % 100) as f32 * 0.01;
    }
    for i in 0..k * d {
        centroids[i] = ((i * 13) % 100) as f32 * 0.1;
    }
    (points, centroids)
}

fn control_sum(data: &[f32]) -> f32 {
    data.iter().sum()
}

fn bench_assign(c: &mut Criterion) {
    let batches = [1_024usize, 16_384, 65_536];
    let ks = [16usize, 256, 2_048];
    let ds = [30usize, 60];

    let gpu = GpuAssign::try_new().ok();
    if gpu.is_none() {
        eprintln!("GPU adapter unavailable — CPU-only arms will run");
    }

    let mut group = c.benchmark_group("emeans_assign_cpu_vs_gpu");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(12);

    for &d in &ds {
        for &batch in &batches {
            for &k in &ks {
                let (points, centroids) = synth(batch, d, k);
                let work = (batch * k * d) as u64;
                group.throughput(Throughput::Elements(work));

                let id_ctrl = format!("control_n{batch}_d{d}_k{k}");
                group.bench_with_input(
                    BenchmarkId::from_parameter(&id_ctrl),
                    &points,
                    |b, pts| {
                        b.iter(|| black_box(control_sum(black_box(pts))));
                    },
                );

                let id_cpu = format!("cpu_n{batch}_d{d}_k{k}");
                group.bench_with_input(
                    BenchmarkId::from_parameter(&id_cpu),
                    &(points.clone(), centroids.clone()),
                    |b, (pts, cents)| {
                        let mut out = vec![0u32; batch];
                        b.iter(|| {
                            CpuLoopAssign
                                .assign(
                                    black_box(pts),
                                    batch,
                                    d,
                                    black_box(cents),
                                    k,
                                    black_box(&mut out),
                                )
                                .expect("cpu");
                            black_box(&out);
                        });
                    },
                );

                if let Some(ref g) = gpu {
                    let id_gpu = format!("gpu_n{batch}_d{d}_k{k}");
                    group.bench_with_input(
                        BenchmarkId::from_parameter(&id_gpu),
                        &(points.clone(), centroids.clone()),
                        |b, (pts, cents)| {
                            let mut out = vec![0u32; batch];
                            b.iter(|| {
                                g.assign(
                                    black_box(pts),
                                    batch,
                                    d,
                                    black_box(cents),
                                    k,
                                    black_box(&mut out),
                                )
                                .expect("gpu");
                                black_box(&out);
                            });
                        },
                    );
                }
            }
        }
    }
    group.finish();
}

criterion_group!(benches, bench_assign);
criterion_main!(benches);
