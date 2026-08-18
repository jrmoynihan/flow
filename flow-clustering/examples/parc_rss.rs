//! Peak RSS / wall for PARC end-to-end across n×d and k-NN backends.
//!
//! ```text
//! cargo run -p flow-clustering --release --example parc_rss --features parc
//! # Isolated cell (fresh process): n_d_k_{exact|hnsw}_{seq|rayon}
//! PARC_RSS_FILTER=50000_20_30_hnsw_rayon cargo run -p flow-clustering --release --example parc_rss --features parc
//! ```
//!
//! On macOS, `ru_maxrss` is bytes; on Linux it is kilobytes (converted here).

use flow_clustering::{KeepLocalDist, Parc, ParcConfig};
use flow_knn::{HnswParams, KnnMethod};
use ndarray::Array2;
use std::time::Instant;

fn synth_clouds(n: usize, d: usize) -> Array2<f64> {
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

#[cfg(unix)]
fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return 0;
    }
    let usage = unsafe { usage.assume_init() };
    let raw = usage.ru_maxrss as u64;
    if cfg!(target_os = "macos") {
        raw
    } else {
        raw.saturating_mul(1024)
    }
}

#[cfg(not(unix))]
fn peak_rss_bytes() -> u64 {
    0
}

fn fmt_bytes(b: u64) -> String {
    if b >= 1 << 30 {
        format!("{:.2} GiB", b as f64 / ((1u64 << 30) as f64))
    } else if b >= 1 << 20 {
        format!("{:.2} MiB", b as f64 / ((1u64 << 20) as f64))
    } else if b >= 1 << 10 {
        format!("{:.2} KiB", b as f64 / ((1u64 << 10) as f64))
    } else {
        format!("{b} B")
    }
}

fn hnsw_method() -> KnnMethod {
    KnnMethod::Hnsw(HnswParams {
        m: 24,
        ef_construction: 150,
        ef_search: 100,
        quantization: flow_knn::Quantization::F32,
    })
}

fn main() {
    let cases: &[(usize, usize, usize)] = &[
        (5_000, 20, 20),
        (20_000, 20, 30),
        (50_000, 20, 30),
        (100_000, 20, 30),
    ];

    println!(
        "machine_hint={} cores≈{}",
        std::env::consts::ARCH,
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0)
    );
    println!("n\td\tk\tknn\tparallel\twall_ms\tn_clusters\tpeak_rss\tpeak_rss_bytes");

    let filter = std::env::var("PARC_RSS_FILTER").ok();
    // Default grid: Exact vs HNSW with Rayon prune (production-like).
    // Filter key: n_d_k_{exact|hnsw}_{seq|rayon}
    for &(n, d, knn) in cases {
        for (knn_label, method) in [("exact", KnnMethod::Exact), ("hnsw", hnsw_method())] {
            for parallel in [true] {
                let par_label = if parallel { "rayon" } else { "seq" };
                if let Some(ref f) = filter {
                    let key = format!("{n}_{d}_{knn}_{knn_label}_{par_label}");
                    if f != &key {
                        continue;
                    }
                }
                let data = synth_clouds(n, d);
                let config = ParcConfig {
                    knn,
                    knn_method: Some(method.clone()),
                    keep_all_local_dist: KeepLocalDist::Never,
                    parallel_prune: parallel,
                    too_big_factor: 0.9,
                    ..ParcConfig::default()
                };
                let t0 = Instant::now();
                let result = Parc::fit(&data, &config).expect("parc");
                let wall_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let rss = peak_rss_bytes();
                println!(
                    "{n}\t{d}\t{knn}\t{knn_label}\t{par_label}\t{wall_ms:.1}\t{}\t{}\t{rss}",
                    result.n_clusters,
                    fmt_bytes(rss)
                );
            }
        }
    }
}
