// fcs-qc: PeacoQC quality control for flow cytometry
//
// This crate implements all PeacoQC algorithms and provides
// a trait-based interface that works directly with fcs-file::Fcs

pub mod qc;
pub mod stats;
pub mod error;

pub use error::{PeacoQCError, Result};
pub use qc::{
    peacoqc, PeacoQCConfig, PeacoQCResult, QCMode,
    remove_margins, MarginConfig, MarginResult,
    remove_doublets, DoubletConfig, DoubletResult,
};

use fcs_file::{Fcs, EventDataFrame};
use polars::prelude::*;
use std::sync::Arc;

/// Trait for data structures that can be used with PeacoQC
/// 
/// Implement this on your Fcs struct to enable PeacoQC analysis
pub trait PeacoQCData {
    fn data_frame(&self) -> &EventDataFrame;
    fn n_events(&self) -> usize;
    fn channel_names(&self) -> Vec<String>;
    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)>;
    
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

/// Implement PeacoQCData for fcs-file::Fcs
impl PeacoQCData for Fcs {
    fn data_frame(&self) -> &EventDataFrame {
        &self.data_frame
    }
    
    fn n_events(&self) -> usize {
        self.n_events()
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
        // Use the existing is_fluorescence() method
        self.parameters.values()
            .filter(|p| p.is_fluorescence())
            .map(|p| p.channel_name.to_string())
            .collect()
    }
}
