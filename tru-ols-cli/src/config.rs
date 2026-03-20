//! Configuration management for TRU-OLS CLI

use serde::{Deserialize, Serialize};

/// Configuration for TRU-OLS unmixing (reserved for config-file / future use).
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmixingConfig {
    /// Cutoff percentile for determining irrelevant endmembers
    pub cutoff_percentile: f64,

    /// Strategy for handling irrelevant abundances
    pub strategy: String,

    /// Autofluorescence endmember name
    pub autofluorescence: String,
}

impl Default for UnmixingConfig {
    fn default() -> Self {
        Self {
            cutoff_percentile: 0.995,
            strategy: "zero".to_string(),
            autofluorescence: "Autofluorescence".to_string(),
        }
    }
}
