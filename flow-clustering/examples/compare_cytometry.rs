//! Cytometry paper-suite wall-clock harness (Weber FR-FCM-ZZPH / Stassen PARC).
//!
//! ```text
//! export FLOW_CLUSTERING_BENCH_DATA=/path/to/staged/matrices
//! cargo run -p flow-clustering --release --example compare_cytometry --features parc -- \
//!   --out ./clustering_cytometry_bench_out
//! ```
//!
//! Without `FLOW_CLUSTERING_BENCH_DATA`, exits 0 after a skip message (CI-safe).
//! Never commit clinical/project paths or raw matrices.

use flow_clustering::{FlowSom, FlowSomConfig, Parc, ParcConfig, SomConfig};
use ndarray::Array2;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

const MANIFEST: &str = include_str!("../docs/cytometry_bench_manifest.toml");

#[derive(Debug, Clone)]
struct DatasetSpec {
    id: String,
    n: usize,
    d: usize,
    matrix_file: String,
    labels_file: String,
    weber_flowsom_secs: Option<f64>,
    weber_flowsom_f1: Option<f64>,
}

#[derive(Debug)]
struct RunRow {
    id: String,
    n: usize,
    d: usize,
    present: bool,
    parc_secs: Option<f64>,
    parc_evt_per_s: Option<f64>,
    parc_n_clusters: Option<usize>,
    parc_mean_f1: Option<f64>,
    flowsom_secs: Option<f64>,
    flowsom_evt_per_s: Option<f64>,
    flowsom_n_clusters: Option<usize>,
    flowsom_mean_f1: Option<f64>,
    weber_flowsom_secs: Option<f64>,
    weber_flowsom_f1: Option<f64>,
    note: String,
}

fn main() -> ExitCode {
    let data_root = match env::var_os("FLOW_CLUSTERING_BENCH_DATA") {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!(
                "FLOW_CLUSTERING_BENCH_DATA unset; skipping cytometry paper bench (exit 0)."
            );
            eprintln!("See flow-clustering/docs/comparison-with-parc-paper.md");
            return ExitCode::SUCCESS;
        }
    };
    if !data_root.is_dir() {
        eprintln!(
            "FLOW_CLUSTERING_BENCH_DATA is not a directory: {}",
            data_root.display()
        );
        return ExitCode::FAILURE;
    }

    let out_dir = parse_out_dir();
    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("create out dir: {e}");
        return ExitCode::FAILURE;
    }

    let specs = match parse_manifest(MANIFEST) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("manifest parse error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let rayon_threads = env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".into());
    let mut rows = Vec::with_capacity(specs.len());
    for spec in &specs {
        rows.push(run_one(&data_root, spec));
    }

    let json_path = out_dir.join("throughput_rust.json");
    let md_path = out_dir.join("throughput_report.md");
    if let Err(e) = write_json(&json_path, &rows, &rayon_threads) {
        eprintln!("write json: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = write_markdown(&md_path, &rows, &rayon_threads) {
        eprintln!("write markdown: {e}");
        return ExitCode::FAILURE;
    }
    println!("Wrote {} and {}", json_path.display(), md_path.display());
    ExitCode::SUCCESS
}

fn parse_out_dir() -> PathBuf {
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--out" {
            if let Some(p) = args.next() {
                return PathBuf::from(p);
            }
        }
    }
    PathBuf::from("clustering_cytometry_bench_out")
}

fn parse_manifest(raw: &str) -> Result<Vec<DatasetSpec>, String> {
    let mut specs = Vec::new();
    let mut cur: Option<DatasetSpec> = None;
    for line in raw.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[dataset]]" {
            if let Some(s) = cur.take() {
                specs.push(s);
            }
            cur = Some(DatasetSpec {
                id: String::new(),
                n: 0,
                d: 0,
                matrix_file: String::new(),
                labels_file: String::new(),
                weber_flowsom_secs: None,
                weber_flowsom_f1: None,
            });
            continue;
        }
        let Some(spec) = cur.as_mut() else {
            continue;
        };
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim().trim_matches('"');
        match k {
            "id" => spec.id = v.to_string(),
            "n" => {
                spec.n = v
                    .parse()
                    .map_err(|e| format!("n parse: {e}"))?
            }
            "d" => {
                spec.d = v
                    .parse()
                    .map_err(|e| format!("d parse: {e}"))?
            }
            "matrix_file" => spec.matrix_file = v.to_string(),
            "labels_file" => spec.labels_file = v.to_string(),
            "weber_flowsom_secs" => {
                spec.weber_flowsom_secs = Some(
                    v.parse()
                        .map_err(|e| format!("weber_flowsom_secs: {e}"))?,
                )
            }
            "weber_flowsom_f1" => {
                spec.weber_flowsom_f1 = Some(
                    v.parse()
                        .map_err(|e| format!("weber_flowsom_f1: {e}"))?,
                )
            }
            _ => {}
        }
    }
    if let Some(s) = cur {
        specs.push(s);
    }
    if specs.is_empty() {
        return Err("no [[dataset]] entries".into());
    }
    Ok(specs)
}

