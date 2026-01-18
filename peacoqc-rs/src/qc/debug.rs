//! Debug logging utilities for investigating QC discrepancies
//!
//! This module provides detailed logging to help diagnose differences between
//! Rust and R implementations, particularly around low events/second periods.

use crate::PeacoQCData;
use crate::error::Result;
use std::collections::HashMap;

/// Debug information about bin distribution and QC steps
#[derive(Debug, Clone)]
pub struct BinDebugInfo {
    /// Bin index
    pub bin_idx: usize,
    /// Start cell index
    pub start_cell: usize,
    /// End cell index
    pub end_cell: usize,
    /// Start time (if time channel available)
    pub start_time: Option<f64>,
    /// End time (if time channel available)
    pub end_time: Option<f64>,
    /// Events per second for this bin (if time channel available)
    pub events_per_second: Option<f64>,
    /// Number of cells in this bin
    pub n_cells: usize,
    /// Flagged by Isolation Tree
    pub flagged_by_it: bool,
    /// Flagged by MAD
    pub flagged_by_mad: bool,
    /// Flagged by consecutive filtering
    pub flagged_by_consecutive: bool,
    /// Final status (true = outlier/bad)
    pub is_outlier: bool,
}

/// Calculate events per second for each bin
pub fn calculate_bin_events_per_second<T: PeacoQCData>(
    fcs: &T,
    time_channel: &str,
    breaks: &[(usize, usize)],
) -> Result<Vec<Option<f64>>> {
    let time_values = match fcs.get_channel_f64(time_channel) {
        Ok(times) => times,
        Err(_) => return Ok(vec![None; breaks.len()]),
    };

    let mut bin_rates = Vec::new();

    for (start, end) in breaks {
        if *start >= time_values.len() || *end > time_values.len() {
            bin_rates.push(None);
            continue;
        }

        let bin_times: Vec<f64> = time_values[*start..(*end).min(time_values.len())].to_vec();
        if bin_times.len() < 2 {
            bin_rates.push(None);
            continue;
        }

        let time_start = bin_times.first().copied().unwrap_or(0.0);
        let time_end = bin_times.last().copied().unwrap_or(time_start);
        let time_span = time_end - time_start;

        let rate = if time_span > 0.0 {
            Some((bin_times.len() as f64) / time_span)
        } else {
            None
        };

        bin_rates.push(rate);
    }

    Ok(bin_rates)
}

/// Get time range for each bin
pub fn get_bin_time_ranges<T: PeacoQCData>(
    fcs: &T,
    time_channel: &str,
    breaks: &[(usize, usize)],
) -> Result<Vec<(Option<f64>, Option<f64>)>> {
    let time_values = match fcs.get_channel_f64(time_channel) {
        Ok(times) => times,
        Err(_) => return Ok(vec![(None, None); breaks.len()]),
    };

    let mut ranges = Vec::new();

    for (start, end) in breaks {
        if *start >= time_values.len() || *end > time_values.len() {
            ranges.push((None, None));
            continue;
        }

        let bin_times: Vec<f64> = time_values[*start..(*end).min(time_values.len())].to_vec();
        if bin_times.is_empty() {
            ranges.push((None, None));
            continue;
        }

        let time_start = bin_times.first().copied();
        let time_end = bin_times.last().copied();
        ranges.push((time_start, time_end));
    }

    Ok(ranges)
}

/// Collect debug information for all bins
pub fn collect_bin_debug_info<T: PeacoQCData>(
    fcs: &T,
    time_channel: Option<&str>,
    breaks: &[(usize, usize)],
    it_outliers: &[bool],
    mad_outliers: &[bool],
    consecutive_outliers: &[bool],
    final_outliers: &[bool],
) -> Result<Vec<BinDebugInfo>> {
    let mut debug_info = Vec::new();

    // Get time information if available
    let time_ranges = if let Some(tc) = time_channel {
        get_bin_time_ranges(fcs, tc, breaks).ok()
    } else {
        None
    };

    let events_per_second = if let Some(tc) = time_channel {
        calculate_bin_events_per_second(fcs, tc, breaks).ok()
    } else {
        None
    };

    for (bin_idx, (start, end)) in breaks.iter().enumerate() {
        let n_cells = end - start;
        let (start_time, end_time) = time_ranges
            .as_ref()
            .and_then(|r| r.get(bin_idx).copied())
            .unwrap_or((None, None));
        let eps = events_per_second
            .as_ref()
            .and_then(|r| r.get(bin_idx).copied())
            .flatten();

        debug_info.push(BinDebugInfo {
            bin_idx,
            start_cell: *start,
            end_cell: *end,
            start_time,
            end_time,
            events_per_second: eps,
            n_cells,
            flagged_by_it: it_outliers.get(bin_idx).copied().unwrap_or(false),
            flagged_by_mad: mad_outliers.get(bin_idx).copied().unwrap_or(false),
            flagged_by_consecutive: consecutive_outliers.get(bin_idx).copied().unwrap_or(false),
            is_outlier: final_outliers.get(bin_idx).copied().unwrap_or(false),
        });
    }

    Ok(debug_info)
}

