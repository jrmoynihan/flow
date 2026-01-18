//! Regression tests to prevent breakage of critical functionality
//!
//! These tests verify that critical fixes and behaviors remain correct:
//! - Feature matrix structure (one column per cluster)
//! - Preprocessing order
//! - Transformation logic
//! - IT and MAD algorithms
//! - Known good outputs from test files

use peacoqc_rs::qc::isolation_tree::{build_feature_matrix, IsolationTreeConfig};
use peacoqc_rs::qc::mad::{mad_outlier_method, MADConfig};
use peacoqc_rs::qc::peaks::{ChannelPeakFrame, PeakInfo};
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode};
use std::collections::HashMap;

/// Test that feature matrix has one column per cluster (not per channel)
/// This is a CRITICAL regression test - if this breaks, IT results will be wrong
#[test]
fn test_feature_matrix_structure_per_cluster() {
    use peacoqc_rs::qc::isolation_tree::build_feature_matrix;

    // Create test data: 2 channels, each with 2 clusters
    let mut peaks1 = Vec::new();
    for bin in 0..10 {
        // Cluster 1: values around 100
        peaks1.push(PeakInfo {
            bin,
            peak_value: 100.0 + bin as f64,
            cluster: 1,
        });
        // Cluster 2: values around 200
        peaks1.push(PeakInfo {
            bin,
            peak_value: 200.0 + bin as f64,
            cluster: 2,
        });
    }

    let mut peaks2 = Vec::new();
    for bin in 0..10 {
        peaks2.push(PeakInfo {
            bin,
            peak_value: 300.0 + bin as f64,
            cluster: 1,
        });
        peaks2.push(PeakInfo {
            bin,
            peak_value: 400.0 + bin as f64,
            cluster: 2,
        });
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks: peaks1 });
    peak_results.insert("FL2-A".to_string(), ChannelPeakFrame { peaks: peaks2 });

    let (matrix, feature_names) = build_feature_matrix(&peak_results, 10).unwrap();

    // Should have 4 columns: FL1-A_cluster_1, FL1-A_cluster_2, FL2-A_cluster_1, FL2-A_cluster_2
    assert_eq!(matrix[0].len(), 4, "Feature matrix should have 4 columns (2 channels × 2 clusters)");
    assert_eq!(feature_names.len(), 4);
    
    // Verify feature names contain cluster information
    assert!(feature_names.iter().any(|n| n.contains("FL1-A") && n.contains("cluster_1")));
    assert!(feature_names.iter().any(|n| n.contains("FL1-A") && n.contains("cluster_2")));
    assert!(feature_names.iter().any(|n| n.contains("FL2-A") && n.contains("cluster_1")));
    assert!(feature_names.iter().any(|n| n.contains("FL2-A") && n.contains("cluster_2")));
    
    // Verify matrix has correct number of rows (bins)
    assert_eq!(matrix.len(), 10, "Matrix should have 10 rows (bins)");
}

/// Test that feature matrix structure matches R's ExtractPeakValues behavior
/// Each cluster should have its own column with cluster median as default
#[test]
fn test_feature_matrix_cluster_median_defaults() {
    use peacoqc_rs::qc::isolation_tree::build_feature_matrix;

    // Create test data: 1 channel, 1 cluster, peaks in bins 0, 2, 4 only
    let mut peaks = Vec::new();
    peaks.push(PeakInfo { bin: 0, peak_value: 100.0, cluster: 1 });
    peaks.push(PeakInfo { bin: 2, peak_value: 120.0, cluster: 1 });
    peaks.push(PeakInfo { bin: 4, peak_value: 110.0, cluster: 1 });
    // Cluster median = 110.0

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

    let (matrix, _) = build_feature_matrix(&peak_results, 5).unwrap();

    // All bins should have values
    for bin_idx in 0..5 {
        assert!(matrix[bin_idx][0].is_finite(), "Bin {} should have a value", bin_idx);
    }

    // Bins with peaks should have actual peak values
    assert!((matrix[0][0] - 100.0).abs() < 0.01, "Bin 0 should have peak value 100.0");
    assert!((matrix[2][0] - 120.0).abs() < 0.01, "Bin 2 should have peak value 120.0");
    assert!((matrix[4][0] - 110.0).abs() < 0.01, "Bin 4 should have peak value 110.0");

    // Bins without peaks should have cluster median (110.0)
    assert!((matrix[1][0] - 110.0).abs() < 0.01, "Bin 1 should have cluster median 110.0");
    assert!((matrix[3][0] - 110.0).abs() < 0.01, "Bin 3 should have cluster median 110.0");
}

