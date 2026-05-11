//! Enhanced doublet detection
//!
//! Provides multiple methods for detecting doublet events in flow cytometry data,
//! including the original peacoqc-rs method and improved density-based approaches.

use crate::{Gate, GateError, GateResult};
use flow_fcs::Fcs;
use flow_density::kde::KernelDensity;

/// Configuration for doublet detection
#[derive(Debug, Clone)]
pub struct DoubletGateConfig {
    /// Channel pairs for doublet detection
    /// Each pair is (area_channel, height_channel) or (area_channel, width_channel)
    pub channels: Vec<(String, String)>,
    /// Detection method to use
    pub method: DoubletMethod,
    /// Number of MADs above median (for MAD-based methods)
    pub nmad: Option<f64>,
    /// Density threshold (for density-based methods)
    pub density_threshold: Option<f64>,
    /// Cluster epsilon (for DBSCAN)
    pub cluster_eps: Option<f64>,
    /// Minimum samples for clustering
    pub cluster_min_samples: Option<usize>,
}

/// Doublet detection method
#[derive(Debug, Clone)]
pub enum DoubletMethod {
    /// Ratio-based MAD method (peacoqc-rs approach)
    RatioMAD { nmad: f64 },
    /// Density-based detection using KDE
    DensityBased { threshold: f64 },
    /// Clustering-based detection
    Clustering { eps: f64, min_samples: usize },
    /// Hybrid approach combining multiple methods
    Hybrid,
    /// Ratio histogram: if enough peaks above `min_ratio`, use density valley between early peaks; else `fixed_threshold` on ratio.
    RatioInflectionOrFixed {
        min_peaks: usize,
        min_ratio: f64,
        fixed_threshold: f64,
    },
}

/// Result of doublet detection
#[derive(Debug, Clone)]
pub struct DoubletGateResult {
    /// Exclusion gate for doublets (if generated)
    pub exclusion_gate: Option<Gate>,
    /// Singlet mask (true = singlet, false = doublet)
    pub singlet_mask: Vec<bool>,
    /// Doublet mask (true = doublet, false = singlet)
    pub doublet_mask: Vec<bool>,
    /// Statistics about doublet detection
    pub statistics: DoubletStatistics,
}

/// Statistics for doublet detection
#[derive(Debug, Clone)]
pub struct DoubletStatistics {
    /// Number of singlets detected
    pub n_singlets: usize,
    /// Number of doublets detected
    pub n_doublets: usize,
    /// Percentage of doublets
    pub doublet_percentage: f64,
    /// Method used
    pub method_used: String,
}

/// Singlet mask for one (area, height) pair using `config.method` (and `config.nmad` / `config.density_threshold` for Hybrid).
fn singlet_mask_for_pair(
    area_data: &[f64],
    height_data: &[f64],
    config: &DoubletGateConfig,
) -> GateResult<(Vec<bool>, String)> {
    if area_data.len() != height_data.len() {
        return Err(GateError::Other {
            message: format!(
                "Area and height channels have different lengths: {} vs {}",
                area_data.len(),
                height_data.len()
            ),
            source: None,
        });
    }

    match &config.method {
        DoubletMethod::RatioMAD { nmad } => detect_ratio_mad(area_data, height_data, *nmad),
        DoubletMethod::DensityBased { threshold } => {
            detect_density_based(area_data, height_data, *threshold)
        }
        DoubletMethod::Clustering { eps, min_samples } => {
            detect_clustering(area_data, height_data, *eps, *min_samples)
        }
        DoubletMethod::RatioInflectionOrFixed {
            min_peaks,
            min_ratio,
            fixed_threshold,
        } => detect_ratio_inflection_or_fixed(
            area_data,
            height_data,
            *min_peaks,
            *min_ratio,
            *fixed_threshold,
        ),
        DoubletMethod::Hybrid => {
            let mad_result = detect_ratio_mad(area_data, height_data, config.nmad.unwrap_or(4.0))?;
            let density_result = detect_density_based(
                area_data,
                height_data,
                config.density_threshold.unwrap_or(0.1),
            )?;
            let combined_mask: Vec<bool> = mad_result
                .0
                .iter()
                .zip(density_result.0.iter())
                .map(|(&a, &b)| a && b)
                .collect();
            Ok((combined_mask, "Hybrid".to_string()))
        }
    }
}

