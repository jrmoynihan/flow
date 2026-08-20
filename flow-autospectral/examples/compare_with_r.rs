//! Rust vs AutoSpectralRcpp joint-unmix harness (QC-core headline, e2e secondary).
//!
//! Synthetic overlapping unit-peak spectra (optional tandem-like collinear pair).
//! Never commit real FCS paths; pass `--fcs` only on the CLI.

#![allow(clippy::needless_range_loop)]

use anyhow::{Context, Result, bail};
use faer::Mat;
use flow_autospectral::{
    AfLibrary, JointUnmixConfig, JointUnmixResult, SpectralVariants, force_sequential,
    normalize_unit_peak, swap_af_column, unmix_autospectral_joint, unmix_event_ols,
};
use flow_fcs::file::Fcs;
use flow_fcs::synthetic::{
    default_cytometry_mixture, gaussian_population_columns, gaussian_populations_fcs,
};
use flow_fcs::write::write_fcs_file;
use polars::prelude::{Column, DataFrame};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

struct Cli {
    smoke: bool,
    rust_only: bool,
    e2e: bool,
    large: bool,
    out: PathBuf,
    warmup: usize,
    reps: usize,
    events: Vec<usize>,
    detectors: Vec<usize>,
    fluors: Vec<usize>,
    fcs_paths: Vec<PathBuf>,
    /// Multi-thread count for Rayon and AutoSpectralRcpp OpenMP (1-thread always also run).
    threads: usize,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            smoke: false,
            rust_only: false,
            e2e: false,
            large: false,
            out: PathBuf::from("target/autospectral-r-compare"),
            warmup: 1,
            reps: 3,
            events: vec![10_000],
            detectors: vec![20],
            fluors: vec![8],
            fcs_paths: Vec::new(),
            threads: hardware_threads(),
        }
    }
}

fn hardware_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

fn parse_csv_usize(s: &str) -> Result<Vec<usize>> {
    s.split(',')
        .map(|p| {
            p.trim()
                .parse::<usize>()
                .with_context(|| format!("parse usize from {p:?}"))
        })
        .collect()
}

fn parse_cli(args: &[String]) -> Result<Cli> {
    let mut cli = Cli::default();
    let mut events_set = false;
    let mut detectors_set = false;
    let mut fluors_set = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--smoke" => cli.smoke = true,
            "--rust-only" => cli.rust_only = true,
            "--e2e" => cli.e2e = true,
            "--large" => cli.large = true,
            "--out" => {
                i += 1;
                cli.out = PathBuf::from(args.get(i).context("--out needs a path")?);
            }
            "--warmup" => {
                i += 1;
                cli.warmup = args.get(i).context("--warmup")?.parse()?;
            }
            "--reps" => {
                i += 1;
                cli.reps = args.get(i).context("--reps")?.parse()?;
            }
            "--events" => {
                i += 1;
                cli.events = parse_csv_usize(args.get(i).context("--events")?)?;
                events_set = true;
            }
            "--detectors" => {
                i += 1;
                cli.detectors = parse_csv_usize(args.get(i).context("--detectors")?)?;
                detectors_set = true;
            }
            "--fluors" => {
                i += 1;
                cli.fluors = parse_csv_usize(args.get(i).context("--fluors")?)?;
                fluors_set = true;
            }
            "--fcs" => {
                i += 1;
                cli.fcs_paths
                    .push(PathBuf::from(args.get(i).context("--fcs")?));
            }
            "--threads" => {
                i += 1;
                cli.threads = args
                    .get(i)
                    .context("--threads")?
                    .parse()
                    .context("--threads")?;
                if cli.threads < 1 {
                    bail!("--threads must be >= 1");
                }
            }
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }
    if cli.smoke {
        // Pin the smoke panel (d, F, min warmup/reps). Do not clobber `--events`.
        if !events_set {
            cli.events = vec![10_000];
        } else {
            eprintln!(
                "note: --smoke keeping --events {:?}; pinning d=20 F=8 unless those flags were also set",
                cli.events
            );
        }
        if !detectors_set {
            cli.detectors = vec![20];
        }
        if !fluors_set {
            cli.fluors = vec![8];
        }
        cli.warmup = cli.warmup.max(1);
        cli.reps = cli.reps.max(2);
    } else if env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX") {
        cli.events = vec![50_000, 200_000];
        cli.detectors = vec![40, 64];
        cli.fluors = vec![42];
    }
    if (cli.large || env_flag("FLOW_AUTOSPECTRAL_BENCH_MATRIX_LARGE"))
        && !cli.events.contains(&1_000_000)
    {
        cli.events.push(1_000_000);
    }
    Ok(cli)
}