fn run_one(root: &Path, spec: &DatasetSpec) -> RunRow {
    let matrix_path = root.join(&spec.matrix_file);
    if !matrix_path.is_file() {
        return RunRow {
            id: spec.id.clone(),
            n: spec.n,
            d: spec.d,
            present: false,
            parc_secs: None,
            parc_evt_per_s: None,
            parc_n_clusters: None,
            parc_mean_f1: None,
            flowsom_secs: None,
            flowsom_evt_per_s: None,
            flowsom_n_clusters: None,
            flowsom_mean_f1: None,
            weber_flowsom_secs: spec.weber_flowsom_secs,
            weber_flowsom_f1: spec.weber_flowsom_f1,
            note: format!("missing {}", spec.matrix_file),
        };
    }

    let data = match load_csv_matrix(&matrix_path) {
        Ok(a) => a,
        Err(e) => {
            return RunRow {
                id: spec.id.clone(),
                n: spec.n,
                d: spec.d,
                present: true,
                parc_secs: None,
                parc_evt_per_s: None,
                parc_n_clusters: None,
                parc_mean_f1: None,
                flowsom_secs: None,
                flowsom_evt_per_s: None,
                flowsom_n_clusters: None,
                flowsom_mean_f1: None,
                weber_flowsom_secs: spec.weber_flowsom_secs,
                weber_flowsom_f1: spec.weber_flowsom_f1,
                note: format!("load error: {e}"),
            };
        }
    };

    let mut note = String::new();
    if data.nrows() != spec.n || data.ncols() != spec.d {
        note = format!(
            "shape {}×{} (manifest {}×{})",
            data.nrows(),
            data.ncols(),
            spec.n,
            spec.d
        );
    }

    let labels = load_labels(&root.join(&spec.labels_file)).ok();

    let parc_cfg = ParcConfig::default();
    let t0 = Instant::now();
    let parc_res = Parc::fit(&data, &parc_cfg);
    let parc_secs = t0.elapsed().as_secs_f64();
    let (parc_n_clusters, parc_mean_f1, parc_secs, parc_evt) = match parc_res {
        Ok(r) => {
            let f1 = labels
                .as_ref()
                .map(|y| mean_f1_hungarian(&r.assignments, y));
            let evt = data.nrows() as f64 / parc_secs.max(1e-12);
            (Some(r.n_clusters), f1, Some(parc_secs), Some(evt))
        }
        Err(e) => {
            if !note.is_empty() {
                note.push_str("; ");
            }
            note.push_str(&format!("parc error: {e}"));
            (None, None, None, None)
        }
    };

    let som_cfg = FlowSomConfig {
        som: SomConfig {
            width: 10,
            height: 10,
            n_epochs: 10,
            radius: None,
            seed: Some(42),
        },
        n_metaclusters: 10,
        meta_max_iterations: 100,
        meta_seed: Some(42),
    };
    let t1 = Instant::now();
    let som_res = FlowSom::fit(&data, &som_cfg);
    let flowsom_secs = t1.elapsed().as_secs_f64();
    let (flowsom_n_clusters, flowsom_mean_f1, flowsom_secs, flowsom_evt) = match som_res {
        Ok(r) => {
            let n_c = r
                .event_metaclusters
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len();
            let f1 = labels
                .as_ref()
                .map(|y| mean_f1_hungarian(&r.event_metaclusters, y));
            let evt = data.nrows() as f64 / flowsom_secs.max(1e-12);
            (Some(n_c), f1, Some(flowsom_secs), Some(evt))
        }
        Err(e) => {
            if !note.is_empty() {
                note.push_str("; ");
            }
            note.push_str(&format!("flowsom error: {e}"));
            (None, None, None, None)
        }
    };

    RunRow {
        id: spec.id.clone(),
        n: data.nrows(),
        d: data.ncols(),
        present: true,
        parc_secs,
        parc_evt_per_s: parc_evt,
        parc_n_clusters,
        parc_mean_f1,
        flowsom_secs,
        flowsom_evt_per_s: flowsom_evt,
        flowsom_n_clusters,
        flowsom_mean_f1,
        weber_flowsom_secs: spec.weber_flowsom_secs,
        weber_flowsom_f1: spec.weber_flowsom_f1,
        note,
    }
}

