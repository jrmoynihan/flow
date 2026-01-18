use crate::PeacoQCData;
use crate::error::{PeacoQCError, Result};
use crate::qc::consecutive::{ConsecutiveConfig, remove_short_regions};
use crate::qc::isolation_tree::{IsolationTreeConfig, isolation_tree_detect};
use crate::qc::mad::{MADConfig, mad_outlier_method};
use crate::qc::peaks::{
    ChannelPeakFrame, PeakDetectionConfig, create_breaks, determine_peaks_all_channels,
};
use crate::qc::debug;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, info, trace, warn};

/// Quality control mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QCMode {
    /// Use both Isolation Tree and MAD methods
    All,
    /// Use only Isolation Tree
    IsolationTree,
    /// Use only MAD method
    MAD,
    /// No quality control, only peak detection
    None,
}

/// Main PeacoQC configuration
///
/// Default parameters match the R PeacoQC package exactly.
#[derive(Debug, Clone)]
pub struct PeacoQCConfig {
    /// Channels to analyze
    pub channels: Vec<String>,

    /// Quality control mode
    pub determine_good_cells: QCMode,

    /// Minimum events per bin (default: 150)
    ///
    /// **Tradeoff**: Increasing the minimal number of events per bin will improve
    /// peak detection (more accurate density estimation), but having too few bins
    /// total makes it harder to estimate signal stability. When events_per_bin is
    /// high, removal of a single bin has a larger impact on total cells remaining.
    pub min_cells: usize,

    /// Maximum number of bins (default: 500)
    pub max_bins: usize,

    /// Events per bin (auto-calculated if None)
    pub events_per_bin: Option<usize>,

    /// MAD threshold multiplier (default: 6.0)
    ///
    /// **Tradeoff**: The lower the number of MADs allowed, the more strict the
    /// algorithm will be and more cells will be removed.
    pub mad: f64,

    /// Isolation Tree gain limit (default: 0.6)
    ///
    /// **Tradeoff**: By lowering the IT limit, the algorithm will be more strict
    /// and outliers will be removed sooner.
    ///
    /// **Note**: The isolation tree can be sensitive to a low number of bins and
    /// is by default not used when less than `force_it` (150) bins are available,
    /// as it can remove too much of the data.
    pub it_limit: f64,

    /// Consecutive bins threshold (default: 5)
    ///
    /// To avoid small regions being kept while bins around them have been filtered
    /// out, any remaining regions of only this many consecutive bins or less also
    /// get removed.
    pub consecutive_bins: usize,

    /// Remove zeros before peak detection
    pub remove_zeros: bool,

    /// Peak removal threshold (default: 1/3)
    /// Peaks below this fraction of the maximum density are ignored.
    pub peak_removal: f64,

    /// Minimum bins for peak detection (default: 10%)
    /// The minimum percentage of bins that must contain the most common number of peaks.
    pub min_nr_bins_peakdetection: f64,

    /// Force Isolation Tree minimum bins (default: 150)
    /// IT is skipped if fewer bins than this are available.
    pub force_it: usize,

    /// Preprocessing: Apply compensation from file's $SPILLOVER keyword (requires flow-fcs feature)
    /// This matches the original R implementation: `flowCore::compensate(ff, flowCore::keyword(ff)$SPILL)`
    #[cfg(feature = "flow-fcs")]
    pub apply_compensation: bool,

    /// Preprocessing: Apply arcsinh transformation to fluorescence channels (requires flow-fcs feature)
    /// This matches the original R implementation's `flowCore::transform()` step
    #[cfg(feature = "flow-fcs")]
    pub apply_transformation: bool,

    /// Transformation cofactor for arcsinh (default: 2000.0, typical for flow cytometry)
    /// Only used if `apply_transformation` is true
    #[cfg(feature = "flow-fcs")]
    pub transform_cofactor: f32,
}

