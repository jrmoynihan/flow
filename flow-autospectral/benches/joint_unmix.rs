//! Joint unmix throughput (QC-core: events already in RAM).
//!
//! Default: n=10_000, d=20, F=8, K_AF=8, a few variants on one collinear pair.
//! `FLOW_AUTOSPECTRAL_BENCH_MATRIX=1` adds n∈{50k,200k} and d∈{40,64}.
//! `FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE=1` adds n=1_000_000.
//!
//! Keep [`match_matrix`] as the AF-only control group; do not mix residual-match
//! IDs into `joint_unmix`.

#![allow(clippy::needless_range_loop)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use faer::Mat;
use flow_autospectral::{
    AfLibrary, JointUnmixConfig, JointUnmixPrecision, SpectralVariants, normalize_unit_peak,
    unmix_autospectral_joint,
};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn joint_grid() -> (Vec<usize>, Vec<usize>, usize) {
    let mut ns = vec![10_000usize];
    let mut ds = vec![20usize];
    let f = 8usize;
    if env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX") {
        ns.extend([50_000, 200_000]);
        ds.extend([40, 64]);
    }
    if env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE") {
        ns.push(1_000_000);
    }
    ns.sort_unstable();
    ds.sort_unstable();
    (ns, ds, f)
}

fn overlapping_spectra(d: usize, n_fluor: usize) -> Mat<f64> {
    let mut m = Mat::<f64>::zeros(d, n_fluor);
    let sigma = (d as f64 / n_fluor.max(1) as f64).max(1.2);
    for j in 0..n_fluor {
        let peak = if n_fluor <= 1 {
            0.0
        } else {
            j as f64 * (d.saturating_sub(1) as f64) / (n_fluor - 1) as f64
        };
        let mut col = vec![0.0; d];
        for i in 0..d {
            let z = (i as f64 - peak) / sigma;
            col[i] = (-0.5 * z * z).exp();
        }
        if n_fluor >= 2 && j == n_fluor - 1 {
            let partner = n_fluor - 2;
            let ppeak = partner as f64 * (d.saturating_sub(1) as f64) / (n_fluor - 1) as f64;
            for i in 0..d {
                let z = (i as f64 - ppeak) / (sigma * 1.15);
                col[i] = 0.65 * (-0.5 * z * z).exp() + 0.35 * col[i];
            }
        }
        normalize_unit_peak(&mut col);
        for i in 0..d {
            m[(i, j)] = col[i];
        }
    }
    m
}

fn unit_peak_library(k: usize, d: usize, seed: u64) -> AfLibrary {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut signatures = Mat::<f64>::zeros(d, k);
    for j in 0..k {
        let peak = j % d;
        let mut col = vec![0.0; d];
        for (i, slot) in col.iter_mut().enumerate() {
            let base = if i == peak { 1.0 } else { 0.04 };
            *slot = base + rng.random_range(-0.005..0.005);
        }
        normalize_unit_peak(&mut col);
        for i in 0..d {
            signatures[(i, j)] = col[i];
        }
    }
    AfLibrary {
        signatures,
        names: (0..k).map(|j| format!("AF_{j}")).collect(),
        detector_names: (0..d).map(|i| format!("D{i}")).collect(),
        provenance: format!("synthetic unit-peak K={k} d={d}"),
    }
}

fn collinear_variants(fluor: &Mat<f64>, names: &[String]) -> SpectralVariants {
    let d = fluor.nrows();
    let n_fluor = fluor.ncols();
    let mut variants = HashMap::new();
    let mut deltas = HashMap::new();
    if n_fluor >= 2 {
        let a = n_fluor - 2;
        let mut vmat = Mat::<f64>::zeros(d, 4);
        let mut dmat = Mat::<f64>::zeros(d, 4);
        for v in 0..4 {
            for i in 0..d {
                let mut spec = fluor[(i, a)];
                spec += (v as f64 * 0.04) * fluor[(i, n_fluor - 1)];
                vmat[(i, v)] = spec;
            }
            let mut col: Vec<f64> = (0..d).map(|i| vmat[(i, v)]).collect();
            normalize_unit_peak(&mut col);
            for i in 0..d {
                vmat[(i, v)] = col[i];
                dmat[(i, v)] = col[i] - fluor[(i, a)];
            }
        }
        variants.insert(names[a].clone(), vmat);
        deltas.insert(names[a].clone(), dmat);
    }
    SpectralVariants {
        thresholds: vec![0.0; n_fluor],
        fluor_names: names.to_vec(),
        variants,
        deltas,
    }
}

