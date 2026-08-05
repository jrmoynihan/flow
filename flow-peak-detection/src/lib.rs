//! KDE-based peak finding for flow cytometry histograms.

use anyhow::{Result, bail};

/// Configuration for peak isolation.
#[derive(Debug, Clone, Copy)]
pub struct PeakConfig {
    /// Fraction of max density required to count as a peak (0–1).
    pub threshold: f64,
    /// Upper/lower bias within the peak (0.5 = centered 50%). `1.0` keeps the full peak.
    pub peak_bias: f64,
    /// Minimum events retained after isolation.
    pub min_events: usize,
    /// KDE grid resolution.
    pub resolution: usize,
}

impl Default for PeakConfig {
    fn default() -> Self {
        Self {
            threshold: 0.3,
            peak_bias: 1.0,
            min_events: 100,
            resolution: 512,
        }
    }
}

/// Result of isolating a peak region on a 1-D intensity sample.
#[derive(Debug, Clone)]
pub struct PeakResult {
    pub range: (f64, f64),
    pub median: f64,
    pub event_indices: Vec<usize>,
    pub density: f64,
    pub combined_score: f64,
}

/// Gaussian KDE peaks on a uniform grid (Silverman bandwidth when `bandwidth` is `None`).
pub fn detect_peaks_kde(
    data: &[f64],
    bandwidth: Option<f64>,
    resolution: usize,
    threshold: f64,
) -> Vec<(f64, f64)> {
    if data.is_empty() || resolution < 8 {
        return Vec::new();
    }
    let finite: Vec<f64> = data.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.len() < 2 {
        return Vec::new();
    }
    let min = finite.iter().copied().fold(f64::INFINITY, f64::min);
    let max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !(max > min) {
        return Vec::new();
    }
    let n = finite.len() as f64;
    let mean = finite.iter().sum::<f64>() / n;
    let var = finite.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt().max(f64::EPSILON);
    let bw = bandwidth
        .unwrap_or_else(|| 1.06 * std * n.powf(-0.2))
        .max(f64::EPSILON);

    let mut xs = Vec::with_capacity(resolution);
    let mut dens = vec![0.0_f64; resolution];
    for i in 0..resolution {
        let x = min + (max - min) * (i as f64) / ((resolution - 1) as f64);
        xs.push(x);
        let mut d = 0.0;
        for &v in &finite {
            let z = (x - v) / bw;
            d += (-0.5 * z * z).exp();
        }
        dens[i] = d / (n * bw * (std::f64::consts::TAU).sqrt());
    }
    let max_d = dens.iter().copied().fold(0.0_f64, f64::max);
    if max_d <= 0.0 {
        return Vec::new();
    }
    let cut = threshold.clamp(0.0, 1.0) * max_d;
    let mut peaks = Vec::new();
    for i in 1..resolution.saturating_sub(1) {
        if dens[i] >= cut && dens[i] >= dens[i - 1] && dens[i] >= dens[i + 1] {
            peaks.push((xs[i], dens[i]));
        }
    }
    peaks
}

fn median_of(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        0.5 * (sorted[mid - 1] + sorted[mid])
    }
}

#[derive(Clone, Copy)]
enum PeakSide {
    Positive,
    Negative,
}

fn isolate_peak(data: &[f64], config: &PeakConfig, side: PeakSide) -> Result<PeakResult> {
    let peaks = detect_peaks_kde(data, None, config.resolution, config.threshold);
    if peaks.is_empty() {
        bail!("no peaks detected above threshold {}", config.threshold);
    }
    let (peak_x, peak_d) = match side {
        PeakSide::Positive => peaks
            .iter()
            .copied()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(peaks[0]),
        PeakSide::Negative => peaks
            .iter()
            .copied()
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(peaks[0]),
    };

    let finite: Vec<(usize, f64)> = data
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| v.is_finite().then_some((i, v)))
        .collect();
    if finite.is_empty() {
        bail!("no finite events");
    }
    let min = finite.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
    let max = finite
        .iter()
        .map(|(_, v)| *v)
        .fold(f64::NEG_INFINITY, f64::max);
    let half = ((max - min) * 0.25).max(f64::EPSILON);
    let mut in_window: Vec<(usize, f64)> = finite
        .into_iter()
        .filter(|(_, v)| (*v - peak_x).abs() <= half)
        .collect();
    if in_window.is_empty() {
        bail!("no events near peak at {peak_x}");
    }
    in_window.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let bias = config.peak_bias.clamp(0.05, 1.0);
    let keep = ((in_window.len() as f64) * bias).ceil() as usize;
    let keep = keep.max(1).min(in_window.len());
    let trimmed = match side {
        PeakSide::Positive => &in_window[in_window.len() - keep..],
        PeakSide::Negative => &in_window[..keep],
    };
    if trimmed.len() < config.min_events {
        bail!(
            "only {} events in peak (minimum {})",
            trimmed.len(),
            config.min_events
        );
    }
    let values: Vec<f64> = trimmed.iter().map(|(_, v)| *v).collect();
    let lo = values[0];
    let hi = *values.last().unwrap_or(&lo);
    Ok(PeakResult {
        range: (lo, hi),
        median: median_of(&values),
        event_indices: trimmed.iter().map(|(i, _)| *i).collect(),
        density: peak_d,
        combined_score: peak_d * (trimmed.len() as f64).ln_1p(),
    })
}

/// Isolate the brightest dense peak (positive population).
pub fn isolate_positive_peak(data: &[f64], config: &PeakConfig) -> Result<PeakResult> {
    isolate_peak(data, config, PeakSide::Positive)
}

/// Isolate the leftmost peak (negative population).
pub fn isolate_negative_peak(data: &[f64], config: &PeakConfig) -> Result<PeakResult> {
    isolate_peak(data, config, PeakSide::Negative)
}

/// Boolean mask over `data` for the positive peak.
pub fn isolate_positive_peak_mask(
    data: &[f64],
    threshold: f64,
    peak_bias: f64,
) -> Result<Vec<bool>> {
    let config = PeakConfig {
        threshold,
        peak_bias,
        min_events: 1,
        ..PeakConfig::default()
    };
    let peak = isolate_positive_peak(data, &config)?;
    let mut mask = vec![false; data.len()];
    for i in peak.event_indices {
        if i < mask.len() {
            mask[i] = true;
        }
    }
    Ok(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_bimodal_and_isolates_right_peak() {
        let mut data = Vec::new();
        for _ in 0..400 {
            data.push(1.0);
        }
        for _ in 0..400 {
            data.push(10.0);
        }
        for (i, v) in data.iter_mut().enumerate() {
            *v += (i % 7) as f64 * 0.01;
        }
        let peaks = detect_peaks_kde(&data, None, 256, 0.2);
        assert!(!peaks.is_empty());
        let pos = isolate_positive_peak(
            &data,
            &PeakConfig {
                min_events: 50,
                ..PeakConfig::default()
            },
        )
        .unwrap();
        assert!(pos.median > 5.0);
    }
}
