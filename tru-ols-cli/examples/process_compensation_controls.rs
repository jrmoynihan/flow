//! Example: Process Compensation Controls FCS files and generate signal heatmaps
//!
//! This example reads actual FCS files and generates density heatmaps showing
//! the distribution of events across intensity levels for each channel.

use anyhow::{Context, Result};
use flow_fcs::{Fcs, TransformType, Transformable};
use flow_plots::colormap::ColorMaps;
use flow_plots::helpers::density_options_from_fcs;
use flow_plots::options::{AxisOptions, BasePlotOptions, DensityPlotOptions};
use flow_plots::render::RenderConfig;
use flow_plots::{DensityPlot, Plot};
use tru_ols::commands::{apply_mask_to_fcs, clean_fcs_data, isolate_positive_peak_mask};
use tru_ols::synthetic_data::{
    SpectralSignature, generate_signal_heatmap_only,
    generate_spectral_visualization_plots_with_overlay,
};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

fn main() -> Result<()> {
    // Path to the FCS data directory
    let fcs_data_dir = PathBuf::from("../../fcs/src/data");

    // Output directory for plots
    let output_dir = PathBuf::from("../../synthetic_test_data/plots");

    // List of Compensation Controls files to process
    let fcs_files = vec![
        "Compensation Controls_Unstained Control_A01_001.fcs",
        "Compensation Controls_CD4 V660 Stained Control_B01_002.fcs",
        "Compensation Controls_CD8 UV379 Stained Control_C01_003.fcs",
        "Compensation Controls_hTIM3 B510 Stained Control_E01_005.fcs",
        "Compensation Controls_mTIM3 YG585 Stained Control_F01_006.fcs",
        "Compensation Controls_PD-1 UV736 Stained Control_G01_007.fcs",
        "Compensation Controls_LiveDead Fixable Blue UV446 Stained Control_H01_008.fcs",
    ];

    println!("Processing Compensation Controls FCS files...");
    println!("FCS data directory: {}", fcs_data_dir.display());
    println!("Output directory: {}", output_dir.display());
    println!(
        "Using parallel processing with {} threads",
        rayon::current_num_threads()
    );

    // Use Mutex for thread-safe printing
    let print_lock = Mutex::new(());

    // Separate unstained control from single-stain controls
    let mut unstained_files = Vec::new();
    let mut single_stain_files = Vec::new();
    for f in &fcs_files {
        if f.to_uppercase().contains("UNSTAINED") {
            unstained_files.push(f);
        } else {
            single_stain_files.push(f);
        }
    }

    // Process unstained control first (if present) to get baseline autofluorescence medians
    let unstained_medians: Option<HashMap<String, f32>> = if let Some(unstained_file) =
        unstained_files.first()
    {
        println!("\n=== Processing unstained control first ===");
        match process_single_file(
            unstained_file,
            &fcs_data_dir,
            &output_dir,
            &print_lock,
            None, // No unstained medians for unstained itself
        ) {
            Ok(medians) => {
                println!("✓ Unstained control processed successfully");
                Some(medians)
            }
            Err(e) => {
                eprintln!("⚠ Warning: Failed to process unstained control: {}", e);
                None
            }
        }
    } else {
        println!(
            "⚠ No unstained control found - single-stain controls will not have autofluorescence overlay"
        );
        None
    };

    // Process single-stain controls in parallel (with unstained medians overlay if available)
    println!(
        "\n=== Processing {} single-stain controls in parallel ===",
        single_stain_files.len()
    );
    let results: Vec<Result<()>> = single_stain_files
        .par_iter()
        .map(|fcs_filename| {
            let _guard = print_lock.lock().unwrap();
            println!("\nProcessing: {}", fcs_filename);
            drop(_guard); // Release lock before processing

            process_single_file(
                fcs_filename,
                &fcs_data_dir,
                &output_dir,
                &print_lock,
                unstained_medians.as_ref(), // Pass unstained medians for overlay
            )
            .map(|_| ()) // Convert HashMap result to () for compatibility
        })
        .collect();

    // Check for errors (only from single-stain controls, unstained was handled separately)
    let mut errors = Vec::new();
    for (filename, result) in single_stain_files.iter().zip(results.iter()) {
        if let Err(e) = result {
            errors.push((filename, e));
        }
    }

    if !errors.is_empty() {
        eprintln!("\n⚠ Errors occurred during processing:");
        for (filename, error) in &errors {
            eprintln!("  {}: {}", filename, error);
        }
        return Err(anyhow::anyhow!(
            "{} file(s) failed to process",
            errors.len()
        ));
    }

    println!("\n✓ Successfully processed all Compensation Controls files!");
    println!("  Plots saved to: {}", output_dir.display());

    Ok(())
}

