//! Plate-scale end-to-end TRU-OLS timing (same inputs/exports as `compare_with_julia`).
//!
//! Use **`--features e2e_legacy`** with the **pre-optimization** `flow-tru-ols` tree (after
//! `git stash`): runs the historical **`TruOls::new`** path (cutoffs/nonspecific are computed
//! twice: once for timing/export and again inside `new`). Default (no `e2e_legacy`) uses
//! **`TruOls::from_preprocessed`** and matches the optimized pipeline layout.
//!
//! Wall times and throughput are written to `throughput_rust.json` and `throughput_report.md`.

use anyhow::Result;
use flow_fcs::Fcs;
use ndarray::Array2;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn export_matrix_to_csv(
    matrix: &Array2<f64>,
    path: &PathBuf,
    row_names: &[String],
    col_names: &[String],
) -> Result<()> {
    let mut file = File::create(path)?;

    // Write header: first column is row names, then column names
    write!(file, "RowName,")?;
    for (i, col_name) in col_names.iter().enumerate() {
        write!(file, "{}", col_name)?;
        if i < col_names.len() - 1 {
            write!(file, ",")?;
        }
    }
    writeln!(file)?;

    // Write data
    for (row_idx, row_name) in row_names.iter().enumerate() {
        write!(file, "{}", row_name)?;
        for col_idx in 0..matrix.ncols() {
            write!(file, ",{:.10e}", matrix[(row_idx, col_idx)])?;
        }
        writeln!(file)?;
    }

    Ok(())
}

fn export_data_to_csv(data: &Array2<f64>, path: &PathBuf, col_names: &[String]) -> Result<()> {
    let mut file = File::create(path)?;

    // Write header
    for (i, col_name) in col_names.iter().enumerate() {
        write!(file, "{}", col_name)?;
        if i < col_names.len() - 1 {
            write!(file, ",")?;
        }
    }
    writeln!(file)?;

    // Write data
    for row_idx in 0..data.nrows() {
        for col_idx in 0..data.ncols() {
            write!(file, "{:.10e}", data[(row_idx, col_idx)])?;
            if col_idx < data.ncols() - 1 {
                write!(file, ",")?;
            }
        }
        writeln!(file)?;
    }

    Ok(())
}

fn export_vector_to_csv(vector: &[f64], path: &PathBuf, name: &str) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "{}", name)?;
    for &value in vector {
        writeln!(file, "{:.10e}", value)?;
    }
    Ok(())
}

/// Convert `faer::Mat` (same version as `flow-tru-ols`) to ndarray for CSV export without relying
/// on `faer-ext`/`IntoNdarray` (avoids mixing multiple `faer` versions in one crate).
fn faer_mat_f64_to_ndarray(m: &faer::Mat<f64>) -> Array2<f64> {
    let r = m.nrows();
    let c = m.ncols();
    let mut out = Array2::<f64>::zeros((r, c));
    let mr = m.as_ref();
    for j in 0..c {
        for i in 0..r {
            out[(i, j)] = *mr.get(i, j);
        }
    }
    out
}

/// Best-effort CPU description for throughput metadata (same host for Rust vs Julia comparisons).
fn host_cpu_summary() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in s.lines() {
                let line = line.trim();
                if let Some(rest) = line
                    .strip_prefix("model name\t:")
                    .or_else(|| line.strip_prefix("model name :"))
                {
                    return format!("Linux, {}", rest.trim());
                }
            }
        }
        return "Linux (cpu model unknown)".to_string();
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !s.is_empty() {
                    return format!("macOS, {s}");
                }
            }
        }
        return "macOS (CPU model unknown)".to_string();
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

