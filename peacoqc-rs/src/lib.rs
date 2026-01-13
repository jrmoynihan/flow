//! PeacoQC quality control algorithms for flow cytometry
//!
//! This crate implements all PeacoQC algorithms and provides
//! a trait-based interface that works with any FCS data structure.
//!
//! # Quick Start
//!
//! ```no_run
//! use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};
//!
//! // Assuming you have an FCS struct that implements PeacoQCData
//! # struct MyFcs;
//! # impl PeacoQCData for MyFcs {
//! #     fn n_events(&self) -> usize { 0 }
//! #     fn channel_names(&self) -> Vec<String> { vec![] }
//! #     fn get_channel_range(&self, _: &str) -> Option<(f64, f64)> { None }
//! #     fn get_channel_f64(&self, _: &str) -> peacoqc_rs::Result<Vec<f64>> {
//! #         Ok(vec![])
//! #     }
//! # }
//! # let fcs = MyFcs;
//! let config = PeacoQCConfig {
//!     channels: vec!["FL1-A".to_string(), "FL2-A".to_string()],
//!     determine_good_cells: QCMode::All,
//!     ..Default::default()
//! };
//!
//! let result = peacoqc(&fcs, &config)?;
//! println!("Removed {:.2}% of events", result.percentage_removed);
//! # Ok::<(), peacoqc_rs::PeacoQCError>(())
//! ```
//!
//! # Integration Example
//!
//! Here's a complete example of integrating PeacoQC into an application:
//!
//! ```rust,no_run
//! use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc, remove_margins, remove_doublets};
//! use flow_fcs::Fcs; // If using flow-fcs crate
//! use std::time::Instant;
//!
//! // Load FCS file (example using flow-fcs)
//! let mut fcs = Fcs::open("data.fcs")?;
//! let n_events_initial = fcs.n_events();
//!
//! // Step 1: Remove margin events (optional)
//! let margin_config = peacoqc_rs::MarginConfig {
//!     channels: fcs.channel_names(),
//!     ..Default::default()
//! };
//! let margin_result = remove_margins(&fcs, &margin_config)?;
//! if margin_result.percentage_removed > 0.0 {
//!     fcs = fcs.filter(&margin_result.mask)?;
//! }
//!
//! // Step 2: Remove doublets (optional)
//! let doublet_config = peacoqc_rs::DoubletConfig::default();
//! let doublet_result = remove_doublets(&fcs, &doublet_config)?;
//! if doublet_result.percentage_removed > 0.0 {
//!     fcs = fcs.filter(&doublet_result.mask)?;
//! }
//!
//! // Step 3: Run PeacoQC
//! let start_time = Instant::now();
//! let channels = fcs.get_fluorescence_channels(); // Auto-detect channels
//! let config = PeacoQCConfig {
//!     channels,
//!     determine_good_cells: QCMode::All,
//!     mad: 6.0,
//!     it_limit: 0.6,
//!     consecutive_bins: 5,
//!     ..Default::default()
//! };
//!
//! let peacoqc_result = peacoqc(&fcs, &config)?;
//!
//! // Step 4: Apply filter
//! let clean_fcs = fcs.filter(&peacoqc_result.good_cells)?;
//! let n_events_final = clean_fcs.n_events();
//!
//! println!("Events: {} → {} ({:.2}% removed)",
//!     n_events_initial,
//!     n_events_final,
//!     peacoqc_result.percentage_removed);
//! println!("Processing time: {:.2}s", start_time.elapsed().as_secs_f64());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! See `examples/basic_usage.rs` and `examples/tauri_command.rs` for more complete examples.

pub mod error;
pub mod qc;
pub mod stats;

// fcs module provides SimpleFcs for testing and examples
pub mod fcs;

pub use error::{PeacoQCError, Result};
pub use qc::{
    peacoqc, PeacoQCConfig, PeacoQCResult, QCMode,
    remove_margins, MarginConfig, MarginResult,
    remove_doublets, DoubletConfig, DoubletResult,
    create_qc_plots, QCPlotConfig,
};

#[cfg(feature = "flow-fcs")]
pub use crate::flow_fcs_impl::preprocess_fcs;

