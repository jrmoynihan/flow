//! Compare Rust TRU-OLS implementation with Julia implementation
//!
//! This example exports data in CSV format that can be read by Julia,
//! runs both implementations, and compares the results.

use anyhow::Result;
use faer_ext::IntoNdarray;
use flow_fcs::Fcs;
use ndarray::Array2;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

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

    println!("🧬 TRU-OLS Rust vs Julia Comparison");
    println!("====================================");
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

    // Run Rust TRU-OLS preprocessing (convert Array2 to faer views)
    println!("\n⚙️  Running Rust TRU-OLS preprocessing...");
    use faer_ext::IntoFaer;
    let mixing_faer = mixing_matrix.view().into_faer();
    let unstained_faer = unstained_data.view().into_faer();
    let cutoffs = CutoffCalculator::calculate(mixing_faer, unstained_faer, 0.995)?;
    let nonspecific =
        NonspecificObservation::calculate(mixing_faer, unstained_faer, autofluorescence_idx)?;

    println!("  Cutoffs calculated: {} values", cutoffs.cutoffs().nrows());
    println!(
        "  Nonspecific observation: {} detectors",
        nonspecific.observation().nrows()
    );

    // Run Rust TRU-OLS unmixing (convert Array2 to Mat for TruOls, use view for unmix)
    println!("\n🔄 Running Rust TRU-OLS unmixing...");
    let mixing_mat = faer::Mat::from_fn(
        mixing_matrix.nrows(),
        mixing_matrix.ncols(),
        |i, j| mixing_matrix[(i, j)],
    );
    let unstained_mat = faer::Mat::from_fn(
        unstained_data.nrows(),
        unstained_data.ncols(),
        |i, j| unstained_data[(i, j)],
    );
    let tru_ols = TruOls::new(mixing_mat, unstained_mat, autofluorescence_idx)?;
    let stained_faer = stained_data.view().into_faer();
    let rust_unmixed = tru_ols.unmix(stained_faer)?;

    println!(
        "  Unmixed: {} events × {} endmembers",
        rust_unmixed.nrows(),
        rust_unmixed.ncols()
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
    let cutoffs_slice: Vec<f64> =
        (0..cutoffs.cutoffs().nrows()).map(|i| cutoffs.cutoffs()[i]).collect();
    export_vector_to_csv(&cutoffs_slice, &rust_cutoffs_path, "cutoff")?;
    println!("  ✓ Rust cutoffs: {}", rust_cutoffs_path.display());

    let rust_nonspecific_path = output_dir.join("rust_nonspecific.csv");
    let nonspecific_slice: Vec<f64> =
        (0..nonspecific.observation().nrows()).map(|i| nonspecific.observation()[i]).collect();
    export_vector_to_csv(&nonspecific_slice, &rust_nonspecific_path, "nonspecific")?;
    println!(
        "  ✓ Rust nonspecific observation: {}",
        rust_nonspecific_path.display()
    );

    let rust_unmixed_path = output_dir.join("rust_unmixed.csv");
    let rust_unmixed_ndarray = rust_unmixed.as_ref().into_ndarray().to_owned();
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

    println!("\n✅ Export complete!");
    println!("\nTo run Julia comparison:");
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
    use faer_ext::IntoNdarray;
    use flow_tru_ols::fcs_integration::extract_detector_data;

    let detector_refs: Vec<&str> = detector_names.iter().map(|s| s.as_str()).collect();
    extract_detector_data(fcs, &detector_refs)
        .map_err(|e| anyhow::anyhow!("Failed to extract detector data: {}", e))
        .map(|m| m.as_ref().into_ndarray().to_owned())
}

