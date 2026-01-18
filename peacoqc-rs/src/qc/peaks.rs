use crate::PeacoQCData;
use crate::error::{PeacoQCError, Result};
use crate::stats::density::KernelDensity;
use crate::stats::median;
use rayon::prelude::*;
use std::collections::HashMap;

/// Configuration for peak detection
#[derive(Debug, Clone)]
pub struct PeakDetectionConfig {
    /// Number of events per bin
    pub events_per_bin: usize,

    /// Minimum peak height as fraction of max density (default: 1/3)
    pub peak_removal: f64,

    /// Minimum percentage of bins that must contain the most common number of peaks
    pub min_nr_bins_peakdetection: f64,

    /// Whether to remove zeros before peak detection
    pub remove_zeros: bool,
}

impl Default for PeakDetectionConfig {
    fn default() -> Self {
        Self {
            events_per_bin: 1000,
            peak_removal: 1.0 / 3.0,
            min_nr_bins_peakdetection: 10.0,
            remove_zeros: false,
        }
    }
}

/// Peak information for a single bin
#[derive(Debug, Clone)]
pub struct PeakInfo {
    pub bin: usize,
    pub peak_value: f64,
    pub cluster: usize,
}

/// Peak detection results for a channel
#[derive(Debug, Clone)]
pub struct ChannelPeakFrame {
    pub peaks: Vec<PeakInfo>,
}

/// Determine peaks for all channels
///
/// # Algorithm
/// 1. Split data into bins of `events_per_bin` events
/// 2. For each bin and channel, compute KDE and find peaks
/// 3. Cluster peaks across bins using median clustering
/// 4. Remove clusters present in <50% of bins
///
/// # Performance
/// This function processes channels in parallel using `rayon`, providing significant
/// performance improvements on multi-core systems. Each channel's bins are also
/// processed in parallel for maximum throughput.
pub fn determine_peaks_all_channels<T: PeacoQCData>(
    fcs: &T,
    channels: &[String],
    config: &PeakDetectionConfig,
) -> Result<HashMap<String, ChannelPeakFrame>> {
    let mut results = HashMap::new();

    // Create bins with 50% overlap (matching R's SplitWithOverlap)
    let n_events = fcs.n_events();
    let breaks = create_breaks(n_events, config.events_per_bin);
    let n_bins = breaks.len();

    if n_bins == 0 {
        return Err(PeacoQCError::InsufficientData {
            min: config.events_per_bin,
            actual: n_events,
        });
    }

    eprintln!("Calculating peaks for {} channels...", channels.len());

    // Collect channel data first (sequential, as it may not be thread-safe)
    // Then process peaks in parallel
    let channel_data: Vec<(String, Vec<f64>)> = channels
        .iter()
        .filter_map(|ch| fcs.get_channel_f64(ch).ok().map(|data| (ch.clone(), data)))
        .collect();

    // Process channels in parallel
    let channel_results: Vec<(String, Option<ChannelPeakFrame>)> = channel_data
        .par_iter()
        .map(|(channel, data)| {
            let peak_frame = determine_channel_peaks_from_data(data, &breaks, config);
            (channel.clone(), peak_frame)
        })
        .collect();

    // Collect results into HashMap
    for (channel, frame) in channel_results {
        if let Some(frame) = frame {
            results.insert(channel, frame);
        }
    }

    Ok(results)
}

/// Create bin boundaries with 50% overlap (matching R's SplitWithOverlap)
///
/// R equivalent:
/// ```r
/// SplitWithOverlap <- function(vec, seg.length, overlap) {
///     starts=seq(1, length(vec), by=seg.length-overlap)
///     ends  =starts + seg.length - 1
///     ends[ends > length(vec)]=length(vec)
/// }
/// # Called with: overlap = ceiling(events_per_bin/2)
/// ```
///
/// The overlap is 50% of the bin size (ceiling), which means:
/// - Adjacent bins share half their events
/// - This creates ~2x more bins than non-overlapping
/// - Provides smoother signal stability detection
pub fn create_breaks(n_events: usize, events_per_bin: usize) -> Vec<(usize, usize)> {
    // R: overlap = ceiling(events_per_bin/2)
    let overlap = (events_per_bin + 1) / 2;
    let step = events_per_bin - overlap;

    let mut breaks = Vec::new();
    let mut start = 0;

    while start < n_events {
        let end = (start + events_per_bin).min(n_events);
        breaks.push((start, end));
        start += step;
    }

    breaks
}

/// Determine peaks for a single channel (public API, gets data from FCS)
pub(crate) fn determine_channel_peaks<T: PeacoQCData>(
    fcs: &T,
    channel: &str,
    breaks: &[(usize, usize)],
    config: &PeakDetectionConfig,
) -> Result<Option<ChannelPeakFrame>> {
    // Get channel data
    let data = fcs.get_channel_f64(channel)?;
    Ok(determine_channel_peaks_from_data(&data, breaks, config))
}

