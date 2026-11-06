//! Same-host Rust vs Python (ShobiStassen/PARC) wall-clock comparison.
//!
//! Times clustering only (matrix already in memory / load outside Python timer).
//! Interleaves Rust and Python per case to limit session drift.
//!
//! ```text
//! # venv with: pip install parc numpy hnswlib leidenalg python-igraph
//! export PARC_PYTHON=/path/to/venv/bin/python
//! cargo run -p flow-clustering --release --example compare_with_python --features parc -- \
//!   --out ./clustering_parc_py_compare --warmup 1 --reps 3
//! ```
//!
//! Without `PARC_PYTHON` (or `--python`), exits 0 after a skip message (CI-safe).

use flow_clustering::{Parc, ParcConfig};
use ndarray::Array2;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

#[derive(Clone, Copy)]
struct Case {
    n: usize,
    d: usize,
    knn: usize,
}

#[derive(Debug)]
struct SideTiming {
    secs_median: f64,
    #[allow(dead_code)]
    secs_mean: f64,
    evt_per_s: f64,
    n_clusters: usize,
    #[allow(dead_code)]
    secs_all: Vec<f64>,
}

fn main() -> ExitCode {
    let python = resolve_python();
    let Some(python) = python else {
        eprintln!(
            "PARC_PYTHON unset and no --python; skipping Rust↔Python PARC compare (exit 0)."
        );
        eprintln!("See flow-clustering/docs/comparison-with-python.md");
        return ExitCode::SUCCESS;
    };

    let out_dir = arg_value("--out").unwrap_or_else(|| PathBuf::from("clustering_parc_py_compare"));
    let warmup: usize = arg_value("--warmup")
        .and_then(|s| s.to_str()?.parse().ok())
        .unwrap_or(1);
    let reps: usize = arg_value("--reps")
        .and_then(|s| s.to_str()?.parse().ok())
        .unwrap_or(3);
    let smoke = env::args().any(|a| a == "--smoke");

    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("create out dir: {e}");
        return ExitCode::FAILURE;
    }

    let py_script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/compare_with_python.py");
    if !py_script.is_file() {
        eprintln!("missing {}", py_script.display());
        return ExitCode::FAILURE;
    }

    let cases: Vec<Case> = if smoke {
        vec![Case {
            n: 2_000,
            d: 10,
            knn: 15,
        }]
    } else {
        vec![
            Case {
                n: 5_000,
                d: 20,
                knn: 20,
            },
            Case {
                n: 20_000,
                d: 20,
                knn: 30,
            },
            Case {
                n: 50_000,
                d: 20,
                knn: 30,
            },
            Case {
                n: 100_000,
                d: 20,
                knn: 30,
            },
        ]
    };

    let rustc = rustc_version();
    let mut report_rows = Vec::new();

    for (ci, case) in cases.iter().enumerate() {
        let case_dir = out_dir.join(format!("n{}_d{}_k{}", case.n, case.d, case.knn));
        if let Err(e) = fs::create_dir_all(&case_dir) {
            eprintln!("mkdir: {e}");
            return ExitCode::FAILURE;
        }
        let data = synth_clouds(case.n, case.d);
        let matrix_path = case_dir.join("matrix.csv");
        if let Err(e) = write_csv(&matrix_path, &data) {
            eprintln!("write matrix: {e}");
            return ExitCode::FAILURE;
        }

        let config = ParcConfig {
            knn: case.knn,
            ..ParcConfig::default()
        };

        // Warmup (not interleaved into medians).
        for _ in 0..warmup {
            let _ = Parc::fit(&data, &config);
            let warm_json = case_dir.join("py_warmup.json");
            if let Err(e) = run_python(
                &python,
                &py_script,
                &matrix_path,
                &warm_json,
                case.knn,
                0,
                1,
            ) {
                eprintln!("python warmup failed: {e}");
                return ExitCode::FAILURE;
            }
        }

        let mut rust_secs = Vec::with_capacity(reps);
        let mut py_secs = Vec::with_capacity(reps);
        let mut rust_clusters = 0usize;
        let mut py_clusters = 0usize;
        let mut parc_version = String::new();

        for rep in 0..reps {
            let rust_first = (ci + rep) % 2 == 0;
            if rust_first {
                let (s, c) = time_rust(&data, &config);
                rust_secs.push(s);
                rust_clusters = c;
                match time_python(&python, &py_script, &matrix_path, &case_dir, case.knn, rep) {
                    Ok((s, c, ver)) => {
                        py_secs.push(s);
                        py_clusters = c;
                        if parc_version.is_empty() {
                            parc_version = ver;
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                match time_python(&python, &py_script, &matrix_path, &case_dir, case.knn, rep) {
                    Ok((s, c, ver)) => {
                        py_secs.push(s);
                        py_clusters = c;
                        if parc_version.is_empty() {
                            parc_version = ver;
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        return ExitCode::FAILURE;
                    }
                }
                let (s, c) = time_rust(&data, &config);
                rust_secs.push(s);
                rust_clusters = c;
            }
        }

        let rust = summarize(case.n, rust_secs, rust_clusters);
        let py = summarize(case.n, py_secs, py_clusters);
        let speedup = py.secs_median / rust.secs_median.max(1e-12);

        println!(
            "n={} d={} k={}: rust_med={:.3}s ({:.0} evt/s)  py_med={:.3}s ({:.0} evt/s)  speedup={:.2}×",
            case.n,
            case.d,
            case.knn,
            rust.secs_median,
            rust.evt_per_s,
            py.secs_median,
            py.evt_per_s,
            speedup
        );

        report_rows.push(ReportRow {
            n: case.n,
            d: case.d,
            knn: case.knn,
            rust,
            py,
            speedup,
            parc_version,
        });
    }

    let json_path = out_dir.join("throughput_merged.json");
    let md_path = out_dir.join("throughput_report.md");
    if let Err(e) = write_merged_json(&json_path, &report_rows, &rustc, warmup, reps) {
        eprintln!("json: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(e) = write_markdown(&md_path, &report_rows, &rustc, warmup, reps, &python) {
        eprintln!("md: {e}");
        return ExitCode::FAILURE;
    }
    println!("Wrote {} and {}", json_path.display(), md_path.display());
    ExitCode::SUCCESS
}

struct ReportRow {
    n: usize,
    d: usize,
    knn: usize,
    rust: SideTiming,
    py: SideTiming,
    speedup: f64,
    parc_version: String,
}

fn resolve_python() -> Option<PathBuf> {
    if let Some(p) = arg_value("--python") {
        return Some(p);
    }
    env::var_os("PARC_PYTHON").map(PathBuf::from)
}

fn arg_value(flag: &str) -> Option<PathBuf> {
    let mut args = env::args();
    while let Some(a) = args.next() {
        if a == flag {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

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

fn write_csv(path: &Path, data: &Array2<f64>) -> std::io::Result<()> {
    let mut buf = String::with_capacity(data.len() * 12);
    for row in data.rows() {
        for (j, v) in row.iter().enumerate() {
            if j > 0 {
                buf.push(',');
            }
            buf.push_str(&format!("{v}"));
        }
        buf.push('\n');
    }
    fs::write(path, buf)
}

fn time_rust(data: &Array2<f64>, config: &ParcConfig) -> (f64, usize) {
    let t0 = Instant::now();
    let result = Parc::fit(data, config).expect("parc rust");
    (t0.elapsed().as_secs_f64(), result.n_clusters)
}

fn run_python(
    python: &Path,
    script: &Path,
    matrix: &Path,
    out_json: &Path,
    knn: usize,
    warmup: usize,
    reps: usize,
) -> Result<(), String> {
    let status = Command::new(python)
        .arg(script)
        .arg("--matrix")
        .arg(matrix)
        .arg("--out-json")
        .arg(out_json)
        .arg("--knn")
        .arg(knn.to_string())
        .arg("--warmup")
        .arg(warmup.to_string())
        .arg("--reps")
        .arg(reps.to_string())
        .status()
        .map_err(|e| format!("spawn python: {e}"))?;
    if !status.success() {
        return Err(format!("python exited {status}"));
    }
    Ok(())
}

fn time_python(
    python: &Path,
    script: &Path,
    matrix: &Path,
    case_dir: &Path,
    knn: usize,
    rep: usize,
) -> Result<(f64, usize, String), String> {
    let out_json = case_dir.join(format!("py_rep{rep}.json"));
    run_python(python, script, matrix, &out_json, knn, 0, 1)?;
    let text = fs::read_to_string(&out_json).map_err(|e| e.to_string())?;
    let secs = extract_f64(&text, "secs_median").ok_or("secs_median missing")?;
    let n_clusters = extract_usize(&text, "n_clusters").unwrap_or(0);
    let ver = extract_string(&text, "parc_version").unwrap_or_else(|| "?".into());
    Ok((secs, n_clusters, ver))
}

fn extract_f64(json: &str, key: &str) -> Option<f64> {
    let pat = format!("\"{key}\"");
    let i = json.find(&pat)?;
    let after = &json[i + pat.len()..];
    let after = after.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    let end = after
        .find(|c: char| c == ',' || c == '\n' || c == '}')
        .unwrap_or(after.len());
    after[..end].trim().parse().ok()
}

fn extract_usize(json: &str, key: &str) -> Option<usize> {
    extract_f64(json, key).map(|x| x as usize)
}

fn extract_string(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = json.find(&pat)?;
    let after = &json[i + pat.len()..];
    let after = after.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    if !after.starts_with('"') {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn summarize(n: usize, mut secs: Vec<f64>, n_clusters: usize) -> SideTiming {
    secs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let secs_median = secs[secs.len() / 2];
    let secs_mean = secs.iter().sum::<f64>() / secs.len() as f64;
    SideTiming {
        secs_median,
        secs_mean,
        evt_per_s: n as f64 / secs_median.max(1e-12),
        n_clusters,
        secs_all: secs,
    }
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("-V")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn write_merged_json(
    path: &Path,
    rows: &[ReportRow],
    rustc: &str,
    warmup: usize,
    reps: usize,
) -> std::io::Result<()> {
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"rustc\": \"{}\",\n", escape(rustc)));
    s.push_str(&format!("  \"warmup\": {warmup},\n"));
    s.push_str(&format!("  \"reps\": {reps},\n"));
    s.push_str("  \"cases\": [\n");
    for (i, r) in rows.iter().enumerate() {
        s.push_str("    {\n");
        s.push_str(&format!("      \"n\": {}, \"d\": {}, \"knn\": {},\n", r.n, r.d, r.knn));
        s.push_str(&format!(
            "      \"rust_secs_median\": {:.6}, \"rust_evt_per_s\": {:.1}, \"rust_n_clusters\": {},\n",
            r.rust.secs_median, r.rust.evt_per_s, r.rust.n_clusters
        ));
        s.push_str(&format!(
            "      \"python_secs_median\": {:.6}, \"python_evt_per_s\": {:.1}, \"python_n_clusters\": {},\n",
            r.py.secs_median, r.py.evt_per_s, r.py.n_clusters
        ));
        s.push_str(&format!(
            "      \"rust_vs_python_speedup\": {:.3}, \"parc_version\": \"{}\"\n",
            r.speedup,
            escape(&r.parc_version)
        ));
        if i + 1 < rows.len() {
            s.push_str("    },\n");
        } else {
            s.push_str("    }\n");
        }
    }
    s.push_str("  ]\n}\n");
    fs::write(path, s)
}

fn write_markdown(
    path: &Path,
    rows: &[ReportRow],
    rustc: &str,
    warmup: usize,
    reps: usize,
    python: &Path,
) -> std::io::Result<()> {
    let mut md = String::new();
    md.push_str("# Rust vs Python PARC (same host)\n\n");
    md.push_str("Clustering-only wall times on identical synthetic two-cloud matrices. ");
    md.push_str("Python load is outside the timer; Rust uses in-memory `Array2`. ");
    md.push_str("Sides interleaved per case/rep.\n\n");
    md.push_str(&format!("- `{rustc}`\n"));
    md.push_str(&format!("- Python: `{}`\n", python.display()));
    md.push_str(&format!("- warmup={warmup}, reps={reps}\n"));
    md.push_str("- Rust: `ParcConfig::default()` (HNSW)\n");
    md.push_str("- Python: `parc.PARC` defaults aligned with Rust (`knn`, `dist_std_local=3`, …)\n\n");
    md.push_str(
        "| n | d | k | Rust median | Rust evt/s | Python median | Python evt/s | Speedup (Py/Rust) | Rust k | Py k |\n",
    );
    md.push_str("|---|---|---|-------------|------------|---------------|--------------|-------------------|--------|------|\n");
    for r in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {:.3}s | {:.0} | {:.3}s | {:.0} | **{:.2}×** | {} | {} |\n",
            r.n,
            r.d,
            r.knn,
            r.rust.secs_median,
            r.rust.evt_per_s,
            r.py.secs_median,
            r.py.evt_per_s,
            r.speedup,
            r.rust.n_clusters,
            r.py.n_clusters
        ));
    }
    md.push_str("\nSpeedup &gt; 1 means Rust is faster. Cluster counts need not match ");
    md.push_str("(different HNSW libraries); this harness is for throughput.\n");
    fs::write(path, md)
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