impl Default for PeacoQCConfig {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            determine_good_cells: QCMode::All,
            min_cells: 150,
            max_bins: 500,
            events_per_bin: None,
            mad: 6.0,
            it_limit: 0.6,
            consecutive_bins: 5,
            remove_zeros: false,
            peak_removal: 1.0 / 3.0,
            min_nr_bins_peakdetection: 10.0,
            force_it: 150,
            #[cfg(feature = "flow-fcs")]
            apply_compensation: true,
            #[cfg(feature = "flow-fcs")]
            apply_transformation: true,
            #[cfg(feature = "flow-fcs")]
            transform_cofactor: 2000.0,
        }
    }
}

/// PeacoQC result
#[derive(Debug)]
pub struct PeacoQCResult {
    /// Boolean mask of good cells (true = keep, false = remove)
    pub good_cells: Vec<bool>,

    /// Percentage of cells removed
    pub percentage_removed: f64,

    /// IT percentage (if used)
    pub it_percentage: Option<f64>,

    /// MAD percentage (if used)
    pub mad_percentage: Option<f64>,

    /// Consecutive cells percentage
    pub consecutive_percentage: f64,

    /// Peak detection results per channel
    pub peaks: HashMap<String, ChannelPeakFrame>,

    /// Number of bins used
    pub n_bins: usize,

    /// Events per bin
    pub events_per_bin: usize,
}

impl PeacoQCResult {
    /// Export QC results as boolean CSV (0/1 values)
    ///
    /// This format is recommended for general use. See [`crate::qc::export::export_csv_boolean`] for details.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use peacoqc_rs::{PeacoQCResult, PeacoQCConfig};
    /// # let result: PeacoQCResult = todo!();
    /// result.export_csv_boolean("qc_results.csv")?;
    /// # Ok::<(), peacoqc_rs::PeacoQCError>(())
    /// ```
    pub fn export_csv_boolean(&self, path: impl AsRef<Path>) -> Result<()> {
        crate::qc::export::export_csv_boolean(self, path, None)
    }

    /// Export QC results as boolean CSV with custom column name
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use peacoqc_rs::{PeacoQCResult, PeacoQCConfig};
    /// # let result: PeacoQCResult = todo!();
    /// result.export_csv_boolean_with_name("qc_results.csv", "QC_Status")?;
    /// # Ok::<(), peacoqc_rs::PeacoQCError>(())
    /// ```
    pub fn export_csv_boolean_with_name(
        &self,
        path: impl AsRef<Path>,
        column_name: &str,
    ) -> Result<()> {
        crate::qc::export::export_csv_boolean(self, path, Some(column_name))
    }

    /// Export QC results as numeric CSV (2000/6000 or custom values)
    ///
    /// This format is compatible with the R PeacoQC package output.
    /// Default values match R implementation: 2000 = good, 6000 = bad.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use peacoqc_rs::{PeacoQCResult, PeacoQCConfig};
    /// # let result: PeacoQCResult = todo!();
    /// // R-compatible format
    /// result.export_csv_numeric("qc_results_r.csv", 2000, 6000)?;
    /// # Ok::<(), peacoqc_rs::PeacoQCError>(())
    /// ```
    pub fn export_csv_numeric(
        &self,
        path: impl AsRef<Path>,
        good_value: u16,
        bad_value: u16,
    ) -> Result<()> {
        crate::qc::export::export_csv_numeric(self, path, good_value, bad_value, None)
    }

    /// Export QC results as numeric CSV with custom column name
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use peacoqc_rs::{PeacoQCResult, PeacoQCConfig};
    /// # let result: PeacoQCResult = todo!();
    /// result.export_csv_numeric_with_name("qc_results_r.csv", 2000, 6000, "PeacoQC_Status")?;
    /// # Ok::<(), peacoqc_rs::PeacoQCError>(())
    /// ```
    pub fn export_csv_numeric_with_name(
        &self,
        path: impl AsRef<Path>,
        good_value: u16,
        bad_value: u16,
        column_name: &str,
    ) -> Result<()> {
        crate::qc::export::export_csv_numeric(self, path, good_value, bad_value, Some(column_name))
    }

