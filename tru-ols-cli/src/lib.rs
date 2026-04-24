//! TRU-OLS CLI library

#[cfg(feature = "cli_benchmark")]
pub mod benchmark;
pub mod commands;
pub mod config;
pub mod interactive;
pub mod output;
pub mod qc_pipeline;
pub mod state;
pub mod synthetic_data;

// Re-export commonly used functions for examples
pub use commands::{SingleStainConfig, create_mixing_matrix_from_single_stains};

pub use commands::run_command;
pub use qc_pipeline::{
    QcCliOptions, QcPipelineConfig, QcPipelineReport, QcPreset, QcStageRecord, filter_fcs_by_mask,
    run_qc_pipeline,
};
