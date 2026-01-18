//! Integration regression tests with known good outputs
//!
//! These tests verify that processing specific files produces expected results.
//! If these tests fail, it indicates a regression in the QC pipeline.

#[cfg(all(test, feature = "flow-fcs"))]
mod tests {
    use peacoqc_rs::{PeacoQCConfig, QCMode};
    use std::path::PathBuf;

    /// Test that flow_file_start_up.fcs produces expected results
    /// This is a regression test - if results change significantly, investigate
    #[test]
    #[ignore] // Requires test file, run with: cargo test --features flow-fcs -- --ignored
    fn test_flow_file_start_up_regression() {
        use flow_fcs::file::Fcs;
        
        let fcs_path = PathBuf::from("flow_file_start_up.fcs");
        if !fcs_path.exists() {
            eprintln!("Skipping test - file not found: {:?}", fcs_path);
            return;
        }

        let fcs = Fcs::open(&fcs_path).unwrap();
        
        // Get fluorescence channels (exclude Time, FSC, SSC)
        let all_channels: Vec<String> = fcs.channel_names();
        let channels: Vec<String> = all_channels
            .into_iter()
            .filter(|ch: &String| {
                let upper = ch.to_uppercase();
                !upper.contains("TIME") && !upper.contains("FSC") && !upper.contains("SSC")
            })
            .collect();

        let config = PeacoQCConfig {
            channels: channels.clone(),
            determine_good_cells: QCMode::All,
            ..Default::default()
        };

        let result = peacoqc_rs::peacoqc(&fcs, &config).unwrap();

        // Regression checks - these values should remain stable
        // Based on testing: 175,312 events after preprocessing, ~7-8% removed
        
        let n_events_before = fcs.n_events();
        let n_events_after = result.good_cells.iter().filter(|&&x| x).count();
        let pct_removed = result.percentage_removed;

        // Verify preprocessing produces expected event count (~175k)
        // Allow some tolerance for different preprocessing implementations
        assert!(
            n_events_after >= 160_000 && n_events_after <= 180_000,
            "Events after QC should be ~175k, got {}",
            n_events_after
        );

        // Verify removal percentage is reasonable (should be ~7-15% based on R comparison)
        assert!(
            pct_removed >= 5.0 && pct_removed <= 20.0,
            "Removal percentage should be ~7-15%, got {}%",
            pct_removed
        );

        // Verify IT results (should be 0 outlier bins for this file)
        if let Some(it_pct) = result.it_percentage {
            assert!(
                it_pct < 1.0,
                "IT should detect few/no outliers for this file, got {}%",
                it_pct
            );
        }

        // Verify we have peak detection results
        assert!(
            !result.peaks.is_empty(),
            "Should have peak detection results"
        );

        // Verify bin count is reasonable (should be ~350)
        assert!(
            result.n_bins >= 300 && result.n_bins <= 400,
            "Bin count should be ~350, got {}",
            result.n_bins
        );
    }

    /// Test that flow_file_low_medium_high_speed.fcs produces 0% removal
    /// This file is known to be clean - regression test
    #[test]
    #[ignore] // Requires test file
    fn test_clean_file_zero_removal() {
        use flow_fcs::file::Fcs;
        
        let fcs_path = PathBuf::from("flow_file_low_medium_high_speed.fcs");
        if !fcs_path.exists() {
            eprintln!("Skipping test - file not found: {:?}", fcs_path);
            return;
        }

        let fcs = Fcs::open(&fcs_path).unwrap();
        
        let all_channels: Vec<String> = fcs.channel_names();
        let channels: Vec<String> = all_channels
            .into_iter()
            .filter(|ch: &String| {
                let upper = ch.to_uppercase();
                !upper.contains("TIME") && !upper.contains("FSC") && !upper.contains("SSC")
            })
            .collect();

        let config = PeacoQCConfig {
            channels: channels.clone(),
            determine_good_cells: QCMode::All,
            ..Default::default()
        };

        let result = peacoqc_rs::peacoqc(&fcs, &config).unwrap();

        // This file should have 0% removal (known clean file)
        assert!(
            result.percentage_removed < 1.0,
            "Clean file should have <1% removal, got {}%",
            result.percentage_removed
        );

        // IT and MAD should detect no outliers
        if let Some(it_pct) = result.it_percentage {
            assert_eq!(it_pct, 0.0, "IT should detect 0% outliers for clean file");
        }
        if let Some(mad_pct) = result.mad_percentage {
            assert_eq!(mad_pct, 0.0, "MAD should detect 0% outliers for clean file");
        }
    }