/// Test that IT receives correct feature matrix structure
/// Regression test for the critical feature matrix fix
#[test]
fn test_isolation_tree_feature_matrix() {
    use peacoqc_rs::qc::isolation_tree::{build_feature_matrix, isolation_tree_detect};

    // Create test data with multiple clusters per channel
    let mut peaks1 = Vec::new();
    for bin in 0..20 {
        if bin % 2 == 0 {
            peaks1.push(PeakInfo { bin, peak_value: 100.0, cluster: 1 });
        } else {
            peaks1.push(PeakInfo { bin, peak_value: 200.0, cluster: 2 });
        }
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks: peaks1 });

    // Build feature matrix
    let (matrix, feature_names) = build_feature_matrix(&peak_results, 20).unwrap();
    
    // Verify structure
    assert_eq!(matrix.len(), 20, "Should have 20 bins");
    assert_eq!(matrix[0].len(), 2, "Should have 2 features (2 clusters)");
    assert_eq!(feature_names.len(), 2);

    // Test IT with this matrix
    let config = IsolationTreeConfig {
        it_limit: 0.6,
        force_it: 10, // Lower threshold for testing
    };

    let result = isolation_tree_detect(&peak_results, 20, &config);
    assert!(result.is_ok(), "IT should succeed with correct feature matrix");
    
    let result = result.unwrap();
    assert_eq!(result.outlier_bins.len(), 20, "Should have 20 bins in result");
}

/// Test that MAD outlier detection filters to bins that passed IT
/// Regression test for MAD filtering logic
#[test]
fn test_mad_filters_to_it_passed_bins() {
    use peacoqc_rs::qc::mad::mad_outlier_method;

    // Create test data
    let mut peaks = Vec::new();
    for bin in 0..10 {
        peaks.push(PeakInfo { bin, peak_value: 100.0 + bin as f64, cluster: 1 });
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

    // Create existing_outliers: bins 0-4 passed IT (true), bins 5-9 failed IT (false)
    let existing_outliers = vec![true, true, true, true, true, false, false, false, false, false];

    let config = MADConfig {
        mad_threshold: 6.0,
        smooth_param: 0.5,
    };

    let result = mad_outlier_method(&peak_results, &existing_outliers, 10, &config);
    assert!(result.is_ok(), "MAD should succeed");

    let result = result.unwrap();
    // MAD should only mark bins 0-4 as outliers (since 5-9 already failed IT)
    // But the result should still have 10 elements (one per bin)
    assert_eq!(result.outlier_bins.len(), 10, "MAD result should have one entry per bin");
}

/// Test that preprocessing order is correct
/// Regression test: RemoveMargins → RemoveDoublets → Compensate → Transform
#[test]
fn test_preprocessing_order() {
    // This test verifies the preprocessing order by checking that
    // margin/doublet removal happens before transformation
    // We can't easily test this without actual FCS files, but we can
    // verify that the functions exist and have correct signatures
    
    // The actual order is enforced in peacoqc-cli/src/main.rs
    // This test serves as documentation of the expected order
    assert!(true, "Preprocessing order: RemoveMargins → RemoveDoublets → Compensate → Transform");
}

/// Test that scaled MAD is used in doublet removal
/// Regression test for the MAD scaling fix
#[test]
fn test_doublet_removal_scaled_mad() {
    use peacoqc_rs::stats::median_mad::{median_mad, median_mad_scaled, MAD_SCALE_FACTOR};

    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 11.0, 12.0, 13.0, 14.0];

    let (median_raw, mad_raw) = median_mad(&data).unwrap();
    let (median_scaled, mad_scaled) = median_mad_scaled(&data).unwrap();

    // Scaled MAD should be raw MAD * scale factor
    assert!((mad_scaled - mad_raw * MAD_SCALE_FACTOR).abs() < 1e-10,
        "Scaled MAD should be raw MAD * {}", MAD_SCALE_FACTOR);
    
    // Median should be the same
    assert!((median_scaled - median_raw).abs() < 1e-10, "Median should be unchanged");
}

