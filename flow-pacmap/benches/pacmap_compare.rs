//! Three-way PaCMAP throughput: `flow-pacmap` vs `manifolds-rs` vs `oxicuda-manifold`.
//!
//! # Fairness notes
//!
//! - Neighbor counts aligned: near=10, mid-near=5, further/far=20; seed 42; PCA init;
//!   Adam lr=1.0 (manifolds default 0.01 is overridden).
//! - Weight schedules still differ across crates — wall-time A/B is still valid under
//!   matching iters / pair counts / lr.
//! - oxicuda PaCMAP is **f64** end-to-end; f32→f64 conversion is outside timed regions.
//! - oxicuda `pacmap` e2e uses **brute-force** kNN internally; `oxicuda_hnsw` only
//!   appears in the KNN-only group (separate neighbor API).
//!
//! # Env
//!
//! - `FLOW_PACMAP_BENCH_SMOKE=1` — tiny n + short iters for harness smoke.
//! - `FLOW_PACMAP_BENCH_1M=1` — add (1M, 20) to the full grid (ignored in smoke).
//! - `FLOW_PACMAP_BENCH_MAX_N=<n>` — drop size-grid cells with n above this cap.
//! - `FLOW_PACMAP_BENCH_MATRIX=1` — denser Cartesian n×d grid (expensive).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use faer23::Mat;
use flow_pacmap::config::{HnswParams, Init, KnnMethod, PaCMAPConfig};
use flow_pacmap::knn::KnnGraph;
use flow_pacmap::pairs::build_pairs;
use flow_pacmap::pca::pca_init;
use flow_pacmap::{compute_knn, fit_transform};
use manifolds_rs::prelude::{NearestNeighbourParams, PacmapOptimParams, run_ann_search};
use manifolds_rs::{PacmapParams, pacmap as manifolds_pacmap};
use oxicuda_manifold::neighbor::knn_brute::knn_brute;
use oxicuda_manifold::{
    HnswConfig, HnswDistance, PaCMapConfig, PaCMapInit, hnsw_build, hnsw_search,
    pacmap as oxicuda_pacmap,
};
use rand::{RngExt, SeedableRng, rngs::SmallRng};
use std::hint::black_box;
use std::time::Duration;

const SEED: u64 = 42;
const N_NEIGHBORS: usize = 10;
const N_MID_NEAR: usize = 5;
const N_FURTHER: usize = 20;
const MN_CANDIDATE_END: usize = 50;
/// Shared k for KNN-only arms (covers PaCMAP mid-near window + flow's +50 candidates).
const KNN_K: usize = 60;

fn smoke_mode() -> bool {
    std::env::var("FLOW_PACMAP_BENCH_SMOKE").ok().as_deref() == Some("1")
}

fn include_1m() -> bool {
    !smoke_mode() && std::env::var("FLOW_PACMAP_BENCH_1M").ok().as_deref() == Some("1")
}

fn size_grid() -> Vec<(usize, usize)> {
    if smoke_mode() {
        return vec![(1_000, 10)];
    }
    // Sparse Criterion diagonal (full Cartesian is `flow-knn` collect_matrix).
    // Env `FLOW_PACMAP_BENCH_MATRIX=1` expands to a denser event×dim product.
    let mut grid = if std::env::var("FLOW_PACMAP_BENCH_MATRIX")
        .ok()
        .as_deref()
        == Some("1")
    {
        let mut cells = Vec::new();
        for &n in &[10_000usize, 50_000, 100_000, 250_000, 500_000] {
            for &d in &[10usize, 15, 20] {
                cells.push((n, d));
            }
        }
        cells
    } else {
        vec![
            (10_000, 10),
            (50_000, 10),
            (50_000, 20),
            (100_000, 15),
            (250_000, 20),
            (500_000, 20),
        ]
    };
    if include_1m() {
        grid.push((1_000_000, 20));
    }
    if let Ok(max_n) = std::env::var("FLOW_PACMAP_BENCH_MAX_N")
        && let Ok(max_n) = max_n.parse::<usize>()
    {
        grid.retain(|(n, _)| *n <= max_n);
    }
    grid
}

