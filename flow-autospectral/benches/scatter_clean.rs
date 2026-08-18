//! Scatter-clean throughput: CPU Exact (always) and optional GPU Exact.
//!
//! Groups: `scatter_clean_cpu`, `scatter_clean_gpu` (feature `gpu`; skipped when
//! `AnnIndex::build` returns `MethodNotImplemented` or `GpuUnavailable`).
//!
//! Env:
//! - default: n_unstained = n_stained ∈ {10_000, 50_000}, d_scatter = 2
//! - `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1`: also 100_000
//!
//! Synthetic 2-d FSC/SSC-like Gaussians (no FCS).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_autospectral::{CleanConfig, ScatterCleanConfig, ScatterInput, clean_unstained};
use flow_knn::KnnMethod;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::hint::black_box;
use std::time::Duration;

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn scatter_sizes() -> Vec<usize> {
    let mut ns = vec![10_000usize, 50_000];
    if env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX") {
        ns.push(100_000);
    }
    ns
}

/// Two overlapping 2-d clouds (FSC/SSC-like).
fn fsc_ssc_gaussians(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let (fsc_mu, ssc_mu) = if i % 3 == 0 {
            (80.0, 20.0)
        } else {
            (100.0, 40.0)
        };
        out.push(fsc_mu + rng.random_range(-8.0..8.0));
        out.push(ssc_mu + rng.random_range(-6.0..6.0));
    }
    out
}

fn dummy_fluorescence(n: usize, n_det: usize, seed: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n * n_det).map(|_| rng.random_range(0.5..2.0)).collect()
}

fn run_scatter_group(c: &mut Criterion, group_name: &str, knn_method: KnnMethod) {
    let mut group = c.benchmark_group(group_name);
    let n_det = 4usize;
    let scatter_cfg = ScatterCleanConfig {
        knn_method,
        ..ScatterCleanConfig::default()
    };
    let clean_cfg = CleanConfig {
        scatter: Some(scatter_cfg),
        pca: None,
    };
    for &n in &scatter_sizes() {
        let unstained_sc = fsc_ssc_gaussians(n, 11);
        let stained_sc = fsc_ssc_gaussians(n, 13);
        let fluor = dummy_fluorescence(n, n_det, 17);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("n{n}")),
            &fluor,
            |b, fluorescence| {
                b.iter(|| {
                    black_box(
                        clean_unstained(
                            black_box(fluorescence),
                            n,
                            n_det,
                            Some(ScatterInput {
                                unstained: black_box(&unstained_sc),
                                n_unstained: n,
                                stained: black_box(&stained_sc),
                                n_stained: n,
                                dims: 2,
                            }),
                            black_box(&clean_cfg),
                        )
                        .unwrap(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_scatter_clean_cpu(c: &mut Criterion) {
    run_scatter_group(c, "scatter_clean_cpu", KnnMethod::Exact);
}

#[cfg(feature = "gpu")]
fn bench_scatter_clean_gpu(c: &mut Criterion) {
    use flow_knn::{AnnIndex, DistanceMetric, KnnError};

    let probe = vec![0.0f32, 0.0, 1.0, 1.0, 2.0, 2.0];
    match AnnIndex::build(
        &probe,
        3,
        2,
        &KnnMethod::GpuExact,
        DistanceMetric::Euclidean,
    ) {
        Ok(_) => {}
        Err(KnnError::MethodNotImplemented { method }) => {
            eprintln!("skip scatter_clean_gpu: AnnIndex GPU not implemented ({method})");
            return;
        }
        Err(KnnError::GpuUnavailable(msg)) => {
            eprintln!("skip scatter_clean_gpu: GPU unavailable ({msg})");
            return;
        }
        Err(e) => {
            eprintln!("skip scatter_clean_gpu: {e}");
            return;
        }
    }

    run_scatter_group(c, "scatter_clean_gpu", KnnMethod::GpuExact);
}

#[cfg(not(feature = "gpu"))]
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench_scatter_clean_cpu
}

#[cfg(feature = "gpu")]
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench_scatter_clean_cpu, bench_scatter_clean_gpu
}
criterion_main!(benches);
