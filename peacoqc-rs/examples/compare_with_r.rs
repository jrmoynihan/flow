//! PeacoQC Rust vs R throughput harness.

use anyhow::{Context, Result};
use flow_fcs::file::Fcs;
use flow_fcs::keyword::{IntegerKeyword, Keyword, MixedKeyword};
use flow_fcs::metadata::Metadata;
use flow_fcs::version::Version;
use flow_fcs::write::write_fcs_file;
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};
use polars::prelude::*;
use serde::Serialize;
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
    out: Option<PathBuf>,
    config: Option<String>,
    case_dir: Option<PathBuf>,
    warmup: usize,
    reps: usize,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            smoke: false,
            rust_only: false,
            gpu: false,
            out: None,
            config: None,
            case_dir: None,
            warmup: 1,
            reps: 5,
        }
    }
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
    let ssc_a: Vec<f32> = (0..n_events)
        .map(|e| (e as f32).mul_add(0.01, 300.0))
        .collect();

    let mut columns = vec![
        Column::new("Time".into(), time),
        Column::new("FSC-A".into(), fsc_a),
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
            other => anyhow::bail!("unknown argument: {other}"),
        }
        i += 1;
    }
    if parsed.smoke {
        parsed.warmup = 0;
        parsed.reps = 1;
    }
    anyhow::ensure!(parsed.reps > 0, "--reps must be greater than zero");
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
) -> Result<TimingStats> {
    for _ in 0..warmup {
        let _ = peacoqc(fcs, config).context("run QC-core warmup")?;
    }

    let mut times = Vec::with_capacity(reps);
    let mut last_pct = 0.0;
    for _ in 0..reps {
        let started = Instant::now();
        let result = peacoqc(fcs, config).context("run timed QC-core repetition")?;
        times.push(started.elapsed().as_secs_f64());
        last_pct = result.percentage_removed;
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

fn run_worker(config_name: &str, case_dir: &Path, warmup: usize, reps: usize) -> Result<()> {
    anyhow::ensure!(
        matches!(config_name, "rust-cpu-1" | "rust-cpu" | "rust-gpu"),
        "unknown Rust worker config: {config_name}"
    );
    if config_name == "rust-gpu" {
        let reason = if cfg!(feature = "gpu") {
            "GPU QC-core timing is not available through the peacoqc API yet"
        } else {
            "peacoqc-rs was built without the gpu feature"
        };
        return skipped_gpu_result(case_dir, reason);
    }

    let prepared_fcs = case_dir.join("prepared.fcs");
    let fcs = open_fcs(&prepared_fcs)?;
    let channels: Vec<String> = fcs
        .channel_names()
        .into_iter()
        .filter(|name| is_fl_channel(name))
        .collect();
    anyhow::ensure!(
        !channels.is_empty(),
        "prepared FCS has no FL{{n}}-A fluorescence channels"
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
    let stats = time_qc_core(&fcs, &peacoqc_config, warmup, reps)?;
    let case_id = case_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("case directory must end in a valid UTF-8 case id")?;
    let result = WorkerResult {
        config: config_name.to_string(),
        case_id: case_id.to_string(),
        phase: "qc_core",
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

fn spawn_worker(config: &str, case_dir: &Path, warmup: usize, reps: usize) -> Result<()> {
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
    if config == "rust-cpu-1" {
        child.env("RAYON_NUM_THREADS", "1");
    } else if config == "rust-cpu" {
        child.env_remove("RAYON_NUM_THREADS");
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

fn run_smoke(out: &Path, warmup: usize, reps: usize, gpu: bool, rust_only: bool) -> Result<()> {
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
    spawn_worker("rust-cpu-1", case_dir, warmup, reps)?;
    spawn_worker("rust-cpu", case_dir, warmup, reps)?;
    if gpu {
        spawn_worker("rust-gpu", case_dir, warmup, reps)?;
    }
    if !rust_only {
        eprintln!("R comparison is not implemented until Task 3; Rust results were written");
    }
    println!("{}: {}", case.id, case.prepared_fcs.display());
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
        run_worker(config, case_dir, parsed.warmup, parsed.reps)?;
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
        )?;
        return Ok(());
    }

    anyhow::bail!("no mode selected; use --smoke --out <dir> or --config with --case-dir")
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