fn phase_iters() -> [usize; 3] {
    if smoke_mode() {
        [2, 2, 2]
    } else {
        [100, 100, 250]
    }
}

fn total_iters() -> usize {
    phase_iters().iter().sum()
}

fn include_oxicuda_knn(n: usize) -> bool {
    // oxicuda f64 HNSW/brute is much slower; keep it on the small end of the grid only.
    smoke_mode() || n <= 50_000
}

fn knn_sample_size() -> usize {
    if smoke_mode() {
        10
    } else {
        10
    }
}

fn e2e_sample_size() -> usize {
    // Criterion requires sample_size >= 10.
    if smoke_mode() { 10 } else { 10 }
}

fn knn_meas_secs() -> u64 {
    if smoke_mode() { 1 } else { 8 }
}

fn e2e_meas_secs() -> u64 {
    if smoke_mode() { 1 } else { 45 }
}

/// Seeded two-cluster synthetic data (row-major f32) plus peer views prepared outside timed code.
struct Fixture {
    n: usize,
    d: usize,
    data_f32: Vec<f32>,
    data_f64: Vec<f64>,
    /// Column-major samples×features for manifolds-rs (faer 0.23).
    mat_f32: Mat<f32>,
}

impl Fixture {
    fn new(n: usize, d: usize) -> Self {
        let data_f32 = clustered_f32(n, d, SEED);
        let data_f64: Vec<f64> = data_f32.iter().map(|&v| f64::from(v)).collect();
        let mat_f32 = Mat::from_fn(n, d, |i, j| data_f32[i * d + j]);
        Self {
            n,
            d,
            data_f32,
            data_f64,
            mat_f32,
        }
    }
}

fn clustered_f32(n: usize, d: usize, seed: u64) -> Vec<f32> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let n_a = n / 2;
    let n_b = n - n_a;
    let mut data = Vec::with_capacity(n * d);
    for _ in 0..n_a {
        for _ in 0..d {
            data.push(rng.random::<f32>() * 0.5);
        }
    }
    for _ in 0..n_b {
        for _ in 0..d {
            data.push(5.0 + rng.random::<f32>() * 0.5);
        }
    }
    data
}

fn flow_hnsw_params() -> HnswParams {
    HnswParams {
        m: 16,
        ef_construction: 200,
        ef_search: 50,
        ..Default::default()
    }
}

fn flow_config(knn_method: KnnMethod) -> PaCMAPConfig {
    PaCMAPConfig {
        n_neighbors: N_NEIGHBORS,
        mn_ratio: 0.5,
        fp_ratio: 2.0,
        phase_iters: phase_iters(),
        learning_rate: 1.0,
        init: Init::Pca,
        seed: Some(SEED),
        knn_method,
        distance_metric: Default::default(),
    }
}

fn manifolds_nn_params() -> NearestNeighbourParams<f32> {
    let mut p = NearestNeighbourParams::default();
    p.dist_metric = "euclidean".to_string();
    p.m = 16;
    p.ef_construction = 200;
    p.ef_search = 50;
    p
}

fn manifolds_params(ann_type: &str) -> PacmapParams<f32> {
    PacmapParams {
        n_dim: 2,
        ann_type: ann_type.to_string(),
        optimiser_type: "adam_parallel".to_string(),
        n_near: N_NEIGHBORS,
        n_mid_near: N_MID_NEAR,
        n_further: N_FURTHER,
        mn_candidate_start: 4,
        mn_candidate_end: MN_CANDIDATE_END,
        initialisation: "pca".to_string(),
        range: Some(0.01),
        nn_params: manifolds_nn_params(),
        optim_params: PacmapOptimParams::new(
            Some(total_iters()),
            Some(1.0),
            None,
            None,
            None,
            Some(phase_iters()[0]),
            Some(phase_iters()[0] + phase_iters()[1]),
        ),
    }
}