/// Process a single FCS file
/// Returns the raw signal medians (for unstained controls, these represent autofluorescence baseline)
fn process_single_file(
    fcs_filename: &str,
    fcs_data_dir: &PathBuf,
    output_dir: &PathBuf,
    print_lock: &Mutex<()>,
    unstained_medians: Option<&HashMap<String, f32>>,
) -> Result<HashMap<String, f32>> {
    {
        let _guard = print_lock.lock().unwrap();
        println!("  [{}] Starting...", fcs_filename);
    }

    let fcs_path = fcs_data_dir.join(fcs_filename);

    if !fcs_path.exists() {
        return Err(anyhow::anyhow!("File not found: {}", fcs_path.display()));
    }

    // Read the FCS file
    let fcs_pre_cleaning = Fcs::open(fcs_path.to_str().unwrap())
        .with_context(|| format!("Failed to read FCS file: {}", fcs_path.display()))?;

    {
        let _guard = print_lock.lock().unwrap();
        println!(
            "  [{}] Cleaning data (removing margins, doublets, and debris)...",
            fcs_filename
        );
    }

    // Clean data using unified cleaning function
    let n_before = fcs_pre_cleaning.data_frame.height();
    let fcs = match clean_fcs_data(&fcs_pre_cleaning) {
        Ok(cleaned) => {
            let n_after = cleaned.data_frame.height();
            let n_removed = n_before - n_after;
            if n_removed > 0 {
                let _guard = print_lock.lock().unwrap();
                println!(
                    "  [{}]    Removed {} events ({:.2}%)",
                    fcs_filename,
                    n_removed,
                    (n_removed as f64 / n_before as f64) * 100.0
                );
            }
            cleaned
        }
        Err(e) => {
            let _guard = print_lock.lock().unwrap();
            eprintln!(
                "  [{}]    Warning: Failed to clean data: {}",
                fcs_filename, e
            );
            fcs_pre_cleaning.clone()
        }
    };

    // Extract detector/channel names from the FCS file
    // Skip scatter channels (FSC-A, FSC-H, SSC-A) and focus on fluorescence channels
    let detector_names: Vec<String> = fcs
        .data_frame
        .get_column_names()
        .iter()
        .filter(|name| {
            let name_upper = name.to_uppercase();
            !name_upper.starts_with("FSC")
                && !name_upper.starts_with("SSC")
                && !name_upper.starts_with("TIME")
        })
        .map(|s| s.to_string())
        .collect();

    if detector_names.is_empty() {
        return Err(anyhow::anyhow!(
            "No detector channels found in {}",
            fcs_filename
        ));
    }

    {
        let _guard = print_lock.lock().unwrap();
        println!(
            "  [{}] Found {} detector channels",
            fcs_filename,
            detector_names.len()
        );
    }

    // Apply arcsinh transformation before peak detection
    let arcsinh_cofactor = 200.0f32;
    let arcsinh_transform = TransformType::Arcsinh {
        cofactor: arcsinh_cofactor,
    };

    // Try to extract primary detector from filename first (faster heuristic)
    // Pattern: Look for channel names like UV379, V660, B510, YG585, etc. in filename
    let filename_upper = fcs_filename.to_uppercase();
    let mut filename_detector: Option<String> = None;

    // Check each detector name to see if it appears in the filename
    for det_name in &detector_names {
        // Extract base name (e.g., "UV379-A" -> "UV379")
        let base_name = det_name.split('-').next().unwrap_or(det_name);
        let base_upper = base_name.to_uppercase();

        // Check if this base name appears in the filename
        if filename_upper.contains(&base_upper) {
            filename_detector = Some(det_name.clone());
            {
                let _guard = print_lock.lock().unwrap();
                println!(
                    "  [{}] Found primary detector from filename: {} (matched '{}' in filename)",
                    fcs_filename, det_name, base_upper
                );
            }
            break;
        }
    }

    // If filename heuristic found a match, use it; otherwise fall back to median-based selection
    let primary_detector = if let Some(det) = filename_detector {
        det
    } else {
        {
            let _guard = print_lock.lock().unwrap();
            println!(
                "  [{}] Filename heuristic failed, using median-based selection...",
                fcs_filename
            );
        }
        // Find primary detector (channel with highest median signal) before peak isolation
        let mut temp_medians = HashMap::new();
        for det_name in &detector_names {
            if let Ok(series) = fcs.data_frame.column(det_name) {
                if let Ok(f32_vals) = series.f32() {
                    let transformed_values: Vec<f32> = f32_vals
                        .iter()
                        .filter_map(|v| v.map(|x| arcsinh_transform.transform(&x)))
                        .collect();
                    if !transformed_values.is_empty() {
                        temp_medians
                            .insert(det_name.clone(), calculate_median(&transformed_values));
                    }
                }
            }
        }

        temp_medians
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| detector_names.first().cloned().unwrap_or_default())
    };

    {
        let _guard = print_lock.lock().unwrap();
        println!(
            "  [{}] Selected primary detector: {}",
            fcs_filename, primary_detector
        );
    }

    // Get transformed values for primary detector to detect peak
    let primary_values_f64: Vec<f64> = if let Ok(series) = fcs.data_frame.column(&primary_detector)
    {
        if let Ok(f32_vals) = series.f32() {
            f32_vals
                .iter()
                .filter_map(|v| v.map(|x| arcsinh_transform.transform(&x) as f64))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // Check if this is an unstained control - these are handled differently
    // Unstained controls represent baseline autofluorescence and should use all events (no peak selection)
    // Rationale: The spectral signature formula is: S_dye = Norm(Median(Pos) - Median(Neg))
    // - Median(Pos): median intensities from positive cluster (single-stain control with peak selection)
    // - Median(Neg): median intensities from negative/unstained control (baseline autofluorescence)
    // We use ALL cleaned events from unstained control to get a representative baseline autofluorescence
    // that will be subtracted from all positive controls. This is more robust than selecting only
    // the "most autofluorescent" peak, as it represents the typical autofluorescence level.
    let is_unstained = fcs_filename.to_uppercase().contains("UNSTAINED");

    // Calculate peak mask (for overlay calculations)
    let peak_mask: Option<Vec<bool>> = if is_unstained {
        None
    } else {
        // For single-stain controls, isolate positive peak (no bias by default - keep all events in peak)
        // peak_bias = 1.0 means no bias (keep all events within the detected peak)
        let peak_bias = 1.0; // Default: no bias (can be overridden via CLI/function args in the future)

        {
            let _guard = print_lock.lock().unwrap();
            println!(
                "  [{}] Isolating positive peak in {} (no bias - keeping all events in peak)...",
                fcs_filename, primary_detector
            );
        }
        Some(
            match isolate_positive_peak_mask(&primary_values_f64, 0.3, peak_bias) {
                Ok(mask) => {
                    let n_kept = mask.iter().filter(|&&keep| keep).count();
                    let n_total = mask.len();
                    if n_kept < n_total {
                        let _guard = print_lock.lock().unwrap();
                        println!(
                            "  [{}]    Kept {} events ({:.2}%) from positive peak",
                            fcs_filename,
                            n_kept,
                            (n_kept as f64 / n_total as f64) * 100.0
                        );
                    }
                    mask
                }
                Err(e) => {
                    let _guard = print_lock.lock().unwrap();
                    eprintln!(
                        "  [{}]    Warning: Failed to isolate positive peak: {}",
                        fcs_filename, e
                    );
                    vec![true; fcs.data_frame.height()] // Keep all events if peak detection fails
                }
            },
        )
    };

    let fcs_peaked = if is_unstained {
        {
            let _guard = print_lock.lock().unwrap();
            println!(
                "  [{}] Unstained control detected - using all cleaned events (no peak selection)",
                fcs_filename
            );
        }
        // For unstained controls, use all cleaned events (no peak selection)
        // This represents the baseline autofluorescence that will be subtracted from positive controls
        fcs.clone()
    } else {
        // Apply peak mask to original (untransformed) FCS data
        match apply_mask_to_fcs(&fcs, peak_mask.as_ref().unwrap()) {
            Ok(peaked) => peaked,
            Err(e) => {
                let _guard = print_lock.lock().unwrap();
                eprintln!(
                    "  [{}]    Warning: Failed to apply peak mask: {}",
                    fcs_filename, e
                );
                fcs.clone()
            }
        }
    };

    // Extract a signature name from the filename
    // e.g., "Compensation Controls_CD4 V660 Stained Control_B01_002.fcs" -> "CD4_V660"
    let signature_name = extract_signature_name(fcs_filename);

    // Calculate signal medians and geometric means for each channel
    // For single-stain controls: use peak-isolated FCS (which has original untransformed values)
    // For unstained controls: use all cleaned events (represents baseline autofluorescence)
    let mut raw_signals = HashMap::new();
    let mut raw_medians_pos = HashMap::new();
    let mut raw_geometric_means_pos = HashMap::new();

    for det_name in &detector_names {
        if let Ok(series) = fcs_peaked.data_frame.column(det_name) {
            if let Ok(f32_vals) = series.f32() {
                let raw_values: Vec<f32> = f32_vals.iter().filter_map(|v| v.map(|x| x)).collect();
                if !raw_values.is_empty() {
                    // Store raw median (this is Median(Pos) for single-stain, Median(Neg) for unstained)
                    let median_raw = calculate_median(&raw_values);
                    raw_signals.insert(det_name.clone(), median_raw);
                    raw_medians_pos.insert(det_name.clone(), median_raw);

                    // Calculate geometric mean (better for log-normal distributions)
                    if let Some(geo_mean_raw) = calculate_geometric_mean(&raw_values) {
                        raw_geometric_means_pos.insert(det_name.clone(), geo_mean_raw);
                    }
                }
            }
        }
    }

    // Calculate normalized signature according to: S_dye = Norm(Median(Pos) - Median(Neg))
    // Also calculate geometric means: GeoMean(Pos) - GeoMean(Neg) (better for log-normal distributions)
    // 1. Subtract unstained medians/geometric means from positive medians/geometric means (in RAW space)
    // 2. Transform the subtracted values
    // 3. Normalize to max = 1.0
    let mut subtracted_medians = HashMap::new();
    let mut transformed_subtracted_medians = HashMap::new();
    let mut subtracted_geometric_means = HashMap::new();
    let mut transformed_subtracted_geometric_means = HashMap::new();

    if is_unstained {
        // For unstained controls, just store raw medians (no subtraction needed)
        // These will be used later for subtraction from positive controls
        for (det_name, &median) in &raw_medians_pos {
            subtracted_medians.insert(det_name.clone(), median);
            // Transform for visualization/plotting purposes
            let transformed = arcsinh_transform.transform(&median);
            transformed_subtracted_medians.insert(det_name.clone(), transformed);
        }
        // Also store geometric means for unstained
        for (det_name, &geo_mean) in &raw_geometric_means_pos {
            subtracted_geometric_means.insert(det_name.clone(), geo_mean);
            let transformed = arcsinh_transform.transform(&geo_mean);
            transformed_subtracted_geometric_means.insert(det_name.clone(), transformed);
        }
    } else if let Some(unstained_meds) = unstained_medians {
        // For single-stain controls: subtract unstained medians (Median(Pos) - Median(Neg))
        // Also calculate geometric means: GeoMean(Pos) - GeoMean(Neg)
        {
            let _guard = print_lock.lock().unwrap();
            println!(
                "  [{}] Calculating spectral signature: Median(Pos) - Median(Neg) and GeoMean(Pos) - GeoMean(Neg)",
                fcs_filename
            );
        }
        for det_name in &detector_names {
            let pos_median = raw_medians_pos.get(det_name).copied().unwrap_or(0.0);
            let neg_median = unstained_meds.get(det_name).copied().unwrap_or(0.0);

            // Subtract in RAW space: Median(Pos) - Median(Neg)
            let subtracted = (pos_median - neg_median).max(0.0); // Ensure non-negative
            subtracted_medians.insert(det_name.clone(), subtracted);

            // Transform the subtracted value for normalization
            let transformed_subtracted = arcsinh_transform.transform(&subtracted);
            transformed_subtracted_medians.insert(det_name.clone(), transformed_subtracted);

            // Calculate geometric mean subtraction if both are available
            if let Some(&pos_geo_mean) = raw_geometric_means_pos.get(det_name) {
                let neg_geo_mean = unstained_meds.get(det_name).copied().unwrap_or(0.0);
                let subtracted_geo = (pos_geo_mean - neg_geo_mean).max(0.0);
                subtracted_geometric_means.insert(det_name.clone(), subtracted_geo);

                let transformed_subtracted_geo = arcsinh_transform.transform(&subtracted_geo);
                transformed_subtracted_geometric_means
                    .insert(det_name.clone(), transformed_subtracted_geo);
            }

            // Debug logging for key channels
            if det_name == &primary_detector
                || det_name.contains("V660")
                || det_name.contains("YG585")
            {
                let _guard = print_lock.lock().unwrap();
                println!(
                    "  [{}]    {}: Pos={:.2}, Neg={:.2}, Subtracted(raw)={:.2}, Transformed={:.2}",
                    fcs_filename,
                    det_name,
                    pos_median,
                    neg_median,
                    subtracted,
                    transformed_subtracted
                );
            }
        }
    } else {
        // No unstained control available - use raw medians without subtraction
        for (det_name, &median) in &raw_medians_pos {
            subtracted_medians.insert(det_name.clone(), median);
            let transformed = arcsinh_transform.transform(&median);
            transformed_subtracted_medians.insert(det_name.clone(), transformed);
        }
        // Also use raw geometric means
        for (det_name, &geo_mean) in &raw_geometric_means_pos {
            subtracted_geometric_means.insert(det_name.clone(), geo_mean);
            let transformed = arcsinh_transform.transform(&geo_mean);
            transformed_subtracted_geometric_means.insert(det_name.clone(), transformed);
        }
    }

    // Normalize: find max across all channels and normalize to 1.0
    let max_signal = transformed_subtracted_medians
        .values()
        .fold(0.0f32, |a, &b| a.max(b));
    let mut normalized_signals = HashMap::new();

    {
        let _guard = print_lock.lock().unwrap();
        println!(
            "  [{}] Normalizing: max transformed signal = {:.2}",
            fcs_filename, max_signal
        );
    }

    for (det_name, &transformed_subtracted) in &transformed_subtracted_medians {
        if max_signal > 0.0 {
            let normalized = (transformed_subtracted / max_signal) as f64;
            normalized_signals.insert(det_name.clone(), normalized);

            // Debug logging for key channels
            if det_name == &primary_detector
                || det_name.contains("V660")
                || det_name.contains("YG585")
            {
                let _guard = print_lock.lock().unwrap();
                println!(
                    "  [{}]    {}: Normalized = {:.3} (transformed={:.2} / max={:.2})",
                    fcs_filename, det_name, normalized, transformed_subtracted, max_signal
                );
            }
        } else {
            normalized_signals.insert(det_name.clone(), 0.0);
        }
    }

    // Create output directory for this signature
    let plot_dir = output_dir.join(&signature_name);
    std::fs::create_dir_all(&plot_dir)
        .with_context(|| format!("Failed to create plot directory: {}", plot_dir.display()))?;

    // Generate debugging plots: FSC-A vs SSC-A and FSC-A vs FSC-H (pre and post cleaning)
    {
        let _guard = print_lock.lock().unwrap();
        println!("  [{}] Generating debugging plots...", fcs_filename);
    }
    match generate_debugging_plots(&fcs_pre_cleaning, &fcs, &plot_dir, &signature_name) {
        Ok(_) => {
            let _guard = print_lock.lock().unwrap();
            println!(
                "  [{}]    ✓ Generated pre/post cleaning plots",
                fcs_filename
            );
        }
        Err(e) => {
            let _guard = print_lock.lock().unwrap();
            eprintln!(
                "  [{}]    ⚠ Warning: Failed to generate debugging plots: {}",
                fcs_filename, e
            );
        }
    }

    // Generate peak isolation debugging plot: primary detector vs SSC-A (pre/post peak isolation)
    // For unstained controls, this shows "all events" vs "all events" (no peak selection applied)
    {
        let _guard = print_lock.lock().unwrap();
        if is_unstained {
            println!(
                "  [{}] Generating distribution plots (unstained - no peak selection)...",
                fcs_filename
            );
        } else {
            println!(
                "  [{}] Generating peak isolation debugging plots...",
                fcs_filename
            );
        }
    }
    // Calculate peak range from peak_mask for pre-peak plot rectangle
    let peak_range = if !is_unstained {
        if let Some(mask) = peak_mask.as_ref() {
            let primary_values = fcs.get_parameter_events_slice(&primary_detector)?;
            let ssc_values = fcs.get_parameter_events_slice("SSC-A")?;

            // Find min/max of events where mask is true
            let mut x_min = f32::MAX;
            let mut x_max = f32::MIN;
            let mut y_min = f32::MAX;
            let mut y_max = f32::MIN;

            for (i, &keep) in mask.iter().enumerate() {
                if keep && i < primary_values.len() && i < ssc_values.len() {
                    x_min = x_min.min(primary_values[i]);
                    x_max = x_max.max(primary_values[i]);
                    y_min = y_min.min(ssc_values[i]);
                    y_max = y_max.max(ssc_values[i]);
                }
            }

            if x_min < f32::MAX {
                Some((x_min, x_max, y_min, y_max))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Calculate median and geometric mean for post-peak data
    let (median_coord, geo_mean_coord) = if !is_unstained {
        let post_x_values = fcs_peaked.get_parameter_events_slice(&primary_detector)?;
        let post_y_values = fcs_peaked.get_parameter_events_slice("SSC-A")?;

        if !post_x_values.is_empty() && !post_y_values.is_empty() {
            // Calculate median
            let x_median = calculate_median(post_x_values);
            let y_median = calculate_median(post_y_values);

            // Calculate geometric mean
            let x_geo_mean = calculate_geometric_mean(post_x_values);
            let y_geo_mean = calculate_geometric_mean(post_y_values);

            let median = Some((x_median, y_median));
            let geo_mean = if let (Some(x), Some(y)) = (x_geo_mean, y_geo_mean) {
                Some((x, y))
            } else {
                None
            };

            (median, geo_mean)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    match generate_peak_isolation_plots(
        &fcs,
        &fcs_peaked,
        &primary_detector,
        &plot_dir,
        &signature_name,
        peak_mask.as_deref(),
        DensityPlotOverlays {
            peak_range,
            median_coord,
            geometric_mean_coord: geo_mean_coord,
        },
    ) {
        Ok(_) => {
            let _guard = print_lock.lock().unwrap();
            if is_unstained {
                println!(
                    "  [{}]    ✓ Generated distribution plots (unstained)",
                    fcs_filename
                );
            } else {
                println!(
                    "  [{}]    ✓ Generated pre/post peak isolation plots",
                    fcs_filename
                );
            }
        }
        Err(e) => {
            let _guard = print_lock.lock().unwrap();
            eprintln!(
                "  [{}]    ⚠ Warning: Failed to generate peak isolation plots: {}",
                fcs_filename, e
            );
        }
    }

    // Verify peak isolation worked correctly (or note that unstained uses all events)
    let n_events_before_peak = fcs.data_frame.height();
    let n_events_after_peak = fcs_peaked.data_frame.height();
    {
        let _guard = print_lock.lock().unwrap();
        if is_unstained {
            println!(
                "  [{}] Using all {} cleaned events for autofluorescence baseline (no peak selection)",
                fcs_filename, n_events_after_peak
            );
        } else {
            println!(
                "  [{}] Peak isolation: {} events before, {} events after ({:.2}% kept)",
                fcs_filename,
                n_events_before_peak,
                n_events_after_peak,
                (n_events_after_peak as f64 / n_events_before_peak as f64) * 100.0
            );
        }
    }

    // Write peak-isolated FCS to temporary file for plot generation (if we have events)
    use flow_fcs::write_fcs_file;
    let temp_fcs_path = plot_dir.join(format!("{}_peaked_temp.fcs", signature_name));
    let fcs_for_plots = if n_events_after_peak > 0 {
        write_fcs_file(fcs_peaked.clone(), &temp_fcs_path).with_context(|| {
            format!(
                "Failed to write peak-isolated FCS: {}",
                temp_fcs_path.display()
            )
        })?;

        // Verify the written file has the correct event count
        let verify_fcs = Fcs::open(temp_fcs_path.to_str().unwrap())
            .with_context(|| "Failed to verify written FCS file")?;
        let verify_count = verify_fcs.data_frame.height();
        if verify_count != n_events_after_peak {
            let _guard = print_lock.lock().unwrap();
            eprintln!(
                "  [{}] ⚠ Warning: Written FCS has {} events, expected {}",
                fcs_filename, verify_count, n_events_after_peak
            );
        } else {
            let _guard = print_lock.lock().unwrap();
            println!(
                "  [{}] ✓ Verified: Written FCS contains {} events",
                fcs_filename, verify_count
            );
        }

        Some(&temp_fcs_path)
    } else {
        let _guard = print_lock.lock().unwrap();
        eprintln!(
            "  [{}] ⚠ Warning: Peak isolation resulted in zero events! Using original cleaned data for plots.",
            fcs_filename
        );
        None
    };

    // Create signature with actual normalized signals from FCS data
    let signature = SpectralSignature {
        name: signature_name.clone(),
        primary_detector: primary_detector.clone(),
        detector_signals: normalized_signals,
    };

    // Generate visualization plots with Rainbow colormap using peak-isolated data
    // Skip normalized signature plot for unstained controls (they don't need normalization)
    if is_unstained {
        // For unstained, only generate heatmap (no normalized signature plot)
        generate_signal_heatmap_only(
            &signature,
            &detector_names,
            &raw_signals,
            &plot_dir,
            "jpg",
            fcs_for_plots,
            None, // Use the default colormap
            None, // No unstained overlay for unstained itself
            None, // No positive medians overlay for unstained
        )
        .with_context(|| format!("Failed to generate heatmap for {}", fcs_filename))?;
    } else {
        // For single-stain controls, generate both heatmap (with overlays) and normalized signature
        // Calculate transformed medians and geometric means for overlay (subtracted values, transformed)
        let transformed_medians_for_overlay: HashMap<String, f32> =
            transformed_subtracted_medians.clone();
        let transformed_geometric_means_for_overlay: HashMap<String, f32> =
            transformed_subtracted_geometric_means.clone();

        generate_spectral_visualization_plots_with_overlay(
            &signature,
            &detector_names,
            &raw_signals,
            &plot_dir,
            "jpg",
            fcs_for_plots,
            None,                                           // Use the default colormap
            unstained_medians, // Overlay unstained medians (dashed grey)
            Some(&transformed_medians_for_overlay), // Overlay positive medians (dashed blue)
            Some(&transformed_geometric_means_for_overlay), // Overlay positive geometric means (solid orange)
        )
        .with_context(|| format!("Failed to generate plots for {}", fcs_filename))?;
    }

    // Clean up temporary file if it was created
    if n_events_after_peak > 0 {
        let _ = std::fs::remove_file(&temp_fcs_path);
    }

    {
        let _guard = print_lock.lock().unwrap();
        println!(
            "  [{}] ✓ Generated plots in: {}",
            fcs_filename,
            plot_dir.display()
        );
    }

    // Return raw signal medians (for unstained, these are the autofluorescence baseline)
    Ok(raw_signals)
}

/// Generate peak isolation debugging plots
/// Overlay data for density plots
struct DensityPlotOverlays {
    /// Peak selection range for pre-peak plots (x_min, x_max, y_min, y_max)
    peak_range: Option<(f32, f32, f32, f32)>,
    /// Median and geometric mean coordinates for post-peak plots (x, y)
    median_coord: Option<(f32, f32)>,
    geometric_mean_coord: Option<(f32, f32)>,
}

/// Creates density plot of primary detector vs SSC-A (pre and post peak isolation)
fn generate_peak_isolation_plots(
    fcs_pre: &Fcs,
    fcs_post: &Fcs,
    primary_detector: &str,
    plot_dir: &PathBuf,
    signature_name: &str,
    _peak_mask: Option<&[bool]>,
    overlays: DensityPlotOverlays,
) -> Result<()> {
    // Get SSC-A range from debris plot (FSC-A vs SSC-A) - use same range
    // The debris plot helper uses default range for scatter channels: 0..=200_000
    let ssc_a_range = 0f32..=200_000f32; // Same as debris plots use for SSC-A

    // Helper function to calculate nice step above max
    let nice_step_above = |max: f32| -> f32 {
        if max.is_infinite() || max.is_nan() || max == 0.0 {
            return 1.0;
        }
        let step_size = 10_f32.powf((max.log10()).floor());
        ((max / step_size).ceil() + 1.0) * step_size
    };

    /// Add overlays to a rendered density plot
    fn add_overlays_to_density_plot(
        plot_bytes: Vec<u8>,
        options: &DensityPlotOptions,
        overlays: &DensityPlotOverlays,
        is_pre_peak: bool,
    ) -> Result<Vec<u8>> {
        use flow_plots::PlotOptions;
        use image::RgbImage;
        use plotters::prelude::*;
        use std::io::Cursor;

        // Decode JPEG to image
        let img = image::ImageReader::new(Cursor::new(&plot_bytes))
            .with_guessed_format()?
            .decode()?
            .to_rgb8();

        let width = img.width();
        let height = img.height();

        // Convert image to pixel buffer
        let mut pixel_buffer: Vec<u8> = img.into_raw();

        let base = options.base();
        let margin = base.margin;
        let x_label_area_size = base.x_label_area_size;
        let y_label_area_size = base.y_label_area_size;

        // Re-create the chart to draw overlays
        {
            let backend = BitMapBackend::with_buffer(&mut pixel_buffer, (width, height));
            let root = backend.into_drawing_area();

            // Create axis specs (same as in render_pixels)
            let (x_spec, y_spec) = flow_plots::create_axis_specs(
                &options.x_axis.range,
                &options.y_axis.range,
                &options.x_axis.transform,
                &options.y_axis.transform,
            )?;

            let mut chart = ChartBuilder::on(&root)
                .margin(margin)
                .x_label_area_size(x_label_area_size)
                .y_label_area_size(y_label_area_size)
                .build_cartesian_2d(x_spec.start..x_spec.end, y_spec.start..y_spec.end)?;

            // Draw overlays
            if is_pre_peak {
                // Draw rectangle for peak selection range
                if let Some((x_min, x_max, y_min, y_max)) = overlays.peak_range {
                    let rect_color = RGBAColor(255, 0, 0, 0.1); // Red rectangle
                    chart
                        .draw_series(std::iter::once(Rectangle::new(
                            [(x_min, y_min), (x_max, y_max)],
                            rect_color.stroke_width(2).filled(),
                        )))
                        .map_err(|e| anyhow::anyhow!("failed to draw peak range rectangle: {e}"))?;

                    // Draw rectangle border
                    chart
                        .draw_series(std::iter::once(Rectangle::new(
                            [(x_min, y_min), (x_max, y_max)],
                            rect_color.stroke_width(2),
                        )))
                        .map_err(|e| anyhow::anyhow!("failed to draw peak range border: {e}"))?;
                }
            } else {
                // Draw markers for median and geometric mean
                let mut legend_items = Vec::new();

                // Draw median marker (circle) - semi-transparent
                if let Some((x, y)) = overlays.median_coord {
                    let median_color = RGBAColor(0, 0, 255, 0.7); // Blue circle, semi-transparent
                    chart
                        .draw_series(std::iter::once(Circle::new(
                            (x, y),
                            6,
                            median_color.filled(),
                        )))
                        .map_err(|e| anyhow::anyhow!("failed to draw median marker: {e}"))?;
                    legend_items.push(("Median", RGBColor(0, 0, 255))); // Use opaque for legend
                }

                // Draw geometric mean marker (square) - semi-transparent
                if let Some((x, y)) = overlays.geometric_mean_coord {
                    // Check if coordinates are within plot bounds
                    if x >= x_spec.start && x <= x_spec.end && y >= y_spec.start && y <= y_spec.end
                    {
                        let geo_mean_color = RGBAColor(255, 165, 0, 0.8); // Orange square, semi-transparent (increased alpha for visibility)
                        // Draw larger square for better visibility
                        chart
                            .draw_series(std::iter::once(Rectangle::new(
                                [(x - 6.0, y - 6.0), (x + 6.0, y + 6.0)],
                                geo_mean_color.filled(),
                            )))
                            .map_err(|e| {
                                anyhow::anyhow!("failed to draw geometric mean marker: {e}")
                            })?;
                        // Also draw border for better visibility
                        chart
                            .draw_series(std::iter::once(Rectangle::new(
                                [(x - 6.0, y - 6.0), (x + 6.0, y + 6.0)],
                                RGBColor(255, 165, 0).stroke_width(1),
                            )))
                            .map_err(|e| {
                                anyhow::anyhow!("failed to draw geometric mean marker border: {e}")
                            })?;
                        legend_items.push(("Geometric Mean", RGBColor(255, 165, 0))); // Use opaque for legend
                    } else {
                        // Log if coordinates are out of bounds
                        eprintln!(
                            "Warning: Geometric mean coordinates ({:.2}, {:.2}) are outside plot bounds [{:.2}-{:.2}, {:.2}-{:.2}]",
                            x, y, x_spec.start, x_spec.end, y_spec.start, y_spec.end
                        );
                    }
                }

                // Draw legend in the plotting area
                if !legend_items.is_empty() {
                    let plotting_area = chart.plotting_area();
                    let (_x_range, _y_range) = plotting_area.get_pixel_range();

                    // Position legend in top-right corner of plotting area (in data coordinates)
                    let legend_x_data = x_spec.end - (x_spec.end - x_spec.start) * 0.15;
                    let legend_y_start = y_spec.end - (y_spec.end - y_spec.start) * 0.15;

                    for (i, (label, color)) in legend_items.iter().enumerate() {
                        let legend_y_data =
                            legend_y_start - (i as f32 * (y_spec.end - y_spec.start) * 0.08);

                        // Draw marker in data coordinates
                        if i == 0 {
                            // Circle for median
                            chart
                                .draw_series(std::iter::once(Circle::new(
                                    (legend_x_data, legend_y_data),
                                    4,
                                    color.filled(),
                                )))
                                .map_err(|e| {
                                    anyhow::anyhow!("failed to draw legend marker: {e}")
                                })?;
                        } else {
                            // Square for geometric mean
                            chart
                                .draw_series(std::iter::once(Rectangle::new(
                                    [
                                        (legend_x_data - 4.0, legend_y_data - 4.0),
                                        (legend_x_data + 4.0, legend_y_data + 4.0),
                                    ],
                                    color.filled(),
                                )))
                                .map_err(|e| {
                                    anyhow::anyhow!("failed to draw legend marker: {e}")
                                })?;
                        }

                        // Draw label text in data coordinates (offset to the right of marker)
                        let label_x = legend_x_data + (x_spec.end - x_spec.start) * 0.02;
                        chart
                            .draw_series(std::iter::once(Text::new(
                                label.to_string(),
                                (label_x, legend_y_data),
                                ("sans-serif", 12).into_font().color(&color),
                            )))
                            .map_err(|e| anyhow::anyhow!("failed to draw legend label: {e}"))?;
                    }
                }
            }

            root.present()
                .map_err(|e| anyhow::anyhow!("failed to present plotters buffer: {e}"))?;
        };

        // Convert back to image and encode
        let img: RgbImage = image::ImageBuffer::from_vec(width, height, pixel_buffer)
            .ok_or_else(|| anyhow::anyhow!("plot image buffer had unexpected size"))?;

        let mut encoded_data = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded_data, 85);
        encoder
            .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
            .map_err(|e| anyhow::anyhow!("failed to JPEG encode plot: {e}"))?;

        Ok(encoded_data)
    }

    // Helper function to create a density plot with custom ranges and labels
    let create_density_plot = |fcs: &Fcs,
                               x_channel: &str,
                               y_channel: &str,
                               title: &str,
                               plot_overlays: Option<&DensityPlotOverlays>,
                               is_pre_peak: bool|
     -> Result<Vec<u8>> {
        fcs.find_parameter(x_channel)
            .with_context(|| format!("Parameter {} not found", x_channel))?;
        fcs.find_parameter(y_channel)
            .with_context(|| format!("Parameter {} not found", y_channel))?;

        // Get event data
        let x_values = fcs.get_parameter_events_slice(x_channel)?;
        let y_values = fcs.get_parameter_events_slice(y_channel)?;

        // Create data pairs
        let data: Vec<(f32, f32)> = x_values
            .iter()
            .zip(y_values.iter())
            .map(|(&x, &y)| (x, y))
            .collect();

        // For peak isolation plots, use a custom arcsinh transform with cofactor ~100 (half of default 200)
        // This expands near-zero events better and gives more space
        let peak_plot_cofactor = 100.0f32;
        let peak_plot_transform = TransformType::Arcsinh {
            cofactor: peak_plot_cofactor,
        };

        // Calculate range in raw space, then ensure it includes space below zero
        let x_range = if x_channel.contains("FSC") || x_channel.contains("SSC") {
            // Scatter channels: use nice step above max
            let x_max = x_values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let x_range_max = nice_step_above(x_max);
            0f32..=x_range_max
        } else {
            // Fluorescence channels: use percentile-based range with padding below zero
            let percentile_range = flow_plots::get_percentile_bounds(x_values, 0.01, 0.99);
            let min_val = *percentile_range.start();
            let max_val = *percentile_range.end();

            // Add padding: extend min below zero to give space for near-zero events
            // Cap at reasonable negative value to avoid issues with inverse transform
            let padded_min = if min_val < 0.0 {
                (min_val * 1.2).max(-5000.0) // Extend 20% below, but cap at -5000
            } else {
                (min_val * 0.8).max(-1000.0) // If min is positive, extend below zero
            };

            // Cap max at reasonable value (e.g., 10M) to avoid inf in inverse transform
            let max_cap = 10_000_000.0f32;
            let padded_max = (max_val * 1.1).min(max_cap);

            padded_min..=padded_max
        };

        // Use SSC-A range from debris plot
        let y_range = ssc_a_range.clone();

        // Build axis options with labels
        // For fluorescence channels, use custom transform with cofactor 100
        // For scatter channels, use linear
        let x_transform = if x_channel.contains("FSC") || x_channel.contains("SSC") {
            TransformType::Linear
        } else {
            peak_plot_transform
        };

        let x_axis = AxisOptions::new()
            .range(x_range)
            .transform(x_transform)
            .label(x_channel.to_string())
            .build()?;

        let y_axis = AxisOptions::new()
            .range(y_range)
            .transform(TransformType::Linear) // SSC-A uses linear transform
            .label(y_channel.to_string())
            .build()?;

        // Build base options with custom title
        let base_options = BasePlotOptions::new()
            .width(800u32)
            .height(600u32)
            .title(title.to_string())
            .build()?;

        // Build density plot options
        let options = DensityPlotOptions::new()
            .base(base_options)
            .x_axis(x_axis)
            .y_axis(y_axis)
            .colormap(ColorMaps::Viridis)
            .build()?;

        // Render plot
        let plot = DensityPlot::new();
        let mut render_config = RenderConfig::default();
        let mut plot_bytes = plot
            .render(data.into(), &options, &mut render_config)
            .with_context(|| format!("Failed to render {} vs {} plot", x_channel, y_channel))?;

        // Add overlays if provided
        if let Some(plot_overlays) = plot_overlays {
            plot_bytes =
                add_overlays_to_density_plot(plot_bytes, &options, plot_overlays, is_pre_peak)?;
        }

        Ok(plot_bytes)
    };

    // Generate primary detector vs SSC-A plots
    if fcs_pre.find_parameter(primary_detector).is_ok() && fcs_pre.find_parameter("SSC-A").is_ok() {
        if fcs_post.find_parameter(primary_detector).is_ok()
            && fcs_post.find_parameter("SSC-A").is_ok()
        {
            // Pre-peak isolation plot
            let pre_bytes = create_density_plot(
                fcs_pre,
                primary_detector,
                "SSC-A",
                &format!(
                    "{} - {} vs SSC-A (Pre-peak isolation)",
                    signature_name, primary_detector
                ),
                Some(&overlays),
                true, // is_pre_peak
            )?;
            let pre_path = plot_dir.join(format!(
                "{}_{}_vs_SSC-A_pre_peak.jpg",
                signature_name, primary_detector
            ));
            fs::write(&pre_path, pre_bytes)
                .with_context(|| format!("Failed to write plot: {}", pre_path.display()))?;

            // Post-peak isolation plot
            let post_bytes = create_density_plot(
                fcs_post,
                primary_detector,
                "SSC-A",
                &format!(
                    "{} - {} vs SSC-A (Post-peak isolation)",
                    signature_name, primary_detector
                ),
                Some(&overlays),
                false, // is_pre_peak
            )?;
            let post_path = plot_dir.join(format!(
                "{}_{}_vs_SSC-A_post_peak.jpg",
                signature_name, primary_detector
            ));
            fs::write(&post_path, post_bytes)
                .with_context(|| format!("Failed to write plot: {}", post_path.display()))?;
        }
    }

    Ok(())
}

/// Calculate geometric mean of positive f32 values
fn calculate_geometric_mean(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }

    // Filter to positive values only
    let positive_values: Vec<f32> = values.iter().filter(|&&v| v > 0.0).copied().collect();

    if positive_values.is_empty() {
        return None;
    }

    // Calculate geometric mean: exp(mean(ln(values)))
    let log_sum: f64 = positive_values.iter().map(|&v| (v as f64).ln()).sum();
    let n = positive_values.len() as f64;
    Some((log_sum / n).exp() as f32)
}

/// Calculate median of a vector of f32 values
fn calculate_median(values: &[f32]) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Extract a signature name from an FCS filename
/// e.g., "Compensation Controls_CD4 V660 Stained Control_B01_002.fcs" -> "CD4_V660"
fn extract_signature_name(filename: &str) -> String {
    // Remove .fcs extension
    let name = filename.strip_suffix(".fcs").unwrap_or(filename);

    // Try to extract the stain name (between "Compensation Controls_" and " Stained Control")
    if let Some(start) = name.find("Compensation Controls_") {
        let after_prefix = &name[start + "Compensation Controls_".len()..];
        if let Some(end) = after_prefix.find(" Stained Control") {
            let stain_name = &after_prefix[..end];
            // Replace spaces with underscores and clean up
            stain_name.replace(' ', "_")
        } else {
            // Fallback: use the part after "Compensation Controls_"
            after_prefix.replace(' ', "_")
        }
    } else {
        // Fallback: use filename without extension
        name.replace(' ', "_")
    }
}

/// Generate debugging plots comparing pre and post cleaning data
/// Creates FSC-A vs SSC-A and FSC-A vs FSC-H density plots
fn generate_debugging_plots(
    fcs_pre: &Fcs,
    fcs_post: &Fcs,
    plot_dir: &PathBuf,
    signature_name: &str,
) -> Result<()> {
    // Helper function to create a density plot
    let create_density_plot =
        |fcs: &Fcs, x_channel: &str, y_channel: &str, title: &str| -> Result<Vec<u8>> {
            let x_param = fcs
                .find_parameter(x_channel)
                .with_context(|| format!("Parameter {} not found", x_channel))?;
            let y_param = fcs
                .find_parameter(y_channel)
                .with_context(|| format!("Parameter {} not found", y_channel))?;

            // Get event data
            let x_values = fcs.get_parameter_events_slice(x_channel)?;
            let y_values = fcs.get_parameter_events_slice(y_channel)?;

            // Create data pairs
            let data: Vec<(f32, f32)> = x_values
                .iter()
                .zip(y_values.iter())
                .map(|(&x, &y)| (x, y))
                .collect();

            // Create plot options
            let mut builder = density_options_from_fcs(fcs, x_param, y_param)?;

            // Build base options with custom title
            let base_options = BasePlotOptions::new()
                .width(800u32)
                .height(600u32)
                .title(title.to_string())
                .build()?;

            let options = builder
                .base(base_options)
                .colormap(ColorMaps::Viridis)
                .build()?;

            // Render plot
            let plot = DensityPlot::new();
            let mut render_config = RenderConfig::default();
            plot.render(data.into(), &options, &mut render_config)
                .with_context(|| format!("Failed to render {} vs {} plot", x_channel, y_channel))
        };

    // Generate FSC-A vs SSC-A plots
    if fcs_pre.find_parameter("FSC-A").is_ok() && fcs_pre.find_parameter("SSC-A").is_ok() {
        if fcs_post.find_parameter("FSC-A").is_ok() && fcs_post.find_parameter("SSC-A").is_ok() {
            // Pre-cleaning plot
            let pre_bytes = create_density_plot(
                fcs_pre,
                "FSC-A",
                "SSC-A",
                &format!("{} - FSC-A vs SSC-A (Pre-cleaning)", signature_name),
            )?;
            let pre_path = plot_dir.join(format!("{}_FSC-A_vs_SSC-A_pre.jpg", signature_name));
            fs::write(&pre_path, pre_bytes)
                .with_context(|| format!("Failed to write plot: {}", pre_path.display()))?;

            // Post-cleaning plot
            let post_bytes = create_density_plot(
                fcs_post,
                "FSC-A",
                "SSC-A",
                &format!("{} - FSC-A vs SSC-A (Post-cleaning)", signature_name),
            )?;
            let post_path = plot_dir.join(format!("{}_FSC-A_vs_SSC-A_post.jpg", signature_name));
            fs::write(&post_path, post_bytes)
                .with_context(|| format!("Failed to write plot: {}", post_path.display()))?;
        }
    }

    // Generate FSC-A vs FSC-H plots
    if fcs_pre.find_parameter("FSC-A").is_ok() && fcs_pre.find_parameter("FSC-H").is_ok() {
        if fcs_post.find_parameter("FSC-A").is_ok() && fcs_post.find_parameter("FSC-H").is_ok() {
            // Pre-cleaning plot
            let pre_bytes = create_density_plot(
                fcs_pre,
                "FSC-A",
                "FSC-H",
                &format!("{} - FSC-A vs FSC-H (Pre-cleaning)", signature_name),
            )?;
            let pre_path = plot_dir.join(format!("{}_FSC-A_vs_FSC-H_pre.jpg", signature_name));
            fs::write(&pre_path, pre_bytes)
                .with_context(|| format!("Failed to write plot: {}", pre_path.display()))?;

            // Post-cleaning plot
            let post_bytes = create_density_plot(
                fcs_post,
                "FSC-A",
                "FSC-H",
                &format!("{} - FSC-A vs FSC-H (Post-cleaning)", signature_name),
            )?;
            let post_path = plot_dir.join(format!("{}_FSC-A_vs_FSC-H_post.jpg", signature_name));
            fs::write(&post_path, post_bytes)
                .with_context(|| format!("Failed to write plot: {}", post_path.display()))?;
        }
    }

    Ok(())
}

/// Generate diagnostic plot for KDE and peak selection
///
/// Creates a plot showing:
/// - KDE density curve
/// - All detected peaks (up to 3)
/// - Selected peak (highlighted)
/// - Peak region bounds (first and second stage)
/// - Histogram of data values
#[allow(dead_code)]
fn generate_kde_diagnostic_plot(
    values: &[f64],
    channel_name: &str,
    output_path: &PathBuf,
) -> Result<()> {
    use flow_density::KernelDensity;
    use plotters::prelude::*;

    // Estimate KDE
    let kde = match KernelDensity::estimate(values, 0.5, 1024) {
        Ok(kde) => kde,
        Err(e) => {
            eprintln!("KDE estimation failed: {:?}", e);
            return Ok(()); // Skip diagnostic plot if KDE fails
        }
    };

    // Find peaks
    let adjusted_threshold = 0.2;
    let mut peaks = kde.find_peaks(adjusted_threshold);
    peaks.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if peaks.len() > 3 {
        peaks.truncate(3);
    }

    // Evaluate candidates (same logic as in isolate_positive_peak_mask)
    struct PeakCandidate {
        x: f64,
        density: f64,
        intensity: f64,
    }

    let mut candidates: Vec<PeakCandidate> = peaks
        .iter()
        .map(|&peak_x| {
            let density = kde.density_at(peak_x);
            let intensity = peak_x;
            PeakCandidate {
                x: peak_x,
                density,
                intensity,
            }
        })
        .collect();

    candidates.sort_by(|a, b| {
        match b
            .density
            .partial_cmp(&a.density)
            .unwrap_or(std::cmp::Ordering::Equal)
        {
            std::cmp::Ordering::Equal => b
                .intensity
                .partial_cmp(&a.intensity)
                .unwrap_or(std::cmp::Ordering::Equal),
            other => other,
        }
    });

    let main_peak = if let Some(cand) = candidates.first() {
        cand.x
    } else {
        return Ok(()); // No peaks found
    };

    // Calculate peak regions (simplified version of isolate_positive_peak_mask logic)
    let mut sorted_all = values.to_vec();
    sorted_all.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q1_idx = sorted_all.len() / 4;
    let q3_idx = (sorted_all.len() * 3) / 4;
    let iqr = sorted_all[q3_idx] - sorted_all[q1_idx];
    let window = iqr * 2.0;

    // First-stage MAD
    let mut peak_region_values: Vec<f64> = values
        .iter()
        .filter(|&&v| (v - main_peak).abs() < window)
        .copied()
        .collect();

    if peak_region_values.is_empty() {
        peak_region_values = values.to_vec();
    }

    peak_region_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_peak_region = peak_region_values[peak_region_values.len() / 2];

    let deviations: Vec<f64> = peak_region_values
        .iter()
        .map(|&v| (v - median_peak_region).abs())
        .collect();
    let mut sorted_deviations = deviations;
    sorted_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad1 = sorted_deviations[sorted_deviations.len() / 2];

    let peak_width1 = 2.0 * mad1;
    let peak_min1 = main_peak - peak_width1;
    let peak_max1 = main_peak + peak_width1;

    // Second-stage MAD
    let mut filtered_values: Vec<f64> = values
        .iter()
        .filter(|&&v| v >= peak_min1 && v <= peak_max1)
        .copied()
        .collect();

    if filtered_values.is_empty() {
        filtered_values = peak_region_values.clone();
    }

    filtered_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_filtered = filtered_values[filtered_values.len() / 2];

    let deviations2: Vec<f64> = filtered_values
        .iter()
        .map(|&v| (v - median_filtered).abs())
        .collect();
    let mut sorted_deviations2 = deviations2;
    sorted_deviations2.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mad2 = sorted_deviations2[sorted_deviations2.len() / 2];

    let peak_width2 = 2.0 * mad2;
    let peak_min2 = main_peak - peak_width2;
    let peak_max2 = main_peak + peak_width2;

    // Create plot
    let root = BitMapBackend::new(output_path, (1200, 800)).into_drawing_area();
    root.fill(&WHITE)?;

    let x_min = kde.x[0];
    let x_max = kde.x[kde.x.len() - 1];
    let y_max = kde.y.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.1;

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("KDE Diagnostic: {}", channel_name),
            ("sans-serif", 30).into_font(),
        )
        .margin(20)
        .x_label_area_size(40)
        .y_label_area_size(60)
        .build_cartesian_2d(x_min..x_max, 0.0..y_max)?;

    chart
        .configure_mesh()
        .x_desc("Intensity")
        .y_desc("Density")
        .draw()?;

    // Draw KDE curve
    let kde_points: Vec<(f64, f64)> = kde
        .x
        .iter()
        .zip(kde.y.iter())
        .map(|(&x, &y)| (x, y))
        .collect();
    chart.draw_series(LineSeries::new(
        kde_points.iter().map(|(x, y)| (*x, *y)),
        RGBColor(0, 0, 255).stroke_width(2),
    ))?;

    // Draw all candidate peaks
    for (i, cand) in candidates.iter().enumerate() {
        let color = if i == 0 {
            RGBColor(255, 0, 0) // Selected peak - red
        } else {
            RGBColor(0, 255, 0) // Other candidates - green
        };

        let peak_density = kde.density_at(cand.x);
        chart.draw_series(std::iter::once(Circle::new(
            (cand.x, peak_density),
            8,
            color.filled(),
        )))?;

        // Label peaks
        chart.draw_series(std::iter::once(Text::new(
            format!("P{}", i + 1),
            (cand.x, peak_density + y_max * 0.05),
            ("sans-serif", 15).into_font().color(&color),
        )))?;
    }

    // Draw first-stage peak region
    let first_stage_density = kde.density_at(peak_min1).max(kde.density_at(peak_max1));
    chart.draw_series(std::iter::once(Rectangle::new(
        [(peak_min1, 0.0), (peak_max1, first_stage_density)],
        RGBAColor(255, 165, 0, 0.2).filled(),
    )))?;

    chart.draw_series(std::iter::once(Rectangle::new(
        [(peak_min1, 0.0), (peak_max1, first_stage_density)],
        RGBColor(255, 165, 0).stroke_width(2),
    )))?;

    // Draw second-stage peak region
    let second_stage_density = kde.density_at(peak_min2).max(kde.density_at(peak_max2));
    chart.draw_series(std::iter::once(Rectangle::new(
        [(peak_min2, 0.0), (peak_max2, second_stage_density)],
        RGBAColor(255, 0, 0, 0.2).filled(),
    )))?;

    chart.draw_series(std::iter::once(Rectangle::new(
        [(peak_min2, 0.0), (peak_max2, second_stage_density)],
        RGBColor(255, 0, 0).stroke_width(2),
    )))?;

    // Add legend
    let legend_y = y_max * 0.9;
    chart.draw_series(std::iter::once(Circle::new(
        (x_min + (x_max - x_min) * 0.7, legend_y),
        5,
        RGBColor(0, 0, 255).filled(),
    )))?;
    chart.draw_series(std::iter::once(Text::new(
        "KDE Density",
        (x_min + (x_max - x_min) * 0.72, legend_y),
        ("sans-serif", 12).into_font(),
    )))?;

    chart.draw_series(std::iter::once(Circle::new(
        (x_min + (x_max - x_min) * 0.7, legend_y - y_max * 0.05),
        5,
        RGBColor(255, 0, 0).filled(),
    )))?;
    chart.draw_series(std::iter::once(Text::new(
        "Selected Peak",
        (x_min + (x_max - x_min) * 0.72, legend_y - y_max * 0.05),
        ("sans-serif", 12).into_font(),
    )))?;

    chart.draw_series(std::iter::once(Circle::new(
        (x_min + (x_max - x_min) * 0.7, legend_y - y_max * 0.1),
        5,
        RGBColor(0, 255, 0).filled(),
    )))?;
    chart.draw_series(std::iter::once(Text::new(
        "Other Peaks",
        (x_min + (x_max - x_min) * 0.72, legend_y - y_max * 0.1),
        ("sans-serif", 12).into_font(),
    )))?;

    chart.draw_series(std::iter::once(Rectangle::new(
        [
            (x_min + (x_max - x_min) * 0.7, legend_y - y_max * 0.15),
            (x_min + (x_max - x_min) * 0.75, legend_y - y_max * 0.13),
        ],
        RGBAColor(255, 165, 0, 0.2).filled(),
    )))?;
    chart.draw_series(std::iter::once(Text::new(
        "First-stage Region",
        (x_min + (x_max - x_min) * 0.72, legend_y - y_max * 0.14),
        ("sans-serif", 12).into_font(),
    )))?;

    chart.draw_series(std::iter::once(Rectangle::new(
        [
            (x_min + (x_max - x_min) * 0.7, legend_y - y_max * 0.2),
            (x_min + (x_max - x_min) * 0.75, legend_y - y_max * 0.18),
        ],
        RGBAColor(255, 0, 0, 0.2).filled(),
    )))?;
    chart.draw_series(std::iter::once(Text::new(
        "Second-stage Region",
        (x_min + (x_max - x_min) * 0.72, legend_y - y_max * 0.19),
        ("sans-serif", 12).into_font(),
    )))?;

    root.present()?;

    Ok(())
}