fn rustc_verbose_version() -> Option<String> {
    let out = std::process::Command::new("rustc")
        .args(["-vV"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn throughput_env_snapshot() -> serde_json::Value {
    let keys = [
        "FLOW_TRU_OLS_FORCE_SEQUENTIAL",
        "RAYON_NUM_THREADS",
        "JULIA_NUM_THREADS",
        "OMP_NUM_THREADS",
        "OPENBLAS_NUM_THREADS",
        "MKL_NUM_THREADS",
        "VECLIB_MAXIMUM_THREADS",
        "NUMEXPR_NUM_THREADS",
    ];
    let mut map = serde_json::Map::new();
    for k in keys {
        if let Ok(v) = std::env::var(k) {
            map.insert(k.to_string(), json!(v));
        }
    }
    json!(map)
}

fn write_throughput_rust_json(path: &PathBuf, report: &serde_json::Value) -> Result<()> {
    let mut f = File::create(path)?;
    let s = serde_json::to_string_pretty(report)?;
    writeln!(f, "{s}")?;
    Ok(())
}

fn write_throughput_report_md(
    path: &PathBuf,
    rust_json: &serde_json::Value,
    throughput_rust_path: &PathBuf,
    throughput_julia_path: &PathBuf,
) -> Result<()> {
    let mut f = File::create(path)?;
    writeln!(
        f,
        "# TRU-OLS throughput (compare_with_julia)\n\
         \n\
         This report is generated by the `compare_with_julia` example. It summarizes **wall-clock** \
         times and **events/s** for the Rust run. Run the generated `compare_with_julia.jl` to produce \
         `throughput_julia.json` on the **same machine** when comparing languages.\n\
         \n\
         ## Fair comparison checklist\n\
         \n\
         - Use the **same** exported CSV inputs (this output directory).\n\
         - Set **the same** thread limits for BLAS and Rayon when you care about apples-to-apples \
         (for example `OMP_NUM_THREADS`, `OPENBLAS_NUM_THREADS` / `MKL_NUM_THREADS`, `RAYON_NUM_THREADS`).\n\
         - Note **Julia** typically uses **libblastrampoline**; **Rust** `flow-tru-ols` defaults to \
         **faer** (pure Rust). If you build `flow-tru-ols` with the `blas` feature, OpenBLAS-backed paths \
         may apply where the crate uses `ndarray-linalg`.\n\
         - Record **hardware** (CPU model, RAM) and **OS** below if you publish numbers.\n\
         \n\
         | Field | Where to record |\n\
         |-------|-----------------|\n\
         | CPU / machine | `throughput_rust.json` → `environment.cpu` (best-effort) + your model string |\n\
         | Rust linear algebra | default: faer; optional: `flow-tru-ols` `blas` feature + OpenBLAS |\n\
         | Julia BLAS | stdout when running `compare_with_julia.jl`, or `julia_blas_info.txt` |\n\
         | Thread env | `throughput_rust.json` → `environment.relevant_env` |\n\
         \n\
         ## Files\n\
         \n\
         - `{}` — Rust timings and environment snapshot\n\
         - `{}` — Julia timings (after `julia compare_with_julia.jl`)\n\
         \n\
         ## Rust summary (embedded)\n\
         \n\
         ```json\n\
         {}\n\
         ```\n",
        throughput_rust_path.display(),
        throughput_julia_path.display(),
        serde_json::to_string_pretty(rust_json)?
    )?;
    Ok(())
}

fn main() -> Result<()> {
    use flow_tru_ols::preprocessing::{CutoffCalculator, NonspecificObservation};
    use flow_tru_ols::unmixing::TruOls;

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Usage: {} <stained.fcs> <unstained.fcs> <mixing_matrix.csv|controls_dir> <output_dir> [detectors] [endmembers] [autofluorescence]",
            args[0]
        );
        eprintln!("\nExamples:");
        eprintln!("  # With existing mixing matrix CSV:");
        eprintln!(
            "  {} sample.fcs unstained.fcs matrix.csv comparison_output/",
            args[0]
        );
        eprintln!("  # With single-stain controls directory (will generate mixing matrix):");
        eprintln!(
            "  {} sample.fcs unstained.fcs controls_dir/ comparison_output/",
            args[0]
        );
        eprintln!(
            "\nIf detectors/endmembers/autofluorescence not provided, they will be auto-detected"
        );
        std::process::exit(1);
    }

    let stained_path = &args[1];
    let unstained_path = &args[2];
    let mixing_matrix_input = &args[3];
    let output_dir = PathBuf::from(&args[4]);

    // Create output directory
    std::fs::create_dir_all(&output_dir)?;

    println!("🧬 TRU-OLS e2e plate throughput (`e2e_plate_throughput`)");
    println!("=========================================================");
    println!("Stained sample: {}", stained_path);
    println!("Unstained control: {}", unstained_path);
    println!("Mixing matrix/controls: {}", mixing_matrix_input);
    println!("Output directory: {}", output_dir.display());

    // Load FCS files
    println!("\n📂 Loading FCS files...");
    let stained_fcs = Fcs::open(stained_path)?;
    let unstained_fcs = Fcs::open(unstained_path)?;

    // Determine if input is a CSV file or directory (controls)
    let mixing_matrix_input_path = PathBuf::from(mixing_matrix_input);
    let (mixing_matrix, detector_names_from_matrix, endmember_names_from_matrix) =
        if mixing_matrix_input_path.is_file()
            && mixing_matrix_input_path
                .extension()
                .and_then(|s| s.to_str())
                == Some("csv")
        {
            // Load from CSV
            println!("📊 Loading mixing matrix from CSV...");
            let matrix = load_mixing_matrix_csv(&mixing_matrix_input_path)?;
            let endmembers = read_csv_header(&mixing_matrix_input_path)?;
            (matrix, Vec::new(), endmembers)
        } else if mixing_matrix_input_path.is_dir() {
            // Generate from single-stain controls
            println!("📊 Generating mixing matrix from single-stain controls...");
            generate_mixing_matrix_from_controls(
                &mixing_matrix_input_path,
                &unstained_fcs,
                &stained_fcs,
            )?
        } else {
            return Err(anyhow::anyhow!(
                "Mixing matrix input must be either:\n\
             - A CSV file (e.g., mixing_matrix.csv)\n\
             - A directory containing single-stain controls (e.g., controls/)\n\n\
             Provided: {}",
                mixing_matrix_input
            ));
        };

    // Get detector and endmember names
    let detector_names: Vec<String> = if args.len() > 5 {
        args[5].split(',').map(|s| s.trim().to_string()).collect()
    } else if !detector_names_from_matrix.is_empty() {
        detector_names_from_matrix
    } else {
        // Auto-detect from FCS file
        stained_fcs
            .get_parameter_names_from_dataframe()
            .into_iter()
            .filter(|name| !["FSC-A", "FSC-H", "SSC-A", "Time"].contains(&name.as_str()))
            .collect()
    };

    let endmember_names: Vec<String> = if args.len() > 6 {
        args[6].split(',').map(|s| s.trim().to_string()).collect()
    } else if !endmember_names_from_matrix.is_empty() {
        endmember_names_from_matrix
    } else {
        return Err(anyhow::anyhow!(
            "Endmember names must be provided or auto-detected from mixing matrix"
        ));
    };

    let autofluorescence_name = if args.len() > 7 {
        args[7].clone()
    } else {
        "Autofluorescence".to_string()
    };

    let autofluorescence_idx = endmember_names
        .iter()
        .position(|name| name == &autofluorescence_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Autofluorescence '{}' not found in endmembers",
                autofluorescence_name
            )
        })?;

    println!(
        "  Detectors: {} ({})",
        detector_names.len(),
        detector_names.join(", ")
    );
    println!(
        "  Endmembers: {} ({})",
        endmember_names.len(),
        endmember_names.join(", ")
    );
    println!("  Autofluorescence index: {}", autofluorescence_idx);

    // Extract detector data
    println!("\n🔍 Extracting detector data...");
    let stained_data = extract_detector_data(&stained_fcs, &detector_names)?;
    let unstained_data = extract_detector_data(&unstained_fcs, &detector_names)?;

    println!(
        "  Stained: {} events × {} detectors",
        stained_data.nrows(),
        stained_data.ncols()
    );
    println!(
        "  Unstained: {} events × {} detectors",
        unstained_data.nrows(),
        unstained_data.ncols()
    );

    // Run Rust TRU-OLS: use `faer::Mat` from the same crate as `flow-tru-ols` (workspace faer 0.24).
    let mixing_mat = faer::Mat::from_fn(mixing_matrix.nrows(), mixing_matrix.ncols(), |i, j| {
        mixing_matrix[(i, j)]
    });
    let unstained_mat =
        faer::Mat::from_fn(unstained_data.nrows(), unstained_data.ncols(), |i, j| {
            unstained_data[(i, j)]
        });

    let (rust_unmixed, cutoffs, nonspecific, t_preprocess, t_tru_ols_build, t_unmix) = {
        #[cfg(feature = "e2e_legacy")]
        {
            println!("\n⚙️  Running Rust TRU-OLS preprocessing (e2e_legacy)...");
            let t_preprocess = Instant::now();
            let cutoffs =
                CutoffCalculator::calculate(mixing_mat.as_ref(), unstained_mat.as_ref(), 0.995)?;
            let nonspecific = NonspecificObservation::calculate(
                mixing_mat.as_ref(),
                unstained_mat.as_ref(),
                autofluorescence_idx,
            )?;
            let t_preprocess = t_preprocess.elapsed();

            println!("  Cutoffs calculated: {} values", cutoffs.cutoffs().nrows());
            println!(
                "  Nonspecific observation: {} detectors",
                nonspecific.observation().nrows()
            );

            println!("\n🔄 Running Rust TRU-OLS unmixing (TruOls::new)...");
            let t_tru_ols_build = Instant::now();
            let tru_ols = TruOls::new(mixing_mat, unstained_mat, autofluorescence_idx)?;
            let t_tru_ols_build = t_tru_ols_build.elapsed();

            let stained_mat =
                faer::Mat::from_fn(stained_data.nrows(), stained_data.ncols(), |i, j| {
                    stained_data[(i, j)]
                });
            let t_unmix = Instant::now();
            let rust_unmixed = tru_ols.unmix(stained_mat.as_ref())?;
            let t_unmix = t_unmix.elapsed();
            (
                rust_unmixed,
                cutoffs,
                nonspecific,
                t_preprocess,
                t_tru_ols_build,
                t_unmix,
            )
        }
        #[cfg(not(feature = "e2e_legacy"))]
        {
            println!("\n⚙️  Running Rust TRU-OLS preprocessing...");
            let t_preprocess = Instant::now();
            let cutoffs =
                CutoffCalculator::calculate(mixing_mat.as_ref(), unstained_mat.as_ref(), 0.995)?;
            let nonspecific = NonspecificObservation::calculate(
                mixing_mat.as_ref(),
                unstained_mat.as_ref(),
                autofluorescence_idx,
            )?;
            let t_preprocess = t_preprocess.elapsed();

            println!("  Cutoffs calculated: {} values", cutoffs.cutoffs().nrows());
            println!(
                "  Nonspecific observation: {} detectors",
                nonspecific.observation().nrows()
            );

            println!("\n🔄 Running Rust TRU-OLS unmixing (from_preprocessed)...");
            let t_tru_ols_build = Instant::now();
            let tru_ols = TruOls::from_preprocessed(
                mixing_mat,
                unstained_mat,
                cutoffs.cutoffs().clone(),
                nonspecific.observation().clone(),
                autofluorescence_idx,
            )?;
            let t_tru_ols_build = t_tru_ols_build.elapsed();

            let stained_mat =
                faer::Mat::from_fn(stained_data.nrows(), stained_data.ncols(), |i, j| {
                    stained_data[(i, j)]
                });
            let t_unmix = Instant::now();
            let rust_unmixed = tru_ols.unmix(stained_mat.as_ref())?;
            let t_unmix = t_unmix.elapsed();
            (
                rust_unmixed,
                cutoffs,
                nonspecific,
                t_preprocess,
                t_tru_ols_build,
                t_unmix,
            )
        }
    };

    let stained_events = stained_data.nrows();
    let unstained_events = unstained_data.nrows();
    let t_rust_core = t_preprocess + t_tru_ols_build + t_unmix;
    let sec = |d: std::time::Duration| d.as_secs_f64();
    let unmix_eps = sec(t_unmix).max(f64::EPSILON);
    let preprocess_eps = sec(t_preprocess).max(f64::EPSILON);
    let core_eps = sec(t_rust_core).max(f64::EPSILON);

    println!(
        "  Unmixed: {} events × {} endmembers",
        rust_unmixed.nrows(),
        rust_unmixed.ncols()
    );
    println!("\n⏱️  Rust wall times (algorithm, excluding I/O)");
    println!(
        "  preprocess (cutoffs + nonspecific): {:.6} s",
        sec(t_preprocess)
    );
    #[cfg(feature = "e2e_legacy")]
    println!(
        "  TruOls build (new):                 {:.6} s",
        sec(t_tru_ols_build)
    );
    #[cfg(not(feature = "e2e_legacy"))]
    println!(
        "  TruOls build (from_preprocessed):   {:.6} s",
        sec(t_tru_ols_build)
    );
    println!(
        "  unmix:                              {:.6} s",
        sec(t_unmix)
    );
    println!(
        "  preprocess + new + unmix (core):    {:.6} s",
        sec(t_rust_core)
    );
    println!(
        "  throughput unmix:                   {:.0} stained events/s",
        stained_events as f64 / unmix_eps
    );
    println!(
        "  throughput preprocess (unstained):  {:.0} unstained events/s",
        unstained_events as f64 / preprocess_eps
    );
    println!(
        "  throughput core (stained / core):   {:.0} stained events/s",
        stained_events as f64 / core_eps
    );
    println!(
        "  rayon threads (after run):          {}",
        rayon::current_num_threads()
    );

    // Export data for Julia
    println!("\n💾 Exporting data for Julia comparison...");

    // Export mixing matrix
    let matrix_path = output_dir.join("mixing_matrix.csv");
    export_matrix_to_csv(
        &mixing_matrix,
        &matrix_path,
        &detector_names,
        &endmember_names,
    )?;
    println!("  ✓ Mixing matrix: {}", matrix_path.display());

    // Export unstained control data
    let unstained_path = output_dir.join("unstained_data.csv");
    export_data_to_csv(&unstained_data, &unstained_path, &detector_names)?;
    println!("  ✓ Unstained data: {}", unstained_path.display());

    // Export stained sample data
    let stained_path = output_dir.join("stained_data.csv");
    export_data_to_csv(&stained_data, &stained_path, &detector_names)?;
    println!("  ✓ Stained data: {}", stained_path.display());

    // Export Rust results
    let rust_cutoffs_path = output_dir.join("rust_cutoffs.csv");
    let cutoffs_slice: Vec<f64> = (0..cutoffs.cutoffs().nrows())
        .map(|i| cutoffs.cutoffs()[i])
        .collect();
    export_vector_to_csv(&cutoffs_slice, &rust_cutoffs_path, "cutoff")?;
    println!("  ✓ Rust cutoffs: {}", rust_cutoffs_path.display());

    let rust_nonspecific_path = output_dir.join("rust_nonspecific.csv");
    let nonspecific_slice: Vec<f64> = (0..nonspecific.observation().nrows())
        .map(|i| nonspecific.observation()[i])
        .collect();
    export_vector_to_csv(&nonspecific_slice, &rust_nonspecific_path, "nonspecific")?;
    println!(
        "  ✓ Rust nonspecific observation: {}",
        rust_nonspecific_path.display()
    );

    let rust_unmixed_path = output_dir.join("rust_unmixed.csv");
    let rust_unmixed_ndarray = faer_mat_f64_to_ndarray(&rust_unmixed);
    export_data_to_csv(&rust_unmixed_ndarray, &rust_unmixed_path, &endmember_names)?;
    println!(
        "  ✓ Rust unmixed abundances: {}",
        rust_unmixed_path.display()
    );

    // Export endmember names
    let endmember_names_path = output_dir.join("endmember_names.csv");
    {
        let mut file = File::create(&endmember_names_path)?;
        writeln!(file, "endmember")?;
        for name in &endmember_names {
            writeln!(file, "{}", name)?;
        }
    }
    println!("  ✓ Endmember names: {}", endmember_names_path.display());

    // Create Julia comparison script
    let julia_script_path = output_dir.join("compare_with_julia.jl");
    create_julia_comparison_script(&julia_script_path, &output_dir)?;
    println!(
        "  ✓ Julia comparison script: {}",
        julia_script_path.display()
    );

    let throughput_rust_path = output_dir.join("throughput_rust.json");
    let throughput_julia_path = output_dir.join("throughput_julia.json");
    let throughput_report_md = output_dir.join("throughput_report.md");

    let rust_report = {
        #[cfg(not(feature = "e2e_legacy"))]
        {
            json!({
                "rust": {
                    "wall_seconds": {
                        "preprocess": sec(t_preprocess),
                        "tru_ols_build_from_preprocessed": sec(t_tru_ols_build),
                        "unmix": sec(t_unmix),
                        "preprocess_plus_build_plus_unmix": sec(t_rust_core),
                    },
                    "throughput_events_per_sec": {
                        "unmix_stained": stained_events as f64 / unmix_eps,
                        "preprocess_unstained": unstained_events as f64 / preprocess_eps,
                        "core_stained": stained_events as f64 / core_eps,
                    },
                    "counts": {
                        "stained_events": stained_events,
                        "unstained_events": unstained_events,
                        "detectors": detector_names.len(),
                        "endmembers": endmember_names.len(),
                    },
                    "parallelism": {
                        "rayon_threads_after_run": rayon::current_num_threads(),
                        "parallel_unmix_event_threshold": flow_tru_ols::PARALLEL_UNMIX_THRESHOLD,
                        "parallel_independent_events_threshold": flow_tru_ols::PARALLEL_INDEPENDENT_EVENTS_THRESHOLD,
                    },
                    "code_path": "from_preprocessed",
                },
                "environment": {
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                    "cpu": host_cpu_summary(),
                    "rustc": rustc_verbose_version(),
                    "relevant_env": throughput_env_snapshot(),
                    "tru_ols_cli_version": env!("CARGO_PKG_VERSION"),
                    "linear_algebra": "Default flow-tru-ols uses faer (pure Rust). Optional `blas` feature uses OpenBLAS via ndarray-linalg where applicable.",
                },
            })
        }
        #[cfg(feature = "e2e_legacy")]
        {
            json!({
                "rust": {
                    "wall_seconds": {
                        "preprocess": sec(t_preprocess),
                        "tru_ols_build_new": sec(t_tru_ols_build),
                        "unmix": sec(t_unmix),
                        "preprocess_plus_build_plus_unmix": sec(t_rust_core),
                    },
                    "throughput_events_per_sec": {
                        "unmix_stained": stained_events as f64 / unmix_eps,
                        "preprocess_unstained": unstained_events as f64 / preprocess_eps,
                        "core_stained": stained_events as f64 / core_eps,
                    },
                    "counts": {
                        "stained_events": stained_events,
                        "unstained_events": unstained_events,
                        "detectors": detector_names.len(),
                        "endmembers": endmember_names.len(),
                    },
                    "parallelism": {
                        "rayon_threads_after_run": rayon::current_num_threads(),
                    },
                    "code_path": "TruOls::new (e2e_legacy; cutoffs also recomputed inside new)",
                },
                "environment": {
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                    "cpu": host_cpu_summary(),
                    "rustc": rustc_verbose_version(),
                    "relevant_env": throughput_env_snapshot(),
                    "tru_ols_cli_version": env!("CARGO_PKG_VERSION"),
                    "linear_algebra": "Default flow-tru-ols uses faer (pure Rust). Optional `blas` feature uses OpenBLAS via ndarray-linalg where applicable.",
                },
            })
        }
    };

    write_throughput_rust_json(&throughput_rust_path, &rust_report)?;
    println!(
        "  ✓ Rust throughput metadata: {}",
        throughput_rust_path.display()
    );

    write_throughput_report_md(
        &throughput_report_md,
        &rust_report,
        &throughput_rust_path,
        &throughput_julia_path,
    )?;
    println!(
        "  ✓ Throughput report (markdown): {}",
        throughput_report_md.display()
    );

    println!("\n✅ Export complete!");
    println!(
        "\nTo run Julia comparison (writes {}):",
        throughput_julia_path.display()
    );
    println!("  julia {}", julia_script_path.display());

    Ok(())
}