fn oxicuda_config() -> PaCMapConfig {
    PaCMapConfig {
        n_components: 2,
        n_neighbors: N_NEIGHBORS,
        n_mid_near: N_MID_NEAR,
        n_far: N_FURTHER,
        lr: 1.0,
        n_iter: total_iters(),
        init: PaCMapInit::Pca,
        seed: SEED,
    }
}

fn oxicuda_hnsw_config() -> HnswConfig {
    HnswConfig {
        m: 16,
        ef_construction: 200,
        ef_search: 50,
        seed: SEED,
        distance: HnswDistance::Euclidean,
        ..Default::default()
    }
}

fn id(arm: &str, n: usize, d: usize) -> BenchmarkId {
    BenchmarkId::new(arm, format!("n{n}_d{d}"))
}

// ─── Groups ───────────────────────────────────────────────────────────────────

fn pacmap_knn(c: &mut Criterion) {
    let mut group = c.benchmark_group("pacmap_knn");
    group.sample_size(knn_sample_size());
    group.measurement_time(Duration::from_secs(knn_meas_secs()));

    for &(n, d) in &size_grid() {
        let fx = Fixture::new(n, d);
        group.throughput(Throughput::Elements(n as u64));
        let run_exact = n <= 50_000;

        group.bench_function(id("flow_hnsw", n, d), |b| {
            let method = KnnMethod::Hnsw(flow_hnsw_params());
            b.iter(|| {
                black_box(
                    compute_knn(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(KNN_K),
                        black_box(&method),
                        Default::default(),
                    )
                    .expect("flow hnsw knn"),
                )
            });
        });

        group.bench_function(id("manifolds_hnsw", n, d), |b| {
            let nn = manifolds_nn_params();
            b.iter(|| {
                black_box(
                    run_ann_search(
                        black_box(fx.mat_f32.as_ref()),
                        black_box(KNN_K),
                        black_box("hnsw".to_string()),
                        black_box(&nn),
                        black_box(SEED as usize),
                        black_box(0),
                    )
                    .expect("manifolds hnsw knn"),
                )
            });
        });

        #[cfg(feature = "ann-search")]
        group.bench_function(id("flow_ann_hnsw", n, d), |b| {
            let method = KnnMethod::AnnSearchHnsw(flow_hnsw_params());
            b.iter(|| {
                black_box(
                    compute_knn(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(KNN_K),
                        black_box(&method),
                        Default::default(),
                    )
                    .expect("flow ann-search hnsw knn"),
                )
            });
        });

        if include_oxicuda_knn(n) {
            group.bench_function(id("oxicuda_hnsw", n, d), |b| {
                let cfg = oxicuda_hnsw_config();
                b.iter(|| {
                    let index = hnsw_build(
                        black_box(&fx.data_f64),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(&cfg),
                    )
                    .expect("oxicuda hnsw build");
                    black_box(
                        hnsw_search(
                            black_box(&index),
                            black_box(&fx.data_f64),
                            black_box(fx.n),
                            black_box(KNN_K),
                        )
                        .expect("oxicuda hnsw search"),
                    )
                });
            });
        }

        if run_exact {
            group.bench_function(id("flow_exact", n, d), |b| {
                let method = KnnMethod::Exact;
                b.iter(|| {
                    black_box(
                        compute_knn(
                            black_box(&fx.data_f32),
                            black_box(fx.n),
                            black_box(fx.d),
                            black_box(KNN_K),
                            black_box(&method),
                            Default::default(),
                        )
                        .expect("flow exact knn"),
                    )
                });
            });

            group.bench_function(id("manifolds_exhaustive", n, d), |b| {
                let nn = manifolds_nn_params();
                b.iter(|| {
                    black_box(
                        run_ann_search(
                            black_box(fx.mat_f32.as_ref()),
                            black_box(KNN_K),
                            black_box("exhaustive".to_string()),
                            black_box(&nn),
                            black_box(SEED as usize),
                            black_box(0),
                        )
                        .expect("manifolds exhaustive knn"),
                    )
                });
            });

            if smoke_mode() {
                group.bench_function(id("oxicuda_brute", n, d), |b| {
                    b.iter(|| {
                        black_box(
                            knn_brute(
                                black_box(&fx.data_f64),
                                black_box(fx.n),
                                black_box(fx.d),
                                black_box(KNN_K),
                            )
                            .expect("oxicuda brute knn"),
                        )
                    });
                });
            }
        }
    }

    group.finish();
}

