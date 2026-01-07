// This module adapts your existing Fcs struct for use with PeacoQC
// It provides a bridge between your FCS implementation and PeacoQC's trait-based API

use polars::prelude::*;
use std::sync::Arc;

/// Trait that PeacoQC uses to access FCS data
/// 
/// Implement this on your existing Fcs struct to enable PeacoQC analysis
/// without any data conversion or copying.
pub trait PeacoQCData {
    /// Get reference to the Polars DataFrame containing event data
    fn data_frame(&self) -> &Arc<DataFrame>;
    
    /// Get total number of events
    fn n_events(&self) -> usize;
    
    /// Get list of all channel names
    fn channel_names(&self) -> Vec<String>;
    
    /// Get parameter range (min, max) for a channel
    /// Used by RemoveMargins to get detector limits
    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)>;
    
    /// Get list of fluorescence channels (excludes FSC, SSC, Time)
    /// Optional: default implementation filters by naming convention
    fn get_fluorescence_channels(&self) -> Vec<String> {
        self.channel_names().into_iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                !upper.contains("FSC") 
                    && !upper.contains("SSC") 
                    && !upper.contains("TIME")
            })
            .collect()
    }
}

// Example implementation for your Fcs struct
// Add this to your codebase:
/*
impl PeacoQCData for Fcs {
    fn data_frame(&self) -> &Arc<DataFrame> {
        &self.data_frame
    }
    
    fn n_events(&self) -> usize {
        self.metadata.get_number_of_events()
            .map(|n| *n)
            .unwrap_or(0)
    }
    
    fn channel_names(&self) -> Vec<String> {
        self.parameters.values()
            .map(|p| p.channel_name.to_string())
            .collect()
    }
    
    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
        // Find parameter by channel name
        let param = self.parameters.values()
            .find(|p| p.channel_name.as_ref() == channel)?;
        
        // Get range from metadata using $PnR keyword
        let key = format!("$P{}R", param.parameter_number);
        let max_range = self.metadata.get_numeric_keyword(&key)
            .ok()?
            .get_usize();
        
        Some((0.0, *max_range as f64))
    }
    
    fn get_fluorescence_channels(&self) -> Vec<String> {
        self.parameters.values()
            .filter(|p| p.is_fluorescence())  // Use your existing method!
            .map(|p| p.channel_name.to_string())
            .collect()
    }
}
*/

/// Simplified FCS structure for testing without full Fcs implementation
/// Your code should use the trait implementation above instead
pub struct SimpleFcs {
    pub data_frame: DataFrame,
    pub parameter_metadata: std::collections::HashMap<String, ParameterMetadata>,
}

#[derive(Debug, Clone)]
pub struct ParameterMetadata {
    pub min_range: f64,
    pub max_range: f64,
    pub name: String,
}

impl PeacoQCData for SimpleFcs {
    fn data_frame(&self) -> &Arc<DataFrame> {
        // SimpleFcs doesn't use Arc, so we need a workaround for the trait
        // This is a temporary solution - in production, use your Fcs struct
        unimplemented!("SimpleFcs is for testing only. Use trait impl on your Fcs struct.")
    }
    
    fn n_events(&self) -> usize {
        self.data_frame.height()
    }
    
    fn channel_names(&self) -> Vec<String> {
        self.data_frame.get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
    
    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
        self.parameter_metadata.get(channel)
            .map(|meta| (meta.min_range, meta.max_range))
    }
}

impl SimpleFcs {
    /// Get a column by name (for internal use)
    pub fn column(&self, name: &str) -> Option<&Series> {
        self.data_frame.column(name).ok()
    }
}