fn load_mixing_matrix_csv(path: &PathBuf) -> Result<Array2<f64>> {
    use std::io::{BufRead, BufReader};

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Skip header
    let _header = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("Empty CSV file"))??;

    // Read data
    let mut data = Vec::new();
    for line in lines {
        let line = line?;
        let values: Vec<f64> = line
            .split(',')
            .skip(1) // Skip row name
            .map(|s| s.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()?;
        data.push(values);
    }

    if data.is_empty() {
        return Err(anyhow::anyhow!("No data rows in CSV"));
    }

    let nrows = data.len();
    let ncols = data[0].len();

    // Verify all rows have same length
    for (i, row) in data.iter().enumerate() {
        if row.len() != ncols {
            return Err(anyhow::anyhow!(
                "Row {} has {} columns, expected {}",
                i,
                row.len(),
                ncols
            ));
        }
    }

    // Convert to Array2
    let mut matrix = Array2::<f64>::zeros((nrows, ncols));
    for (row_idx, row_data) in data.iter().enumerate() {
        for (col_idx, &value) in row_data.iter().enumerate() {
            matrix[(row_idx, col_idx)] = value;
        }
    }

    Ok(matrix)
}

fn read_csv_header(path: &PathBuf) -> Result<Vec<String>> {
    use std::io::{BufRead, BufReader};

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let header = reader
        .lines()
        .next()
        .ok_or_else(|| anyhow::anyhow!("Empty CSV file"))??;

    let names: Vec<String> = header
        .split(',')
        .skip(1) // Skip "RowName"
        .map(|s| s.trim().to_string())
        .collect();

    Ok(names)
}