fn pacmap_fit_transform(c: &mut Criterion) {
    let mut group = c.benchmark_group("pacmap_fit_transform");
    group.sample_size(e2e_sample_size());
    group.measurement_time(Duration::from_secs(e2e_meas_secs()));

    for &(n, d) in &size_grid() {
        let fx = Fixture::new(n, d);
        group.throughput(Throughput::Elements(n as u64));

        group.bench_function(id("flow_fit_transform", n, d), |b| {
            let config = flow_config(KnnMethod::Hnsw(flow_hnsw_params()));
            b.iter(|| {
                black_box(
                    fit_transform(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(config.clone()),
                        black_box(None),
                        None,
                        None,
                    )
                    .expect("flow fit_transform"),
                )
            });
        });

        #[cfg(feature = "ann-search")]
        group.bench_function(id("flow_ann_fit_transform", n, d), |b| {
            let config = flow_config(KnnMethod::AnnSearchHnsw(flow_hnsw_params()));
            b.iter(|| {
                black_box(
                    fit_transform(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(config.clone()),
                        black_box(None),
                        None,
                        None,
                    )
                    .expect("flow ann-search fit_transform"),
                )
            });
        });

        group.bench_function(id("manifolds_pacmap", n, d), |b| {
            let params = manifolds_params("hnsw");
            b.iter(|| {
                black_box(
                    manifolds_pacmap(
                        black_box(fx.mat_f32.as_ref()),
                        black_box(None),
                        black_box(&params),
                        black_box(SEED as usize),
                        black_box(0),
                    )
                    .expect("manifolds pacmap"),
                )
            });
        });

        // oxicuda e2e is brute-force KNN + f64 — only smoke / tiny n.
        if smoke_mode() {
            group.bench_function(id("oxicuda_pacmap", n, d), |b| {
                let config = oxicuda_config();
                b.iter(|| {
                    black_box(
                        oxicuda_pacmap(
                            black_box(&fx.data_f64),
                            black_box(fx.n),
                            black_box(fx.d),
                            black_box(&config),
                        )
                        .expect("oxicuda pacmap"),
                    )
                });
            });
        }
    }

    group.finish();
}

fn pacmap_knn_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("pacmap_knn_reuse");
    group.sample_size(knn_sample_size());
    group.measurement_time(Duration::from_secs(e2e_meas_secs()));

    // Reuse comparison is expensive; keep a small grid unless smoke/full env expands it.
    let reuse_grid: Vec<(usize, usize)> = if smoke_mode() {
        size_grid()
    } else {
        vec![(50_000, 10)]
    };

    for &(n, d) in &reuse_grid {
        let fx = Fixture::new(n, d);
        group.throughput(Throughput::Elements(n as u64));
        let config = flow_config(KnnMethod::Hnsw(flow_hnsw_params()));
        let k = KnnGraph::required_k_for_pacmap(n, N_NEIGHBORS);
        let method = KnnMethod::Hnsw(flow_hnsw_params());

        group.bench_function(id("flow_cold_3x", n, d), |b| {
            let config = config.clone();
            let method = method.clone();
            b.iter(|| {
                for _ in 0..3 {
                    let knn = compute_knn(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(k),
                        black_box(&method),
                        Default::default(),
                    )
                    .expect("cold knn");
                    black_box(
                        fit_transform(
                            black_box(&fx.data_f32),
                            black_box(fx.n),
                            black_box(fx.d),
                            black_box(config.clone()),
                            black_box(Some(&knn)),
                            None,
                            None,
                        )
                        .expect("cold fit"),
                    );
                }
            });
        });

        group.bench_function(id("flow_hot_1knn_3embed", n, d), |b| {
            let config = config.clone();
            let method = method.clone();
            b.iter(|| {
                let knn = compute_knn(
                    black_box(&fx.data_f32),
                    black_box(fx.n),
                    black_box(fx.d),
                    black_box(k),
                    black_box(&method),
                    Default::default(),
                )
                .expect("hot knn");
                for _ in 0..3 {
                    black_box(
                        fit_transform(
                            black_box(&fx.data_f32),
                            black_box(fx.n),
                            black_box(fx.d),
                            black_box(config.clone()),
                            black_box(Some(&knn)),
                            None,
                            None,
                        )
                        .expect("hot fit"),
                    );
                }
            });
        });
    }

    group.finish();
}