/// Trait for data structures that can be used with PeacoQC
///
/// Implement this trait on your FCS data structure to enable PeacoQC analysis.
/// This trait-based design allows PeacoQC to work with any data format, but we recommend
/// using the `flow-fcs` crate, which itself utilizes the `polars` crate for production applications.
///
/// # Example Implementation
///
/// ```rust
/// use peacoqc_rs::{PeacoQCData, Result};
///
/// struct MyFcs {
///     data: Vec<HashMap<String, Vec<f64>>>,
///     channels: Vec<String>,
/// }
///
/// impl PeacoQCData for MyFcs {
///     fn n_events(&self) -> usize {
///         self.data.get(0).map(|d| d.values().next().map(|v| v.len())).flatten().unwrap_or(0)
///     }
///
///     fn channel_names(&self) -> Vec<String> {
///         self.channels.clone()
///     }
///
///     fn get_channel_range(&self, _channel: &str) -> Option<(f64, f64)> {
///         Some((0.0, 262144.0)) // Return channel range if available
///     }
///
///     fn get_channel_f64(&self, channel: &str) -> Result<Vec<f64>> {
///         self.data.get(0)
///             .and_then(|d| d.get(channel).cloned())
///             .ok_or_else(|| peacoqc_rs::PeacoQCError::ChannelNotFound(channel.to_string()))
///     }
/// }
/// ```
pub trait PeacoQCData {
    fn n_events(&self) -> usize;
    fn channel_names(&self) -> Vec<String>;
    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)>;

    /// Get channel data as f64 values
    fn get_channel_f64(&self, channel: &str) -> Result<Vec<f64>>;

    fn get_fluorescence_channels(&self) -> Vec<String> {
        self.channel_names()
            .into_iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
            })
            .collect()
    }
}

/// Extension trait for FCS data structures to add filtering capabilities
///
/// Implement this trait to enable filtering of events using a boolean mask.
/// This is required to apply PeacoQC results to your data.
///
/// # Example Implementation
///
/// ```rust
/// use peacoqc_rs::{FcsFilter, Result, PeacoQCError};
///
/// struct MyFcs {
///     data: Vec<Vec<f64>>, // rows x channels
/// }
///
/// impl FcsFilter for MyFcs {
///     fn filter(&self, mask: &[bool]) -> Result<Self> {
///         if mask.len() != self.data.len() {
///             return Err(PeacoQCError::StatsError(format!(
///                 "Mask length {} doesn't match event count {}",
///                 mask.len(),
///                 self.data.len()
///             )));
///         }
///
///         let filtered_data: Vec<Vec<f64>> = self.data
///             .iter()
///             .enumerate()
///             .filter_map(|(i, row)| if mask[i] { Some(row.clone()) } else { None })
///             .collect();
///
///         Ok(MyFcs { data: filtered_data })
///     }
/// }
/// ```
pub trait FcsFilter: Sized {
    /// Filter events using a boolean mask
    ///
    /// # Arguments
    /// * `mask` - Boolean slice where `true` means keep the event, `false` means remove
    ///
    /// # Returns
    /// A new instance with filtered data
    ///
    /// # Errors
    /// Returns an error if the mask length doesn't match the number of events
    fn filter(&self, mask: &[bool]) -> Result<Self>;
}

#[cfg(feature = "flow-fcs")]
mod flow_fcs_impl {
    use super::*;
    use flow_fcs::{file::Fcs, keyword::FloatableKeyword};
    use polars::prelude::*;
    use std::sync::Arc;

    impl PeacoQCData for Fcs {
        fn n_events(&self) -> usize {
            self.get_event_count_from_dataframe()
        }

        fn channel_names(&self) -> Vec<String> {
            self.parameters
                .values()
                .map(|p| p.channel_name.to_string())
                .collect()
        }

        fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
            // Find parameter by channel name
            let param = self
                .parameters
                .values()
                .find(|p| p.channel_name.as_ref() == channel)?;

            // Get range from metadata using $PnR keyword
            let key = format!("$P{}R", param.parameter_number);
            let max_range = self.metadata.get_float_keyword(&key).ok()?.get_f32();

            Some((0.0, *max_range as f64))
        }

        fn get_channel_f64(&self, channel: &str) -> Result<Vec<f64>> {
            let series = self
                .data_frame
                .column(channel)
                .map_err(|_| PeacoQCError::ChannelNotFound(channel.to_string()))?;

            // Handle both f32 and f64 columns (FCS files typically use f32)
            let values = if let Ok(f64_vals) = series.f64() {
                f64_vals.into_iter().filter_map(|x| x).collect()
            } else if let Ok(f32_vals) = series.f32() {
                // Cast f32 to f64
                f32_vals
                    .into_iter()
                    .filter_map(|x| x.map(|v| v as f64))
                    .collect()
            } else {
                return Err(PeacoQCError::InvalidChannel(format!(
                    "Channel {} is not numeric (dtype: {:?})",
                    channel,
                    series.dtype()
                )));
            };
            Ok(values)
        }