/// Detect doublets using specified method
///
/// When multiple channel pairs are configured, each pair produces a singlet mask and results are
/// combined with element-wise **AND** (event is a singlet only if all pairs classify it as singlet).
///
/// # Arguments
/// * `fcs` - FCS file data
/// * `config` - Doublet detection configuration
///
/// # Returns
/// DoubletGateResult with masks and statistics
pub fn detect_doublets(fcs: &Fcs, config: &DoubletGateConfig) -> GateResult<DoubletGateResult> {
    if config.channels.is_empty() {
        return Err(GateError::Other {
            message: "No channel pairs specified for doublet detection".to_string(),
            source: None,
        });
    }

    let mut singlet_mask: Option<Vec<bool>> = None;
    let mut single_pair_method: Option<String> = None;
    let mut multi_pair_method_parts: Vec<String> = Vec::new();

    for (area_channel, height_channel) in &config.channels {
        let area_data_f32 =
            fcs.get_parameter_events_slice(area_channel)
                .map_err(|e| GateError::Other {
                    message: format!("Failed to get area channel {}: {}", area_channel, e),
                    source: None,
                })?;
        let height_data_f32 = fcs
            .get_parameter_events_slice(height_channel)
            .map_err(|e| GateError::Other {
                message: format!("Failed to get height channel {}: {}", height_channel, e),
                source: None,
            })?;

        let area_data: Vec<f64> = area_data_f32.iter().map(|&x| x as f64).collect();
        let height_data: Vec<f64> = height_data_f32.iter().map(|&x| x as f64).collect();

        let (pair_mask, pair_method) = singlet_mask_for_pair(&area_data, &height_data, config)?;

        singlet_mask = Some(match singlet_mask {
            None => pair_mask,
            Some(acc) => acc
                .iter()
                .zip(pair_mask.iter())
                .map(|(&a, &b)| a && b)
                .collect(),
        });
        if config.channels.len() == 1 {
            single_pair_method = Some(pair_method);
        } else {
            multi_pair_method_parts.push(format!(
                "{}|{}:{}",
                area_channel, height_channel, pair_method
            ));
        }
    }

    let singlet_mask = singlet_mask.expect("non-empty channels checked above");
    let method_name = if config.channels.len() == 1 {
        single_pair_method.unwrap_or_default()
    } else {
        format!(
            "MultiPair({}): {}",
            config.channels.len(),
            multi_pair_method_parts.join("; ")
        )
    };

    let doublet_mask: Vec<bool> = singlet_mask.iter().map(|&x| !x).collect();
    let n_doublets = doublet_mask.iter().filter(|&&x| x).count();
    let n_singlets = singlet_mask.len() - n_doublets;
    let doublet_percentage = if singlet_mask.is_empty() {
        0.0
    } else {
        (n_doublets as f64 / singlet_mask.len() as f64) * 100.0
    };

    let statistics = DoubletStatistics {
        n_singlets,
        n_doublets,
        doublet_percentage,
        method_used: method_name,
    };

    let exclusion_gate = None;

    Ok(DoubletGateResult {
        exclusion_gate,
        singlet_mask,
        doublet_mask,
        statistics,
    })
}

/// Detect doublets using ratio-based MAD method (peacoqc-rs approach)
fn detect_ratio_mad(
    area_data: &[f64],
    height_data: &[f64],
    nmad: f64,
) -> GateResult<(Vec<bool>, String)> {
    // Calculate ratios
    let ratios: Vec<f64> = area_data
        .iter()
        .zip(height_data.iter())
        .map(|(&a, &h)| a / (1e-10 + h))
        .collect();

    // Calculate median and MAD
    let mut sorted_ratios = ratios.clone();
    sorted_ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let median = if sorted_ratios.len() % 2 == 0 {
        (sorted_ratios[sorted_ratios.len() / 2 - 1] + sorted_ratios[sorted_ratios.len() / 2]) / 2.0
    } else {
        sorted_ratios[sorted_ratios.len() / 2]
    };

    // Calculate MAD (median absolute deviation)
    let deviations: Vec<f64> = ratios.iter().map(|&r| (r - median).abs()).collect();
    let mut sorted_deviations = deviations.clone();
    sorted_deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mad = if sorted_deviations.len() % 2 == 0 {
        (sorted_deviations[sorted_deviations.len() / 2 - 1]
            + sorted_deviations[sorted_deviations.len() / 2])
            / 2.0
    } else {
        sorted_deviations[sorted_deviations.len() / 2]
    };

    // Scaled MAD (R's default: constant = 1.4826)
    let scaled_mad = mad * 1.4826;
    let threshold = median + nmad * scaled_mad;

    // Create mask (singlets have ratio < threshold)
    let mask: Vec<bool> = ratios.iter().map(|&r| r < threshold).collect();

    Ok((mask, format!("RatioMAD(nmad={})", nmad)))
}