#[derive(Serialize)]
struct TimingRow {
    config: String,
    case_id: String,
    phase: String,
    mean_s: f64,
    std_s: f64,
    events: usize,
    detectors: usize,
    fluors: usize,
    k_af: usize,
    n_variants_mean: f64,
    events_per_s: f64,
    threads: usize,
    reps: usize,
    skipped: bool,
    reason: Option<String>,
}

struct AgreementRow {
    case_id: String,
    mean_cosine: f64,
    min_cosine: f64,
    af_index_match: f64,
}

struct RMeta {
    r_version: String,
    autospectralrcpp_version: String,
    autospectral_version: Option<String>,
}

enum Sidecar {
    Timed {
        row: TimingRow,
        meta: RMeta,
    },
    Skipped(String),
    Failed(String),
}

#[derive(Clone)]
struct Panel {
    n: usize,
    d: usize,
    n_fluor: usize,
    k_af: usize,
    fluor: Mat<f64>,
    names: Vec<String>,
    library: AfLibrary,
    variants: SpectralVariants,
    events: Vec<f64>,
    true_fluor: Vec<Option<usize>>,
    true_af: Vec<usize>,
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

fn unit_peak_library(k: usize, d: usize) -> AfLibrary {
    let mut signatures = Mat::<f64>::zeros(d, k);
    for j in 0..k {
        let peak = j % d;
        let mut col = vec![0.03; d];
        col[peak] = 1.0;
        normalize_unit_peak(&mut col);
        for i in 0..d {
            signatures[(i, j)] = col[i];
        }
    }
    AfLibrary {
        signatures,
        names: (0..k).map(|j| format!("AF_{j}")).collect(),
        detector_names: (0..d).map(|i| format!("FL{}-A", i + 1)).collect(),
        provenance: "harness".into(),
    }
}

fn collinear_variants(fluor: &Mat<f64>, names: &[String]) -> SpectralVariants {
    let d = fluor.nrows();
    let n_fluor = fluor.ncols();
    let mut variants = HashMap::new();
    let mut deltas = HashMap::new();
    if n_fluor >= 2 {
        let a = n_fluor - 2;
        let b = n_fluor - 1;
        let b_peak = (0..d)
            .max_by(|i, j| fluor[(*i, b)].total_cmp(&fluor[(*j, b)]))
            .unwrap_or(d / 2);
        let mut vmat = Mat::<f64>::zeros(d, 4);
        let mut dmat = Mat::<f64>::zeros(d, 4);
        for v in 0..4 {
            let mut col = vec![0.0; d];
            for i in 0..d {
                col[i] = fluor[(i, a)];
            }
            // Bump a detector that is *not* a linear mix of master A and B so OLS
            // on the panel cannot absorb it; joint can commit this variant.
            col[b_peak] = (col[b_peak] + 0.2 * (v as f64 + 1.0)).min(1.2);
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

fn build_panel(n: usize, d: usize, n_fluor: usize, k_af: usize, seed: u64) -> Panel {
    let fluor = overlapping_spectra(d, n_fluor);
    let names: Vec<String> = (0..n_fluor).map(|j| format!("F{j}")).collect();
    let library = unit_peak_library(k_af, d);
    let variants = collinear_variants(&fluor, &names);
    let mut events = Vec::with_capacity(n * d);
    let mut true_fluor = Vec::with_capacity(n);
    let mut true_af = Vec::with_capacity(n);
    for i in 0..n {
        let af = (i.wrapping_mul(seed as usize + 3)) % k_af.max(1);
        let kind = i % (n_fluor + 1);
        let af_scale = 40.0 + ((i % 17) as f64);
        true_af.push(af);
        let f_idx = if kind == n_fluor { None } else { Some(kind) };
        true_fluor.push(f_idx);
        let f_scale = 250.0 + ((i % 11) as f64) * 10.0;
        let partner = n_fluor.saturating_sub(2);
        for c in 0..d {
            let mut v = library.signatures[(c, af)] * af_scale;
            if let Some(fi) = f_idx {
                // True-A cells use a collinear variant so OLS leaks into the partner
                // and joint can recover by picking the variant (BUV661/APC analogue).
                if fi == partner {
                    if let Some(vmat) = variants.variants.get(&names[fi]) {
                        let vi = vmat.ncols().saturating_sub(1);
                        v += vmat[(c, vi)] * f_scale;
                    } else {
                        v += fluor[(c, fi)] * f_scale;
                    }
                } else {
                    v += fluor[(c, fi)] * f_scale;
                }
            }
            events.push(v);
        }
    }
    Panel {
        n,
        d,
        n_fluor,
        k_af,
        fluor,
        names,
        library,
        variants,
        events,
        true_fluor,
        true_af,
    }
}

fn mean_std(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let m = xs.iter().sum::<f64>() / xs.len() as f64;
    let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / xs.len() as f64;
    (m, var.sqrt())
}

fn time_reps<T>(warmup: usize, reps: usize, mut f: impl FnMut() -> Result<T>) -> Result<(f64, f64)> {
    for _ in 0..warmup {
        let _ = f()?;
    }
    let mut times = Vec::with_capacity(reps);
    for _ in 0..reps {
        let t0 = Instant::now();
        let _ = f()?;
        times.push(t0.elapsed().as_secs_f64());
    }
    Ok(mean_std(&times))
}

fn fmt_secs(s: f64) -> String {
    if s < 0.1 {
        format!("{:.2} ms", s * 1000.0)
    } else {
        format!("{:.3}s", s)
    }
}

fn joint_cfg_threads(threads: usize) -> JointUnmixConfig {
    let mut cfg = JointUnmixConfig::default();
    if threads <= 1 {
        cfg.parallel_event_threshold = usize::MAX;
    }
    cfg
}

fn run_joint(panel: &Panel, cfg: &JointUnmixConfig) -> Result<JointUnmixResult> {
    unmix_autospectral_joint(
        &panel.events,
        panel.n,
        panel.fluor.as_ref(),
        &panel.names,
        &panel.library,
        &panel.variants,
        cfg,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn run_rust_e2e(panel: &Panel, case_dir: &Path, cfg: &JointUnmixConfig) -> Result<()> {
    let path = case_dir.join("prepared.fcs");
    let fcs = Fcs::open(path.to_str().context("utf8")?)?;
    let (events, n) = fluorescence_from_fcs(&fcs, panel.d)?;
    let mut p = panel.clone();
    p.events = events;
    p.n = n;
    let _ = run_joint(&p, cfg)?;
    write_fcs_file(fcs, case_dir.join("unmixed.fcs"))?;
    Ok(())
}

fn detector_colnames(panel: &Panel) -> Vec<String> {
    if panel.library.detector_names.len() == panel.d {
        panel.library.detector_names.clone()
    } else {
        (0..panel.d).map(|i| format!("FL{}-A", i + 1)).collect()
    }
}

fn write_named_matrix(
    path: &Path,
    rownames: &[String],
    mat: &Mat<f64>,
    col_names: &[String],
) -> Result<()> {
    if col_names.len() != mat.nrows() {
        bail!(
            "detector names {} != matrix rows {}",
            col_names.len(),
            mat.nrows()
        );
    }
    let mut out = String::new();
    out.push_str("name");
    for name in col_names {
        out.push_str(&format!(",{name}"));
    }
    out.push('\n');
    for (c, name) in rownames.iter().enumerate() {
        out.push_str(name);
        for r in 0..mat.nrows() {
            out.push_str(&format!(",{}", mat[(r, c)]));
        }
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("write {}", path.display()))
}

fn write_events_csv(
    path: &Path,
    events: &[f64],
    n: usize,
    d: usize,
    col_names: &[String],
) -> Result<()> {
    if col_names.len() != d {
        bail!("event colnames {} != d {d}", col_names.len());
    }
    let mut out = String::new();
    for (j, name) in col_names.iter().enumerate() {
        if j > 0 {
            out.push(',');
        }
        out.push_str(name);
    }
    out.push('\n');
    for e in 0..n {
        for j in 0..d {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!("{}", events[e * d + j]));
        }
        out.push('\n');
    }
    fs::write(path, out).with_context(|| format!("write {}", path.display()))
}

fn write_mixture_fcs(path: &Path, panel: &Panel) -> Result<()> {
    let seed = 0xA5u64 ^ (panel.n as u64) << 8 ^ panel.d as u64;
    let spec = default_cytometry_mixture(panel.n, panel.d, seed);
    let mut columns = gaussian_population_columns(&spec).context("gaussian columns")?;
    let mut fl_i = 0usize;
    for (name, values) in columns.iter_mut() {
        if name.starts_with("FL") && name.ends_with("-A") {
            for e in 0..panel.n {
                values[e] = panel.events[e * panel.d + fl_i] as f32;
            }
            fl_i += 1;
        }
    }
    let polars_cols: Vec<Column> = columns
        .into_iter()
        .map(|(name, values)| Column::new(name.into(), values))
        .collect();
    let df = DataFrame::new_infer_height(polars_cols).context("DataFrame")?;
    let mut fcs = gaussian_populations_fcs(&spec).context("synthetic Fcs")?;
    fcs.data_frame = Arc::new(df);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_fcs_file(fcs, path).context("write prepared FCS")?;
    Ok(())
}

fn fluorescence_from_fcs(fcs: &Fcs, d: usize) -> Result<(Vec<f64>, usize)> {
    let names: Vec<String> = (1..=d).map(|i| format!("FL{i}-A")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let cols = fcs.columns(&refs).context("FCS fluorescence columns")?;
    let n = cols.first().map(|c| c.len()).unwrap_or(0);
    let mut out = vec![0.0; n * d];
    for e in 0..n {
        for c in 0..d {
            out[e * d + c] = f64::from(cols[c][e]);
        }
    }
    Ok((out, n))
}

fn mad(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let med = v[v.len() / 2];
    let mut dev: Vec<f64> = v.iter().map(|x| (x - med).abs()).collect();
    dev.sort_by(|a, b| a.total_cmp(b));
    dev[dev.len() / 2]
}

fn collinear_quality(panel: &Panel, joint: &JointUnmixResult) -> Result<Option<(f64, f64, f64)>> {
    if panel.n_fluor < 2 {
        return Ok(Some((0.0, 0.0, 0.0)));
    }
    // Master + AF column is F+1 wide; OLS helper QR-asserts detectors >= emitters.
    if panel.d < panel.n_fluor + 1 {
        return Ok(None);
    }
    let a = panel.n_fluor - 2;
    let b = panel.n_fluor - 1;
    let mut ols_b = Vec::new();
    let mut joint_b = Vec::new();
    let mut n_true_a = 0usize;
    let mut n_commit = 0usize;
    for e in 0..panel.n {
        if panel.true_fluor[e] != Some(a) {
            continue;
        }
        n_true_a += 1;
        if joint.variant_index[e * panel.n_fluor + a].is_some() {
            n_commit += 1;
        }
        let y = &panel.events[e * panel.d..(e + 1) * panel.d];
        let m = swap_af_column(panel.fluor.as_ref(), &panel.library, panel.true_af[e])
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        let ols = unmix_event_ols(m.as_ref(), y).map_err(|err| anyhow::anyhow!("{err}"))?;
        ols_b.push(ols[b].abs());
        if let Some(row) = joint.event_abundances(e) {
            joint_b.push(row[b].abs());
        }
    }
    let commit_rate = if n_true_a == 0 {
        0.0
    } else {
        n_commit as f64 / n_true_a as f64
    };
    Ok(Some((mad(&ols_b), mad(&joint_b), commit_rate)))
}

fn write_case_inputs(dir: &Path, panel: &Panel) -> Result<()> {
    fs::create_dir_all(dir)?;
    let det = detector_colnames(panel);
    write_named_matrix(&dir.join("spectra.csv"), &panel.names, &panel.fluor, &det)?;
    write_named_matrix(
        &dir.join("af.csv"),
        &panel.library.names,
        &panel.library.signatures,
        &det,
    )?;
    write_events_csv(
        &dir.join("events.csv"),
        &panel.events,
        panel.n,
        panel.d,
        &det,
    )?;
    let mut thr = String::from("fluor,threshold\n");
    for (n, t) in panel.names.iter().zip(&panel.variants.thresholds) {
        thr.push_str(&format!("{n},{t}\n"));
    }
    fs::write(dir.join("thresholds.csv"), thr)?;
    let vdir = dir.join("variants");
    let ddir = dir.join("deltas");
    fs::create_dir_all(&vdir)?;
    fs::create_dir_all(&ddir)?;
    for name in &panel.names {
        if let Some(m) = panel.variants.variants.get(name) {
            let rnames: Vec<String> = (0..m.ncols()).map(|i| format!("{name}_{i}")).collect();
            write_named_matrix(&vdir.join(format!("{name}.csv")), &rnames, m, &det)?;
        }
        if let Some(m) = panel.variants.deltas.get(name) {
            let rnames: Vec<String> = (0..m.ncols()).map(|i| format!("{name}_{i}")).collect();
            write_named_matrix(&ddir.join(format!("{name}.csv")), &rnames, m, &det)?;
        }
    }
    let meta = serde_json::json!({
        "n": panel.n,
        "d": panel.d,
        "F": panel.n_fluor,
        "K_AF": panel.k_af,
        "fluor_names": panel.names,
        "n_variants_mean": panel.variants.n_variants_mean(),
    });
    fs::write(dir.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".into())
        .trim()
        .to_string()
}

fn sidecar_r(
    case_dir: &Path,
    out_json: &Path,
    phase: &str,
    warmup: usize,
    reps: usize,
    write_unmixed: bool,
    threads: usize,
) -> Result<Sidecar> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/compare_with_r.R");
    if !script.exists() {
        return Ok(Sidecar::Failed(format!(
            "missing sidecar script {}",
            script.display()
        )));
    }
    let mut cmd = Command::new("Rscript");
    cmd.arg(&script)
        .arg("--case-dir")
        .arg(case_dir)
        .arg("--out-json")
        .arg(out_json)
        .arg("--phase")
        .arg(phase)
        .arg("--warmup")
        .arg(warmup.to_string())
        .arg("--reps")
        .arg(reps.to_string())
        .arg("--threads")
        .arg(threads.to_string())
        .env("OMP_NUM_THREADS", threads.to_string())
        .env("OPENBLAS_NUM_THREADS", "1")
        .env("MKL_NUM_THREADS", "1")
        .env("VECLIB_MAXIMUM_THREADS", "1");
    if write_unmixed {
        cmd.arg("--write-unmixed").arg("1");
    }
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Sidecar::Skipped("Rscript not on PATH".into()));
        }
        Err(e) => return Ok(Sidecar::Failed(format!("spawn Rscript: {e}"))),
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        return Ok(Sidecar::Failed(format!(
            "Rscript {}: {stderr}{stdout}",
            output.status
        )));
    }
    if !out_json.exists() {
        return Ok(Sidecar::Failed(format!(
            "no JSON written; stderr={stderr} stdout={stdout}"
        )));
    }
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(out_json)?)
        .with_context(|| format!("parse {}", out_json.display()))?;
    if v["skipped"].as_bool().unwrap_or(false) {
        let reason = v["reason"]
            .as_str()
            .unwrap_or("skipped with no reason")
            .to_string();
        return Ok(Sidecar::Skipped(reason));
    }
    let meta = RMeta {
        r_version: v["r_version"].as_str().unwrap_or("unknown").to_string(),
        autospectralrcpp_version: v["autospectralrcpp_version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        autospectral_version: v["autospectral_version"]
            .as_str()
            .map(str::to_string),
    };
    Ok(Sidecar::Timed {
        row: TimingRow {
            config: "r_joint".into(),
            case_id: case_dir
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            phase: phase.into(),
            mean_s: v["mean_s"].as_f64().unwrap_or(0.0),
            std_s: v["std_s"].as_f64().unwrap_or(0.0),
            events: v["events"].as_u64().unwrap_or(0) as usize,
            detectors: v["detectors"].as_u64().unwrap_or(0) as usize,
            fluors: v["fluors"].as_u64().unwrap_or(0) as usize,
            k_af: v["k_af"].as_u64().unwrap_or(0) as usize,
            n_variants_mean: v["n_variants_mean"].as_f64().unwrap_or(0.0),
            events_per_s: v["events_per_s"].as_f64().unwrap_or(0.0),
            threads: v["threads"].as_u64().unwrap_or(threads as u64) as usize,
            reps,
            skipped: false,
            reason: None,
        },
        meta,
    })
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

fn parse_unmixed_r(path: &Path) -> Result<(Vec<Vec<f64>>, Vec<usize>)> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = text.lines();
    let header = lines.next().context("empty unmixed_r.csv")?;
    let cols: Vec<String> = header
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .collect();
    let n_cols = cols.len();
    if n_cols < 3 {
        bail!("unmixed_r.csv expected fluor + AF + AF Index columns");
    }
    let n_fluor = n_cols - 2;
    let mut fluor_cols = vec![Vec::new(); n_fluor];
    let mut af_index = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != n_cols {
            bail!("unmixed_r.csv row has {} columns, header {n_cols}", parts.len());
        }
        for j in 0..n_fluor {
            fluor_cols[j].push(
                parts[j]
                    .trim()
                    .trim_matches('"')
                    .parse::<f64>()
                    .with_context(|| format!("parse fluor col {j}"))?,
            );
        }
        let idx = parts[n_cols - 1]
            .trim()
            .trim_matches('"')
            .parse::<f64>()
            .context("parse AF Index")?;
        af_index.push(idx.round() as usize);
    }
    Ok((fluor_cols, af_index))
}

fn agreement(
    case_id: &str,
    rust_joint: &JointUnmixResult,
    r_path: &Path,
) -> Result<AgreementRow> {
    let (r_fluors, r_af) = parse_unmixed_r(r_path)?;
    if r_fluors.len() != rust_joint.n_fluor {
        bail!(
            "R fluor columns {} != rust {}",
            r_fluors.len(),
            rust_joint.n_fluor
        );
    }
    let n = rust_joint.n_events;
    let w = rust_joint.n_fluor + 1;
    let mut cosines = Vec::with_capacity(rust_joint.n_fluor);
    for j in 0..rust_joint.n_fluor {
        let mut rust_col = vec![0.0; n];
        for e in 0..n {
            rust_col[e] = rust_joint.abundances[e * w + j];
        }
        if rust_col.len() != r_fluors[j].len() {
            bail!(
                "event count rust {} vs R {}",
                rust_col.len(),
                r_fluors[j].len()
            );
        }
        cosines.push(cosine(&rust_col, &r_fluors[j]));
    }
    let r_af_aligned: Vec<usize> = if r_af.iter().any(|&x| x == 0) {
        r_af
    } else {
        r_af.into_iter().map(|x| x.saturating_sub(1)).collect()
    };
    let matched = rust_joint
        .af_index
        .iter()
        .zip(&r_af_aligned)
        .filter(|(a, b)| a == b)
        .count();
    let mean_cosine = if cosines.is_empty() {
        0.0
    } else {
        cosines.iter().sum::<f64>() / cosines.len() as f64
    };
    let min_cosine = cosines.iter().copied().fold(f64::INFINITY, f64::min);
    Ok(AgreementRow {
        case_id: case_id.to_string(),
        mean_cosine,
        min_cosine: if min_cosine.is_finite() {
            min_cosine
        } else {
            0.0
        },
        af_index_match: if n == 0 {
            0.0
        } else {
            matched as f64 / n as f64
        },
    })
}

fn row(
    config: &str,
    case_id: &str,
    phase: &str,
    panel: &Panel,
    mean_s: f64,
    std_s: f64,
    reps: usize,
    threads: usize,
) -> TimingRow {
    TimingRow {
        config: config.into(),
        case_id: case_id.into(),
        phase: phase.into(),
        mean_s,
        std_s,
        events: panel.n,
        detectors: panel.d,
        fluors: panel.n_fluor,
        k_af: panel.k_af,
        n_variants_mean: panel.variants.n_variants_mean(),
        events_per_s: if mean_s > 0.0 {
            panel.n as f64 / mean_s
        } else {
            0.0
        },
        threads,
        reps,
        skipped: false,
        reason: None,
    }
}

fn write_report(
    out: &Path,
    rows: &[TimingRow],
    quality: &[(String, f64, f64, f64)],
    agreement_rows: &[AgreementRow],
    r_meta: Option<&RMeta>,
) -> Result<()> {
    let mut md = String::from("# flow-autospectral vs AutoSpectralRcpp\n\n");
    md.push_str("QC-core is the headline clock (events in RAM). e2e is secondary.\n\n");
    md.push_str("| config | case | phase | threads | n | d | F | K_AF | mean_s | events/s |\n");
    md.push_str("|--------|------|-------|---------|---|---|---|------|--------|----------|\n");
    for r in rows {
        if r.skipped {
            continue;
        }
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {:.4} | {:.0} |\n",
            r.config, r.case_id, r.phase, r.threads, r.events, r.detectors, r.fluors, r.k_af,
            r.mean_s, r.events_per_s
        ));
    }
    if !agreement_rows.is_empty() {
        md.push_str("\n## Agreement (Rust vs AutoSpectralRcpp joint)\n\n");
        md.push_str("| case | mean cosine | min cosine | AF-index match |\n");
        md.push_str("|------|-------------|------------|----------------|\n");
        for a in agreement_rows {
            md.push_str(&format!(
                "| {} | {:.4} | {:.4} | {:.3} |\n",
                a.case_id, a.mean_cosine, a.min_cosine, a.af_index_match
            ));
        }
    }
    if !quality.is_empty() {
        md.push_str("\n## Collinear-pair spillover MAD (true single-positives)\n\n");
        md.push_str("| case | OLS MAD | joint MAD | variant commit |\n|------|---------|----------|----------------|\n");
        for (id, ols, j, commit) in quality {
            md.push_str(&format!("| {id} | {ols:.4} | {j:.4} | {commit:.3} |\n"));
        }
    }
    md.push_str(&format!(
        "\nMachine: {}\nrustc: {}\nDate: 2026-08-19\nhardware threads: {}\nR OpenMP: AutoSpectralRcpp links libomp; sidecar sets OMP_NUM_THREADS to the row's thread count and BLAS to 1.\n",
        std::env::consts::ARCH,
        rustc_version(),
        hardware_threads()
    ));
    if let Some(m) = r_meta {
        md.push_str(&format!(
            "R: {}\nAutoSpectralRcpp: {}\n",
            m.r_version, m.autospectralrcpp_version
        ));
        if let Some(asv) = &m.autospectral_version {
            md.push_str(&format!("AutoSpectral: {asv}\n"));
        }
    }
    fs::write(out.join("throughput_report.md"), md)?;
    fs::write(out.join("throughput_merged.json"), serde_json::to_string_pretty(rows)?)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse_cli(&args)?;
    fs::create_dir_all(&cli.out)?;
    if cli.threads > 1 {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(cli.threads)
            .build_global();
    }
    let mt = cli.threads;
    let run_mt = mt > 1 && !force_sequential();
    if force_sequential() && mt > 1 {
        eprintln!(
            "FLOW_AUTOSPECTRAL_FORCE_SEQUENTIAL=1: rust_joint multi-thread row skipped; R still runs threads={mt}"
        );
    }