fn pacmap_stage_breakdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("pacmap_stage_breakdown");
    group.sample_size(knn_sample_size());
    group.measurement_time(Duration::from_secs(e2e_meas_secs()));

    let stage_grid: Vec<(usize, usize)> = if smoke_mode() {
        size_grid()
    } else {
        vec![(50_000, 10)]
    };

    for &(n, d) in &stage_grid {
        let fx = Fixture::new(n, d);
        group.throughput(Throughput::Elements(n as u64));
        let config = flow_config(KnnMethod::Hnsw(flow_hnsw_params()));
        let k = KnnGraph::required_k_for_pacmap(n, N_NEIGHBORS);
        let method = KnnMethod::Hnsw(flow_hnsw_params());
        let n_nb = N_NEIGHBORS.min(n - 1);
        let n_mn = (N_NEIGHBORS as f32 * 0.5).floor() as usize;
        let n_fp = (N_NEIGHBORS as f32 * 2.0).floor() as usize;

        group.bench_function(id("flow_stage_knn", n, d), |b| {
            let method = method.clone();
            b.iter(|| {
                black_box(
                    compute_knn(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(k),
                        black_box(&method),
                        Default::default(),
                    )
                    .expect("stage knn"),
                )
            });
        });

        // Precompute knn once outside iter for pairs / init / post-knn fits.
        let knn = compute_knn(&fx.data_f32, fx.n, fx.d, k, &method, Default::default())
            .expect("precompute knn for stages");

        group.bench_function(id("flow_stage_pairs", n, d), |b| {
            b.iter(|| {
                black_box(
                    build_pairs(
                        black_box(&knn.neighbors),
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(n_nb),
                        black_box(n_mn),
                        black_box(n_fp),
                        black_box(Some(SEED)),
                    )
                    .expect("stage pairs"),
                )
            });
        });

        group.bench_function(id("flow_stage_pca_init", n, d), |b| {
            b.iter(|| {
                black_box(
                    pca_init(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                    )
                    .expect("stage pca"),
                )
            });
        });

        // pairs + init + Adam (KNN skipped via Some)
        group.bench_function(id("flow_stage_post_knn", n, d), |b| {
            let config = config.clone();
            b.iter(|| {
                black_box(
                    fit_transform(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(config.clone()),
                        black_box(Some(&knn)),
                        None,
                        None,
                    )
                    .expect("stage post-knn fit"),
                )
            });
        });

        group.bench_function(id("flow_stage_full_none", n, d), |b| {
            let config = config.clone();
            b.iter(|| {
                black_box(
                    fit_transform(
                        black_box(&fx.data_f32),
                        black_box(fx.n),
                        black_box(fx.d),
                        black_box(config.clone()),
                        black_box(None),
                        None,
                        None,
                    )
                    .expect("stage full none"),
                )
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    pacmap_knn,
    pacmap_fit_transform,
    pacmap_knn_reuse,
    pacmap_stage_breakdown
);
criterion_main!(benches);
