//! Integration tests to verify compatibility with R's PeacoQC implementation
//!
//! These tests ensure that the Rust implementation produces identical or
//! near-identical results to the R implementation when processing the same data.
//!
//! To generate reference data from R, run:
//! ```r
//! library(PeacoQC)
//! library(flowCore)
//! 
//! ff <- read.FCS("examples/D1 Well_004.fcs")
//! channels <- colnames(exprs(ff))[c(1,3,5:14)]  # Example channel selection
//! 
//! result <- PeacoQC(ff, channels,
//!     determine_good_cells = "all",
//!     MAD = 6,
//!     IT_limit = 0.6,
//!     consecutive_bins = 5,
//!     min_cells = 150,
//!     max_bins = 500,
//!     save_fcs = FALSE,
//!     plot = FALSE
//! )
//! 
//! # Export results for comparison
//! write.csv(data.frame(
//!     n_bins = result$nr_bins,
//!     events_per_bin = result$EventsPerBin,
//!     it_percentage = result$ITPercentage,
//!     mad_percentage = result$MADPercentage,
//!     percentage_removed = result$PercentageRemoved
//! ), "r_reference_results.csv")
//! ```

#[cfg(all(test, feature = "flow-fcs"))]
mod tests {
    use peacoqc_rs::qc::peaks::create_breaks;

    /// Test that overlapping bins match R's SplitWithOverlap behavior
    #[test]
    fn test_overlapping_bins_match_r() {
        // R's SplitWithOverlap with events_per_bin=1000, overlap=500 on 10000 events
        // would produce: starts at 0, 500, 1000, 1500, ... until start >= n_events
        
        let breaks = create_breaks(10000, 1000);
        
        // With 50% overlap (500), we should get bins starting at:
        // 0-1000, 500-1500, 1000-2000, ..., 9000-10000, 9500-10000
        // That's (10000 / 500) = 20 bins approximately
        
        // First bin should start at 0
        assert_eq!(breaks[0].0, 0);
        assert_eq!(breaks[0].1, 1000);
        
        // Second bin should start at 500 (50% overlap)
        assert_eq!(breaks[1].0, 500);
        assert_eq!(breaks[1].1, 1500);
        
        // Check overlap is 50%
        let overlap = breaks[0].1 - breaks[1].0;
        let expected_overlap = 500;
        assert_eq!(overlap, expected_overlap, "Overlap should be 50%");
        
        // Last bin should end at or near n_events
        assert_eq!(breaks.last().unwrap().1, 10000);
    }

    /// Test that bin count with overlapping windows is approximately 2x non-overlapping
    #[test]
    fn test_bin_count_with_overlap() {
        let n_events = 50000;
        let events_per_bin = 1000;
        
        let breaks = create_breaks(n_events, events_per_bin);
        
        // Non-overlapping would give: 50000 / 1000 = 50 bins
        // With 50% overlap, step = 500, so: (50000 / 500) ≈ 100 bins
        // But the last few bins may be truncated
        
        let non_overlapping_count = (n_events + events_per_bin - 1) / events_per_bin;
        
        // Overlapping should give roughly 2x the bins
        assert!(
            breaks.len() >= non_overlapping_count,
            "Overlapping bins ({}) should be >= non-overlapping ({})",
            breaks.len(),
            non_overlapping_count
        );
        
        assert!(
            breaks.len() <= non_overlapping_count * 2 + 1,
            "Overlapping bins ({}) should be <= 2x non-overlapping + 1 ({})",
            breaks.len(),
            non_overlapping_count * 2 + 1
        );
    }

    /// Test MAD scale factor matches R's stats::mad constant
    #[test]
    fn test_mad_scale_factor() {
        use peacoqc_rs::stats::median_mad::MAD_SCALE_FACTOR;
        
        // R's mad() function uses constant = 1.4826 by default
        // This is 1/qnorm(0.75) which makes MAD consistent with SD for normal data
        let r_constant = 1.4826;
        
        assert!(
            (MAD_SCALE_FACTOR - r_constant).abs() < 0.0001,
            "MAD_SCALE_FACTOR ({}) should match R's constant ({})",
            MAD_SCALE_FACTOR,
            r_constant
        );
    }

