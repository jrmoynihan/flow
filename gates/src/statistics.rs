use crate::filtering::filter_events_by_gate;
use crate::types::Gate;
use flow_fcs::Fcs;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Statistics calculated for a gated population.
///
/// This structure provides comprehensive statistical analysis of events that
/// pass through a gate, including event counts, percentages, centroids, and
/// detailed parameter statistics.
///
/// # Example
///
/// ```rust
/// use flow_gates::{GateStatistics, Gate};
/// use flow_fcs::Fcs;
///
/// // Load FCS file and create gate
/// let fcs = Fcs::from_file("data.fcs")?;
/// let gate = /* ... create gate ... */;
///
/// // Calculate statistics
/// let stats = GateStatistics::calculate(&fcs, &gate)?;
///
/// println!("Event count: {}", stats.event_count);
/// println!("Percentage: {:.2}%", stats.percentage);
/// println!("Centroid: ({:.2}, {:.2})", stats.centroid.0, stats.centroid.1);
/// println!("X mean: {:.2}", stats.x_stats.mean);
/// println!("Y median: {:.2}", stats.y_stats.median);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateStatistics {
    /// Number of events in the gate
    pub event_count: usize,
    /// Percentage of total events (0.0 to 100.0)
    pub percentage: f64,
    /// 2D centroid (x, y) in raw data space
    pub centroid: (f64, f64),
    /// Statistics for the X parameter
    pub x_stats: ParameterStatistics,
    /// Statistics for the Y parameter
    pub y_stats: ParameterStatistics,
}

/// Statistics for a single parameter (channel) within a gate.
///
/// Provides comprehensive statistical measures including central tendency
/// (mean, median, geometric mean), dispersion (std dev, CV), and distribution
/// (min, max, quartiles).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterStatistics {
    /// Parameter (channel) name
    pub parameter: String,
    /// Mean value
    pub mean: f64,
    /// Geometric mean
    pub geometric_mean: f64,
    /// Median value
    pub median: f64,
    /// Standard deviation
    pub std_dev: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// 25th percentile (Q1)
    pub q1: f64,
    /// 75th percentile (Q3)
    pub q3: f64,
    /// Coefficient of variation (CV) = std_dev / mean
    pub cv: f64,
}

impl GateStatistics {
    /// Calculate statistics for a gate applied to FCS data
    pub fn calculate(fcs: &Fcs, gate: &Gate) -> Result<Self> {
        // Get filtered event indices
        let indices = filter_events_by_gate(fcs, gate, None)?;
        let event_count = indices.len();

        if event_count == 0 {
            return Ok(Self::empty(gate));
        }

        // Get parameter data as views (no full allocation)
        let x_param = gate.x_parameter_channel_name();
        let y_param = gate.y_parameter_channel_name();

        let x_slice = fcs
            .get_parameter_events_slice(x_param)
            .with_context(|| format!("Failed to get parameter data for {}", x_param))?;
        let y_slice = fcs
            .get_parameter_events_slice(y_param)
            .with_context(|| format!("Failed to get parameter data for {}", y_param))?;

        // Extract filtered values (only allocate the filtered subset)
        let x_values: Vec<f64> = indices.iter().map(|&i| x_slice[i] as f64).collect();
        let y_values: Vec<f64> = indices.iter().map(|&i| y_slice[i] as f64).collect();

        // Calculate total events for percentage
        let total_events = fcs.data_frame.height();
        let percentage = (event_count as f64 / total_events as f64) * 100.0;

        // Calculate centroid
        let centroid = (
            x_values.iter().sum::<f64>() / event_count as f64,
            y_values.iter().sum::<f64>() / event_count as f64,
        );

        // Calculate parameter statistics
        let x_stats = ParameterStatistics::calculate(x_param, &x_values)?;
        let y_stats = ParameterStatistics::calculate(y_param, &y_values)?;

        Ok(Self {
            event_count,
            percentage,
            centroid,
            x_stats,
            y_stats,
        })
    }