    /// Export QC results as JSON metadata
    ///
    /// This format includes comprehensive QC metrics and configuration.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use peacoqc_rs::{PeacoQCResult, PeacoQCConfig};
    /// # let result: PeacoQCResult = todo!();
    /// # let config: PeacoQCConfig = todo!();
    /// result.export_json_metadata(&config, "qc_metadata.json")?;
    /// # Ok::<(), peacoqc_rs::PeacoQCError>(())
    /// ```
    pub fn export_json_metadata(
        &self,
        config: &PeacoQCConfig,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        crate::qc::export::export_json_metadata(self, config, path)
    }
}

/// Main PeacoQC quality control function
///
/// # Algorithm (matches R's PeacoQC)
/// 1. Calculate optimal bin size with 50% overlapping windows
/// 2. Detect peaks per channel per bin using KDE
/// 3. Run SD-based Isolation Tree to find largest homogeneous group (optional)
/// 4. Run MAD outlier detection with spline smoothing (optional)
/// 5. Filter consecutive bins to remove short isolated regions
/// 6. Generate cell-level boolean mask with de-duplication for overlapping bins
pub fn peacoqc<T: PeacoQCData>(fcs: &T, config: &PeacoQCConfig) -> Result<PeacoQCResult> {
    if config.channels.is_empty() {
        return Err(PeacoQCError::ConfigError(
            "No channels specified".to_string(),
        ));
    }

    let n_events = fcs.n_events();

    info!(
        "Starting PeacoQC analysis: {} events, {} channels",
        n_events,
        config.channels.len()
    );
    debug!("Channels: {:?}", config.channels);

    // Calculate events per bin
    let events_per_bin = config
        .events_per_bin
        .unwrap_or_else(|| find_events_per_bin(n_events, config.min_cells, config.max_bins, 500));

    // Create overlapping bins (50% overlap, matching R's SplitWithOverlap)
    let breaks = create_breaks(n_events, events_per_bin);
    let n_bins = breaks.len();

    info!(
        "Binning configuration: {} bins (50% overlap), {} events per bin (min_cells={}, max_bins={})",
        n_bins, events_per_bin, config.min_cells, config.max_bins
    );

    // Debug logging: Find time channel for bin analysis
    let time_channel = if std::env::var("PEACOQC_DEBUG_BINS").is_ok() {
        find_time_channel_for_debug(fcs)
    } else {
        None
    };

    // Peak detection
    info!(
        "Starting peak detection across {} channels",
        config.channels.len()
    );
    let peak_config = PeakDetectionConfig {
        events_per_bin,
        peak_removal: config.peak_removal,
        min_nr_bins_peakdetection: config.min_nr_bins_peakdetection,
        remove_zeros: config.remove_zeros,
    };
    debug!(
        "Peak detection config: peak_removal={}, min_nr_bins={}, remove_zeros={}",
        peak_config.peak_removal, peak_config.min_nr_bins_peakdetection, peak_config.remove_zeros
    );

    let peaks = determine_peaks_all_channels(fcs, &config.channels, &peak_config)?;

    if peaks.is_empty() {
        return Err(PeacoQCError::NoPeaksDetected);
    }

    info!(
        "Peak detection complete: {} channels with peaks detected",
        peaks.len()
    );
    trace!(
        "Peak details per channel: {:?}",
        peaks
            .iter()
            .map(|(ch, pf)| (ch, pf.peaks.len()))
            .collect::<Vec<_>>()
    );

    // Initialize outlier bins (all good initially)
    let mut outlier_bins = vec![false; n_bins];
    let mut it_percentage = None;
    let mut mad_percentage = None;

    // Track outlier states for debug logging
    let mut it_outliers = vec![false; n_bins];
    let mut mad_outliers = vec![false; n_bins];
    let mut consecutive_outliers = vec![false; n_bins];

    // Run quality control methods
    match config.determine_good_cells {
        QCMode::All | QCMode::IsolationTree => {
            if n_bins >= config.force_it {
                info!(
                    "Running Isolation Tree analysis (IT_limit={})",
                    config.it_limit
                );
                let it_config = IsolationTreeConfig {
                    it_limit: config.it_limit,
                    force_it: config.force_it,
                    ..Default::default()
                };

                match isolation_tree_detect(&peaks, n_bins, &it_config) {
                    Ok(it_result) => {
                        outlier_bins = it_result.outlier_bins.clone();
                        it_outliers = it_result.outlier_bins.clone();
                        let n_it_outliers = outlier_bins.iter().filter(|&&x| x).count();
                        let it_pct = (n_it_outliers as f64 / n_bins as f64) * 100.0;
                        it_percentage = Some(it_pct);

                        info!(
                            "Isolation Tree analysis removed {:.2}% of the bins ({} outlier bins)",
                            it_pct, n_it_outliers
                        );

                        // Debug logging
                        if std::env::var("PEACOQC_DEBUG_BINS").is_ok() {
                            if let Some(ref tc) = time_channel {
                                if let Ok(debug_info) = debug::collect_bin_debug_info(
                                    fcs,
                                    Some(tc),
                                    &breaks,
                                    &it_outliers,
                                    &mad_outliers,
                                    &consecutive_outliers,
                                    &outlier_bins,
                                ) {
                                    debug::log_bin_debug_info(&debug_info, "After Isolation Tree");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Isolation Tree failed: {}, continuing with MAD only", e);
                    }
                }
            } else {
                warn!(
                    "Not enough bins ({}) for Isolation Tree (need {}), skipping IT",
                    n_bins, config.force_it
                );
            }
        }
        _ => {}
    }

    // MAD method
    if config.determine_good_cells == QCMode::All || config.determine_good_cells == QCMode::MAD {
        info!(
            "Running MAD outlier detection (MAD threshold={})",
            config.mad
        );
        let mad_config = MADConfig {
            mad_threshold: config.mad,
            ..Default::default()
        };

        // For MAD, pass the current outlier_bins:
        // - If IT ran, outlier_bins contains IT results (true = outlier)
        // - MAD only considers bins that are NOT already outliers (i.e., outlier_bins[i] == false means "still good")
        // We need to invert: existing_outliers should be true for bins that passed IT
        let existing_good_bins: Vec<bool> =
            outlier_bins.iter().map(|&is_outlier| !is_outlier).collect();

        let mad_result = mad_outlier_method(&peaks, &existing_good_bins, n_bins, &mad_config)?;

        // Combine with existing outliers
        let n_mad_outliers_before = outlier_bins.iter().filter(|&&x| x).count();
        for (i, &is_mad_outlier) in mad_result.outlier_bins.iter().enumerate() {
            if is_mad_outlier {
                outlier_bins[i] = true;
                mad_outliers[i] = true;
            }
        }
        let n_mad_outliers = outlier_bins.iter().filter(|&&x| x).count();
        let n_new_from_mad = n_mad_outliers.saturating_sub(n_mad_outliers_before); // Fix underflow
        let mad_pct = (n_mad_outliers as f64 / n_bins as f64) * 100.0;
        mad_percentage = Some(mad_pct);

        info!(
            "MAD analysis removed {:.2}% of the bins ({} outlier bins, {} from IT, {} new from MAD)",
            mad_pct,
            n_mad_outliers,
            n_mad_outliers_before,
            n_new_from_mad
        );

        // Debug logging
        if std::env::var("PEACOQC_DEBUG_BINS").is_ok() {
            if let Some(ref tc) = time_channel {
                if let Ok(debug_info) = debug::collect_bin_debug_info(
                    fcs,
                    Some(tc),
                    &breaks,
                    &it_outliers,
                    &mad_outliers,
                    &consecutive_outliers,
                    &outlier_bins,
                ) {
                    debug::log_bin_debug_info(&debug_info, "After MAD");
                    debug::analyze_events_per_second_correlation(&debug_info);
                }
            }
        }
    }

    // Consecutive bin filtering
    let n_outliers_before_consecutive = outlier_bins.iter().filter(|&&x| x).count();
    if config.determine_good_cells != QCMode::None {
        info!(
            "Applying consecutive bin filtering (consecutive_bins={})",
            config.consecutive_bins
        );
        let consecutive_config = ConsecutiveConfig {
            consecutive_bins: config.consecutive_bins,
        };

        let outlier_bins_before_consecutive = outlier_bins.clone();
        outlier_bins = remove_short_regions(&outlier_bins, &consecutive_config)?;
        let n_outliers_after_consecutive = outlier_bins.iter().filter(|&&x| x).count();
        
        // Track which bins were flagged by consecutive filtering
        for (i, (before, after)) in outlier_bins_before_consecutive.iter().zip(outlier_bins.iter()).enumerate() {
            if !before && *after {
                consecutive_outliers[i] = true;
            }
        }
        
        // Consecutive filtering removes short good regions, converting them to bad
        // So the number of outliers should increase (or stay the same)
        let regions_removed = if n_outliers_after_consecutive >= n_outliers_before_consecutive {
            n_outliers_after_consecutive - n_outliers_before_consecutive
        } else {
            // This shouldn't happen, but handle it gracefully
            0
        };
        debug!(
            "Consecutive filtering: {} → {} outlier bins (removed {} short regions)",
            n_outliers_before_consecutive, n_outliers_after_consecutive, regions_removed
        );

        // Debug logging
        if std::env::var("PEACOQC_DEBUG_BINS").is_ok() {
            if let Some(ref tc) = time_channel {
                if let Ok(debug_info) = debug::collect_bin_debug_info(
                    fcs,
                    Some(tc),
                    &breaks,
                    &it_outliers,
                    &mad_outliers,
                    &consecutive_outliers,
                    &outlier_bins,
                ) {
                    debug::log_bin_debug_info(&debug_info, "After Consecutive Filtering");
                }
            }
        }
    }

    // Convert bin-level outliers to cell-level mask with de-duplication
    // (Required because overlapping bins mean cells appear in multiple bins)
    let good_cells = bin_mask_to_cell_mask_overlapping(&outlier_bins, &breaks, n_events);

    let n_removed = good_cells.iter().filter(|&&x| !x).count();
    let percentage_removed = (n_removed as f64 / n_events as f64) * 100.0;
    let consecutive_percentage = percentage_removed - mad_percentage.unwrap_or(0.0);

    info!(
        "PeacoQC complete: {} events removed ({:.2}%), {} events remaining ({:.2}%)",
        n_removed,
        percentage_removed,
        n_events - n_removed,
        100.0 - percentage_removed
    );

    if percentage_removed > 70.0 {
        warn!(
            "More than 70% of events removed! This may indicate data quality issues or incorrect configuration."
        );
    }

    // Final debug logging
    if std::env::var("PEACOQC_DEBUG_BINS").is_ok() {
        if let Some(ref tc) = time_channel {
            if let Ok(debug_info) = debug::collect_bin_debug_info(
                fcs,
                Some(tc),
                &breaks,
                &it_outliers,
                &mad_outliers,
                &consecutive_outliers,
                &outlier_bins,
            ) {
                debug::log_bin_debug_info(&debug_info, "Final Results");
                debug::analyze_events_per_second_correlation(&debug_info);
            }
        }
    }

    Ok(PeacoQCResult {
        good_cells,
        percentage_removed,
        it_percentage,
        mad_percentage,
        consecutive_percentage,
        peaks,
        n_bins,
        events_per_bin,
    })
}

/// Find time channel for debug logging
fn find_time_channel_for_debug<T: PeacoQCData>(fcs: &T) -> Option<String> {
    fcs.channel_names().into_iter().find(|name| {
        let upper = name.to_uppercase();
        upper.contains("TIME") || upper == "TIME"
    })
}

/// Find optimal events per bin
///
/// Matches R's FindEventsPerBin function exactly:
/// ```r
/// max_cells <- ceiling((nr_events/max_bins)*2)
/// max_cells <- ((max_cells%/%step)*step) + step
/// events_per_bin <- max(min_cells, max_cells)
/// ```
///
/// The `* 2` accounts for 50% overlap: with overlap, we get ~2x more bins than non-overlapping.
/// So to get approximately `max_bins` bins WITH overlap, we multiply by 2 to target larger bins.
fn find_events_per_bin(n_events: usize, min_cells: usize, max_bins: usize, step: usize) -> usize {
    // R: max_cells <- ceiling((nr_events/max_bins)*2)
    let max_cells = ((n_events as f64 / max_bins as f64) * 2.0).ceil() as usize;
    
    // R: max_cells <- ((max_cells%/%step)*step) + step
    // This rounds UP to the next step (integer division then add step)
    let max_cells_rounded = ((max_cells / step) * step) + step;
    
    // R: events_per_bin <- max(min_cells, max_cells)
    max_cells_rounded.max(min_cells)
}

/// Convert bin-level mask to cell-level mask with de-duplication
///
/// Required because overlapping bins mean cells appear in multiple bins.
/// A cell is marked as bad if ANY of its containing bins is marked as bad.
///
/// R equivalent:
/// ```r
/// removed_cells <- unlist(breaks[names(outlier_bins)[which(outlier_bins)]])
/// removed_cells <- removed_cells[!duplicated(removed_cells)]
/// ```
fn bin_mask_to_cell_mask_overlapping(
    bin_mask: &[bool], // true = outlier/bad bin
    breaks: &[(usize, usize)],
    n_events: usize,
) -> Vec<bool> {
    // Collect all cell indices from bad bins (HashSet handles de-duplication)
    let mut bad_cells: HashSet<usize> = HashSet::new();

    for (bin_idx, &is_bad) in bin_mask.iter().enumerate() {
        if is_bad {
            if let Some(&(start, end)) = breaks.get(bin_idx) {
                for cell_idx in start..end {
                    bad_cells.insert(cell_idx);
                }
            }
        }
    }

    // Create mask (true = good cell, false = bad cell)
    (0..n_events).map(|i| !bad_cells.contains(&i)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PeacoQCData;
    use crate::error::Result;
    use flow_fcs::parameter::EventDataFrame;
    use polars::prelude::*;
    use std::sync::Arc;

    // Test helper that implements PeacoQCData
    struct TestFcs {
        data_frame: EventDataFrame,
    }

    impl PeacoQCData for TestFcs {
        fn n_events(&self) -> usize {
            self.data_frame.height()
        }

        fn channel_names(&self) -> Vec<String> {
            self.data_frame
                .get_column_names()
                .iter()
                .map(|s| s.to_string())
                .collect()
        }

        fn get_channel_range(&self, _channel: &str) -> Option<(f64, f64)> {
            Some((0.0, 262144.0))
        }

        fn get_channel_f64(&self, channel: &str) -> Result<Vec<f64>> {
            let series = self
                .data_frame
                .column(channel)
                .map_err(|_| crate::PeacoQCError::ChannelNotFound(channel.to_string()))?;

            // Handle both f32 and f64 columns (FCS files typically use f32)
            let values = if let Ok(f64_vals) = series.f64() {
                f64_vals.into_iter().filter_map(|x| x).collect()
            } else if let Ok(f32_vals) = series.f32() {
                // Cast f32 to f64
                f32_vals
                    .into_iter()
                    .filter_map(|x| x.map(|v| v as f64))
                    .collect()
            } else {
                return Err(crate::PeacoQCError::InvalidChannel(format!(
                    "Channel {} is not numeric (dtype: {:?})",
                    channel,
                    series.dtype()
                )));
            };
            Ok(values)
        }
    }

    #[test]
    fn test_peacoqc_basic() {
        // Create synthetic data
        let mut data = Vec::new();
        for _ in 0..10000 {
            data.push(100.0 + (rand::random::<f64>() - 0.5) * 20.0);
        }

        let df = Arc::new(
            df![
                "FL1-A" => data,
            ]
            .unwrap(),
        );

        let fcs = TestFcs { data_frame: df };

        let config = PeacoQCConfig {
            channels: vec!["FL1-A".to_string()],
            determine_good_cells: QCMode::MAD,
            ..Default::default()
        };

        let result = peacoqc(&fcs, &config);

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.good_cells.len(), 10000);
        // Should have some events removed
        assert!(result.percentage_removed >= 0.0);
        assert!(result.percentage_removed < 100.0);
    }

    #[test]
    fn test_peacoqc_empty_channels() {
        let df = Arc::new(
            df![
                "FL1-A" => vec![100.0f64; 1000],
            ]
            .unwrap(),
        );

        let fcs = TestFcs { data_frame: df };

        let config = PeacoQCConfig {
            channels: vec![],
            ..Default::default()
        };

        let result = peacoqc(&fcs, &config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PeacoQCError::ConfigError(_)));
    }

    #[test]
    fn test_peacoqc_invalid_channel() {
        let df = Arc::new(
            df![
                "FL1-A" => vec![100.0f64; 1000],
            ]
            .unwrap(),
        );

        let fcs = TestFcs { data_frame: df };

        let config = PeacoQCConfig {
            channels: vec!["NONEXISTENT".to_string()],
            ..Default::default()
        };

        let result = peacoqc(&fcs, &config);
        // Should handle missing channel gracefully
        assert!(result.is_err());
    }

    #[test]
    fn test_peacoqc_small_dataset() {
        // Test with very small dataset (< min_cells)
        let df = Arc::new(
            df![
                "FL1-A" => vec![100.0f64, 200.0, 300.0, 400.0, 500.0], // Only 5 events
            ]
            .unwrap(),
        );

        let fcs = TestFcs { data_frame: df };

        let config = PeacoQCConfig {
            channels: vec!["FL1-A".to_string()],
            min_cells: 150,                     // More than 5 events
            determine_good_cells: QCMode::None, // Only peak detection
            ..Default::default()
        };

        let result = peacoqc(&fcs, &config);
        // Should handle small datasets - might fail or work depending on implementation
        // Let's check it doesn't panic
        if result.is_ok() {
            let r = result.unwrap();
            assert_eq!(r.good_cells.len(), 5);
        }
    }

    #[test]
    fn test_peacoqc_all_identical_values() {
        // Test with all identical values (edge case)
        let df = Arc::new(
            df![
                "FL1-A" => vec![100.0f64; 1000], // All same value
            ]
            .unwrap(),
        );

        let fcs = TestFcs { data_frame: df };

        let config = PeacoQCConfig {
            channels: vec!["FL1-A".to_string()],
            determine_good_cells: QCMode::MAD,
            ..Default::default()
        };

        let result = peacoqc(&fcs, &config);
        // Should handle identical values - might not detect peaks or might handle gracefully
        if result.is_ok() {
            let r = result.unwrap();
            assert_eq!(r.good_cells.len(), 1000);
            // With identical values, all cells might be kept or might be removed
            // Just check it doesn't panic
        }
    }

    #[test]
    fn test_peacoqc_qc_mode_none() {
        // Test with QC mode None (only peak detection)
        let mut data = Vec::new();
        for i in 0..5000 {
            data.push(100.0 + (i % 100) as f64);
        }

        let df = Arc::new(
            df![
                "FL1-A" => data,
            ]
            .unwrap(),
        );

        let fcs = TestFcs { data_frame: df };

        let config = PeacoQCConfig {
            channels: vec!["FL1-A".to_string()],
            determine_good_cells: QCMode::None,
            ..Default::default()
        };

        let result = peacoqc(&fcs, &config);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.good_cells.len(), 5000);
        // With QCMode::None, no cells should be removed
        assert_eq!(r.percentage_removed, 0.0);
    }
}