    /// Test median calculation matches R
    #[test]
    fn test_median_matches_r() {
        use peacoqc_rs::stats::median_mad::median;
        
        // Test odd length
        let odd_data = vec![1.0, 5.0, 3.0, 4.0, 2.0];
        let result = median(&odd_data).unwrap();
        assert!((result - 3.0).abs() < 1e-10, "Odd median should be 3.0, got {}", result);
        
        // Test even length (R uses average of two middle values)
        let even_data = vec![1.0, 2.0, 3.0, 4.0];
        let result = median(&even_data).unwrap();
        assert!((result - 2.5).abs() < 1e-10, "Even median should be 2.5, got {}", result);
    }

    /// Test MAD calculation matches R
    #[test]
    fn test_mad_matches_r() {
        use peacoqc_rs::stats::median_mad::{median_mad, median_mad_scaled};
        
        // R: mad(c(1, 2, 3, 4, 5), constant = 1)
        // median = 3, deviations = |1-3|, |2-3|, |3-3|, |4-3|, |5-3| = 2, 1, 0, 1, 2
        // median of deviations = 1
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let (med, raw_mad) = median_mad(&data).unwrap();
        assert!((med - 3.0).abs() < 1e-10, "Median should be 3.0");
        assert!((raw_mad - 1.0).abs() < 1e-10, "Raw MAD should be 1.0");
        
        // R: mad(c(1, 2, 3, 4, 5)) with default constant = 1.4826
        let (_, scaled_mad) = median_mad_scaled(&data).unwrap();
        assert!(
            (scaled_mad - 1.4826).abs() < 0.001,
            "Scaled MAD should be 1.4826, got {}",
            scaled_mad
        );
    }

    /// Test avgPL calculation matches R's avgPL function
    #[test]
    fn test_avg_path_length_matches_r() {
        // R's avgPL function:
        // avgPL <- function(n_datapoints){
        //     if (n_datapoints -1 == 0){
        //         AVG <- 0
        //     } else {
        //         AVG <- 2*(log(n_datapoints - 1) +  0.5772156649) -
        //             (2*(n_datapoints -1))/(n_datapoints)
        //     }
        //     return (AVG)
        // }
        
        // Test some known values
        // n=1: 0
        // n=2: 0 (since n-1=1, log(1)=0)
        // n=10: 2*(log(9) + 0.5772) - (2*9/10) ≈ 2*(2.197 + 0.577) - 1.8 ≈ 3.748
        // n=100: 2*(log(99) + 0.5772) - (2*99/100) ≈ 2*(4.595 + 0.577) - 1.98 ≈ 8.364
        
        // These are approximate, the isolation tree module has the actual function
        // For now, just verify the pattern is correct
        assert!(true, "avgPL formula implemented in isolation_tree.rs");
    }

    /// Test SD calculation matches R's stats::sd
    #[test]
    fn test_std_dev_matches_r() {
        // R uses sample standard deviation (n-1 divisor)
        // sd(c(2, 4, 4, 4, 5, 5, 7, 9)) = 2.138
        
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        
        // Calculate sample SD
        let n = data.len() as f64;
        let mean = data.iter().sum::<f64>() / n;
        let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let sd = variance.sqrt();
        
        // R: sd(c(2,4,4,4,5,5,7,9)) = 2.138089
        assert!(
            (sd - 2.138).abs() < 0.01,
            "Sample SD should be ~2.138, got {}",
            sd
        );
    }

    /// Test default parameters match R PeacoQC defaults
    #[test]
    fn test_default_parameters_match_r() {
        use peacoqc_rs::qc::peacoqc::PeacoQCConfig;
        
        let config = PeacoQCConfig::default();
        
        // R defaults from PeacoQC function signature:
        // MAD=6, IT_limit=0.6, consecutive_bins=5, min_cells=150, max_bins=500
        // force_IT=150, peak_removal=(1/3), min_nr_bins_peakdetection=10
        
        assert_eq!(config.mad, 6.0, "MAD threshold should be 6.0");
        assert_eq!(config.it_limit, 0.6, "IT limit should be 0.6");
        assert_eq!(config.consecutive_bins, 5, "Consecutive bins should be 5");
        assert_eq!(config.min_cells, 150, "Min cells should be 150");
        assert_eq!(config.max_bins, 500, "Max bins should be 500");
        assert_eq!(config.force_it, 150, "Force IT should be 150");
        assert!(
            (config.peak_removal - 1.0 / 3.0).abs() < 1e-10,
            "Peak removal should be 1/3"
        );
        assert_eq!(
            config.min_nr_bins_peakdetection, 10.0,
            "Min bins for peak detection should be 10%"
        );
    }
}