    /// Test that preprocessing order is correct
    /// Regression test: RemoveMargins → RemoveDoublets → Compensate → Transform
    /// Note: This test documents the expected preprocessing order
    /// Actual preprocessing is tested through the full pipeline in other tests
    #[test]
    #[ignore] // Requires test file
    fn test_preprocessing_order_regression() {
        use flow_fcs::file::Fcs;
        use peacoqc_rs::preprocess_fcs;
        
        let fcs_path = PathBuf::from("flow_file_start_up.fcs");
        if !fcs_path.exists() {
            eprintln!("Skipping test - file not found: {:?}", fcs_path);
            return;
        }

        let fcs = Fcs::open(&fcs_path).unwrap();
        let n_events_initial = fcs.n_events();

        // Apply preprocessing in correct order using preprocess_fcs
        let all_channels: Vec<String> = fcs.channel_names();
        let channels: Vec<String> = all_channels
            .into_iter()
            .filter(|ch: &String| {
                let upper = ch.to_uppercase();
                !upper.contains("TIME") && !upper.contains("FSC") && !upper.contains("SSC")
            })
            .collect();

        let preprocessed = preprocess_fcs(&fcs, &channels).unwrap();
        let n_after_preprocessing = preprocessed.n_events();

        // Verify preprocessing reduces event count
        assert!(
            n_after_preprocessing <= n_events_initial,
            "Preprocessing should not increase events"
        );

        // Expected: ~198k → ~175k after preprocessing
        // Allow tolerance
        assert!(
            n_after_preprocessing >= 170_000 && n_after_preprocessing <= 180_000,
            "After preprocessing should have ~175k events, got {}",
            n_after_preprocessing
        );
    }

    /// Test that feature matrix structure remains correct
    /// CRITICAL regression test - if this breaks, IT will be wrong
    #[test]
    #[ignore] // Requires test file
    fn test_feature_matrix_structure_integration() {
        use flow_fcs::file::Fcs;
        use peacoqc_rs::qc::isolation_tree::build_feature_matrix;
        
        let fcs_path = PathBuf::from("flow_file_start_up.fcs");
        if !fcs_path.exists() {
            eprintln!("Skipping test - file not found: {:?}", fcs_path);
            return;
        }

        let fcs = Fcs::open(&fcs_path).unwrap();
        
        let all_channels: Vec<String> = fcs.channel_names();
        let channels: Vec<String> = all_channels
            .into_iter()
            .filter(|ch: &String| {
                let upper = ch.to_uppercase();
                !upper.contains("TIME") && !upper.contains("FSC") && !upper.contains("SSC")
            })
            .collect();

        // Run peak detection
        let config = PeacoQCConfig {
            channels: channels.clone(),
            determine_good_cells: QCMode::None, // Only peak detection
            ..Default::default()
        };

        let result = peacoqc_rs::peacoqc(&fcs, &config).unwrap();

        // Build feature matrix and verify structure
        let (matrix, feature_names) = build_feature_matrix(&result.peaks, result.n_bins).unwrap();

        // Verify: should have one column per cluster per channel
        // Should have more columns than channels (because clusters > 1 per channel)
        assert!(
            feature_names.len() >= channels.len(),
            "Should have at least one feature per channel (one per cluster), got {} features for {} channels",
            feature_names.len(),
            channels.len()
        );

        // Verify feature names contain cluster information
        for name in &feature_names {
            assert!(
                name.contains("_cluster_"),
                "Feature name should contain '_cluster_': {}",
                name
            );
        }

        // Verify matrix dimensions
        assert_eq!(matrix.len(), result.n_bins, "Matrix should have one row per bin");
        assert_eq!(
            matrix[0].len(),
            feature_names.len(),
            "Matrix should have one column per feature"
        );
    }
}