/// Detect doublets using density-based method
fn detect_density_based(
    area_data: &[f64],
    height_data: &[f64],
    threshold: f64,
) -> GateResult<(Vec<bool>, String)> {
    // Calculate ratios
    let ratios: Vec<f64> = area_data
        .iter()
        .zip(height_data.iter())
        .map(|(&a, &h)| a / (1e-10 + h))
        .collect();

    // Use KDE to estimate density of ratios
    let kde = KernelDensity::estimate(&ratios, 1.0, 512).map_err(|e| GateError::Other {
        message: format!("KDE failed: {:?}", e),
        source: None,
    })?;

    // Find peak (main population)
    let peaks = kde.find_peaks(threshold);
    if peaks.is_empty() {
        return Err(GateError::Other {
            message: "No peaks found in ratio distribution".to_string(),
            source: None,
        });
    }

    let main_peak = peaks[0];

    // Calculate spread around peak
    let mut distances: Vec<f64> = ratios.iter().map(|&r| (r - main_peak).abs()).collect();
    distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Use 95th percentile as threshold
    let threshold_idx = (distances.len() as f64 * 0.95) as usize;
    let threshold_dist = distances[threshold_idx.min(distances.len() - 1)];

    // Create mask (singlets are within threshold distance of peak)
    let mask: Vec<bool> = ratios
        .iter()
        .map(|&r| (r - main_peak).abs() <= threshold_dist)
        .collect();

    Ok((mask, format!("DensityBased(threshold={})", threshold)))
}

/// Detect doublets using clustering method
fn detect_clustering(
    _area_data: &[f64],
    _height_data: &[f64],
    _eps: f64,
    _min_samples: usize,
) -> GateResult<(Vec<bool>, String)> {
    // TODO: Implement clustering-based detection once linfa API is fixed
    // For now, fall back to ratio MAD
    Err(GateError::Other {
        message: "Clustering-based doublet detection not yet implemented (pending linfa API fix)"
            .to_string(),
        source: None,
    })
}

/// Area/height ratio: KDE peaks at or above `min_ratio`; if at least `min_peaks` peaks remain, threshold is the
/// density valley between the two left-most such peaks; otherwise use `fixed_threshold` on ratio.
fn detect_ratio_inflection_or_fixed(
    area_data: &[f64],
    height_data: &[f64],
    min_peaks: usize,
    min_ratio: f64,
    fixed_threshold: f64,
) -> GateResult<(Vec<bool>, String)> {
    let ratios: Vec<f64> = area_data
        .iter()
        .zip(height_data.iter())
        .map(|(&a, &h)| a / (1e-10 + h))
        .collect();

    let clean: Vec<f64> = ratios.iter().filter(|r| r.is_finite()).copied().collect();
    let ratio_threshold = if clean.len() < 8 || min_peaks < 2 {
        fixed_threshold
    } else {
        let kde = KernelDensity::estimate(&clean, 1.0, 256).map_err(|e| GateError::Other {
            message: format!("KDE failed: {:?}", e),
            source: None,
        })?;
        let mut peaks = kde.find_peaks(0.12);
        peaks.retain(|&x| x >= min_ratio);
        peaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        if peaks.len() >= min_peaks && peaks.len() >= 2 {
            let lo = peaks[0].min(peaks[1]);
            let hi = peaks[0].max(peaks[1]);
            let mut best_x = fixed_threshold;
            let mut best_y = f64::INFINITY;
            for (i, &x) in kde.x.iter().enumerate() {
                if x >= lo && x <= hi && kde.y[i] < best_y {
                    best_y = kde.y[i];
                    best_x = x;
                }
            }
            best_x
        } else {
            fixed_threshold
        }
    };

    let mask: Vec<bool> = ratios
        .iter()
        .map(|&r| r.is_finite() && r < ratio_threshold)
        .collect();

    Ok((
        mask,
        format!("RatioInflectionOrFixed(threshold={ratio_threshold:.4})"),
    ))
}