fn load_csv_matrix(path: &Path) -> Result<Array2<f64>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    let mut d = 0usize;
    for (li, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let vals: Result<Vec<f64>, _> = line
            .split(',')
            .map(|t| t.trim().parse::<f64>())
            .collect();
        let vals = vals.map_err(|e| format!("line {}: {e}", li + 1))?;
        if rows.is_empty() {
            d = vals.len();
        } else if vals.len() != d {
            return Err(format!(
                "line {}: expected {d} cols, got {}",
                li + 1,
                vals.len()
            ));
        }
        rows.push(vals);
    }
    if rows.is_empty() {
        return Err("empty matrix".into());
    }
    let n = rows.len();
    let flat: Vec<f64> = rows.into_iter().flatten().collect();
    Array2::from_shape_vec((n, d), flat).map_err(|e| e.to_string())
}

fn load_labels(path: &Path) -> Result<Vec<i64>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.trim()
                .parse::<i64>()
                .map_err(|e| format!("label parse: {e}"))
        })
        .collect()
}

/// Mean F1 after Hungarian matching of predicted clusters to non-negative truth labels.
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
            // Maximize F1 → minimize (1 - F1)
            cost[ti][pi] = 1.0 - f1;
        }
    }

    let assignment = hungarian(&cost);
    let mut sum = 0.0;
    let mut count = 0usize;
    for (ti, &pi) in assignment.iter().enumerate() {
        if pi < pred_ids.len() {
            sum += 1.0 - cost[ti][pi];
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

/// Hungarian (Kuhn–Munkres) for rectangular cost matrices; returns col index per row
/// (or `usize::MAX` if unassigned). Minimizes cost.
fn hungarian(cost: &[Vec<f64>]) -> Vec<usize> {
    let n_rows = cost.len();
    if n_rows == 0 {
        return Vec::new();
    }
    let n_cols = cost[0].len();
    let n = n_rows.max(n_cols);
    let mut a = vec![vec![0.0f64; n + 1]; n + 1];
    for i in 1..=n_rows {
        for j in 1..=n_cols {
            a[i][j] = cost[i - 1][j - 1];
        }
    }
    let mut u = vec![0.0f64; n + 1];
    let mut v = vec![0.0f64; n + 1];
    let mut p = vec![0usize; n + 1];
    let mut way = vec![0usize; n + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0usize;
        let mut minv = vec![f64::INFINITY; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = f64::INFINITY;
            let mut j1 = 0usize;
            for j in 1..=n {
                if used[j] {
                    continue;
                }
                let cur = a[i0][j] - u[i0] - v[j];
                if cur < minv[j] {
                    minv[j] = cur;
                    way[j] = j0;
                }
                if minv[j] < delta {
                    delta = minv[j];
                    j1 = j;
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    let mut result = vec![usize::MAX; n_rows];
    for j in 1..=n_cols {
        if p[j] >= 1 && p[j] <= n_rows {
            result[p[j] - 1] = j - 1;
        }
    }
    result
}

fn write_json(path: &Path, rows: &[RunRow], rayon_threads: &str) -> std::io::Result<()> {
    let mut datasets = Vec::new();
    for r in rows {
        let mut m = HashMap::new();
        m.insert("id".into(), format!("\"{}\"", r.id));
        m.insert("n".into(), r.n.to_string());
        m.insert("d".into(), r.d.to_string());
        m.insert("present".into(), r.present.to_string());
        push_opt_f64(&mut m, "parc_secs", r.parc_secs);
        push_opt_f64(&mut m, "parc_evt_per_s", r.parc_evt_per_s);
        push_opt_usize(&mut m, "parc_n_clusters", r.parc_n_clusters);
        push_opt_f64(&mut m, "parc_mean_f1", r.parc_mean_f1);
        push_opt_f64(&mut m, "flowsom_secs", r.flowsom_secs);
        push_opt_f64(&mut m, "flowsom_evt_per_s", r.flowsom_evt_per_s);
        push_opt_usize(&mut m, "flowsom_n_clusters", r.flowsom_n_clusters);
        push_opt_f64(&mut m, "flowsom_mean_f1", r.flowsom_mean_f1);
        push_opt_f64(&mut m, "weber_flowsom_secs", r.weber_flowsom_secs);
        push_opt_f64(&mut m, "weber_flowsom_f1", r.weber_flowsom_f1);
        m.insert("note".into(), format!("\"{}\"", escape_json(&r.note)));
        datasets.push(m);
    }
    let mut out = String::from("{\n");
    out.push_str(&format!(
        "  \"rayon_num_threads\": \"{}\",\n",
        escape_json(rayon_threads)
    ));
    out.push_str("  \"parc_config\": \"ParcConfig::default()\",\n");
    out.push_str(
        "  \"flowsom_config\": \"10x10 SOM, 10 epochs, 10 metaclusters, seed 42\",\n",
    );
    out.push_str("  \"datasets\": [\n");
    for (i, m) in datasets.iter().enumerate() {
        out.push_str("    {\n");
        let keys = [
            "id",
            "n",
            "d",
            "present",
            "parc_secs",
            "parc_evt_per_s",
            "parc_n_clusters",
            "parc_mean_f1",
            "flowsom_secs",
            "flowsom_evt_per_s",
            "flowsom_n_clusters",
            "flowsom_mean_f1",
            "weber_flowsom_secs",
            "weber_flowsom_f1",
            "note",
        ];
        for (ki, k) in keys.iter().enumerate() {
            let v = m.get(*k).map(|s| s.as_str()).unwrap_or("null");
            let comma = if ki + 1 < keys.len() { "," } else { "" };
            out.push_str(&format!("      \"{k}\": {v}{comma}\n"));
        }
        if i + 1 < datasets.len() {
            out.push_str("    },\n");
        } else {
            out.push_str("    }\n");
        }
    }
    out.push_str("  ]\n}\n");
    fs::write(path, out)
}

fn push_opt_f64(m: &mut HashMap<String, String>, k: &str, v: Option<f64>) {
    m.insert(
        k.into(),
        v.map(|x| format!("{x:.6}"))
            .unwrap_or_else(|| "null".into()),
    );
}

fn push_opt_usize(m: &mut HashMap<String, String>, k: &str, v: Option<usize>) {
    m.insert(
        k.into(),
        v.map(|x| x.to_string()).unwrap_or_else(|| "null".into()),
    );
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_markdown(path: &Path, rows: &[RunRow], rayon_threads: &str) -> std::io::Result<()> {
    let mut md = String::new();
    md.push_str("# Cytometry clustering throughput (Rust)\n\n");
    md.push_str(&format!("`RAYON_NUM_THREADS={rayon_threads}`\n\n"));
    md.push_str(
        "PARC: `ParcConfig::default()`. FlowSOM: 10×10 SOM, 10 epochs, 10 metaclusters.\n\n",
    );
    md.push_str(
        "Weber FlowSOM seconds are from Weber & Robinson 2016 Table 3 (different stack/hardware).\n\n",
    );
    md.push_str(
        "| Dataset | n | d | PARC s | PARC evt/s | FlowSOM s | FlowSOM evt/s | Weber FlowSOM s | PARC F1 | FlowSOM F1 | Weber F1 | Note |\n",
    );
    md.push_str(
        "|---------|---|---|--------|------------|-----------|---------------|-----------------|---------|------------|----------|------|\n",
    );
    for r in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.id,
            r.n,
            r.d,
            fmt_opt_f64(r.parc_secs),
            fmt_opt_f64(r.parc_evt_per_s),
            fmt_opt_f64(r.flowsom_secs),
            fmt_opt_f64(r.flowsom_evt_per_s),
            fmt_opt_f64(r.weber_flowsom_secs),
            fmt_opt_f64(r.parc_mean_f1),
            fmt_opt_f64(r.flowsom_mean_f1),
            fmt_opt_f64(r.weber_flowsom_f1),
            if r.note.is_empty() { "—" } else { &r.note },
        ));
    }
    md.push_str("\nSee `docs/comparison-with-parc-paper.md`.\n");
    fs::write(path, md)
}

fn fmt_opt_f64(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3}"))
        .unwrap_or_else(|| "—".into())
}