    let mut rows = Vec::new();
    let mut quality = Vec::new();
    let mut agreement_rows = Vec::new();
    let mut r_meta: Option<RMeta> = None;

    let mut cases: Vec<(String, Panel)> = Vec::new();
    if cli.fcs_paths.is_empty() {
        for &n in &cli.events {
            for &d in &cli.detectors {
                for &f in &cli.fluors {
                    let k_af = if f >= 42 { 100 } else { 8 };
                    if d < f + 1 {
                        eprintln!(
                            "skip n{n}_d{d}_F{f}: detectors {d} < F+1={} (underdetermined OLS / R agreement is not comparable)",
                            f + 1
                        );
                        continue;
                    }
                    let id = format!("n{n}_d{d}_F{f}");
                    cases.push((id, build_panel(n, d, f, k_af, 17)));
                }
            }
        }
    }

    let cfg_1 = joint_cfg_threads(1);
    let cfg_mt = joint_cfg_threads(mt);

    for (id, panel) in &cases {
        let case_dir = cli.out.join("cases").join(id);
        write_case_inputs(&case_dir, panel)?;
        write_mixture_fcs(&case_dir.join("prepared.fcs"), panel)?;

        let (mean_1, std_1) = time_reps(cli.warmup, cli.reps, || run_joint(panel, &cfg_1))?;
        rows.push(row(
            "rust_joint",
            id,
            "qc_core",
            panel,
            mean_1,
            std_1,
            cli.reps,
            1,
        ));
        println!(
            "{id} rust qc_core threads=1 {} ({:.0} ev/s)",
            fmt_secs(mean_1),
            panel.n as f64 / mean_1.max(1e-12)
        );

        if run_mt {
            let (mean_mt, std_mt) = time_reps(cli.warmup, cli.reps, || run_joint(panel, &cfg_mt))?;
            rows.push(row(
                "rust_joint",
                id,
                "qc_core",
                panel,
                mean_mt,
                std_mt,
                cli.reps,
                mt,
            ));
            println!(
                "{id} rust qc_core threads={mt} {} ({:.0} ev/s)",
                fmt_secs(mean_mt),
                panel.n as f64 / mean_mt.max(1e-12)
            );
        }

        let (io_mean, io_std) = time_reps(cli.warmup, cli.reps, || {
            let fcs = Fcs::open(
                case_dir
                    .join("prepared.fcs")
                    .to_str()
                    .context("utf8 path")?,
            )?;
            let _ = fluorescence_from_fcs(&fcs, panel.d)?;
            Ok(())
        })?;
        rows.push(row(
            "rust_io",
            id,
            "io_only",
            panel,
            io_mean,
            io_std,
            cli.reps,
            1,
        ));

        if cli.e2e {
            let (e2e_1, e2e_1_std) = time_reps(cli.warmup, cli.reps, || {
                run_rust_e2e(panel, &case_dir, &cfg_1)
            })?;
            rows.push(row(
                "rust_joint",
                id,
                "e2e",
                panel,
                e2e_1,
                e2e_1_std,
                cli.reps,
                1,
            ));
            println!(
                "{id} rust e2e threads=1 {} ({:.0} ev/s)",
                fmt_secs(e2e_1),
                panel.n as f64 / e2e_1.max(1e-12)
            );
            if run_mt {
                let (e2e_mt, e2e_mt_std) = time_reps(cli.warmup, cli.reps, || {
                    run_rust_e2e(panel, &case_dir, &cfg_mt)
                })?;
                rows.push(row(
                    "rust_joint",
                    id,
                    "e2e",
                    panel,
                    e2e_mt,
                    e2e_mt_std,
                    cli.reps,
                    mt,
                ));
                println!(
                    "{id} rust e2e threads={mt} {} ({:.0} ev/s)",
                    fmt_secs(e2e_mt),
                    panel.n as f64 / e2e_mt.max(1e-12)
                );
            }
        }

        let joint = run_joint(panel, &cfg_1)?;
        match collinear_quality(panel, &joint)? {
            Some((ols_mad, joint_mad, commit)) => {
                quality.push((id.clone(), ols_mad, joint_mad, commit));
                println!(
                    "{id} collinear MAD OLS={ols_mad:.3} joint={joint_mad:.3} variant commit={commit:.3}"
                );
            }
            None => {
                eprintln!(
                    "{id} collinear MAD skipped (d={} < F+1={}; OLS helper needs detectors >= emitters)",
                    panel.d,
                    panel.n_fluor + 1
                );
            }
        }

        if !cli.rust_only {
            let write_unmixed = panel.n <= 50_000;
            record_r(
                &case_dir,
                id,
                &joint,
                &cli,
                1,
                "qc_core",
                write_unmixed,
                &mut rows,
                &mut agreement_rows,
                &mut r_meta,
            )?;
            if mt > 1 {
                record_r(
                    &case_dir,
                    id,
                    &joint,
                    &cli,
                    mt,
                    "qc_core",
                    false,
                    &mut rows,
                    &mut agreement_rows,
                    &mut r_meta,
                )?;
            }
            if cli.e2e {
                record_r(
                    &case_dir,
                    id,
                    &joint,
                    &cli,
                    1,
                    "e2e",
                    false,
                    &mut rows,
                    &mut agreement_rows,
                    &mut r_meta,
                )?;
                if mt > 1 {
                    record_r(
                        &case_dir,
                        id,
                        &joint,
                        &cli,
                        mt,
                        "e2e",
                        false,
                        &mut rows,
                        &mut agreement_rows,
                        &mut r_meta,
                    )?;
                }
            }
        }
    }

