//! Convert CSV unmixed abundances to FCS file
//!
//! This example reads the Rust unmixed abundances CSV and creates an FCS file
//! that can be opened in flow cytometry analysis software.

use anyhow::Result;
use flow_fcs::{Fcs, Metadata, Parameter, ParameterMap, TransformType};
use polars::prelude::*;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_csv> <output_fcs>", args[0]);
        eprintln!("\nExample:");
        eprintln!(
            "  {} comparison_output/rust_unmixed.csv comparison_output/rust_unmixed.fcs",
            args[0]
        );
        std::process::exit(1);
    }

    let csv_path = PathBuf::from(&args[1]);
    let fcs_path = PathBuf::from(&args[2]);

    println!("📊 Converting CSV to FCS");
    println!("=========================");
    println!("Input CSV: {}", csv_path.display());
    println!("Output FCS: {}", fcs_path.display());

    // Read CSV file
    println!("\n📂 Reading CSV file...");
    let file = File::open(&csv_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Read header
    let header_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("CSV file is empty"))??;
    let column_names: Vec<String> = header_line
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    println!(
        "  Found {} columns: {}",
        column_names.len(),
        column_names.join(", ")
    );

    // Read data
    let mut data_rows: Vec<Vec<f32>> = Vec::new();
    let mut row_count = 0;
    for line in lines {
        let line = line?;
        let values: Vec<f32> = line
            .split(',')
            .map(|s| s.trim().parse::<f64>().unwrap_or(0.0) as f32)
            .collect();

        if values.len() != column_names.len() {
            eprintln!(
                "Warning: Row {} has {} values, expected {}",
                row_count + 1,
                values.len(),
                column_names.len()
            );
            continue;
        }

        data_rows.push(values);
        row_count += 1;
    }

    println!("  Read {} events", row_count);

    // Create DataFrame columns
    let mut columns: Vec<Column> = Vec::new();
    for (col_idx, col_name) in column_names.iter().enumerate() {
        let values: Vec<f32> = data_rows.iter().map(|row| row[col_idx]).collect();
        columns.push(Column::new(col_name.as_str().into(), values));
    }

    let df = DataFrame::new(row_count, columns)?;
    println!(
        "  Created DataFrame: {} events × {} parameters",
        df.height(),
        df.width()
    );

    // Create parameter map
    let mut params = ParameterMap::default();
    for (idx, col_name) in column_names.iter().enumerate() {
        let param_num = idx + 1;
        let col_name_arc: std::sync::Arc<str> = col_name.as_str().into();
        params.insert(
            col_name_arc.clone(),
            Parameter::new(&param_num, col_name, col_name, &TransformType::Linear),
        );
    }

    // Create metadata
    let mut metadata = Metadata::from_dataframe_and_parameters(&df, &params)?;

    // Add some useful keywords
    // Use the output FCS filename stem (without extension) for $FIL
    let fil_name = fcs_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("rust_ols_unmixed")
        .to_string();
    metadata.insert_string_keyword("$FIL".to_string(), fil_name);
    metadata.insert_string_keyword("$CYT".to_string(), "TRU-OLS Unmixed".to_string());
    metadata.insert_string_keyword(
        "$COM".to_string(),
        format!(
            "Unmixed abundances from Rust TRU-OLS. {} endmembers, {} events.",
            column_names.len(),
            row_count
        ),
    );

    // Create a temporary file for file_access (required but not used when writing)
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_file = std::env::temp_dir().join(format!("temp_{}.tmp", timestamp));
    std::fs::File::create(&temp_file)?;
    let file_access = flow_fcs::file::AccessWrapper::new(temp_file.to_str().unwrap_or(""))?;

    // Create FCS struct
    let fcs = Fcs::for_testing(
        flow_fcs::Header::new(),
        metadata,
        params,
        Arc::new(df),
        file_access,
    );

    // Write FCS file
    println!("\n💾 Writing FCS file...");
    flow_fcs::write::write_fcs_file(fcs, &fcs_path)?;
    println!("  ✓ Saved: {}", fcs_path.display());

    println!("\n✅ Conversion complete!");
    println!(
        "\nYou can now open {} in flow cytometry analysis software.",
        fcs_path.display()
    );

    Ok(())
}
