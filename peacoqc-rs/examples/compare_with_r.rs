//! PeacoQC Rust vs R throughput harness.

use anyhow::{Context, Result};
use flow_fcs::file::Fcs;
use flow_fcs::keyword::{IntegerKeyword, Keyword, MixedKeyword};
use flow_fcs::metadata::Metadata;
use flow_fcs::version::Version;
use flow_fcs::write::write_fcs_file;
use peacoqc_rs::{
    DoubletConfig, MarginConfig, PeacoQCConfig, PeacoQCData, QCMode, peacoqc, remove_doublets,
    remove_margins,
};
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

/// Mirrors `flow_fcs::corpus::path` until the worktree `flow-fcs` crate includes
/// the corpus module (present on main workspace, not yet on this branch).
fn corpus_seed_path(file_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../gates/Gating-ML.v1.5.081030.Compliance-tests.081030/List-mode Data Files")
        .join(file_name)
}

/// Synthetic fixture instrument name (no spaces — worktree `flow-fcs` predates V3_1
/// doubled-delimiter escaping; spaces in `$CYT` would corrupt TEXT on write).
const SYNTHETIC_CYT: &str = "flow-crates-peacoqc-r-compare-synthetic";

struct CaseSpec {
    id: String,
    n_events: usize,
    n_channels: usize,
    prepared_fcs: PathBuf,
}

#[derive(Debug, Serialize)]
struct TimingStats {
    mean_s: f64,
    std_s: f64,
    events_per_s: f64,
    pct_removed: f64,
    reps: usize,
}

#[derive(Debug)]
struct CliArgs {
    smoke: bool,
    rust_only: bool,
    gpu: bool,
    e2e: bool,
    include_margins_doublets: bool,
    synthetic: bool,
    no_synthetic: bool,
    out: Option<PathBuf>,
    config: Option<String>,
    case_dir: Option<PathBuf>,
    warmup: usize,
    reps: usize,
    events: Vec<usize>,
    channels: Vec<usize>,
    fcs_paths: Vec<PathBuf>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            smoke: false,
            rust_only: false,
            gpu: false,
            e2e: false,
            include_margins_doublets: false,
            synthetic: false,
            no_synthetic: false,
            out: None,
            config: None,
            case_dir: None,
            warmup: 1,
            reps: 5,
            events: vec![50_000, 200_000, 1_000_000],
            channels: vec![5, 15, 30],
            fcs_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TimingRow {
    config: String,
    case_id: String,
    phase: String,
    mean_s: f64,
    std_s: f64,
    events: usize,
    channels: usize,
    events_per_s: f64,
    pct_removed: f64,
    reps: usize,
    #[serde(default)]
    skipped: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    rayon_num_threads: Option<String>,
    #[serde(default)]
    rustc: Option<String>,
    #[serde(default)]
    peacoqc_rs_version: Option<String>,
    #[serde(default)]
    r_version: Option<String>,
    #[serde(default)]
    peacoqc_version: Option<String>,
    #[serde(default)]
    flowcore_version: Option<String>,
}

#[derive(Serialize)]
struct WorkerResult {
    config: String,
    case_id: String,
    phase: &'static str,
    mean_s: f64,
    std_s: f64,
    events: usize,
    channels: usize,
    events_per_s: f64,
    pct_removed: f64,
    reps: usize,
    rayon_num_threads: String,
    rustc: String,
    peacoqc_rs_version: &'static str,
    skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

fn write_synthetic_prepared_fcs(path: &Path, n_events: usize, n_fl_channels: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir {}", parent.display()))?;
    }

    let time: Vec<f32> = (0..n_events).map(|e| e as f32).collect();
    let fsc_a: Vec<f32> = (0..n_events)
        .map(|e| (e as f32).mul_add(0.01, 500.0))
        .collect();
    let fsc_h: Vec<f32> = fsc_a
        .iter()
        .map(|&area| area * 0.85 + 10.0)
        .collect();
    let ssc_a: Vec<f32> = (0..n_events)
        .map(|e| (e as f32).mul_add(0.01, 300.0))
        .collect();

    let mut columns = vec![
        Column::new("Time".into(), time),
        Column::new("FSC-A".into(), fsc_a),
        Column::new("FSC-H".into(), fsc_h),
        Column::new("SSC-A".into(), ssc_a),
    ];

