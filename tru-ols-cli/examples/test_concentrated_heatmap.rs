//! Test script to verify signal heatmap with concentrated dataset
//!
//! Creates a contrived FCS file with events concentrated at specific intensity levels
//! to verify that the heatmap correctly shows white space where no events exist.

use anyhow::{Context, Result};
use flow_fcs::file::AccessWrapper;
use flow_fcs::parameter::ParameterMap;
use flow_fcs::{TransformType, write_fcs_file};
use flow_plots::colormap::ColorMaps;
use polars::prelude::*;
use rand::RngExt;
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tru_ols::synthetic_data::{SpectralSignature, generate_spectral_visualization_plots};

fn main() -> Result<()> {
    let output_dir = PathBuf::from("../../synthetic_test_data/plots/concentrated_test");
    let fcs_output_dir = PathBuf::from("../../synthetic_test_data/controls");

    fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;
    fs::create_dir_all(&fcs_output_dir).with_context(|| {
        format!(
            "Failed to create FCS directory: {}",
            fcs_output_dir.display()
        )
    })?;

    println!("Creating concentrated test dataset...");

    // Create a contrived dataset where events are concentrated at specific intensity levels
    // Channel 1: Events concentrated around intensity 1000 (transformed)
    // Channel 2: Events concentrated around intensity 5000 (transformed)
    // Channel 3: Events concentrated around intensity 10000 (transformed)
    // Channel 4: Very few events at low intensity
    // Channel 5: No events (should show as white space)

    let n_events = 10000;
    let mut rng = rand::rng();

    // Define concentration points (in original signal space, before arcsinh)
    let concentration_points = vec![
        ("Channel1", 1000.0, 200.0),   // Mean=1000, StdDev=200
        ("Channel2", 5000.0, 500.0),   // Mean=5000, StdDev=500
        ("Channel3", 10000.0, 1000.0), // Mean=10000, StdDev=1000
        ("Channel4", 500.0, 50.0),     // Mean=500, StdDev=50 (low intensity)
        ("Channel5", 0.0, 0.0),        // No events
    ];

    let detector_names: Vec<String> = concentration_points
        .iter()
        .map(|(name, _, _)| name.to_string())
        .collect();

    // Create columns for each channel
    let mut columns: Vec<Column> = Vec::new();
    let mut params = ParameterMap::default();

    // Add scatter channels (required for FCS files)
    let fsc_a: Vec<f32> = (0..n_events)
        .map(|_| rng.random_range(50000.0..200000.0))
        .collect();
    let fsc_h: Vec<f32> = (0..n_events)
        .map(|_| rng.random_range(50000.0..200000.0))
        .collect();
    let ssc_a: Vec<f32> = (0..n_events)
        .map(|_| rng.random_range(10000.0..100000.0))
        .collect();

    columns.push(Column::new("FSC-A".into(), fsc_a.clone()));
    columns.push(Column::new("FSC-H".into(), fsc_h.clone()));
    columns.push(Column::new("SSC-A".into(), ssc_a.clone()));

    params.insert(
        "FSC-A".into(),
        flow_fcs::Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
    );
    params.insert(
        "FSC-H".into(),
        flow_fcs::Parameter::new(&2, "FSC-H", "FSC-H", &TransformType::Linear),
    );
    params.insert(
        "SSC-A".into(),
        flow_fcs::Parameter::new(&3, "SSC-A", "SSC-A", &TransformType::Linear),
    );

    // Generate concentrated data for each channel
    let mut param_idx = 4;
    let mut raw_signals = HashMap::new();

    for (channel_name, mean, std_dev) in &concentration_points {
        if *mean == 0.0 {
            // Channel 5: No events
            let values = vec![0.0f32; n_events];
            columns.push(Column::new(channel_name.to_string().into(), values));
            raw_signals.insert(channel_name.to_string(), 0.0);
        } else {
            // Create concentrated distribution
            let dist = Normal::new(*mean as f64, *std_dev as f64)
                .with_context(|| format!("Failed to create distribution for {}", channel_name))?;

            let values: Vec<f32> = (0..n_events)
                .map(|_| dist.sample(&mut rng) as f32)
                .map(|v| v.max(0.0)) // Ensure non-negative
                .collect();

            columns.push(Column::new(channel_name.to_string().into(), values.clone()));

            // Calculate median for raw_signals
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = if sorted.len() % 2 == 0 {
                (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
            } else {
                sorted[sorted.len() / 2]
            };
            raw_signals.insert(channel_name.to_string(), median);
        }

        params.insert(
            channel_name.to_string().into(),
            flow_fcs::Parameter::new(
                &param_idx,
                channel_name,
                channel_name,
                &TransformType::Linear,
            ),
        );
        param_idx += 1;
    }

    // Create DataFrame
    let df = DataFrame::new(n_events, columns).with_context(|| "Failed to create DataFrame")?;

    // Create FCS file
    let fcs_path = fcs_output_dir.join("concentrated_test.fcs");

    // Create metadata using helper function
    let metadata = flow_fcs::Metadata::from_dataframe_and_parameters(&df, &params)
        .with_context(|| "Failed to create metadata from DataFrame and ParameterMap")?;

    // Create temporary empty file for AccessWrapper (it requires a real file to memory-map)
    let temp_file =
        std::env::temp_dir().join(format!("concentrated_test_{}.fcs", std::process::id()));
    {
        // Create an empty temporary file
        std::fs::File::create(&temp_file)
            .with_context(|| format!("Failed to create temporary file: {}", temp_file.display()))?;
    }

    let fcs = flow_fcs::Fcs::for_testing(
        flow_fcs::Header::new(),
        metadata,
        params,
        Arc::new(df),
        AccessWrapper::new(temp_file.to_str().unwrap())
            .with_context(|| "Failed to create AccessWrapper")?,
    );

    // Write FCS file
    write_fcs_file(fcs, &fcs_path)
        .with_context(|| format!("Failed to write FCS file: {}", fcs_path.display()))?;

    // Clean up temporary file (ignore errors)
    let _ = fs::remove_file(&temp_file);

    println!("✓ Created FCS file: {}", fcs_path.display());

    // Create signature (empty - will be calculated from FCS)
    let signature = SpectralSignature {
        name: "ConcentratedTest".to_string(),
        primary_detector: detector_names.first().cloned().unwrap_or_default(),
        detector_signals: HashMap::new(),
    };

    // Generate visualization plots
    println!("Generating heatmap...");
    generate_spectral_visualization_plots(
        &signature,
        &detector_names,
        &raw_signals,
        &output_dir,
        "jpg",
        Some(&fcs_path),
        Some(ColorMaps::Rainbow),
    )
    .with_context(|| "Failed to generate plots")?;

    println!("✓ Generated plots in: {}", output_dir.display());
    println!("\nExpected behavior:");
    println!("  - Channel1: Concentrated around intensity ~1000 (low)");
    println!("  - Channel2: Concentrated around intensity ~5000 (medium)");
    println!("  - Channel3: Concentrated around intensity ~10000 (high)");
    println!("  - Channel4: Very few events at low intensity");
    println!("  - Channel5: Should be completely white (no events)");
    println!("\nEach channel should show:");
    println!("  - Colored rectangles only where events exist");
    println!("  - White space where no events exist");
    println!("  - Rectangles should be small (not full-height bars)");

    Ok(())
}