    for (i, path) in cli.fcs_paths.iter().enumerate() {
        let id = format!("real_{:02}", i + 1);
        let fcs = Fcs::open(path.to_str().context("utf8")?)?;
        println!("opened {id} (path omitted from report)");
        let _ = fcs;
        let _ = id;
    }

    write_report(&cli.out, &rows, &quality, &agreement_rows, r_meta.as_ref())?;
    println!("wrote {}", cli.out.join("throughput_report.md").display());
    Ok(())
}

fn record_r(
    case_dir: &Path,
    id: &str,
    joint: &JointUnmixResult,
    cli: &Cli,
    threads: usize,
    phase: &str,
    write_unmixed: bool,
    rows: &mut Vec<TimingRow>,
    agreement_rows: &mut Vec<AgreementRow>,
    r_meta: &mut Option<RMeta>,
) -> Result<()> {
    let r_json = case_dir.join(format!("throughput_r_t{threads}_{phase}.json"));
    match sidecar_r(
        case_dir,
        &r_json,
        phase,
        cli.warmup,
        cli.reps,
        write_unmixed,
        threads,
    )? {
        Sidecar::Timed { row: rrow, meta } => {
            println!(
                "  R {phase} threads={} {} ({:.0} ev/s) AutoSpectralRcpp {}",
                rrow.threads,
                fmt_secs(rrow.mean_s),
                rrow.events_per_s,
                meta.autospectralrcpp_version
            );
            if r_meta.is_none() {
                *r_meta = Some(meta);
            }
            if write_unmixed && phase == "qc_core" {
                let unmixed = case_dir.join("unmixed_r.csv");
                match agreement(id, joint, &unmixed) {
                    Ok(a) => {
                        println!(
                            "  agreement mean cosine={:.4} min={:.4} AF-index match={:.3}",
                            a.mean_cosine, a.min_cosine, a.af_index_match
                        );
                        agreement_rows.push(a);
                    }
                    Err(e) => eprintln!("  agreement skipped: {e:#}"),
                }
            }
            rows.push(rrow);
        }
        Sidecar::Skipped(reason) => {
            eprintln!("  R sidecar threads={threads} skipped: {reason}");
        }
        Sidecar::Failed(err) => {
            eprintln!("  R sidecar threads={threads} failed: {err}");
        }
    }
    Ok(())
}