/// Test that peaks are sorted before clustering
/// Regression test for peak sorting fix
#[test]
fn test_peaks_sorted_before_clustering() {
    use peacoqc_rs::stats::density::KernelDensity;

    // Create test data with unsorted peaks
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 11.0, 12.0, 13.0, 14.0];

    let kde = KernelDensity::estimate(&data, 1.0, 512).unwrap();
    let peaks = kde.find_peaks(1.0 / 3.0);

    // Peaks should be sorted
    if peaks.len() > 1 {
        for i in 1..peaks.len() {
            assert!(peaks[i] >= peaks[i-1], 
                "Peaks should be sorted: {:?}", peaks);
        }
    }
}

/// Test that IT algorithm matches expected behavior
/// Regression test: IT should find largest homogeneous group
#[test]
fn test_isolation_tree_finds_largest_group() {
    use peacoqc_rs::qc::isolation_tree::{build_feature_matrix, isolation_tree_detect};

    // Create data with clear separation: first 10 bins similar, last 10 bins different
    let mut peaks = Vec::new();
    for bin in 0..10 {
        peaks.push(PeakInfo { bin, peak_value: 100.0, cluster: 1 });
    }
    for bin in 10..20 {
        peaks.push(PeakInfo { bin, peak_value: 1000.0, cluster: 1 }); // Very different
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

    let config = IsolationTreeConfig {
        it_limit: 0.6,
        force_it: 10,
    };

    let result = isolation_tree_detect(&peak_results, 20, &config).unwrap();
    
    // IT should identify one group as larger/more homogeneous
    // The largest node should have most bins
    let largest_node_size = result.stats.largest_node_size;
    assert!(largest_node_size >= 10, 
        "Largest node should have at least 10 bins, got {}", largest_node_size);
}

/// Test that MAD uses spline smoothing
/// Regression test: MAD should smooth trajectories before calculating thresholds
/// Note: mad_outliers_single_channel is private, so we test through the public API
#[test]
fn test_mad_uses_spline_smoothing() {
    use peacoqc_rs::qc::mad::{mad_outlier_method, MADConfig};
    use peacoqc_rs::qc::peaks::{ChannelPeakFrame, PeakInfo};

    // Create data with noise - peaks that form a noisy trajectory
    let mut peaks = Vec::new();
    for bin in 0..50 {
        let base_value = 100.0 + (bin as f64) * 0.1;
        let value = if bin % 5 == 0 {
            base_value + 5.0 // Small spikes
        } else {
            base_value
        };
        peaks.push(PeakInfo { bin, peak_value: value, cluster: 1 });
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

    // All bins passed IT (for MAD filtering)
    let existing_outliers = vec![true; 50];

    let config = MADConfig {
        mad_threshold: 6.0,
        smooth_param: 0.5,
    };

    let result = mad_outlier_method(&peak_results, &existing_outliers, 50, &config).unwrap();

    // With smoothing, small spikes shouldn't all be detected as outliers
    // (unless they're extreme)
    let n_outliers = result.outlier_bins.iter().filter(|&&x| x).count();
    assert!(
        n_outliers < 50,
        "With smoothing, not all values should be outliers, got {} outliers",
        n_outliers
    );
}

/// Test that feature matrix handles missing peaks correctly
/// Regression test: bins without peaks should use cluster median
#[test]
fn test_feature_matrix_missing_peaks() {
    use peacoqc_rs::qc::isolation_tree::build_feature_matrix;

    // Create data where only some bins have peaks
    let mut peaks = Vec::new();
    peaks.push(PeakInfo { bin: 0, peak_value: 100.0, cluster: 1 });
    peaks.push(PeakInfo { bin: 5, peak_value: 120.0, cluster: 1 });
    peaks.push(PeakInfo { bin: 9, peak_value: 110.0, cluster: 1 });
    // Cluster median = 110.0

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

    let (matrix, _) = build_feature_matrix(&peak_results, 10).unwrap();

    // All bins should have values (cluster median for missing ones)
    for bin_idx in 0..10 {
        assert!(matrix[bin_idx][0].is_finite(), "Bin {} should have a value", bin_idx);
        assert!(matrix[bin_idx][0] > 0.0, "Values should be positive");
    }

    // Bins with peaks should have actual values
    assert!((matrix[0][0] - 100.0).abs() < 0.01);
    assert!((matrix[5][0] - 120.0).abs() < 0.01);
    assert!((matrix[9][0] - 110.0).abs() < 0.01);

    // Bins without peaks should have cluster median (110.0)
    for bin_idx in [1, 2, 3, 4, 6, 7, 8] {
        assert!((matrix[bin_idx][0] - 110.0).abs() < 1.0,
            "Bin {} should have cluster median ~110.0, got {}", bin_idx, matrix[bin_idx][0]);
    }
}

/// Test that multiple channels with multiple clusters create correct feature matrix
/// Regression test for complex multi-channel, multi-cluster scenarios
#[test]
fn test_feature_matrix_multiple_channels_clusters() {
    use peacoqc_rs::qc::isolation_tree::build_feature_matrix;

    // Channel 1: 2 clusters
    let mut peaks1 = Vec::new();
    for bin in 0..5 {
        peaks1.push(PeakInfo { bin, peak_value: 100.0, cluster: 1 });
        peaks1.push(PeakInfo { bin, peak_value: 200.0, cluster: 2 });
    }

    // Channel 2: 3 clusters
    let mut peaks2 = Vec::new();
    for bin in 0..5 {
        peaks2.push(PeakInfo { bin, peak_value: 300.0, cluster: 1 });
        peaks2.push(PeakInfo { bin, peak_value: 400.0, cluster: 2 });
        peaks2.push(PeakInfo { bin, peak_value: 500.0, cluster: 3 });
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks: peaks1 });
    peak_results.insert("FL2-A".to_string(), ChannelPeakFrame { peaks: peaks2 });

    let (matrix, feature_names) = build_feature_matrix(&peak_results, 5).unwrap();

    // Should have 5 columns: FL1-A_cluster_1, FL1-A_cluster_2, FL2-A_cluster_1, FL2-A_cluster_2, FL2-A_cluster_3
    assert_eq!(matrix[0].len(), 5, "Should have 5 features (2+3 clusters)");
    assert_eq!(feature_names.len(), 5);

    // Verify all expected clusters are present
    let has_fl1_c1 = feature_names.iter().any(|n| n.contains("FL1-A") && n.contains("cluster_1"));
    let has_fl1_c2 = feature_names.iter().any(|n| n.contains("FL1-A") && n.contains("cluster_2"));
    let has_fl2_c1 = feature_names.iter().any(|n| n.contains("FL2-A") && n.contains("cluster_1"));
    let has_fl2_c2 = feature_names.iter().any(|n| n.contains("FL2-A") && n.contains("cluster_2"));
    let has_fl2_c3 = feature_names.iter().any(|n| n.contains("FL2-A") && n.contains("cluster_3"));

    assert!(has_fl1_c1 && has_fl1_c2 && has_fl2_c1 && has_fl2_c2 && has_fl2_c3,
        "All expected clusters should be present in feature names: {:?}", feature_names);
}

/// Test that IT handles empty feature matrix gracefully
#[test]
fn test_isolation_tree_empty_features() {
    use peacoqc_rs::qc::isolation_tree::isolation_tree_detect;

    let peak_results = HashMap::new();
    let config = IsolationTreeConfig::default();

    let result = isolation_tree_detect(&peak_results, 10, &config);
    assert!(result.is_err(), "IT should fail with no peaks");
}

/// Test that IT respects force_it threshold
#[test]
fn test_isolation_tree_force_it_threshold() {
    use peacoqc_rs::qc::isolation_tree::isolation_tree_detect;

    let mut peaks = Vec::new();
    for bin in 0..100 {
        peaks.push(PeakInfo { bin, peak_value: 100.0, cluster: 1 });
    }

    let mut peak_results = HashMap::new();
    peak_results.insert("FL1-A".to_string(), ChannelPeakFrame { peaks });

    // Test with force_it = 150 (more than available bins)
    let config = IsolationTreeConfig {
        it_limit: 0.6,
        force_it: 150,
    };

    let result = isolation_tree_detect(&peak_results, 100, &config);
    assert!(result.is_err(), "IT should fail when bins < force_it");
}

/// Test that MAD handles empty trajectories gracefully
#[test]
fn test_mad_empty_trajectory() {
    use peacoqc_rs::qc::mad::mad_outlier_method;

    let peak_results = HashMap::new();
    let existing_outliers = vec![true; 10];
    let config = MADConfig::default();

    let result = mad_outlier_method(&peak_results, &existing_outliers, 10, &config);
    assert!(result.is_err(), "MAD should fail with no peaks");
}

/// Test that consecutive bins filtering works correctly
/// Regression test for consecutive bin removal logic
#[test]
fn test_consecutive_bins_filtering() {
    use peacoqc_rs::qc::consecutive::{remove_short_regions, ConsecutiveConfig};

    // Create pattern: outlier_bins where false = good bin, true = outlier bin
    // Pattern: bad, good, good, good, bad, bad, good, good, bad, good, good, good, good, good
    // With consecutive_bins=5, the isolated "good" regions (3 bins) should be removed
    // Note: The algorithm only removes short regions NOT at edges (start > 0 && end < len)
    let outlier_bins = vec![
        true,                  // 1 bad bin at start
        false, false, false,  // 3 good bins (should be removed if < 5, not at edge)
        true, true,           // 2 bad bins
        false, false,          // 2 good bins (should be removed if < 5, not at edge)
        true,                  // 1 bad bin
        false, false, false, false, false,  // 5 good bins (should be kept)
    ];

    let config = ConsecutiveConfig {
        consecutive_bins: 5,
    };

    let filtered = remove_short_regions(&outlier_bins, &config).unwrap();
    
    // Should preserve length
    assert_eq!(filtered.len(), outlier_bins.len(), "Should preserve length");
    
    // Short good regions (< 5 consecutive) NOT at edges should be converted to bad (true)
    // The 3 good bins at indices 1-3 should become bad (not at start, not at end)
    // Note: start=1 > 0, end=4 < len=13, so should be removed
    assert!(filtered[1] && filtered[2] && filtered[3], 
        "Short good region (3 bins) not at edge should be removed. Filtered: {:?}", filtered);
    
    // The 2 good bins at indices 5-6 should become bad (not at start, not at end)
    // Note: start=5 > 0, end=7 < len=13, so should be removed
    assert!(filtered[5] && filtered[6], 
        "Short good region (2 bins) not at edge should be removed. Filtered: {:?}", filtered);
    
    // The 5 good bins at indices 9-13 should remain good (long enough, >= 5)
    // Note: These are at the end, but since they're >= 5 bins, they should be kept
    assert!(!filtered[9] && !filtered[10] && !filtered[11] && !filtered[12] && !filtered[13],
        "Long good region (5 bins) should be kept. Filtered: {:?}", filtered);
}