    for fl in 1..=n_fl_channels {
        let name = format!("FL{fl}-A");
        let values: Vec<f32> = (0..n_events)
            .map(|e| (e as f32).mul_add(0.001, fl as f32 * 1000.0))
            .collect();
        columns.push(Column::new(name.into(), values));
    }

    let param_names: Vec<String> = columns.iter().map(|c| c.name().to_string()).collect();

    let df = DataFrame::new_infer_height(columns).context("build synthetic DataFrame")?;

    let seed = corpus_seed_path("int-10000_events_random.fcs");
    let seed_str = seed
        .to_str()
        .context("corpus seed path is not valid UTF-8")?;
    let mut fcs = Fcs::open(seed_str).context("open corpus seed FCS")?;
    fcs.header.version = Version::V3_1;

    let mut metadata = Metadata::new();
    metadata.insert_string_keyword("$BYTEORD".into(), "1,2,3,4".into());
    metadata.insert_string_keyword("$DATATYPE".into(), "F".into());
    metadata.insert_string_keyword("$MODE".into(), "L".into());
    metadata.insert_string_keyword("$NEXTDATA".into(), "0".into());
    metadata.insert_string_keyword("$CYT".into(), SYNTHETIC_CYT.into());

    for (p, name) in param_names.iter().enumerate() {
        let idx = p + 1;
        metadata.insert_string_keyword(format!("$P{idx}N"), name.clone());
        metadata
            .keywords
            .insert(format!("$P{idx}B"), Keyword::Int(IntegerKeyword::PnB(32)));
        metadata.keywords.insert(
            format!("$P{idx}R"),
            Keyword::Int(IntegerKeyword::PnR(262_144)),
        );
        metadata.keywords.insert(
            format!("$P{idx}E"),
            Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
        );
    }

    fcs.metadata = metadata;
    fcs.data_frame = Arc::new(df);

    write_fcs_file(fcs, path).context("write prepared FCS")?;
    Ok(())
}

fn parse_args(args: &[String]) -> Result<CliArgs> {
    let mut parsed = CliArgs::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--smoke" => parsed.smoke = true,
            "--rust-only" => parsed.rust_only = true,
            "--gpu" => parsed.gpu = true,
            "--e2e" => parsed.e2e = true,
            "--include-margins-doublets" => parsed.include_margins_doublets = true,
            "--synthetic" => parsed.synthetic = true,
            "--no-synthetic" => parsed.no_synthetic = true,
            "--out" => {
                i += 1;
                if i >= args.len() {
                    anyhow::bail!("--out requires a path");
                }
                parsed.out = Some(PathBuf::from(&args[i]));
            }
            "--config" => {
                i += 1;
                if i >= args.len() {
                    anyhow::bail!("--config requires rust-cpu-1, rust-cpu, or rust-gpu");
                }
                parsed.config = Some(args[i].clone());
            }
            "--case-dir" => {
                i += 1;
                if i >= args.len() {
                    anyhow::bail!("--case-dir requires a path");
                }
                parsed.case_dir = Some(PathBuf::from(&args[i]));
            }
            "--warmup" => {
                i += 1;
                parsed.warmup = parse_usize_arg(args, i, "--warmup")?;
            }
            "--reps" => {
                i += 1;
                parsed.reps = parse_usize_arg(args, i, "--reps")?;
            }
            "--events" => {
                i += 1;
                parsed.events = parse_usize_list(args, i, "--events")?;
            }
            "--channels" => {
                i += 1;
                parsed.channels = parse_usize_list(args, i, "--channels")?;
            }
            "--fcs" => {
                i += 1;
                if i >= args.len() {
                    anyhow::bail!("--fcs requires a path");
                }
                parsed.fcs_paths.push(PathBuf::from(&args[i]));
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
        i += 1;
    }
    if parsed.smoke {
        parsed.warmup = 0;
        parsed.reps = 1;
    }
    anyhow::ensure!(parsed.reps > 0, "--reps must be greater than zero");
    anyhow::ensure!(!parsed.events.is_empty(), "--events must list at least one size");
    anyhow::ensure!(
        !parsed.channels.is_empty(),
        "--channels must list at least one FL count"
    );
    Ok(parsed)
}

fn parse_usize_arg(args: &[String], index: usize, flag: &str) -> Result<usize> {
    let value = args
        .get(index)
        .with_context(|| format!("{flag} requires an integer"))?;
    value
        .parse()
        .with_context(|| format!("{flag} must be a non-negative integer, got {value}"))
}

fn parse_usize_list(args: &[String], index: usize, flag: &str) -> Result<Vec<usize>> {
    let value = args
        .get(index)
        .with_context(|| format!("{flag} requires a comma-separated list"))?;
    value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<usize>()
                .with_context(|| format!("{flag} entry `{part}` is not a positive integer"))
        })
        .collect()
}

