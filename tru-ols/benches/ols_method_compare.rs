//! Compare OLS code paths: per-event QR/LS, CPU normal equations, GPU-assisted RHS + CPU Cholesky.
//!
//! Build: `cargo bench -p flow-tru-ols --no-default-features --features cubecl --bench ols_method_compare`
//!
//! Optional 1M-event grid (slow): `FLOW_TRU_OLS_BENCH_1M=1 cargo bench ...`
//!
//! Set `FLOW_TRU_OLS_FORCE_SEQUENTIAL=1` to A/B Rayon in `run_ols`. GPU benches run only if
//! `try_shared_gpu_context` succeeds (WGPU smoke GEMM).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use faer::Mat;
use flow_tru_ols::benchmark::run_ols;
use flow_tru_ols::try_shared_gpu_context;
use flow_tru_ols::{run_ols_normal_equations, run_ols_normal_equations_gpu_rhs};
use rand::RngExt;
use std::hint::black_box;

fn generate_fixture(
    n_events: usize,
    n_detectors: usize,
    n_endmembers: usize,
) -> (Mat<f64>, Mat<f64>) {
    let mut rng = rand::rng();
    let mixing_matrix = Mat::from_fn(n_detectors, n_endmembers, |i, j| {
        if i == j {
            0.8 + rng.random_range(0.0..0.2)
        } else {
            rng.random_range(0.0..0.1)
        }
    });
    let observations = Mat::from_fn(n_events, n_detectors, |_, _| rng.random_range(0.0..100.0));
    (mixing_matrix, observations)
}

fn bench_grid(
    c: &mut Criterion,
    group_name: &str,
    event_grid: &[usize],
    sample_size: usize,
    meas_secs: u64,
) {
    let gpu_ctx = try_shared_gpu_context().ok();
    let mut group = c.benchmark_group(group_name);
    group.sample_size(sample_size);
    group.measurement_time(std::time::Duration::from_secs(meas_secs));

    for &n_events in event_grid {
        let (mixing, observations) = generate_fixture(n_events, 10, 10);
        group.throughput(Throughput::Elements(n_events as u64));

        let m_qr = mixing.clone();
        let o_qr = observations.clone();
        group.bench_function(
            BenchmarkId::new("run_ols_qr_parallel", n_events),
            move |b| {
                b.iter(|| run_ols(black_box(o_qr.as_ref()), black_box(m_qr.as_ref())).unwrap());
            },
        );

        let m_ne = mixing.clone();
        let o_ne = observations.clone();
        group.bench_function(
            BenchmarkId::new("normal_equations_cpu_f64", n_events),
            move |b| {
                b.iter(|| {
                    run_ols_normal_equations(black_box(o_ne.as_ref()), black_box(m_ne.as_ref()))
                        .unwrap()
                });
            },
        );

        if let Some(gpu) = gpu_ctx {
            let m_g = mixing.clone();
            let o_g = observations.clone();
            group.bench_function(
                BenchmarkId::new("normal_equations_gpu_rhs_f32", n_events),
                move |b| {
                    b.iter(|| {
                        run_ols_normal_equations_gpu_rhs(
                            black_box(o_g.as_ref()),
                            black_box(m_g.as_ref()),
                            gpu,
                        )
                        .unwrap()
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_ols_method_matrix(c: &mut Criterion) {
    let pressure = std::env::var("FLOW_TRU_OLS_BENCH_PRESSURE").ok().as_deref() == Some("1")
        || std::env::var("FLOW_TRU_OLS_BENCH_1M").ok().as_deref() == Some("1");
    // FCS-scale default; pressure adds 500k / 1M events.
    bench_grid(c, "ols_method_matrix", &[50_000, 200_000], 15, 8);
    if pressure {
        bench_grid(c, "ols_method_matrix_pressure", &[500_000, 1_000_000], 10, 20);
    }
}

criterion_group!(benches, bench_ols_method_matrix);
criterion_main!(benches);