/// Log bin debug information to stderr
pub fn log_bin_debug_info(debug_info: &[BinDebugInfo], step: &str) {
    eprintln!("\n=== Bin Debug Info: {} ===", step);
    eprintln!(
        "{:>6} {:>10} {:>10} {:>12} {:>12} {:>12} {:>6} {:>4} {:>4} {:>4} {:>4}",
        "Bin", "Start", "End", "StartTime", "EndTime", "Events/sec", "Cells", "IT", "MAD", "Con", "Out"
    );

    for info in debug_info.iter().take(100) {
        // Only show first 100 bins to avoid overwhelming output
        let start_time_str = info.start_time.map(|t| format!("{:.2}", t)).unwrap_or_else(|| "N/A".to_string());
        let end_time_str = info.end_time.map(|t| format!("{:.2}", t)).unwrap_or_else(|| "N/A".to_string());
        let eps_str = info.events_per_second.map(|r| format!("{:.2}", r)).unwrap_or_else(|| "N/A".to_string());

        eprintln!(
            "{:>6} {:>10} {:>10} {:>12} {:>12} {:>12} {:>6} {:>4} {:>4} {:>4} {:>4}",
            info.bin_idx,
            info.start_cell,
            info.end_cell,
            start_time_str,
            end_time_str,
            eps_str,
            info.n_cells,
            if info.flagged_by_it { "X" } else { "" },
            if info.flagged_by_mad { "X" } else { "" },
            if info.flagged_by_consecutive { "X" } else { "" },
            if info.is_outlier { "X" } else { "" },
        );
    }

    if debug_info.len() > 100 {
        eprintln!("... (showing first 100 of {} bins)", debug_info.len());
    }

    // Summary statistics
    let total_bins = debug_info.len();
    let bins_with_time = debug_info.iter().filter(|i| i.start_time.is_some()).count();
    
    if bins_with_time > 0 {
        let low_eps_bins: Vec<_> = debug_info
            .iter()
            .filter(|i| {
                i.events_per_second
                    .map(|eps| eps < 1000.0) // Threshold for "low" events/second
                    .unwrap_or(false)
            })
            .collect();

        let low_eps_outliers = low_eps_bins.iter().filter(|i| i.is_outlier).count();
        let low_eps_total = low_eps_bins.len();

        eprintln!("\nSummary:");
        eprintln!("  Total bins: {}", total_bins);
        eprintln!("  Bins with time info: {}", bins_with_time);
        eprintln!("  Low events/sec bins (<1000): {}", low_eps_total);
        eprintln!("  Low events/sec bins flagged as outliers: {} ({:.1}%)",
            low_eps_outliers,
            if low_eps_total > 0 {
                (low_eps_outliers as f64 / low_eps_total as f64) * 100.0
            } else {
                0.0
            }
        );

        // Calculate average events/second for outlier vs non-outlier bins
        let outlier_eps: Vec<f64> = debug_info
            .iter()
            .filter_map(|i| if i.is_outlier { i.events_per_second } else { None })
            .collect();
        let good_eps: Vec<f64> = debug_info
            .iter()
            .filter_map(|i| if !i.is_outlier { i.events_per_second } else { None })
            .collect();

        if !outlier_eps.is_empty() {
            let avg_outlier_eps = outlier_eps.iter().sum::<f64>() / outlier_eps.len() as f64;
            eprintln!("  Average events/sec for outlier bins: {:.2}", avg_outlier_eps);
        }
        if !good_eps.is_empty() {
            let avg_good_eps = good_eps.iter().sum::<f64>() / good_eps.len() as f64;
            eprintln!("  Average events/sec for good bins: {:.2}", avg_good_eps);
        }
    }
    eprintln!("=== End Bin Debug Info ===\n");
}

/// Analyze correlation between events/second and outlier status
pub fn analyze_events_per_second_correlation(debug_info: &[BinDebugInfo]) {
    let bins_with_eps: Vec<_> = debug_info
        .iter()
        .filter_map(|i| i.events_per_second.map(|eps| (eps, i.is_outlier)))
        .collect();

    if bins_with_eps.is_empty() {
        eprintln!("No bins with events/second data available for correlation analysis");
        return;
    }

    // Group by events/second ranges
    let mut ranges: HashMap<&str, (usize, usize)> = HashMap::new();
    ranges.insert("0-500", (0, 0));
    ranges.insert("500-1000", (0, 0));
    ranges.insert("1000-2000", (0, 0));
    ranges.insert("2000+", (0, 0));

    for (eps, is_outlier) in &bins_with_eps {
        let range = if *eps < 500.0 {
            "0-500"
        } else if *eps < 1000.0 {
            "500-1000"
        } else if *eps < 2000.0 {
            "1000-2000"
        } else {
            "2000+"
        };

        let (total, outliers) = ranges.get_mut(range).unwrap();
        *total += 1;
        if *is_outlier {
            *outliers += 1;
        }
    }

    eprintln!("\n=== Events/Second vs Outlier Status ===");
    eprintln!("{:>12} {:>10} {:>10} {:>10}", "Range", "Total", "Outliers", "% Outlier");
    
    // Print in order
    let ordered_ranges = vec!["0-500", "500-1000", "1000-2000", "2000+"];
    for range in ordered_ranges {
        if let Some((total, outliers)) = ranges.get(range) {
            if *total > 0 {
                let pct = (*outliers as f64 / *total as f64) * 100.0;
                eprintln!("{:>12} {:>10} {:>10} {:>10.1}", range, total, outliers, pct);
            }
        }
    }
    eprintln!("=== End Correlation Analysis ===\n");
}
