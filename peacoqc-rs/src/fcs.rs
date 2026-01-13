// Test helpers for PeacoQC tests
// This module provides SimpleFcs for testing without requiring a full Fcs implementation

use crate::PeacoQCData;
use crate::error::Result;
use flow_fcs::parameter::EventDataFrame;
use std::collections::HashMap;

/// Simplified FCS structure for testing without full Fcs implementation
/// Your code should use the trait implementation above instead
pub struct SimpleFcs {
    pub data_frame: EventDataFrame,
    pub parameter_metadata: HashMap<String, ParameterMetadata>,
}

#[derive(Debug, Clone)]
pub struct ParameterMetadata {
    pub min_range: f64,
    pub max_range: f64,
    pub name: String,
}

impl PeacoQCData for SimpleFcs {
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

    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
        self.parameter_metadata
            .get(channel)
            .map(|meta| (meta.min_range, meta.max_range))
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
            f32_vals.into_iter().filter_map(|x| x.map(|v| v as f64)).collect()
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
