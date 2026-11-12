//! Compare e-means against same-kernel Lloyd on synthetic Gaussian populations.
//!
//! ```text
//! cargo run -p flow-clustering --example compare_emeans_lloyd --features emeans --release
//! FLOW_CLUSTERING_BENCH_MATRIX=1 cargo run -p flow-clustering --example compare_emeans_lloyd --features emeans --release
//! ```
//!
//! Default grid: 100k and 1M events × 30/60 parameters × 32/256/2048 clusters.
//! `FLOW_CLUSTERING_BENCH_LOWER_BOUND=1`: 1k–50k × 10/30/60 × 8/32/256 (crossover hunt).
//! Writes JSON + markdown under `target/emeans_compare/` (or `--out DIR`).

use flow_clustering::{EMeans, EMeansConfig, Finalization, InitMethod, Lloyd, LloydConfig};
use flow_fcs::synthetic::{GaussianComponent, GaussianPopulationsSpec, gaussian_population_columns};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn multi_pop_spec(n_events: usize, d: usize, n_pops: usize, seed: u64) -> GaussianPopulationsSpec {
    let channel_names: Vec<String> = (0..d).map(|i| format!("F{i}")).collect();
    let mut components = Vec::with_capacity(n_pops);
    for p in 0..n_pops {
        let mut mean = vec![0.0f32; d];
        let mut std_dev = vec![1.0f32; d];
        for j in 0..d {
            // Spread population centres across a [0, 40] box with mild jitter.
            let base = ((p * 17 + j * 3) % 41) as f32;
            mean[j] = base;
            std_dev[j] = 1.5 + ((p + j) % 5) as f32 * 0.3;
        }
        // Unequal weights: geometric-ish.
        let weight = 1.0 / (1.0 + (p as f64) * 0.15);
        components.push(GaussianComponent {
            weight,
            mean,
            std_dev,
        });
    }
    GaussianPopulationsSpec {
        n_events,
        channel_names,
        components,
        seed,
    }
}

/// Flatten named column vectors into row-major `n × d`.
fn columns_to_row_major(cols: &[(String, Vec<f32>)], n: usize, d: usize) -> Vec<f32> {
    assert_eq!(cols.len(), d);
    let mut out = vec![0.0f32; n * d];
    for (j, (_name, col)) in cols.iter().enumerate() {
        for i in 0..n {
            out[i * d + j] = col[i];
        }
    }
    out
}

/// Soft labels: nearest component mean in Euclidean distance (for F1).
fn soft_truth_labels(data: &[f32], n: usize, d: usize, means: &[Vec<f32>]) -> Vec<i64> {
    let mut labels = vec![0i64; n];
    for i in 0..n {
        let row = &data[i * d..(i + 1) * d];
        let mut best = 0usize;
        let mut best_d = f64::INFINITY;
        for (p, m) in means.iter().enumerate() {
            let mut dist = 0.0f64;
            for t in 0..d {
                let diff = row[t] as f64 - m[t] as f64;
                dist += diff * diff;
            }
            if dist < best_d {
                best_d = dist;
                best = p;
            }
        }
        labels[i] = best as i64;
    }
    labels
}

