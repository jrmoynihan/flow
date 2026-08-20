//! Throughput of AF discovery and residual matching (event counts, not quality).
//!
//! Interleave baseline/HEAD Criterion runs and keep an untouched control bench
//! when A/B-ing on this machine (see `docs/PERF_AB.md`).
//! n×K×d residual/NN matrix (naive vs factored vs seq): `benches/match_matrix.rs`.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use faer::Mat;
use flow_autospectral::{
    DiscoverConfig, DiscoveryBackend, MatchConfig, MatchStrategy, discover_af_library, match_events,
};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use std::hint::black_box;

fn two_af_events(n: usize, d: usize, seed: u64) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut events = Vec::with_capacity(n * d);
    for i in 0..n {
        let peak = if i < n / 2 { 0 } else { d.saturating_sub(1) };
        for c in 0..d {
            let base = if c == peak { 8.0 } else { 1.0 };
            events.push(base + rng.random_range(-0.05..0.05));
        }
    }
    events
}

fn bench_discover(c: &mut Criterion) {
    let mut group = c.benchmark_group("discover_af_library");
    let d = 8usize;
    let detectors: Vec<String> = (0..d).map(|i| format!("D{i}")).collect();
    let cfg = DiscoverConfig {
        backend: DiscoveryBackend::KMeans,
        fixed_k: Some(4),
        seed: Some(1),
        ..DiscoverConfig::default()
    };

    for &n in &[2_000usize, 10_000, 50_000] {
        let events = two_af_events(n, d, 7);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &events, |b, ev| {
            b.iter(|| {
                discover_af_library(black_box(ev), n, d, black_box(&detectors), black_box(&cfg))
                    .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_match_residual(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_events_residual");
    let d = 8usize;
    let detectors: Vec<String> = (0..d).map(|i| format!("D{i}")).collect();
    let unstained = two_af_events(4_000, d, 3);
    let cfg = DiscoverConfig {
        backend: DiscoveryBackend::KMeans,
        fixed_k: Some(4),
        seed: Some(1),
        ..DiscoverConfig::default()
    };
    let lib = discover_af_library(&unstained, 4_000, d, &detectors, &cfg).unwrap();
    let fluor = Mat::<f64>::zeros(d, 2);
    let match_cfg = MatchConfig {
        strategy: MatchStrategy::ResidualOls,
        parallel_event_threshold: 256,
        ..MatchConfig::default()
    };

    for &n in &[2_000usize, 10_000, 50_000] {
        let stained = two_af_events(n, d, 9);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &stained, |b, ev| {
            b.iter(|| {
                match_events(
                    black_box(ev),
                    n,
                    black_box(&lib),
                    fluor.as_ref(),
                    black_box(&match_cfg),
                )
                .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_match_nn_control(c: &mut Criterion) {
    // Untouched-control-style counterpart: NN matching, same sizes as residual.
    let mut group = c.benchmark_group("match_events_nn");
    let d = 8usize;
    let detectors: Vec<String> = (0..d).map(|i| format!("D{i}")).collect();
    let unstained = two_af_events(4_000, d, 3);
    let cfg = DiscoverConfig {
        backend: DiscoveryBackend::KMeans,
        fixed_k: Some(4),
        seed: Some(1),
        ..DiscoverConfig::default()
    };
    let lib = discover_af_library(&unstained, 4_000, d, &detectors, &cfg).unwrap();
    let fluor = Mat::<f64>::zeros(d, 0);
    let match_cfg = MatchConfig {
        strategy: MatchStrategy::NearestNeighbor,
        parallel_event_threshold: 256,
        ..MatchConfig::default()
    };

    for &n in &[2_000usize, 10_000, 50_000] {
        let stained = two_af_events(n, d, 9);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &stained, |b, ev| {
            b.iter(|| {
                match_events(
                    black_box(ev),
                    n,
                    black_box(&lib),
                    fluor.as_ref(),
                    black_box(&match_cfg),
                )
                .unwrap()
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_discover,
    bench_match_residual,
    bench_match_nn_control
);
criterion_main!(benches);