/// Determine peaks for a single channel from pre-extracted data (internal, used for parallel processing)
fn determine_channel_peaks_from_data(
    data: &[f64],
    breaks: &[(usize, usize)],
    config: &PeakDetectionConfig,
) -> Option<ChannelPeakFrame> {
    // Process bins in parallel
    let bin_peaks: Vec<Vec<f64>> = breaks
        .par_iter()
        .map(|(start, end)| {
            let bin_data: Vec<f64> = data[*start..*end].to_vec();

            let bin_data = if config.remove_zeros {
                bin_data.into_iter().filter(|&x| x != 0.0).collect()
            } else {
                bin_data
            };

            if bin_data.len() < 3 {
                return Vec::new();
            }

            // Compute KDE and find peaks
            // R's FindThemPeaks returns peaks sorted by x-value (from dens$x)
            // We need to sort peaks to match R's column ordering in the matrix
            let mut peaks = match KernelDensity::estimate(&bin_data, 1.0, 512) {
                Ok(kde) => kde.find_peaks(config.peak_removal),
                Err(_) => Vec::new(),
            };
            // Sort peaks by value to match R's behavior (peaks are in dens$x order, which is sorted)
            peaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            peaks
        })
        .collect();

    // Convert to PeakInfo structures
    let mut all_peaks: Vec<PeakInfo> = Vec::new();
    for (bin_idx, peaks) in bin_peaks.iter().enumerate() {
        for &peak_value in peaks {
            all_peaks.push(PeakInfo {
                bin: bin_idx,
                peak_value,
                cluster: 0, // Will be assigned later
            });
        }
    }

    if all_peaks.is_empty() {
        return None;
    }

    // Cluster peaks across bins
    if cluster_peaks(&mut all_peaks, &bin_peaks, config).is_err() {
        return None;
    }

    // Remove small clusters
    if remove_small_clusters(&mut all_peaks, breaks.len()).is_err() {
        return None;
    }

    if all_peaks.is_empty() {
        return None;
    }

    Some(ChannelPeakFrame { peaks: all_peaks })
}

/// Cluster peaks across bins using median clustering
fn cluster_peaks(
    all_peaks: &mut [PeakInfo],
    bin_peaks: &[Vec<f64>],
    config: &PeakDetectionConfig,
) -> Result<()> {
    // Count number of peaks per bin
    let peak_counts: Vec<usize> = bin_peaks.iter().map(|p| p.len()).collect();

    // Find the most common number of peaks
    let mut count_freq: HashMap<usize, usize> = HashMap::new();
    for &count in &peak_counts {
        *count_freq.entry(count).or_insert(0) += 1;
    }

    // Find the most frequent peak count that appears in enough bins
    let min_bins =
        (config.min_nr_bins_peakdetection / 100.0 * peak_counts.len() as f64).ceil() as usize;

    let most_common_count = count_freq
        .iter()
        .filter(|(_, freq)| *freq >= &min_bins)
        .max_by_key(|(count, _)| *count)
        .map(|(count, _)| *count)
        .unwrap_or(1);

    // Get bins with the most common peak count
    let mut reference_peaks: Vec<Vec<f64>> = Vec::new();
    for peaks in bin_peaks {
        if peaks.len() == most_common_count {
            reference_peaks.push(peaks.clone());
        }
    }

    if reference_peaks.is_empty() {
        // No reference peaks found, assign all peaks to a single cluster
        for peak in all_peaks.iter_mut() {
            peak.cluster = 1;
        }
        return Ok(());
    }

    // Calculate median position for each cluster
    let n_clusters = most_common_count;
    let mut cluster_medians: Vec<f64> = Vec::new();

    for cluster_idx in 0..n_clusters {
        let values: Vec<f64> = reference_peaks
            .iter()
            .filter_map(|peaks| peaks.get(cluster_idx).copied())
            .collect();

        if !values.is_empty() {
            cluster_medians.push(median(&values)?);
        }
    }

    if cluster_medians.is_empty() {
        return Ok(());
    }

    // Assign each peak to nearest cluster
    for peak in all_peaks.iter_mut() {
        let mut min_dist = f64::INFINITY;
        let mut best_cluster = 0;

        for (cluster_idx, &cluster_median) in cluster_medians.iter().enumerate() {
            let dist = (peak.peak_value - cluster_median).abs();
            if dist < min_dist {
                min_dist = dist;
                best_cluster = cluster_idx + 1; // 1-indexed
            }
        }

        peak.cluster = best_cluster;
    }

    Ok(())
}

/// Remove clusters that appear in less than 50% of bins
fn remove_small_clusters(all_peaks: &mut Vec<PeakInfo>, n_bins: usize) -> Result<()> {
    // Count bins per cluster
    let mut cluster_bin_counts: HashMap<usize, std::collections::HashSet<usize>> = HashMap::new();

    for peak in all_peaks.iter() {
        cluster_bin_counts
            .entry(peak.cluster)
            .or_insert_with(std::collections::HashSet::new)
            .insert(peak.bin);
    }

    // Find clusters to remove
    let min_bins = (n_bins as f64 * 0.5).ceil() as usize;
    let clusters_to_keep: Vec<usize> = cluster_bin_counts
        .iter()
        .filter(|(_, bins)| bins.len() >= min_bins)
        .map(|(cluster, _)| *cluster)
        .collect();

    // Filter peaks
    all_peaks.retain(|peak| clusters_to_keep.contains(&peak.cluster));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fcs::SimpleFcs;
    use polars::df;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Arc;

    #[test]
    fn test_peak_detection_basic() {
        // Create synthetic data with stable peak
        let mut data = Vec::new();
        for _ in 0..5000 {
            data.push(100.0 + rand::random::<f64>() * 10.0);
        }

        let df = Arc::new(
            df![
                "FL1-A" => data,
            ]
            .unwrap(),
        );

        let fcs = SimpleFcs {
            data_frame: df,
            parameter_metadata: StdHashMap::new(),
        };

        let config = PeakDetectionConfig {
            events_per_bin: 1000,
            ..Default::default()
        };

        let result = determine_peaks_all_channels(&fcs, &["FL1-A".to_string()], &config);

        assert!(result.is_ok());
    }
}
