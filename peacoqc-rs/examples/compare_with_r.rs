//! PeacoQC Rust vs R throughput harness (scaffold).
//!
//! Task 1: synthetic prepared FCS writer and `--smoke` entry point.

use anyhow::{Context, Result};
use flow_fcs::file::Fcs;
use flow_fcs::keyword::{IntegerKeyword, Keyword, MixedKeyword};
use flow_fcs::metadata::Metadata;
use flow_fcs::version::Version;
use flow_fcs::write::write_fcs_file;
use polars::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

#[allow(dead_code)]
struct CaseSpec {
    id: String,
    n_events: usize,
    n_channels: usize,
    prepared_fcs: PathBuf,
}

fn write_synthetic_prepared_fcs(
    path: &Path,
    n_events: usize,
    n_fl_channels: usize,
) -> Result<()> {
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
        metadata
            .keywords
            .insert(format!("$P{idx}R"), Keyword::Int(IntegerKeyword::PnR(262_144)));
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

fn parse_args(args: &[String]) -> Result<(bool, Option<PathBuf>)> {
    let mut smoke = false;
    let mut out = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--smoke" => smoke = true,
            "--out" => {
                i += 1;
                if i >= args.len() {
                    anyhow::bail!("--out requires a path");
                }
                out = Some(PathBuf::from(&args[i]));
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
        i += 1;
    }
    Ok((smoke, out))
}

fn run_smoke(out: &Path) -> Result<()> {
    let case = CaseSpec {
        id: "smoke_10k_x5".to_string(),
        n_events: 10_000,
        n_channels: 5,
        prepared_fcs: out.join("cases/smoke_10k_x5/prepared.fcs"),
    };

    write_synthetic_prepared_fcs(&case.prepared_fcs, case.n_events, case.n_channels)?;

    let prepared_str = case
        .prepared_fcs
        .to_str()
        .with_context(|| format!("prepared path is not valid UTF-8: {}", case.prepared_fcs.display()))?;
    let fcs = Fcs::open(prepared_str).with_context(|| {
        format!(
            "reopen prepared FCS for smoke verification: {}",
            case.prepared_fcs.display()
        )
    })?;
    anyhow::ensure!(
        fcs.data_frame.height() == case.n_events,
        "smoke fixture event count: expected {}, got {}",
        case.n_events,
        fcs.data_frame.height()
    );

    println!("{}", case.prepared_fcs.display());
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (smoke, out) = parse_args(&args)?;

    if smoke {
        let out_dir = out.context("--smoke requires --out <dir>")?;
        run_smoke(&out_dir)?;
        return Ok(());
    }

    anyhow::bail!("no mode selected; use --smoke --out <dir> for the Task 1 scaffold")
}
