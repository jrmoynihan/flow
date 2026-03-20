//! TRU-OLS CLI library

pub mod commands;
pub mod config;
pub mod interactive;
pub mod output;
pub mod synthetic_data;

// Re-export commonly used functions for examples
pub use commands::{SingleStainConfig, create_mixing_matrix_from_single_stains};

pub use commands::run_command;