        fn get_fluorescence_channels(&self) -> Vec<String> {
            // Use the existing is_fluorescence() method
            self.parameters
                .values()
                .filter(|p| p.is_fluorescence())
                .map(|p| p.channel_name.to_string())
                .collect()
        }
    }

    impl FcsFilter for Fcs {
        fn filter(&self, mask: &[bool]) -> Result<Self> {
            let n_events = self.get_event_count_from_dataframe();
            if mask.len() != n_events {
                return Err(PeacoQCError::StatsError(format!(
                    "Mask length {} doesn't match event count {}",
                    mask.len(),
                    n_events
                )));
            }

            // Convert boolean slice to Vec<bool> for Series creation
            let mask_vec: Vec<bool> = mask.to_vec();

            // Create a boolean Series from the mask
            let mask_series = Series::new("mask".into(), mask_vec);

            // Get the boolean ChunkedArray from the Series
            let mask_ca = mask_series.bool().map_err(|e| {
                PeacoQCError::StatsError(format!("Failed to convert mask to boolean array: {}", e))
            })?;

            // Filter DataFrame using boolean mask
            let filtered_df = self
                .data_frame
                .filter(&mask_ca)
                .map_err(|e| PeacoQCError::PolarsError(e))?;

            // Create new Fcs with filtered data
            // Clone the Fcs and replace the data_frame
            let mut filtered_fcs = self.clone();
            filtered_fcs.data_frame = Arc::new(filtered_df);

            // Note: Metadata and parameters are preserved, but the event count
            // in the DataFrame will reflect the filtered data
            Ok(filtered_fcs)
        }
    }

    /// Apply preprocessing steps (compensation and transformation) to FCS data
    ///
    /// This function applies compensation and/or transformation before running PeacoQC,
    /// matching the original R implementation's preprocessing steps:
    /// ```r
    /// ff <- flowCore::compensate(ff, flowCore::keyword(ff)$SPILL)
    /// ff <- flowCore::transform(ff, flowCore::estimateLogicle(ff, ...))
    /// ```
    ///
    /// **Important**: Transformation (biexponential/logicle) should almost always be applied before PeacoQC.
    /// Without transformation, raw fluorescence values have huge dynamic ranges that cause
    /// the MAD detection to remove far more events than expected. The R implementation
    /// typically works with FlowJo-exported data that has already been biexponentially transformed.
    ///
    /// # Arguments
    /// * `fcs` - FCS data structure
    /// * `apply_compensation` - Whether to apply compensation from file's $SPILLOVER keyword
    /// * `apply_transformation` - Whether to apply biexponential transformation to fluorescence channels (matching FlowJo defaults)
    /// * `transform_cofactor` - Unused parameter (kept for API compatibility)
    ///
    /// # Returns
    /// A new Fcs instance with preprocessing applied, or the original if no preprocessing requested
    ///
    /// # Errors
    /// Returns an error if compensation is requested but no $SPILLOVER keyword is found
    pub fn preprocess_fcs(
        mut fcs: Fcs,
        apply_compensation: bool,
        apply_transformation: bool,
        _transform_cofactor: f32, // Currently unused, apply_default_arcsinh_transform uses its own default
    ) -> anyhow::Result<Fcs> {
        use tracing::info;

        // Step 1: Apply compensation (if requested)
        if apply_compensation {
            if !fcs.has_compensation() {
                return Err(anyhow::anyhow!(
                    "Compensation requested but no $SPILLOVER keyword found in FCS file"
                ));
            }
            let compensated_df = fcs
                .apply_file_compensation()
                .map_err(|e| anyhow::anyhow!("Failed to apply compensation: {}", e))?;
            // EventDataFrame is already Arc<DataFrame>, no need to wrap again
            fcs.data_frame = compensated_df;
            info!("Applied compensation from $SPILLOVER keyword");
        }

        // Step 2: Apply transformation (if requested)
        // Use arcsinh transformation with cofactor=2000 to match R PeacoQC behavior
        // This cofactor value produces results closer to R PeacoQC than the default cofactor=200
        // (R PeacoQC typically works with FlowJo-exported data that has been biexponentially transformed,
        //  but arcsinh with cofactor=2000 approximates this well for outlier detection purposes)
        if apply_transformation {
            let transformed_df = fcs
                .apply_default_arcsinh_transform()
                .map_err(|e| anyhow::anyhow!("Failed to apply transformation: {}", e))?;
            // EventDataFrame is already Arc<DataFrame>, no need to wrap again
            fcs.data_frame = transformed_df;
            info!(
                "Applied arcsinh transformation to fluorescence channels with default cofactor of 2000"
            );
        }

        Ok(fcs)
    }
}
