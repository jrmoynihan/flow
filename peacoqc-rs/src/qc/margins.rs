use crate::PeacoQCData;
use crate::error::{PeacoQCError, Result};
use std::collections::HashMap;

/// Configuration for margin removal
#[derive(Debug, Clone)]
pub struct MarginConfig {
    /// Channels to check for margin events
    pub channels: Vec<String>,

    /// Override channel specifications (minRange, maxRange)
    pub channel_specifications: Option<HashMap<String, (f64, f64)>>,

    /// Channels to check for minimum margins (defaults to all channels)
    pub remove_min: Option<Vec<String>>,

    /// Channels to check for maximum margins (defaults to all channels)
    pub remove_max: Option<Vec<String>>,
}

impl Default for MarginConfig {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            channel_specifications: None,
            remove_min: None,
            remove_max: None,
        }
    }
}

/// Result of margin removal analysis
#[derive(Debug)]
pub struct MarginResult {
    /// Boolean mask indicating which events to keep (true = keep, false = remove)
    pub mask: Vec<bool>,

    /// Number of events removed per channel (min and max)
    pub margin_matrix: HashMap<String, (usize, usize)>, // (min_removed, max_removed)

    /// Total percentage removed
    pub percentage_removed: f64,
}

/// Remove margin events from flow cytometry data
///
/// Margin events occur when detector saturation causes events to pile up at
/// the minimum or maximum detector range. This function identifies and removes
/// such events based on the parameter ranges specified in the FCS metadata.
///
/// # Algorithm
/// For each channel:
/// - Lower margin: value <= max(min(minRange, 0), min(data))
/// - Upper margin: value > min(maxRange, max(data))
///
/// # Arguments
/// * `fcs` - FCS file data (any type implementing PeacoQCData)
/// * `config` - Configuration specifying which channels to check
///
/// # Returns
/// * `MarginResult` containing the boolean mask and statistics
pub fn remove_margins<T: PeacoQCData>(fcs: &T, config: &MarginConfig) -> Result<MarginResult> {
    if config.channels.is_empty() {
        return Err(PeacoQCError::ConfigError(
            "No channels specified for margin removal".to_string(),
        ));
    }

    let n_events = fcs.n_events();
    let mut mask = vec![true; n_events];
    let mut margin_matrix = HashMap::new();

    // Get lists of channels to check for min/max margins
    let remove_min = config.remove_min.as_ref().unwrap_or(&config.channels);
    let remove_max = config.remove_max.as_ref().unwrap_or(&config.channels);

    for channel in &config.channels {
        // Get channel data
        let values = fcs.get_channel_f64(channel)?;

        // Calculate min/max from data
        let data_min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let data_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        // Get or override parameter ranges
        let (min_range, max_range) = if let Some(specs) = &config.channel_specifications {
            if let Some(&(min, max)) = specs.get(channel) {
                (min, max)
            } else {
                // Get from FCS metadata via trait
                fcs.get_channel_range(channel).unwrap_or_else(|| {
                    // Fallback to data range
                    (data_min.min(0.0), data_max.max(262144.0))
                })
            }
        } else {
            // Get from FCS metadata via trait
            fcs.get_channel_range(channel).unwrap_or_else(|| {
                (data_min.min(0.0), data_max.max(262144.0))
            })
        };

        let mut min_removed = 0;
        let mut max_removed = 0;

        // Check minimum margins
        if remove_min.contains(channel) {
            let threshold = min_range.min(0.0).max(data_min);

            for (i, &v) in values.iter().enumerate() {
                if v <= threshold {
                    mask[i] = false;
                    min_removed += 1;
                }
            }
        }

        // Check maximum margins
        // R: max_margin_ev <- e[, d] > min(meta[d, "maxRange"], max(e[, d]))
        // Note: R uses > (strictly greater than), not >=
        if remove_max.contains(channel) {
            let threshold = max_range.min(data_max);

            for (i, &v) in values.iter().enumerate() {
                // Remove events strictly above the threshold (matching R's > operator)
                if v > threshold && mask[i] {
                    mask[i] = false;
                    max_removed += 1;
                }
            }
        }

        margin_matrix.insert(channel.clone(), (min_removed, max_removed));
    }

    let n_removed = mask.iter().filter(|&&x| !x).count();
    let percentage_removed = (n_removed as f64 / n_events as f64) * 100.0;

    // Warn if more than 10% removed
    if percentage_removed > 10.0 {
        eprintln!(
            "Warning: More than {:.2}% of events removed as margin events. This should be verified.",
            percentage_removed
        );
    }

    Ok(MarginResult {
        mask,
        margin_matrix,
        percentage_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fcs::{ParameterMetadata, SimpleFcs};
    use polars::df;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_remove_margins_basic() {
        // Create test data
        let df = Arc::new(df![
            "FSC-A" => &[100.0, 200.0, 300.0, 0.0, 262144.0, 150.0],
            "SSC-A" => &[50.0, 100.0, 150.0, 200.0, 250.0, 300.0],
        ]
        .unwrap());

        let mut metadata = HashMap::new();
        metadata.insert(
            "FSC-A".to_string(),
            ParameterMetadata {
                min_range: 0.0,
                max_range: 262144.0,
                name: "FSC-A".to_string(),
            },
        );
        metadata.insert(
            "SSC-A".to_string(),
            ParameterMetadata {
                min_range: 0.0,
                max_range: 262144.0,
                name: "SSC-A".to_string(),
            },
        );

        let fcs = SimpleFcs {
            data_frame: df,
            parameter_metadata: metadata,
        };

        let config = MarginConfig {
            channels: vec!["FSC-A".to_string()],
            ..Default::default()
        };

        let result = remove_margins(&fcs, &config).unwrap();

        // Event at 0.0 and 262144.0 should be removed
        assert_eq!(result.mask.iter().filter(|&&x| !x).count(), 2);
        assert!(result.percentage_removed > 0.0);
    }
}
