use thiserror::Error;

#[derive(Error, Debug)]
pub enum PeacoQCError {
    #[error("Invalid channel: {0}")]
    InvalidChannel(String),
    
    #[error("Channel not found in FCS file: {0}")]
    ChannelNotFound(String),
    
    #[error("Insufficient data: need at least {min} events, got {actual}")]
    InsufficientData { min: usize, actual: usize },
    
    #[error("Polars error: {0}")]
    PolarsError(#[from] polars::error::PolarsError),
    
    #[error("Statistical computation failed: {0}")]
    StatsError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("No peaks detected")]
    NoPeaksDetected,
    
    #[error("Plot generation error: {0}")]
    PlotError(String),
}

pub type Result<T> = std::result::Result<T, PeacoQCError>;