fn is_fl_channel(name: &str) -> bool {
    name.strip_prefix("FL")
        .and_then(|rest| rest.strip_suffix("-A"))
        .is_some_and(|digits| {
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn summarize_times(times: &[f64], events: usize, pct_removed: f64) -> Result<TimingStats> {
    anyhow::ensure!(
        !times.is_empty(),
        "at least one timed repetition is required"
    );
    anyhow::ensure!(
        times.iter().all(|time| time.is_finite() && *time > 0.0),
        "all measured durations must be positive and finite"
    );
    let mean_s = times.iter().sum::<f64>() / times.len() as f64;
    let variance = times
        .iter()
        .map(|time| {
            let deviation = time - mean_s;
            deviation * deviation
        })
        .sum::<f64>()
        / times.len() as f64;
    Ok(TimingStats {
        mean_s,
        std_s: variance.sqrt(),
        events_per_s: events as f64 / mean_s,
        pct_removed,
        reps: times.len(),
    })
}

fn time_qc_core(
    fcs: &Fcs,
    config: &PeacoQCConfig,
    warmup: usize,
    reps: usize,
    include_margins_doublets: bool,
) -> Result<TimingStats> {
    let margin_channels: Vec<String> = fcs
        .channel_names()
        .into_iter()
        .filter(|name| name == "FSC-A" || name == "SSC-A" || is_fl_channel(name))
        .collect();
    let margin_config = MarginConfig {
        channels: margin_channels,
        ..Default::default()
    };
    let doublet_config = DoubletConfig::default();

    let run_once = || -> Result<f64> {
        if include_margins_doublets {
            let _ = remove_margins(fcs, &margin_config).context("remove_margins")?;
            let _ = remove_doublets(fcs, &doublet_config).context("remove_doublets")?;
        }
        let result = peacoqc(fcs, config).context("peacoqc")?;
        Ok(result.percentage_removed)
    };

    for _ in 0..warmup {
        let _ = run_once()?;
    }

    let mut times = Vec::with_capacity(reps);
    let mut last_pct = 0.0;
    for _ in 0..reps {
        let started = Instant::now();
        last_pct = run_once()?;
        times.push(started.elapsed().as_secs_f64());
    }
    summarize_times(&times, fcs.n_events(), last_pct)
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn open_fcs(path: &Path) -> Result<Fcs> {
    let path_str = path
        .to_str()
        .with_context(|| format!("FCS path is not valid UTF-8: {}", path.display()))?;
    Fcs::open(path_str).with_context(|| format!("open prepared FCS {}", path.display()))
}

fn worker_output_path(case_dir: &Path, config: &str) -> PathBuf {
    case_dir.join(format!("throughput_rust_{config}.json"))
}

fn write_worker_result(case_dir: &Path, config: &str, result: &WorkerResult) -> Result<()> {
    let output_path = worker_output_path(case_dir, config);
    let json = serde_json::to_vec_pretty(result).context("serialize Rust throughput result")?;
    std::fs::write(&output_path, json)
        .with_context(|| format!("write Rust throughput result {}", output_path.display()))
}

fn skipped_gpu_result(case_dir: &Path, reason: &str) -> Result<()> {
    let case_id = case_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("case directory must end in a valid UTF-8 case id")?;
    let result = WorkerResult {
        config: "rust-gpu".to_string(),
        case_id: case_id.to_string(),
        phase: "qc_core",
        mean_s: 0.0,
        std_s: 0.0,
        events: 0,
        channels: 0,
        events_per_s: 0.0,
        pct_removed: 0.0,
        reps: 0,
        rayon_num_threads: std::env::var("RAYON_NUM_THREADS")
            .unwrap_or_else(|_| "default".to_string()),
        rustc: rustc_version(),
        peacoqc_rs_version: peacoqc_rs::VERSION,
        skipped: true,
        reason: Some(reason.to_string()),
    };
    write_worker_result(case_dir, "rust-gpu", &result)
}

fn run_worker(
    config_name: &str,
    case_dir: &Path,
    warmup: usize,
    reps: usize,
    include_margins_doublets: bool,
) -> Result<()> {
    anyhow::ensure!(
        matches!(config_name, "rust-cpu-1" | "rust-cpu" | "rust-gpu"),
        "unknown Rust worker config: {config_name}"
    );
    if config_name == "rust-gpu" {
        if !cfg!(feature = "gpu") {
            return skipped_gpu_result(
                case_dir,
                "peacoqc-rs was built without the gpu feature",
            );
        }
        #[cfg(feature = "gpu")]
        {
            // Ensure FORCE_CPU is not inherited for this worker.
            // SAFETY: first env mutation in this worker process before peacoqc/GPU init.
            unsafe {
                std::env::remove_var("PEACOQC_FORCE_CPU");
            }
            if !peacoqc_rs::gpu::is_gpu_available() {
                return skipped_gpu_result(case_dir, "no GPU adapter available at runtime");
            }
        }
    }

    let prepared_fcs = case_dir.join("prepared.fcs");
    let fcs = open_fcs(&prepared_fcs)?;
    let channels = fluorescence_channels(&fcs);
    anyhow::ensure!(
        !channels.is_empty(),
        "prepared FCS has no fluorescence channels for PeacoQC"
    );
    let peacoqc_config = PeacoQCConfig {
        channels,
        determine_good_cells: QCMode::All,
        mad: 6.0,
        it_limit: 0.6,
        consecutive_bins: 5,
        remove_zeros: false,
        ..Default::default()
    };
    let stats = time_qc_core(
        &fcs,
        &peacoqc_config,
        warmup,
        reps,
        include_margins_doublets,
    )?;
    let case_id = case_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("case directory must end in a valid UTF-8 case id")?;
    let phase = if include_margins_doublets {
        "qc_core_margins_doublets"
    } else {
        "qc_core"
    };
    let result = WorkerResult {
        config: config_name.to_string(),
        case_id: case_id.to_string(),
        phase,
        mean_s: stats.mean_s,
        std_s: stats.std_s,
        events: fcs.n_events(),
        channels: peacoqc_config.channels.len(),
        events_per_s: stats.events_per_s,
        pct_removed: stats.pct_removed,
        reps: stats.reps,
        rayon_num_threads: std::env::var("RAYON_NUM_THREADS")
            .unwrap_or_else(|_| "default".to_string()),
        rustc: rustc_version(),
        peacoqc_rs_version: peacoqc_rs::VERSION,
        skipped: false,
        reason: None,
    };
    write_worker_result(case_dir, config_name, &result)
}

fn spawn_worker(
    config: &str,
    case_dir: &Path,
    warmup: usize,
    reps: usize,
    include_margins_doublets: bool,
) -> Result<()> {
    let executable = std::env::current_exe().context("locate compare_with_r executable")?;
    let mut child = Command::new(&executable);
    child
        .arg("--config")
        .arg(config)
        .arg("--case-dir")
        .arg(case_dir)
        .arg("--warmup")
        .arg(warmup.to_string())
        .arg("--reps")
        .arg(reps.to_string());
    if include_margins_doublets {
        child.arg("--include-margins-doublets");
    }
    if config == "rust-cpu-1" {
        child.env("RAYON_NUM_THREADS", "1");
        child.env("PEACOQC_FORCE_CPU", "1");
    } else if config == "rust-cpu" {
        child.env_remove("RAYON_NUM_THREADS");
        child.env("PEACOQC_FORCE_CPU", "1");
    } else if config == "rust-gpu" {
        child.env_remove("PEACOQC_FORCE_CPU");
    }
    let status = child
        .status()
        .with_context(|| format!("spawn {config} throughput worker"))?;
    anyhow::ensure!(
        status.success(),
        "{config} throughput worker failed with {status}"
    );
    Ok(())
}

fn r_script_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/compare_with_r.R")
}

fn find_rscript() -> Option<PathBuf> {
    which_binary("Rscript")
}

fn which_binary(name: &str) -> Option<PathBuf> {
    let output = Command::new("which").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn fluorescence_channels(fcs: &Fcs) -> Vec<String> {
    // Prefer FL{n}-A when present (synthetic fixtures); otherwise use PeacoQCData
    // fluorescence detection so real spectral/unmixed names work.
    let fl_named: Vec<String> = fcs
        .channel_names()
        .into_iter()
        .filter(|name| is_fl_channel(name))
        .collect();
    if !fl_named.is_empty() {
        fl_named
    } else {
        fcs.get_fluorescence_channels()
    }
}

fn spawn_r_worker(case_dir: &Path, warmup: usize, reps: usize, phase: &str) -> Result<()> {
    let rscript = find_rscript().context(
        "Rscript not found on PATH; install R or pass --rust-only to skip the R baseline",
    )?;
    let script = r_script_path();
    anyhow::ensure!(
        script.is_file(),
        "R companion script missing at {}",
        script.display()
    );

    let prepared = case_dir.join("prepared.fcs");
    let fcs = open_fcs(&prepared)?;
    let channels = fluorescence_channels(&fcs);
    anyhow::ensure!(
        !channels.is_empty(),
        "prepared FCS has no FL{{n}}-A channels for R PeacoQC"
    );
    let out_json = case_dir.join(if phase == "e2e" {
        "throughput_r_e2e.json"
    } else {
        "throughput_r.json"
    });

    let mut cmd = Command::new(&rscript);
    cmd.arg(&script)
        .arg("--case-dir")
        .arg(case_dir)
        .arg("--warmup")
        .arg(warmup.to_string())
        .arg("--reps")
        .arg(reps.to_string())
        .arg("--channels")
        .arg(channels.join(","))
        .arg("--out-json")
        .arg(&out_json)
        .arg("--phase")
        .arg(phase);
    if phase == "qc_core_margins_doublets" {
        cmd.arg("--include-margins-doublets");
    }
    let status = cmd.status().context("spawn Rscript PeacoQC companion")?;
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        if code == 2 {
            anyhow::bail!(
                "R companion exited 2 (missing PeacoQC/flowCore). Install with BiocManager::install('PeacoQC') or use --rust-only"
            );
        }
        anyhow::bail!("R companion failed with {status}");
    }
    anyhow::ensure!(
        out_json.is_file(),
        "R companion did not write {}",
        out_json.display()
    );
    Ok(())
}

fn run_smoke(
    out: &Path,
    warmup: usize,
    reps: usize,
    gpu: bool,
    rust_only: bool,
    e2e: bool,
    include_margins_doublets: bool,
) -> Result<()> {
    let case = CaseSpec {
        id: "smoke_10k_x5".to_string(),
        n_events: 10_000,
        n_channels: 5,
        prepared_fcs: out.join("cases/smoke_10k_x5/prepared.fcs"),
    };

    write_synthetic_prepared_fcs(&case.prepared_fcs, case.n_events, case.n_channels)?;

    let fcs = open_fcs(&case.prepared_fcs).context("reopen prepared FCS for smoke verification")?;
    anyhow::ensure!(
        fcs.data_frame.height() == case.n_events,
        "smoke fixture event count: expected {}, got {}",
        case.n_events,
        fcs.data_frame.height()
    );

    let case_dir = case
        .prepared_fcs
        .parent()
        .context("prepared FCS must have a case directory")?;
    run_case_workers(
        case_dir,
        warmup,
        reps,
        gpu,
        rust_only,
        e2e,
        include_margins_doublets,
    )?;
    let rows = collect_case_rows(case_dir)?;
    write_report(out, &rows, warmup, reps, rust_only, gpu)?;
    println!("{}: {}", case.id, case.prepared_fcs.display());
    Ok(())
}

fn run_case_workers(
    case_dir: &Path,
    warmup: usize,
    reps: usize,
    gpu: bool,
    rust_only: bool,
    e2e: bool,
    include_margins_doublets: bool,
) -> Result<()> {
    spawn_worker(
        "rust-cpu-1",
        case_dir,
        warmup,
        reps,
        include_margins_doublets,
    )?;
    spawn_worker("rust-cpu", case_dir, warmup, reps, include_margins_doublets)?;
    if gpu {
        spawn_worker("rust-gpu", case_dir, warmup, reps, include_margins_doublets)?;
    }
    if rust_only {
        eprintln!(
            "--rust-only: skipped R PeacoQC baseline for {}",
            case_dir.display()
        );
    } else {
        let phase = if include_margins_doublets {
            "qc_core_margins_doublets"
        } else {
            "qc_core"
        };
        spawn_r_worker(case_dir, warmup, reps, phase)?;
        if e2e {
            spawn_r_worker(case_dir, warmup, reps, "e2e")?;
        }
    }
    Ok(())
}

fn read_timing_row(path: &Path) -> Result<TimingRow> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read timing JSON {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse timing JSON {}", path.display()))
}

fn collect_case_rows(case_dir: &Path) -> Result<Vec<TimingRow>> {
    let mut rows = Vec::new();
    for entry in std::fs::read_dir(case_dir)
        .with_context(|| format!("list case directory {}", case_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with("throughput_") && name.ends_with(".json") {
            rows.push(read_timing_row(&path)?);
        }
    }
    rows.sort_by(|a, b| {
        (&a.case_id, &a.phase, &a.config).cmp(&(&b.case_id, &b.phase, &b.config))
    });
    Ok(rows)
}

fn machine_cpu_string() -> String {
    if cfg!(target_os = "macos") {
        Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown-macos-cpu".to_string())
    } else if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("model name"))
                    .map(|rest| rest.trim_start_matches(':').trim().to_string())
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown-linux-cpu".to_string())
    } else {
        "unknown-cpu".to_string()
    }
}

fn speedup(baseline: f64, candidate: f64) -> Option<f64> {
    if baseline > 0.0 && candidate > 0.0 {
        Some(baseline / candidate)
    } else {
        None
    }
}

fn format_speedup(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{v:.2}×"),
        None => "n/a".to_string(),
    }
}

fn write_report(
    out: &Path,
    rows: &[TimingRow],
    warmup: usize,
    reps: usize,
    rust_only: bool,
    gpu: bool,
) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("create out dir {}", out.display()))?;

    let merged_path = out.join("throughput_merged.json");
    let merged = serde_json::to_vec_pretty(rows).context("serialize merged throughput JSON")?;
    std::fs::write(&merged_path, merged)
        .with_context(|| format!("write {}", merged_path.display()))?;

    let mut by_case: BTreeMap<String, Vec<&TimingRow>> = BTreeMap::new();
    for row in rows {
        by_case.entry(row.case_id.clone()).or_default().push(row);
    }

    let mut md = String::new();
    md.push_str("# PeacoQC Rust vs R throughput report\n\n");
    md.push_str(&format!(
        "- Date (UTC): {}\n",
        chrono_like_utc_now()
    ));
    md.push_str(&format!("- CPU: {}\n", machine_cpu_string()));
    md.push_str(&format!("- OS: {}\n", std::env::consts::OS));
    md.push_str(&format!("- Warmup / reps: {warmup} / {reps}\n"));
    md.push_str(&format!(
        "- Modes: rust_only={rust_only}, gpu_requested={gpu}\n"
    ));
    md.push_str("- Headline phase: `qc_core` (PeacoQC only; load excluded)\n\n");

    for (case_id, case_rows) in &by_case {
        let qc_rows: Vec<&TimingRow> = case_rows
            .iter()
            .copied()
            .filter(|r| r.phase == "qc_core" || r.phase == "qc_core_margins_doublets")
            .collect();
        if qc_rows.is_empty() {
            continue;
        }
        let events = qc_rows[0].events;
        let channels = qc_rows[0].channels;
        md.push_str(&format!(
            "## Case `{case_id}` ({events} events × {channels} FL channels)\n\n"
        ));
        md.push_str(
            "| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |\n",
        );
        md.push_str("|---|---:|---:|---:|---:|---:|---:|\n");

        let r_mean = qc_rows
            .iter()
            .find(|r| r.config == "r" && !r.skipped)
            .map(|r| r.mean_s);
        let rust1_mean = qc_rows
            .iter()
            .find(|r| r.config == "rust-cpu-1" && !r.skipped)
            .map(|r| r.mean_s);

        let order = ["r", "rust-cpu-1", "rust-cpu", "rust-gpu"];
        for config in order {
            if let Some(row) = qc_rows.iter().find(|r| r.config == config) {
                if row.skipped {
                    md.push_str(&format!(
                        "| {config} | skipped | — | — | — | — | — |\n"
                    ));
                    if let Some(reason) = &row.reason {
                        md.push_str(&format!("  - skip reason: {reason}\n"));
                    }
                    continue;
                }
                let vs_r = r_mean.and_then(|base| speedup(base, row.mean_s));
                let vs_1 = rust1_mean.and_then(|base| speedup(base, row.mean_s));
                md.push_str(&format!(
                    "| {} | {:.4} | {:.4} | {:.0} | {:.2} | {} | {} |\n",
                    config,
                    row.mean_s,
                    row.std_s,
                    row.events_per_s,
                    row.pct_removed,
                    format_speedup(vs_r),
                    format_speedup(vs_1)
                ));
            }
        }
        md.push('\n');
    }

    if let Some(r_row) = rows.iter().find(|r| r.config == "r" && !r.skipped) {
        if let Some(v) = &r_row.r_version {
            md.push_str(&format!("- R: {v}\n"));
        }
        if let Some(v) = &r_row.peacoqc_version {
            md.push_str(&format!("- PeacoQC: {v}\n"));
        }
        if let Some(v) = &r_row.flowcore_version {
            md.push_str(&format!("- flowCore: {v}\n"));
        }
    }
    if let Some(rust_row) = rows
        .iter()
        .find(|r| r.config.starts_with("rust-") && !r.skipped)
    {
        if let Some(v) = &rust_row.rustc {
            md.push_str(&format!("- rustc: {v}\n"));
        }
        if let Some(v) = &rust_row.peacoqc_rs_version {
            md.push_str(&format!("- peacoqc-rs: {v}\n"));
        }
    }
    md.push_str("\nSee also `docs/comparison-with-r.md` for fairness notes.\n");

    let report_path = out.join("throughput_report.md");
    std::fs::write(&report_path, md).with_context(|| format!("write {}", report_path.display()))?;
    println!("wrote {}", report_path.display());
    Ok(())
}

