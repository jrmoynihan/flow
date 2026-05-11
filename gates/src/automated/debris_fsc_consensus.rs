//! Consensus FSC-A threshold using fluorescence channels (debris / low-scatter fallback).
//!
//! Builds a per-fluorescence threshold on FSC for positively biased event subsets, then takes the
//! median as a single debris cutoff: events below the cutoff are treated as non-keepers.

use crate::GateError;
use crate::GateResult;
use flow_fcs::Fcs;
use flow_density::kde::KernelDensity;

/// Parameters for consensus FSC debris gating.
#[derive(Debug, Clone)]
pub struct ConsensusFscConfig {
    /// Forward scatter area channel name.
    pub fsc_channel: String,
    /// Minimum relative peak height passed to [`KernelDensity::find_peaks`].
    pub peak_removal: f64,
    /// Events at or above this quantile of each fluor channel form the subset for that channel’s FSC histogram.
    pub positive_quantile: f64,
}

impl Default for ConsensusFscConfig {
    fn default() -> Self {
        Self {
            fsc_channel: "FSC-A".to_string(),
            peak_removal: 0.22,
            positive_quantile: 0.35,
        }
    }
}

/// Outcome of consensus FSC thresholding.
#[derive(Debug, Clone)]
pub struct FscConsensusResult {
    /// Median (or fallback) FSC cutoff; keep events with FSC >= threshold.
    pub threshold: f64,
    /// Per-fluorescence candidate thresholds before aggregation.
    pub per_channel_thresholds: Vec<(String, f64)>,
    /// `true` = keep event (not debris by this rule).
    pub keep_mask: Vec<bool>,
}

/// Compute a consensus FSC-A threshold and a keep mask.
pub fn consensus_fsc_threshold(
    fcs: &Fcs,
    config: &ConsensusFscConfig,
) -> GateResult<FscConsensusResult> {
    let fsc_all = fcs
        .get_parameter_events_slice(&config.fsc_channel)
        .map_err(|e| GateError::Other {
            message: format!("consensus FSC: {}", e),
            source: None,
        })?;
    let fsc_all: Vec<f64> = fsc_all.iter().map(|&x| x as f64).collect();
    let n = fsc_all.len();
    if n < 20 {
        return Err(GateError::Other {
            message: "consensus_fsc_threshold: need at least 20 events".to_string(),
            source: None,
        });
    }

    let kde_global = KernelDensity::estimate(&fsc_all, 1.0, 384).map_err(|e| GateError::Other {
        message: format!("KDE global FSC: {:?}", e),
        source: None,
    })?;
    let ref_peaks = kde_global.find_peaks(config.peak_removal);
    let ref_x = if ref_peaks.is_empty() {
        let mut s = fsc_all.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        s[s.len() / 2]
    } else {
        *ref_peaks
            .iter()
            .max_by(|a, b| {
                kde_global
                    .density_at(**a)
                    .partial_cmp(&kde_global.density_at(**b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("ref_peaks non-empty")
    };

    let fluor_channels: Vec<String> = fcs
        .parameters
        .values()
        .filter(|p| p.is_fluorescence())
        .map(|p| p.channel_name.to_string())
        .collect();

    let mut per_channel = Vec::new();

    for ch in &fluor_channels {
        let fluor: Vec<f64> = match fcs.get_parameter_events_slice(ch) {
            Ok(s) => s.iter().map(|&x| x as f64).collect(),
            Err(_) => continue,
        };
        if fluor.len() != n {
            continue;
        }
        let mut sorted = fluor.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q = ((sorted.len() as f64 * config.positive_quantile).floor() as usize)
            .min(sorted.len().saturating_sub(1));
        let cutoff = sorted[q];

        let fsc_sub: Vec<f64> = fsc_all
            .iter()
            .zip(fluor.iter())
            .filter(|&(_xf, yf)| *yf >= cutoff)
            .map(|(xf, _)| *xf)
            .collect();

        if fsc_sub.len() < 30 {
            continue;
        }

        let kde = match KernelDensity::estimate(&fsc_sub, 1.0, 384) {
            Ok(k) => k,
            Err(_) => continue,
        };
        let mut peaks = kde.find_peaks(config.peak_removal);
        let span = ref_x.abs() * 0.6 + 500.0;
        peaks.retain(|&p| (p - ref_x).abs() <= span);
        peaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let t_chan = if peaks.len() >= 2 {
            let lo = peaks[0].min(peaks[1]);
            let hi = peaks[0].max(peaks[1]);
            let mut best_x = lo;
            let mut best_y = f64::INFINITY;
            for (i, &x) in kde.x.iter().enumerate() {
                if x >= lo && x <= hi && kde.y[i] < best_y {
                    best_y = kde.y[i];
                    best_x = x;
                }
            }
            best_x
        } else if peaks.len() == 1 {
            let px = peaks[0];
            let fmin = fsc_sub.iter().copied().fold(f64::INFINITY, f64::min);
            let fmax = fsc_sub.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let w = (fmax - fmin) * 0.08;
            (px - w).max(fmin)
        } else {
            let mut s = fsc_sub.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx = ((s.len() as f64 * 0.08).floor() as usize).min(s.len().saturating_sub(1));
            s[idx]
        };
        per_channel.push((ch.clone(), t_chan));
    }

    let threshold = if per_channel.is_empty() {
        let mut s = fsc_all.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((s.len() as f64 * 0.1).floor() as usize).min(s.len().saturating_sub(1));
        s[idx]
    } else {
        let mut vals: Vec<f64> = per_channel.iter().map(|(_, t)| *t).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = vals.len() / 2;
        if vals.len() % 2 == 0 {
            (vals[mid - 1] + vals[mid]) / 2.0
        } else {
            vals[mid]
        }
    };

    let keep_mask: Vec<bool> = fsc_all.iter().map(|&f| f >= threshold).collect();

    Ok(FscConsensusResult {
        threshold,
        per_channel_thresholds: per_channel,
        keep_mask,
    })
}
