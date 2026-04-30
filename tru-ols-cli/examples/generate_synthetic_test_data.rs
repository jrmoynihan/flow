//! Example: Generate comprehensive synthetic test data with 25 channels and 10 fluorophores
//!
//! This example creates:
//! - 25 channels (5 per laser: UV, Violet, Blue, Yellow-Green, Red)
//! - 10 fluorophores with realistic spectral signatures
//! - 3 fully-stained samples with varying cell expression patterns
//! - 1 unstained control with autofluorescence in middle UV and Violet channels
//! - 10 single-stain controls (one per fluorophore) with visualization plots

use anyhow::Result;
use std::path::PathBuf;
use tru_ols::synthetic_data::generate_comprehensive_synthetic_data;

fn main() -> Result<()> {
    let output_dir = PathBuf::from("synthetic_test_data");
    let plot_format = "jpg";

    println!("Generating comprehensive synthetic test data...");
    println!("Output directory: {}", output_dir.display());
    println!("  - 25 channels (5 per laser)");
    println!("  - 10 fluorophores");
    println!("  - 3 fully-stained samples");
    println!("  - 1 unstained control");
    println!("  - 10 single-stain controls");

    generate_comprehensive_synthetic_data(&output_dir, plot_format)?;

    println!("\n✓ Successfully generated all synthetic test data!");

    Ok(())
}