fn extract_detector_data(fcs: &Fcs, detector_names: &[String]) -> Result<Array2<f64>> {
    use flow_tru_ols::fcs_integration::extract_detector_data;

    let detector_refs: Vec<&str> = detector_names.iter().map(|s| s.as_str()).collect();
    extract_detector_data(fcs, &detector_refs)
        .map_err(|e| anyhow::anyhow!("Failed to extract detector data: {}", e))
        .map(|m| faer_mat_f64_to_ndarray(&m))
}

fn generate_mixing_matrix_from_controls(
    controls_dir: &PathBuf,
    unstained_fcs: &Fcs,
    sample_fcs: &Fcs,
) -> Result<(Array2<f64>, Vec<String>, Vec<String>)> {
    use tru_ols::{QcCliOptions, SingleStainConfig, create_mixing_matrix_from_single_stains};

    // Auto-detect detectors
    let detector_names: Vec<String> = sample_fcs
        .get_parameter_names_from_dataframe()
        .into_iter()
        .filter(|name| !["FSC-A", "FSC-H", "SSC-A", "Time"].contains(&name.as_str()))
        .collect();

    // Get endmember names from control files
    let mut endmember_names: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(controls_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("fcs") {
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                if !filename.to_lowercase().contains("unstained") {
                    endmember_names.push(filename.to_string());
                }
            }
        }
    }
    endmember_names.sort();

    // Add autofluorescence
    let autofluorescence_name = "Autofluorescence".to_string();
    if !endmember_names.contains(&autofluorescence_name) {
        endmember_names.push(autofluorescence_name.clone());
    }

    // Create config with defaults
    let config = SingleStainConfig {
        peak_detection: true,
        peak_threshold: 0.3,
        peak_bias: 0.5,
        peak_bias_negative: 0.5,
        use_negative_events: false,
        autofluorescence_mode: "universal".to_string(),
        af_weight: 0.7,
        min_negative_events: 100,
        qc_options: QcCliOptions::default(),
    };

    // Call the function to create the mixing matrix
    let (matrix, detector_names_from_func, _, _) = create_mixing_matrix_from_single_stains(
        controls_dir,
        unstained_fcs,
        &detector_names,
        &endmember_names,
        &autofluorescence_name,
        &config,
        None,
        true,  // auto_gate
        false, // debug_control_plots
        None,  // diagnostic_plot_dir
    )?;

    Ok((matrix, detector_names_from_func, endmember_names))
}