fn generate_mixing_matrix_from_controls(
    controls_dir: &PathBuf,
    unstained_fcs: &Fcs,
    sample_fcs: &Fcs,
) -> Result<(Array2<f64>, Vec<String>, Vec<String>)> {
    use flow_tru_ols_cli::{SingleStainConfig, create_mixing_matrix_from_single_stains};

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
    };

    // Call the function to create the mixing matrix
    let (matrix, detector_names_from_func, _, _) = create_mixing_matrix_from_single_stains(
        controls_dir,
        unstained_fcs,
        &detector_names,
        &endmember_names,
        &autofluorescence_name,
        &config,
        true, // auto_gate
        None, // diagnostic_plot_dir
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

    writeln!(file, "using CSV, DataFrames, LinearAlgebra, StatsBase")?;
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
        "mixing_matrix_df = CSV.read(\"{}/mixing_matrix.csv\", DataFrame)",
        output_dir.display()
    )?;
    writeln!(
        file,
        "unstained_df = CSV.read(\"{}/unstained_data.csv\", DataFrame, missingstring=\"\")",
        output_dir.display()
    )?;
    writeln!(
        file,
        "stained_df = CSV.read(\"{}/stained_data.csv\", DataFrame, missingstring=\"\")",
        output_dir.display()
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
    writeln!(file)?;
    writeln!(file, "println(\"Mixing matrix: \", size(mixmat))")?;
    writeln!(file, "println(\"Unstained: \", size(unstained_mat))")?;
    writeln!(file, "println(\"Stained: \", size(stained_mat))")?;
    writeln!(file)?;
    writeln!(file, "# Run Julia TRU-OLS")?;
    writeln!(file, "println(\"\\nRunning Julia TRU-OLS...\")")?;
    writeln!(
        file,
        "neg_abunds, cutoff = mean_unmix(mixmat, unstained_mat, 0.995)"
    )?;
    writeln!(file)?;
    writeln!(file, "# Calculate nonspecific observation")?;
    writeln!(file, "zero_baseline_mat = copy(mixmat)")?;
    writeln!(
        file,
        "zero_baseline_mat[:, end] .= 0.0  # Zero out autofluorescence column"
    )?;
    writeln!(file, "baseline = zero_baseline_mat * neg_abunds")?;
    writeln!(file)?;
    writeln!(file, "# Adjust stained data")?;
    writeln!(file, "new_tube = copy(stained_mat)")?;
    writeln!(file, "for i in 1:size(new_tube, 1)")?;
    writeln!(
        file,
        "    new_tube[i, :] = Vector(stained_mat[i, :]) .- baseline"
    )?;
    writeln!(file, "end")?;
    writeln!(file)?;
    writeln!(file, "# Run TRU-OLS")?;
    writeln!(
        file,
        "# Convert to Array types for TRU_OLS function signature"
    )?;
    writeln!(
        file,
        "# Vector is Array{{T,1}}, convert to ensure type compatibility"
    )?;
    writeln!(file, "cutoff_array = Array{{Float64, 1}}(cutoff)")?;
    writeln!(
        file,
        "endmember_names_array = Array{{String, 1}}(endmember_names)"
    )?;
    writeln!(
        file,
        "unmixed, namel, removed_cols_dict = TRU_OLS(mixmat, new_tube, cutoff_array, endmember_names_array)"
    )?;
    writeln!(file)?;
    writeln!(
        file,
        "# Create complete dataframe (for comparison with Rust results)"
    )?;
    writeln!(
        file,
        "# Note: create_complete_dataframe internally calls mean_unmix and TRU_OLS, but we call them separately above to export intermediate results"
    )?;
    writeln!(
        file,
        "result_df = create_complete_dataframe(mixmat, endmember_names_array, stained_mat, unstained_mat, false, percen=0.995)"
    )?;
    writeln!(file)?;
    writeln!(file, "# Export Julia results")?;
    writeln!(
        file,
        "CSV.write(\"{}/julia_cutoffs.csv\", DataFrame(cutoff=cutoff))",
        output_dir.display()
    )?;
    writeln!(
        file,
        "CSV.write(\"{}/julia_nonspecific.csv\", DataFrame(nonspecific=baseline))",
        output_dir.display()
    )?;
    writeln!(
        file,
        "CSV.write(\"{}/julia_unmixed.csv\", result_df)",
        output_dir.display()
    )?;
    writeln!(file)?;
    writeln!(file, "println(\"\\n✅ Julia TRU-OLS complete!\")")?;
    writeln!(file, "println(\"Results saved to:\")")?;
    writeln!(
        file,
        "println(\"  - {}/julia_cutoffs.csv\")",
        output_dir.display()
    )?;
    writeln!(
        file,
        "println(\"  - {}/julia_nonspecific.csv\")",
        output_dir.display()
    )?;
    writeln!(
        file,
        "println(\"  - {}/julia_unmixed.csv\")",
        output_dir.display()
    )?;

    Ok(())
}
