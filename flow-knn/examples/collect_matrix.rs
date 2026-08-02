//! Widen the (n × d) KNN performance matrix and append JSONL records.
//!
//! ```text
//! cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search"
//!
//! # With GPU backends (exact / IVF / NN-Descent):
//! cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"
//!
//! # Smoke (tiny grid, few repeats):
//! FLOW_KNN_MATRIX_SMOKE=1 cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search,gpu"
//!
//! # Cap max n:
//! FLOW_KNN_MATRIX_MAX_N=50000 cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search"
//!
//! # Custom output path (append):
//! FLOW_KNN_MATRIX_OUT=./my_matrix.jsonl cargo run -p flow-knn --release --example collect_matrix --features "hnsw,ann-search"
//! ```
//!
//! Default event × dimension Cartesian product (FCS-oriented):
//!   n ∈ {10k, 50k, 100k, 250k, 500k}
//!   d ∈ {10, 15, 20}
//! Exact / exact_gpu skipped when n > 100k (too slow for routine matrix growth).

use flow_knn::{
    DistanceMetric, HnswParams, KnnMethod, PerfRecord, compute_knn,
};
use rand::{RngExt, SeedableRng, rngs::SmallRng};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SEED: u64 = 42;
const K: usize = 60;
const REPEATS: usize = 3;

fn smoke() -> bool {
    std::env::var("FLOW_KNN_MATRIX_SMOKE").ok().as_deref() == Some("1")
}

fn max_n() -> Option<usize> {
    std::env::var("FLOW_KNN_MATRIX_MAX_N")
        .ok()
        .and_then(|s| s.parse().ok())
}

fn out_path() -> PathBuf {
    if let Ok(p) = std::env::var("FLOW_KNN_MATRIX_OUT") {
        return PathBuf::from(p);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/knn_perf_matrix.jsonl")
}

fn matrix_grid() -> Vec<(usize, usize)> {
    if smoke() {
        return vec![(2_000, 10), (2_000, 15)];
    }
    let ns = [10_000usize, 50_000, 100_000, 250_000, 500_000];
    let ds = [10usize, 15, 20];
    let mut cells = Vec::new();
    for &n in &ns {
        for &d in &ds {
            cells.push((n, d));
        }
    }
    if let Some(cap) = max_n() {
        cells.retain(|(n, _)| *n <= cap);
    }
    cells
}

fn methods() -> Vec<(&'static str, KnnMethod)> {
    let mut out = vec![("exact", KnnMethod::Exact)];
    #[cfg(feature = "hnsw")]
    out.push((
        "hnsw_usearch",
        KnnMethod::Hnsw(HnswParams::default()),
    ));
    #[cfg(feature = "ann-search")]
    out.push((
        "hnsw_ann_search",
        KnnMethod::AnnSearchHnsw(HnswParams::default()),
    ));
    #[cfg(feature = "gpu")]
    if flow_knn::gpu_adapter_available() {
        out.push(("exact_gpu", KnnMethod::GpuExact));
        out.push((
            "ivf_gpu",
            KnnMethod::GpuIvf(flow_knn::IvfGpuParams::default()),
        ));
        out.push((
            "nndescent_gpu",
            KnnMethod::GpuNnDescent(flow_knn::NnDescentGpuParams::default()),
        ));
    } else {
        eprintln!("GPU methods skipped (no WGPU adapter)");
    }
    out
}

fn synthetic(n: usize, d: usize) -> Vec<f32> {
    let mut rng = SmallRng::seed_from_u64(SEED);
    let mut data = Vec::with_capacity(n * d);
    for i in 0..n {
        let cluster = if i % 2 == 0 { 0.0f32 } else { 5.0 };
        for j in 0..d {
            data.push(cluster + rng.random::<f32>() + (j as f32) * 0.01);
        }
    }
    data
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = xs.len() / 2;
    if xs.len() % 2 == 0 {
        (xs[mid - 1] + xs[mid]) / 2.0
    } else {
        xs[mid]
    }
}

fn time_method(data: &[f32], n: usize, d: usize, method: &KnnMethod) -> Duration {
    let t0 = Instant::now();
    let _ = compute_knn(data, n, d, K, method, DistanceMetric::Euclidean)
        .expect("compute_knn");
    t0.elapsed()
}

fn append_record(path: &Path, rec: &PerfRecord) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut f, rec).map_err(std::io::Error::other)?;
    f.write_all(b"\n")?;
    Ok(())
}

fn main() {
    let path = out_path();
    let machine = hostname();
    let captured_at = chrono_date();
    let repeats = if smoke() { 1 } else { REPEATS };

    eprintln!(
        "collect_matrix → {} ({} repeats, smoke={})",
        path.display(),
        repeats,
        smoke()
    );

    for (n, d) in matrix_grid() {
        eprintln!("\n=== n={n} d={d} ===");
        let data = synthetic(n, d);
        for (method_id, method) in methods() {
            if (method_id == "exact" || method_id == "exact_gpu") && n > 100_000 && !smoke() {
                eprintln!("  skip {method_id} (n>{})", 100_000);
                continue;
            }
            let mut samples = Vec::with_capacity(repeats);
            for _ in 0..repeats {
                samples.push(time_method(&data, n, d, &method).as_secs_f64());
            }
            let med = median(samples);
            let thrpt = n as f64 / med;
            let rec = PerfRecord {
                method: method_id.to_string(),
                n,
                d,
                k: K,
                median_secs: med,
                throughput_elem_per_s: thrpt,
                machine: Some(machine.clone()),
                captured_at: Some(captured_at.clone()),
            };
            eprintln!(
                "  {method_id:16} median={med:.3}s  thrpt={thrpt:.0} elem/s"
            );
            if let Err(e) = append_record(&path, &rec) {
                eprintln!("  write error: {e}");
            }
        }
    }
    eprintln!("\nDone. Appended to {}", path.display());
    eprintln!("Recommend check: {:?}", flow_knn::recommend_method(100_000, 15, &Default::default()));
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "local".into())
}

fn chrono_date() -> String {
    // Avoid pulling chrono; use system date when available.
    std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}
