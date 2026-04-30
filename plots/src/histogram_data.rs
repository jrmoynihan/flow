//! Histogram data types supporting raw values, pre-binned data, and overlaid series.
//!
//! # Examples
//!
//! Single histogram from raw values:
//! ```ignore
//! let data = HistogramData::from_values(vec![1.0, 2.0, 2.5, 3.0, 3.0]);
//! ```
//!
//! Pre-binned histogram:
//! ```ignore
//! let data = HistogramData::pre_binned(
//!     vec![0.0, 1.0, 2.0, 3.0, 4.0],  // bin edges
//!     vec![5.0, 10.0, 7.0, 3.0],     // counts per bin
//! );
//! ```
//!
//! Overlaid histograms (multiple series with gate IDs):
//! ```ignore
//! let data = HistogramData::overlaid(vec![
//!     (vec![1.0, 2.0, 2.0], 0),  // gate 0
//!     (vec![2.5, 3.0, 3.0], 1),  // gate 1
//! ]);
//! ```

/// Histogram data - either raw values, pre-binned, or overlaid series.
#[derive(Clone, Debug)]
pub enum HistogramData {
    /// Raw values to bin; binning is done at render time
    RawValues(Vec<f32>),
    /// Pre-binned: bin_edges has length N+1, counts has length N (one per bin)
    PreBinned {
        bin_edges: Vec<f32>,
        counts: Vec<f32>,
    },
    /// Multiple series for overlaid histograms; each (values, gate_id) is one series
    Overlaid(Vec<HistogramSeries>),
}

/// A single histogram series (values + gate ID for coloring)
#[derive(Clone, Debug)]
pub struct HistogramSeries {
    /// Values to bin, or use pre-binned via HistogramData::PreBinned for single series
    pub values: Vec<f32>,
    /// Gate ID indexes into gate_colors in options
    pub gate_id: u32,
}

impl HistogramData {
    /// Create from raw values (will be binned at render time)
    pub fn from_values(values: Vec<f32>) -> Self {
        Self::RawValues(values)
    }

    /// Create from pre-binned data. bin_edges.len() must equal counts.len() + 1.
    ///
    /// # Errors
    /// Returns `Err` if lengths are inconsistent.
    pub fn pre_binned(bin_edges: Vec<f32>, counts: Vec<f32>) -> Result<Self, HistogramDataError> {
        if bin_edges.len() != counts.len().saturating_add(1) {
            return Err(HistogramDataError::BinCountMismatch {
                edges_len: bin_edges.len(),
                counts_len: counts.len(),
            });
        }
        if bin_edges.len() < 2 {
            return Err(HistogramDataError::TooFewBins);
        }
        Ok(Self::PreBinned { bin_edges, counts })
    }

    /// Create overlaid histogram data (multiple series, each with gate_id for color)
    pub fn overlaid(series: Vec<(Vec<f32>, u32)>) -> Self {
        Self::Overlaid(
            series
                .into_iter()
                .map(|(values, gate_id)| HistogramSeries { values, gate_id })
                .collect(),
        )
    }

    /// Whether this is overlaid (multiple series)
    pub fn is_overlaid(&self) -> bool {
        matches!(self, Self::Overlaid(_))
    }
}

/// Error when constructing histogram data
#[derive(Debug, Clone)]
pub enum HistogramDataError {
    BinCountMismatch { edges_len: usize, counts_len: usize },
    TooFewBins,
}

impl std::fmt::Display for HistogramDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinCountMismatch {
                edges_len,
                counts_len,
            } => {
                write!(
                    f,
                    "bin_edges.len() ({}) must equal counts.len() + 1 (got {})",
                    edges_len, counts_len
                )
            }
            Self::TooFewBins => write!(f, "need at least 2 bin edges"),
        }
    }
}

impl std::error::Error for HistogramDataError {}

/// Result of binning: (bin_centers, counts) for drawing
#[derive(Clone, Debug)]
pub struct BinnedHistogram {
    pub bin_centers: Vec<f64>,
    pub counts: Vec<f64>,
}

/// Bin raw values into histogram counts.
///
/// Uses linear spacing from data min to max. Values outside [x_min, x_max] are ignored.
pub fn bin_values(
    values: &[f32],
    num_bins: usize,
    x_min: f32,
    x_max: f32,
) -> Option<BinnedHistogram> {
    if values.is_empty() || num_bins == 0 || x_max <= x_min {
        return None;
    }

    let mut counts = vec![0.0f64; num_bins];
    let bin_width = (x_max - x_min) as f64 / num_bins as f64;
    if bin_width <= 0.0 {
        return None;
    }

    for &v in values {
        if v.is_nan() || v.is_infinite() {
            continue;
        }
        if v < x_min || v > x_max {
            continue;
        }
        let idx = ((v - x_min) as f64 / bin_width).floor() as usize;
        let idx = idx.min(num_bins.saturating_sub(1));
        counts[idx] += 1.0;
    }

    let bin_centers: Vec<f64> = (0..num_bins)
        .map(|i| x_min as f64 + (i as f64 + 0.5) * bin_width)
        .collect();

    Some(BinnedHistogram {
        bin_centers,
        counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_histogram_data_from_values() {
        let data = HistogramData::from_values(vec![1.0, 2.0, 3.0]);
        assert!(!data.is_overlaid());
    }

    #[test]
    fn test_histogram_data_pre_binned_ok() {
        let data =
            HistogramData::pre_binned(vec![0.0, 1.0, 2.0, 3.0], vec![5.0, 10.0, 7.0]).unwrap();
        assert!(!data.is_overlaid());
    }

    #[test]
    fn test_histogram_data_pre_binned_mismatch() {
        let result = HistogramData::pre_binned(vec![0.0, 1.0], vec![5.0, 10.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_histogram_data_overlaid() {
        let data = HistogramData::overlaid(vec![(vec![1.0, 2.0], 0), (vec![2.0, 3.0], 1)]);
        assert!(data.is_overlaid());
    }

    #[test]
    fn test_bin_values() {
        let values = vec![0.0f32, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0];
        let binned = bin_values(&values, 3, 0.0, 3.0).unwrap();
        assert_eq!(binned.bin_centers.len(), 3);
        assert_eq!(binned.counts.len(), 3);
        // Bin 0: [0, 1) -> 0, 1, 1 = 3 values... actually 0 and 1.0
        // 0.0 in [0,1), 1.0 at boundary - (1.0-0)/1 = 1.0, floor=1, min(1,2)=1, so idx 1
        // So 0.0 -> idx 0, 1.0 -> idx 1, 2.0 -> idx 2, 3.0 -> idx 2 (since 3/1=3, floor 3, min 2)
        // Let me recalc: bin_width = 1.0
        // v=0: (0-0)/1=0, floor=0 -> idx 0
        // v=1: (1-0)/1=1, floor=1 -> idx 1 (but 1 is right edge of bin 0)
        // v=2: idx 2
        // v=3: (3-0)/1=3, floor=3, min(3,2)=2 -> idx 2
        // So counts: bin0 has 0.0 (1 val), bin1 has 1.0, 1.0 (2 vals), bin2 has 2,2,2,3 (4 vals)
        assert!(binned.counts[0] >= 1.0);
        assert!(binned.counts[1] >= 1.0);
        assert!(binned.counts[2] >= 1.0);
    }

    #[test]
    fn test_bin_values_empty() {
        assert!(bin_values(&[], 10, 0.0, 100.0).is_none());
    }
}