fn chrono_like_utc_now() -> String {
    // Avoid adding a chrono dependency to the example; use UTC via `date -u` when available.
    Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-time".to_string())
}

fn run_synthetic_grid(args: &CliArgs, out: &Path) -> Result<Vec<TimingRow>> {
    let mut all_rows = Vec::new();
    for &n_events in &args.events {
        for &n_channels in &args.channels {
            let case_id = format!("synth_{n_events}_x{n_channels}");
            let case_dir = out.join("cases").join(&case_id);
            let prepared = case_dir.join("prepared.fcs");
            eprintln!("preparing {case_id}…");
            write_synthetic_prepared_fcs(&prepared, n_events, n_channels)?;
            run_case_workers(
                &case_dir,
                args.warmup,
                args.reps,
                args.gpu,
                args.rust_only,
                args.e2e,
                args.include_margins_doublets,
            )?;
            all_rows.extend(collect_case_rows(&case_dir)?);
        }
    }
    Ok(all_rows)
}

fn run_real_fcs_cases(args: &CliArgs, out: &Path) -> Result<Vec<TimingRow>> {
    let mut all_rows = Vec::new();
    for (idx, path) in args.fcs_paths.iter().enumerate() {
        // Anonymous case ids only — never embed source path or original filename in artifacts.
        let case_id = format!("real_{:02}", idx + 1);
        let case_dir = out.join("cases").join(&case_id);
        std::fs::create_dir_all(&case_dir)
            .with_context(|| format!("create {}", case_dir.display()))?;
        let prepared = case_dir.join("prepared.fcs");
        // For now, treat caller-supplied FCS as already prepared (analysis space).
        std::fs::copy(path, &prepared).with_context(|| {
            format!(
                "copy input FCS #{} -> {}",
                idx + 1,
                prepared.display()
            )
        })?;
        eprintln!(
            "running real case {case_id} (input #{}, {} bytes)…",
            idx + 1,
            std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
        );
        match run_case_workers(
            &case_dir,
            args.warmup,
            args.reps,
            args.gpu,
            args.rust_only,
            args.e2e,
            args.include_margins_doublets,
        ) {
            Ok(()) => all_rows.extend(collect_case_rows(&case_dir)?),
            Err(err) => {
                eprintln!("case {case_id} failed: {err:#}; continuing");
            }
        }
    }
    Ok(all_rows)
}

