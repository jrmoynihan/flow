use crate::PeacoQCData;
use crate::error::Result;
use crate::stats::median_mad::median_mad;

/// Configuration for doublet removal
#[derive(Debug, Clone)]
pub struct DoubletConfig {
    /// First channel (typically FSC-A)
    pub channel1: String,

    /// Second channel (typically FSC-H)
    pub channel2: String,

    /// Number of MADs above median to use as threshold
    pub nmad: f64,

    /// Optional shift parameter
    pub b: f64,
}

impl Default for DoubletConfig {
    fn default() -> Self {
        Self {
            channel1: "FSC-A".to_string(),
            channel2: "FSC-H".to_string(),
            nmad: 4.0,
            b: 0.0,
        }
    }
}

/// Result of doublet removal
#[derive(Debug)]
pub struct DoubletResult {
    /// Boolean mask (true = keep, false = doublet)
    pub mask: Vec<bool>,

    /// Median ratio
    pub median_ratio: f64,

    /// MAD of ratios
    pub mad_ratio: f64,

    /// Threshold used
    pub threshold: f64,

    /// Percentage removed
    pub percentage_removed: f64,
}

/// Remove doublet events based on area/height ratio
///
/// Doublets (two cells passing through the detector simultaneously) have
/// a different FSC-A/FSC-H ratio than singlets. This function identifies
/// doublets as outliers in this ratio distribution.
///
/// # Algorithm
/// 1. Calculate ratio = channel1 / (1e-10 + channel2 + b)
/// 2. threshold = median(ratio) + nmad * MAD(ratio)
/// 3. Keep events where ratio < threshold
///
/// # Arguments
/// * `fcs` - FCS file data (any type implementing PeacoQCData)
/// * `config` - Configuration for doublet detection
pub fn remove_doublets<T: PeacoQCData>(fcs: &T, config: &DoubletConfig) -> Result<DoubletResult> {
    // Get channel data
    let values1 = fcs.get_channel_f64(&config.channel1)?;
    let values2 = fcs.get_channel_f64(&config.channel2)?;

    // Calculate ratios
    let mut ratios = Vec::with_capacity(fcs.n_events());
    for (a, h) in values1.iter().zip(values2.iter()) {
        let ratio = *a / (1e-10 + *h + config.b);
        ratios.push(ratio);
    }

    // Calculate median and MAD
    let (median, mad) = median_mad(&ratios)?;
    let threshold = median + config.nmad * mad;

    // Create mask
    let mask: Vec<bool> = ratios.iter().map(|&r| r < threshold).collect();

    let n_removed = mask.iter().filter(|&&x| !x).count();
    let percentage_removed = (n_removed as f64 / fcs.n_events() as f64) * 100.0;

    Ok(DoubletResult {
        mask,
        median_ratio: median,
        mad_ratio: mad,
        threshold,
        percentage_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fcs::SimpleFcs;
    use polars::df;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_remove_doublets() {
        // Create test data with some doublets (high ratio)
        let df = Arc::new(
            df![
                "FSC-A" => &[100.0, 200.0, 300.0, 400.0, 1000.0], // Last one is doublet
                "FSC-H" => &[50.0, 100.0, 150.0, 200.0, 100.0],
            ]
            .unwrap(),
        );

        let fcs = SimpleFcs {
            data_frame: df,
            parameter_metadata: HashMap::new(),
        };

        let config = DoubletConfig::default();

        let result = remove_doublets(&fcs, &config).unwrap();

        // Should detect the outlier ratio
        assert!(result.percentage_removed > 0.0);
        assert!(result.threshold > result.median_ratio);
    }
}