fn mean_f1_hungarian(pred: &[usize], truth: &[i64]) -> f64 {
    let n = pred.len().min(truth.len());
    if n == 0 {
        return 0.0;
    }
    let mut truth_ids: Vec<i64> = truth[..n]
        .iter()
        .copied()
        .filter(|&t| t >= 0)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    truth_ids.sort_unstable();
    let mut pred_ids: Vec<usize> = pred[..n]
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    pred_ids.sort_unstable();
    if truth_ids.is_empty() || pred_ids.is_empty() {
        return 0.0;
    }
    let mut cost = vec![vec![0.0f64; pred_ids.len()]; truth_ids.len()];
    for (ti, &tlab) in truth_ids.iter().enumerate() {
        for (pi, &plab) in pred_ids.iter().enumerate() {
            let mut tp = 0usize;
            let mut fp = 0usize;
            let mut fn_ = 0usize;
            for i in 0..n {
                let p = pred[i] == plab;
                let t = truth[i] == tlab;
                match (p, t) {
                    (true, true) => tp += 1,
                    (true, false) => fp += 1,
                    (false, true) => fn_ += 1,
                    (false, false) => {}
                }
            }
            let prec = if tp + fp == 0 {
                0.0
            } else {
                tp as f64 / (tp + fp) as f64
            };
            let rec = if tp + fn_ == 0 {
                0.0
            } else {
                tp as f64 / (tp + fn_) as f64
            };
            let f1 = if prec + rec == 0.0 {
                0.0
            } else {
                2.0 * prec * rec / (prec + rec)
            };
            cost[ti][pi] = 1.0 - f1;
        }
    }
    // Greedy matching (adequate for report; full Hungarian lives in compare_cytometry).
    let mut used = vec![false; pred_ids.len()];
    let mut sum = 0.0;
    let mut count = 0usize;
    for row in &cost {
        let mut best_j = None;
        let mut best_c = f64::INFINITY;
        for (j, &c) in row.iter().enumerate() {
            if !used[j] && c < best_c {
                best_c = c;
                best_j = Some(j);
            }
        }
        if let Some(j) = best_j {
            used[j] = true;
            sum += 1.0 - best_c;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

fn held_out_mse(data: &[f32], n: usize, d: usize, centroids: &[f32], k: usize) -> f64 {
    let mut sum = 0.0f64;
    for i in 0..n {
        let row = &data[i * d..(i + 1) * d];
        let mut best = f64::INFINITY;
        for j in 0..k {
            let c = &centroids[j * d..(j + 1) * d];
            let mut dist = 0.0f64;
            for t in 0..d {
                let diff = row[t] as f64 - c[t] as f64;
                dist += diff * diff;
            }
            best = best.min(dist);
        }
        sum += best;
    }
    sum / n as f64
}

#[derive(Debug)]
struct Row {
    method: String,
    n: usize,
    d: usize,
    k: usize,
    seed: u64,
    train_mse: f64,
    heldout_mse: f64,
    mean_f1: f64,
    empty_clusters: usize,
    wall_ms: f64,
    epochs_or_iters: usize,
    batch: usize,
    alpha0: f32,
    n_revivals: usize,
}

fn main() {
    let mut out_dir = PathBuf::from("target/emeans_compare");
    let args: Vec<String> = env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--out")
        && let Some(p) = args.get(i + 1)
    {
        out_dir = PathBuf::from(p);
    }
    fs::create_dir_all(&out_dir).expect("mkdir");

    let matrix = env_flag("FLOW_CLUSTERING_BENCH_MATRIX");
    let large = env_flag("FLOW_CLUSTERING_BENCH_MATRIX_LARGE");
    let smoke = env_flag("FLOW_CLUSTERING_BENCH_SMOKE");
    let lower_bound = env_flag("FLOW_CLUSTERING_BENCH_LOWER_BOUND");

    let mut ns = vec![100_000usize, 1_000_000];
    let mut ds = vec![30usize, 60];
    let mut ks = vec![32usize, 256, 2_048];
    if matrix {
        ns.push(5_000_000);
        ds.extend([10, 40]);
        ks.extend([8, 128, 1_024, 8_192]);
    }
    if large {
        ns.push(10_000_000);
    }
    if lower_bound {
        // Hunt the practical crossover vs same-kernel Lloyd@25.
        ns = vec![1_000, 5_000, 10_000, 25_000, 50_000];
        ds = vec![10, 30, 60];
        ks = vec![8, 32, 256];
    }
    if smoke {
        ns = vec![5_000];
        ds = vec![10];
        ks = vec![8, 32];
    }

    let seeds: &[u64] = if smoke {
        &[1]
    } else if matrix {
        &[1, 2, 3, 4, 5]
    } else {
        &[1, 2, 3]
    };
    let emeans_epochs: &[usize] = if smoke {
        &[1, 3]
    } else if lower_bound {
        // Default recipe (3) plus neighbors for early-match checks.
        &[1, 2, 3]
    } else {
        &[1, 2, 3, 5, 10]
    };
    let heldout_n = if smoke {
        1_000usize
    } else if lower_bound {
        5_000usize
    } else {
        20_000usize
    };

    let mut rows: Vec<Row> = Vec::new();

    for &n in &ns {
        for &d in &ds {
            for &k in &ks {
                if k > n {
                    continue;
                }
                let n_pops = (20 + (k / 16)).min(200).min(n / 50).max(2);
                for &seed in seeds {
                    let spec = multi_pop_spec(n + heldout_n, d, n_pops, seed);
                    let cols = match gaussian_population_columns(&spec) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("synth failed n={n} d={d}: {e}");
                            continue;
                        }
                    };
                    let all = columns_to_row_major(&cols, n + heldout_n, d);
                    let train = &all[..n * d];
                    let held = &all[n * d..];
                    let means: Vec<Vec<f32>> =
                        spec.components.iter().map(|c| c.mean.clone()).collect();
                    let truth = soft_truth_labels(train, n, d, &means);

                    // Lloyd @ 25
                    {
                        let cfg = LloydConfig {
                            n_clusters: k,
                            max_iterations: 25,
                            until_convergence: false,
                            init: InitMethod::Random,
                            seed: Some(seed),
                            ..Default::default()
                        };
                        let t0 = Instant::now();
                        match Lloyd::fit(train, n, d, &cfg) {
                            Ok(r) => {
                                let wall_ms = t0.elapsed().as_secs_f64() * 1e3;
                                let pred: Vec<usize> =
                                    r.assignments.iter().map(|&a| a as usize).collect();
                                rows.push(Row {
                                    method: "lloyd25".into(),
                                    n,
                                    d,
                                    k,
                                    seed,
                                    train_mse: r.inertia,
                                    heldout_mse: held_out_mse(held, heldout_n, d, &r.centroids, k),
                                    mean_f1: mean_f1_hungarian(&pred, &truth),
                                    empty_clusters: r.counts.iter().filter(|&&c| c == 0).count(),
                                    wall_ms,
                                    epochs_or_iters: r.iterations,
                                    batch: n,
                                    alpha0: 0.0,
                                    n_revivals: 0,
                                });
                            }
                            Err(e) => eprintln!("lloyd failed: {e}"),
                        }
                    }

                    for &epochs in emeans_epochs {
                        let cfg = EMeansConfig {
                            n_clusters: k,
                            epochs,
                            seed: Some(seed),
                            finalization: Finalization::LastEpochMean,
                            ..Default::default()
                        };
                        let t0 = Instant::now();
                        match EMeans::fit(train, n, d, &cfg) {
                            Ok(r) => {
                                let wall_ms = t0.elapsed().as_secs_f64() * 1e3;
                                let pred: Vec<usize> =
                                    r.assignments.iter().map(|&a| a as usize).collect();
                                rows.push(Row {
                                    method: format!("emeans_e{epochs}"),
                                    n,
                                    d,
                                    k,
                                    seed,
                                    train_mse: r.inertia,
                                    heldout_mse: held_out_mse(held, heldout_n, d, &r.centroids, k),
                                    mean_f1: mean_f1_hungarian(&pred, &truth),
                                    empty_clusters: r.counts.iter().filter(|&&c| c == 0).count(),
                                    wall_ms,
                                    epochs_or_iters: r.epochs_run,
                                    batch: r.batch_size,
                                    alpha0: r.alpha0,
                                    n_revivals: r.n_revivals,
                                });
                            }
                            Err(e) => eprintln!("emeans failed: {e}"),
                        }
                    }

                    eprintln!(
                        "done n={n} d={d} k={k} seed={seed} ({} rows so far)",
                        rows.len()
                    );
                }
            }
        }
    }

    let json_path = out_dir.join("results.json");
    let md_path = out_dir.join("results.md");
    let json: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "method": r.method,
                "n": r.n,
                "d": r.d,
                "k": r.k,
                "seed": r.seed,
                "train_mse": r.train_mse,
                "heldout_mse": r.heldout_mse,
                "mean_f1": r.mean_f1,
                "empty_clusters": r.empty_clusters,
                "wall_ms": r.wall_ms,
                "epochs_or_iters": r.epochs_or_iters,
                "batch": r.batch,
                "alpha0": r.alpha0,
                "n_revivals": r.n_revivals,
            })
        })
        .collect();
    fs::write(&json_path, serde_json::to_string_pretty(&json).unwrap()).expect("write json");

    let mut md = String::from("# e-means vs Lloyd (synthetic)\n\n");
    md.push_str("| method | n | d | k | seed | train_mse | heldout_mse | mean_f1 | empty | wall_ms | epochs/iters |\n");
    md.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for r in &rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.6} | {:.6} | {:.4} | {} | {:.1} | {} |\n",
            r.method,
            r.n,
            r.d,
            r.k,
            r.seed,
            r.train_mse,
            r.heldout_mse,
            r.mean_f1,
            r.empty_clusters,
            r.wall_ms,
            r.epochs_or_iters
        ));
    }
    fs::write(&md_path, md).expect("write md");
    eprintln!("wrote {} and {}", json_path.display(), md_path.display());
}