fn run_full(args: &CliArgs) -> Result<()> {
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from("target/peacoqc-r-compare/latest"));
    std::fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;

    let run_synthetic = if args.no_synthetic {
        false
    } else if args.synthetic || args.fcs_paths.is_empty() {
        true
    } else {
        args.synthetic
    };

    let mut rows = Vec::new();
    if run_synthetic {
        rows.extend(run_synthetic_grid(args, &out)?);
    }
    if !args.fcs_paths.is_empty() {
        rows.extend(run_real_fcs_cases(args, &out)?);
    }
    anyhow::ensure!(!rows.is_empty(), "no timing rows produced");
    write_report(&out, &rows, args.warmup, args.reps, args.rust_only, args.gpu)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args
        .windows(2)
        .any(|pair| pair[0] == "--config" && pair[1] == "rust-cpu-1")
    {
        // SAFETY: this is the first operation after argv collection, before any
        // library work or threads can observe the process environment.
        unsafe {
            std::env::set_var("RAYON_NUM_THREADS", "1");
        }
    }
    let parsed = parse_args(&args)?;

    if let Some(config) = parsed.config.as_deref() {
        let case_dir = parsed
            .case_dir
            .as_deref()
            .context("--config requires --case-dir <dir>")?;
        run_worker(
            config,
            case_dir,
            parsed.warmup,
            parsed.reps,
            parsed.include_margins_doublets,
        )?;
        return Ok(());
    }

    if parsed.smoke {
        let out_dir = parsed.out.context("--smoke requires --out <dir>")?;
        run_smoke(
            &out_dir,
            parsed.warmup,
            parsed.reps,
            parsed.gpu,
            parsed.rust_only,
            parsed.e2e,
            parsed.include_margins_doublets,
        )?;
        return Ok(());
    }

    if parsed.config.is_none() {
        run_full(&parsed)?;
        return Ok(());
    }

    anyhow::bail!("no mode selected; use --smoke --out <dir>, full grid with --out, or --config with --case-dir")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluorescence_channel_match_is_strict() {
        assert!(is_fl_channel("FL1-A"));
        assert!(is_fl_channel("FL30-A"));
        assert!(!is_fl_channel("Time"));
        assert!(!is_fl_channel("FSC-A"));
        assert!(!is_fl_channel("FL-A"));
        assert!(!is_fl_channel("FL1-H"));
    }

    #[test]
    fn timing_summary_uses_population_standard_deviation() {
        let stats = summarize_times(&[1.0, 2.0, 3.0], 600, 12.5)
            .expect("three positive timings should summarize");

        assert_eq!(stats.mean_s, 2.0);
        assert!((stats.std_s - (2.0_f64 / 3.0).sqrt()).abs() < 1e-12);
        assert_eq!(stats.events_per_s, 300.0);
        assert_eq!(stats.pct_removed, 12.5);
        assert_eq!(stats.reps, 3);
    }

    #[test]
    fn timing_summary_rejects_no_repetitions() {
        assert!(summarize_times(&[], 600, 0.0).is_err());
    }

    #[test]
    fn smoke_overrides_timing_defaults() {
        let args = [
            "compare_with_r",
            "--smoke",
            "--rust-only",
            "--out",
            "/tmp/output",
        ]
        .map(str::to_string);
        let parsed = parse_args(&args).expect("valid smoke arguments should parse");

        assert!(parsed.smoke);
        assert!(parsed.rust_only);
        assert_eq!(parsed.warmup, 0);
        assert_eq!(parsed.reps, 1);
    }
}