fn create_julia_comparison_script(script_path: &PathBuf, output_dir: &PathBuf) -> Result<()> {
    let mut file = File::create(script_path)?;

    // Try to find TRU-OLS.jl - check common locations
    let tru_ols_jl_path = if PathBuf::from("/Users/kfls271/Rust/TRU-OLS/TRU-OLS.jl").exists() {
        "/Users/kfls271/Rust/TRU-OLS/TRU-OLS.jl"
    } else if PathBuf::from("../../TRU-OLS/TRU-OLS.jl").exists() {
        "../../TRU-OLS/TRU-OLS.jl"
    } else {
        // Default fallback - user can update manually
        "/Users/kfls271/Rust/TRU-OLS/TRU-OLS.jl"
    };

    // Forward slashes work on Julia on Windows too
    let out_dir = output_dir.to_string_lossy().replace('\\', "/");

    writeln!(file, "using CSV, DataFrames, LinearAlgebra, StatsBase")?;
    writeln!(
        file,
        "const _COMPARE_OUT = raw\"{}\"",
        out_dir.trim_end_matches('/')
    )?;
    writeln!(file, "# Load TRU-OLS implementation")?;
    writeln!(
        file,
        "# Update this path if TRU-OLS.jl is in a different location"
    )?;
    writeln!(file, "include(\"{}\")", tru_ols_jl_path)?;
    writeln!(file)?;
    writeln!(file, "# Load data")?;
    writeln!(file, "println(\"Loading data...\")")?;
    writeln!(
        file,
        "mixing_matrix_df = CSV.read(joinpath(_COMPARE_OUT, \"mixing_matrix.csv\"), DataFrame)"
    )?;
    writeln!(
        file,
        "unstained_df = CSV.read(joinpath(_COMPARE_OUT, \"unstained_data.csv\"), DataFrame, missingstring=\"\")"
    )?;
    writeln!(
        file,
        "stained_df = CSV.read(joinpath(_COMPARE_OUT, \"stained_data.csv\"), DataFrame, missingstring=\"\")"
    )?;
    writeln!(file)?;
    writeln!(
        file,
        "# Get endmember names from mixing matrix column names (following README pattern)"
    )?;
    writeln!(
        file,
        "endmember_names = names(mixing_matrix_df)[2:end]  # Skip RowName column"
    )?;
    writeln!(file)?;
    writeln!(
        file,
        "# Cast DataFrames to Matrices (following README pattern)"
    )?;
    writeln!(
        file,
        "# CSV has detectors as rows, endmembers as columns - no transpose needed"
    )?;
    writeln!(
        file,
        "mixmat = Matrix{{Float64}}(mixing_matrix_df[!, 2:end])  # detectors × endmembers"
    )?;
    writeln!(file, "unstained_mat = Matrix{{Float64}}(unstained_df)")?;
    writeln!(file, "stained_mat = Matrix{{Float64}}(stained_df)")?;
    writeln!(file, "stained_n = size(stained_mat, 1)")?;
    writeln!(file, "unstained_n = size(unstained_mat, 1)")?;
    writeln!(file)?;
    writeln!(file, "println(\"Mixing matrix: \", size(mixmat))")?;
    writeln!(file, "println(\"Unstained: \", size(unstained_mat))")?;
    writeln!(file, "println(\"Stained: \", size(stained_mat))")?;
    writeln!(file)?;
    writeln!(
        file,
        "# BLAS / runtime (also written to julia_blas_info.txt for records)"
    )?;
    writeln!(file, "blas_lines = String[]")?;
    writeln!(file, "try")?;
    writeln!(
        file,
        "    push!(blas_lines, \"BLAS.get_config(): \" * string(BLAS.get_config()))"
    )?;
    writeln!(file, "catch")?;
    writeln!(
        file,
        "    push!(blas_lines, \"BLAS.vendor(): \" * string(BLAS.vendor()))"
    )?;
    writeln!(file, "end")?;
    writeln!(file, "try")?;
    writeln!(
        file,
        "    push!(blas_lines, \"LinearAlgebra.BLAS.get_num_threads: \" * string(LinearAlgebra.BLAS.get_num_threads()))"
    )?;
    writeln!(file, "catch")?;
    writeln!(file, "end")?;
    writeln!(
        file,
        "push!(blas_lines, \"Sys.CPU_NAME: \" * string(Sys.CPU_NAME))"
    )?;
    writeln!(
        file,
        "open(joinpath(_COMPARE_OUT, \"julia_blas_info.txt\"), \"w\") do io"
    )?;
    writeln!(file, "    for line in blas_lines; println(io, line); end")?;
    writeln!(file, "    println(io, \"Julia VERSION: \", VERSION)")?;
    writeln!(file, "end")?;
    writeln!(file, "println(\"\\n=== Julia runtime / BLAS ===\")")?;
    writeln!(file, "for line in blas_lines; println(line); end")?;
    writeln!(file)?;
    writeln!(file, "println(\"\\nRunning Julia TRU-OLS (timed)...\")")?;
    writeln!(
        file,
        "t_mean_unmix = @elapsed begin; neg_abunds, cutoff = mean_unmix(mixmat, unstained_mat, 0.995); end"
    )?;
    writeln!(file)?;
    writeln!(file, "t_adjust = @elapsed begin")?;
    writeln!(file, "    zero_baseline_mat = copy(mixmat)")?;
    writeln!(
        file,
        "    zero_baseline_mat[:, end] .= 0.0  # Zero out autofluorescence column"
    )?;
    writeln!(file, "    baseline = zero_baseline_mat * neg_abunds")?;
    writeln!(file, "    new_tube = copy(stained_mat)")?;
    writeln!(file, "    for i in 1:size(new_tube, 1)")?;
    writeln!(
        file,
        "        new_tube[i, :] = Vector(stained_mat[i, :]) .- baseline"
    )?;
    writeln!(file, "    end")?;
    writeln!(file, "end")?;
    writeln!(file)?;
    writeln!(file, "cutoff_array = Array{{Float64, 1}}(cutoff)")?;
    writeln!(
        file,
        "endmember_names_array = Array{{String, 1}}(endmember_names)"
    )?;
    writeln!(file, "t_tru_ols = @elapsed begin")?;
    writeln!(
        file,
        "    unmixed, namel, removed_cols_dict = TRU_OLS(mixmat, new_tube, cutoff_array, endmember_names_array)"
    )?;
    writeln!(file, "end")?;
    writeln!(file)?;
    writeln!(
        file,
        "# Full export path (same intermediates as the stepwise run above)"
    )?;
    writeln!(
        file,
        "t_create_complete = @elapsed result_df = create_complete_dataframe(mixmat, endmember_names_array, stained_mat, unstained_mat, false, percen=0.995)"
    )?;
    writeln!(file, "t_pipeline = t_mean_unmix + t_adjust + t_tru_ols")?;
    writeln!(file, "epsf(x) = max(x, floatmin(Float64))")?;
    writeln!(file)?;
    writeln!(file, "println(\"\\n⏱️  Julia wall times (seconds)\")")?;
    writeln!(
        file,
        "println(\"  mean_unmix:                 \", t_mean_unmix)"
    )?;
    writeln!(
        file,
        "println(\"  baseline + adjust stained:  \", t_adjust)"
    )?;
    writeln!(
        file,
        "println(\"  TRU_OLS:                    \", t_tru_ols)"
    )?;
    writeln!(
        file,
        "println(\"  pipeline (sum of above):    \", t_pipeline)"
    )?;
    writeln!(
        file,
        "println(\"  create_complete_dataframe:  \", t_create_complete, \" (duplicate work; for CSV export)\")"
    )?;
    writeln!(
        file,
        "println(\"  throughput TRU_OLS:         \", stained_n / epsf(t_tru_ols), \" stained events/s\")"
    )?;
    writeln!(
        file,
        "println(\"  throughput mean_unmix:      \", unstained_n / epsf(t_mean_unmix), \" unstained events/s\")"
    )?;
    writeln!(
        file,
        "println(\"  throughput pipeline:        \", stained_n / epsf(t_pipeline), \" stained events/s\")"
    )?;
    writeln!(file)?;
    writeln!(
        file,
        "blas_vendor_short = try; string(BLAS.vendor()); catch; \"unknown\"; end"
    )?;
    writeln!(
        file,
        "open(joinpath(_COMPARE_OUT, \"throughput_julia.json\"), \"w\") do io"
    )?;
    writeln!(file, "    println(io, \"{{\")")?;
    writeln!(file, "    println(io, \"  \\\"julia\\\": {{\")")?;
    writeln!(
        file,
        "    println(io, \"    \\\"version\\\": \\\"\", string(VERSION), \"\\\",\")"
    )?;
    writeln!(file, "    println(io, \"    \\\"wall_seconds\\\": {{\")")?;
    writeln!(
        file,
        "    println(io, \"      \\\"mean_unmix\\\": \", t_mean_unmix, \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"adjust_baseline_and_stained\\\": \", t_adjust, \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"tru_ols\\\": \", t_tru_ols, \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"pipeline_comparable\\\": \", t_pipeline, \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"create_complete_dataframe\\\": \", t_create_complete)"
    )?;
    writeln!(file, "    println(io, \"    }},\")")?;
    writeln!(
        file,
        "    println(io, \"    \\\"throughput_events_per_sec\\\": {{\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"tru_ols_stained\\\": \", stained_n / epsf(t_tru_ols), \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"mean_unmix_unstained\\\": \", unstained_n / epsf(t_mean_unmix), \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"pipeline_stained\\\": \", stained_n / epsf(t_pipeline))"
    )?;
    writeln!(file, "    println(io, \"    }},\")")?;
    writeln!(file, "    println(io, \"    \\\"counts\\\": {{\")")?;
    writeln!(
        file,
        "    println(io, \"      \\\"stained_events\\\": \", stained_n, \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"unstained_events\\\": \", unstained_n, \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"detectors\\\": \", size(mixmat, 1), \",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"      \\\"endmembers\\\": \", size(mixmat, 2), \",\")"
    )?;
    writeln!(file, "    println(io, \"    }},\")")?;
    writeln!(
        file,
        "    println(io, \"    \\\"blas_vendor\\\": \\\"\", blas_vendor_short, \"\\\",\")"
    )?;
    writeln!(
        file,
        "    println(io, \"    \\\"blas_detail_file\\\": \\\"julia_blas_info.txt\\\"\")"
    )?;
    writeln!(file, "    println(io, \"  }}\")")?;
    writeln!(file, "    println(io, \"}}\")")?;
    writeln!(file, "end")?;
    writeln!(file)?;
    writeln!(file, "# Export Julia results")?;
    writeln!(
        file,
        "CSV.write(joinpath(_COMPARE_OUT, \"julia_cutoffs.csv\"), DataFrame(cutoff=cutoff))"
    )?;
    writeln!(
        file,
        "CSV.write(joinpath(_COMPARE_OUT, \"julia_nonspecific.csv\"), DataFrame(nonspecific=baseline))"
    )?;
    writeln!(
        file,
        "CSV.write(joinpath(_COMPARE_OUT, \"julia_unmixed.csv\"), result_df)"
    )?;
    writeln!(file)?;
    writeln!(file, "println(\"\\n✅ Julia TRU-OLS complete!\")")?;
    writeln!(file, "println(\"Results saved to:\")")?;
    writeln!(
        file,
        "println(\"  - \", joinpath(_COMPARE_OUT, \"julia_cutoffs.csv\"))"
    )?;
    writeln!(
        file,
        "println(\"  - \", joinpath(_COMPARE_OUT, \"julia_nonspecific.csv\"))"
    )?;
    writeln!(
        file,
        "println(\"  - \", joinpath(_COMPARE_OUT, \"julia_unmixed.csv\"))"
    )?;
    writeln!(
        file,
        "println(\"  - \", joinpath(_COMPARE_OUT, \"throughput_julia.json\"))"
    )?;
    writeln!(
        file,
        "println(\"  - \", joinpath(_COMPARE_OUT, \"julia_blas_info.txt\"))"
    )?;

    Ok(())
}