    /// Create empty statistics (for gates with no events)
    fn empty(gate: &Gate) -> Self {
        Self {
            event_count: 0,
            percentage: 0.0,
            centroid: (0.0, 0.0),
            x_stats: ParameterStatistics::empty(gate.x_parameter_channel_name()),
            y_stats: ParameterStatistics::empty(gate.y_parameter_channel_name()),
        }
    }
}

impl ParameterStatistics {
    /// Calculate statistics for a parameter
    pub fn calculate(parameter: &str, values: &[f64]) -> Result<Self> {
        if values.is_empty() {
            return Ok(Self::empty(parameter));
        }

        let n = values.len() as f64;

        // Mean
        let mean = values.iter().sum::<f64>() / n;

        // Geometric mean (only for positive values)
        let geometric_mean = if values.iter().all(|&v| v > 0.0) {
            let log_sum: f64 = values.iter().map(|&v| v.ln()).sum();
            (log_sum / n).exp()
        } else {
            f64::NAN
        };

        // Variance and standard deviation
        let variance = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        // Coefficient of variation
        let cv = if mean != 0.0 {
            (std_dev / mean.abs()) * 100.0
        } else {
            f64::NAN
        };

        // Min and Max
        let min = values
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(f64::NAN);
        let max = values
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(f64::NAN);

        // Median and percentiles (requires sorting)
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = percentile(&sorted, 50.0);
        let q1 = percentile(&sorted, 25.0);
        let q3 = percentile(&sorted, 75.0);

        Ok(Self {
            parameter: parameter.to_string(),
            mean,
            geometric_mean,
            median,
            std_dev,
            min,
            max,
            q1,
            q3,
            cv,
        })
    }

    /// Create empty statistics
    fn empty(parameter: &str) -> Self {
        Self {
            parameter: parameter.to_string(),
            mean: f64::NAN,
            geometric_mean: f64::NAN,
            median: f64::NAN,
            std_dev: f64::NAN,
            min: f64::NAN,
            max: f64::NAN,
            q1: f64::NAN,
            q3: f64::NAN,
            cv: f64::NAN,
        }
    }
}

/// Calculate percentile from sorted data
///
/// Uses linear interpolation between ranks
fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    if sorted_values.is_empty() {
        return f64::NAN;
    }

    let n = sorted_values.len();
    if n == 1 {
        return sorted_values[0];
    }

    // Calculate the rank
    let rank = (p / 100.0) * (n - 1) as f64;
    let lower_index = rank.floor() as usize;
    let upper_index = rank.ceil() as usize;

    if lower_index == upper_index {
        sorted_values[lower_index]
    } else {
        // Linear interpolation
        let lower_value = sorted_values[lower_index];
        let upper_value = sorted_values[upper_index];
        let fraction = rank - lower_index as f64;
        lower_value + fraction * (upper_value - lower_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&data, 0.0), 1.0);
        assert_eq!(percentile(&data, 50.0), 3.0);
        assert_eq!(percentile(&data, 100.0), 5.0);
    }

    #[test]
    fn test_parameter_statistics() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = ParameterStatistics::calculate("test", &values).expect("stats");

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.median, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert!((stats.std_dev - 1.4142).abs() < 0.01);
    }

    #[test]
    fn test_geometric_mean() {
        let values = vec![1.0, 2.0, 4.0, 8.0];
        let stats = ParameterStatistics::calculate("test", &values).expect("stats");

        // Geometric mean of [1,2,4,8] = (1*2*4*8)^(1/4) = 64^0.25 = sqrt(8) ≈ 2.828
        assert!((stats.geometric_mean - 2.828).abs() < 0.01);
    }

    #[test]
    fn test_coefficient_of_variation() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let stats = ParameterStatistics::calculate("test", &values).expect("stats");

        // Mean = 30, StdDev ≈ 14.14, CV = (14.14/30)*100 ≈ 47.14%
        assert!((stats.cv - 47.14).abs() < 1.0);
    }

    #[test]
    fn test_empty_statistics() {
        let stats = ParameterStatistics::calculate("test", &[]).expect("stats");
        assert!(stats.mean.is_nan());
        assert!(stats.median.is_nan());
    }
}