fn stained_mix(n: usize, library: &AfLibrary, fluor: &Mat<f64>, seed: u64) -> Vec<f64> {
    let d = library.n_detectors();
    let k = library.n_signatures().max(1);
    let n_fluor = fluor.ncols().max(1);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut events = Vec::with_capacity(n * d);
    for i in 0..n {
        let af = i % k;
        let af_scale = rng.random_range(20.0..80.0);
        let f_idx = i % n_fluor;
        let f_scale = rng.random_range(80.0..400.0);
        for c in 0..d {
            let mut v = library.signatures[(c, af)] * af_scale;
            v += fluor[(c, f_idx)] * f_scale;
            v += rng.random_range(-0.5..0.5);
            events.push(v);
        }
    }
    events
}

fn bench_joint_unmix(c: &mut Criterion) {
    let mut group = c.benchmark_group("joint_unmix");
    let (ns, ds, n_fluor) = joint_grid();
    let cfg_f64 = JointUnmixConfig {
        parallel_event_threshold: 256,
        precision: JointUnmixPrecision::F64,
        ..JointUnmixConfig::default()
    };
    let cfg_f32 = JointUnmixConfig {
        precision: JointUnmixPrecision::F32,
        ..cfg_f64.clone()
    };
    let k_af = 8usize;
    for &n in &ns {
        for &d in &ds {
            let fluor = overlapping_spectra(d, n_fluor);
            let names: Vec<String> = (0..n_fluor).map(|j| format!("F{j}")).collect();
            let library = unit_peak_library(k_af, d, 3);
            let variants = collinear_variants(&fluor, &names);
            let events = stained_mix(n, &library, &fluor, 11);
            group.throughput(Throughput::Elements(n as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("n{n}_d{d}_F{n_fluor}_K{k_af}")),
                &events,
                |b, ev| {
                    b.iter(|| {
                        black_box(
                            unmix_autospectral_joint(
                                black_box(ev),
                                n,
                                fluor.as_ref(),
                                black_box(&names),
                                black_box(&library),
                                black_box(&variants),
                                black_box(&cfg_f64),
                            )
                            .unwrap(),
                        )
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("n{n}_d{d}_F{n_fluor}_K{k_af}_f32")),
                &events,
                |b, ev| {
                    b.iter(|| {
                        black_box(
                            unmix_autospectral_joint(
                                black_box(ev),
                                n,
                                fluor.as_ref(),
                                black_box(&names),
                                black_box(&library),
                                black_box(&variants),
                                black_box(&cfg_f32),
                            )
                            .unwrap(),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_joint_af_only_control(c: &mut Criterion) {
    let mut group = c.benchmark_group("joint_af_only");
    let n = 10_000usize;
    let d = 20usize;
    let n_fluor = 8usize;
    let fluor = overlapping_spectra(d, n_fluor);
    let names: Vec<String> = (0..n_fluor).map(|j| format!("F{j}")).collect();
    let library = unit_peak_library(8, d, 3);
    let variants = SpectralVariants::af_only(names.clone(), vec![0.0; n_fluor]).unwrap();
    let events = stained_mix(n, &library, &fluor, 11);
    let cfg = JointUnmixConfig::default();
    group.throughput(Throughput::Elements(n as u64));
    group.bench_function("n10000_d20_F8_K8_af_only", |b| {
        b.iter(|| {
            black_box(
                unmix_autospectral_joint(
                    black_box(&events),
                    n,
                    fluor.as_ref(),
                    black_box(&names),
                    black_box(&library),
                    black_box(&variants),
                    black_box(&cfg),
                )
                .unwrap(),
            )
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench_joint_unmix, bench_joint_af_only_control
}
criterion_main!(benches);
