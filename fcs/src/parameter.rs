use derive_builder::Builder;
use polars::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::TransformType;

// New Polars-based types for columnar storage
/// Event data stored as a Polars DataFrame for efficient columnar access
/// Each column represents one parameter (e.g., FSC-A, SSC-A, FL1-A)
/// Benefits:
/// - Zero-copy column access
/// - Built-in SIMD operations
/// - Lazy evaluation for complex queries
/// - Apache Arrow interop
pub type EventDataFrame = Arc<DataFrame>;
pub type EventDatum = f32;
pub type ChannelName = Arc<str>;
pub type LabelName = Arc<str>;
pub type ParameterMap = FxHashMap<ChannelName, Parameter>;

/// Instructions for parameter processing transformations
///
/// These variants indicate what transformations should be applied to the data,
/// not the current state of the data (which may already be processed).
/// This is used to track the processing pipeline for parameters (compensation, unmixing, etc.)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub enum ParameterProcessing {
    /// Raw, unprocessed data from FCS file
    Raw,
    /// Compensated for spectral overlap
    Compensated,
    /// Spectrally unmixed
    Unmixed,
    /// Both compensated and spectrally unmixed
    UnmixedCompensated,
}

impl Default for ParameterProcessing {
    fn default() -> Self {
        ParameterProcessing::Raw
    }
}

/// Category for grouping parameters in user interfaces
///
/// This enum helps organize parameters by their processing state and type,
/// making it easier to present options to users in plotting or analysis interfaces.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub enum ParameterCategory {
    /// Raw parameters (FSC, SSC, Time, custom params)
    Raw,
    /// Fluorescence channels (raw)
    Fluorescence,
    /// Compensated fluorescence
    Compensated,
    /// Transformed parameters (arcsinh applied)
    Transformed,
    /// Both compensated and transformed
    CompensatedTransformed,
    /// Spectrally unmixed
    Unmixed,
}

/// A parameter option for plotting that includes display information
///
/// This struct combines a `Parameter` with UI-specific metadata like display labels
/// and categories, making it easy to present parameter options to users in plotting interfaces.
#[derive(Serialize, Debug, Clone)]
pub struct ParameterOption {
    /// Unique identifier for this option (e.g., "comp_trans::UV379-A")
    pub id: String,
    /// Display label for UI (e.g., "Comp::UV379-A::CD8[T]")
    pub display_label: String,
    /// The actual parameter to use
    pub parameter: Parameter,
    /// Category for UI grouping
    pub category: ParameterCategory,
}

#[derive(Serialize, Debug, Clone, Builder, Hash)]
#[builder(setter(into))]
pub struct Parameter {
    /// The offset of the parameter in the FCS file's event data (1-based index)
    pub parameter_number: usize,
    /// The name of the channel ($PnN keyword)
    pub channel_name: ChannelName,
    /// The label name of the parameter ($PnS keyword)
    pub label_name: LabelName,
    /// The default transform to apply to the parameter
    pub transform: TransformType,

    /// Instructions for parameter processing (compensation, unmixing, etc.)
    /// This enum indicates what transformations should be applied.
    #[builder(default)]
    #[serde(default)]
    pub state: ParameterProcessing,

    /// Excitation wavelength in nanometers (from $PnL keyword, if available)
    #[builder(default)]
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub excitation_wavelength: Option<usize>,
}
impl Parameter {
    /// Creates a new `Parameter` with the specified properties
    ///
    /// # Arguments
    /// * `parameter_number` - The 1-based index of the parameter in the FCS file
    /// * `channel_name` - The channel name from the `$PnN` keyword (e.g., "FSC-A", "FL1-A")
    /// * `label_name` - The label name from the `$PnS` keyword (e.g., "CD8", "CD4")
    /// * `transform` - The default transformation type to apply
    ///
    /// # Returns
    /// A new `Parameter` with `Raw` processing state and no excitation wavelength
    #[must_use]
    pub fn new(
        parameter_number: &usize,
        channel_name: &str,
        label_name: &str,
        transform: &TransformType,
    ) -> Self {
        Self {
            parameter_number: *parameter_number,
            channel_name: channel_name.into(),
            label_name: label_name.into(),
            transform: transform.clone(),
            state: ParameterProcessing::default(),
            excitation_wavelength: None,
        }
    }

    /// Check if this parameter is fluorescence (should be transformed by default)
    /// Excludes FSC (forward scatter), SSC (side scatter), and Time
    #[must_use]
    pub fn is_fluorescence(&self) -> bool {
        let upper = self.channel_name.to_uppercase();
        !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
    }

    /// Get the display label for this parameter
    /// Format examples:
    /// - Raw: "UV379-A::CD8" or just "FSC-A"
    /// - Compensated: "Comp::UV379-A::CD8"
    /// - Unmixed: "Unmix::UV379-A::CD8"
    /// - UnmixedCompensated: "Comp+Unmix::UV379-A::CD8"
    #[must_use]
    pub fn get_display_label(&self) -> String {
        let prefix = match self.state {
            ParameterProcessing::Raw => "",
            ParameterProcessing::Compensated => "Comp::",
            ParameterProcessing::Unmixed => "Unmix::",
            ParameterProcessing::UnmixedCompensated => "Comp+Unmix::",
        };

        // If label_name is empty or same as channel, just use channel
        if self.label_name.is_empty() || self.label_name.as_ref() == self.channel_name.as_ref() {
            format!("{}{}", prefix, self.channel_name)
        } else {
            format!("{}{}::{}", prefix, self.channel_name, self.label_name)
        }
    }

    /// Get the short label (without state prefix)
    #[must_use]
    pub fn get_short_label(&self) -> String {
        if self.label_name.is_empty() || self.label_name.as_ref() == self.channel_name.as_ref() {
            self.channel_name.to_string()
        } else {
            format!("{}::{}", self.channel_name, self.label_name)
        }
    }

    /// Create a new parameter with updated state
    #[must_use]
    pub fn with_state(&self, state: ParameterProcessing) -> Self {
        Self {
            state,
            ..self.clone()
        }
    }

    /// Create a new parameter with updated transform
    #[must_use]
    pub fn with_transform(&self, transform: TransformType) -> Self {
        Self {
            transform,
            ..self.clone()
        }
    }

    /// Generate parameter options for plotting interfaces
    ///
    /// Creates a list of `ParameterOption` structs representing different processing
    /// states of this parameter that can be used for plotting.
    ///
    /// **For fluorescence parameters:**
    /// - Always returns transformed versions (arcsinh applied)
    /// - If `include_compensated` is true, also includes compensated+transformed versions
    /// - Includes unmixed versions if compensation is available
    ///
    /// **For non-fluorescence parameters (FSC, SSC, Time):**
    /// - Returns raw (untransformed) versions only
    ///
    /// # Arguments
    /// * `include_compensated` - Whether to include compensated and unmixed variants
    ///
    /// # Returns
    /// A vector of `ParameterOption` structs ready for use in plotting UIs
    pub fn generate_plot_options(&self, include_compensated: bool) -> Vec<ParameterOption> {
        let mut options = Vec::new();

        if self.is_fluorescence() {
            // For fluorescence: always return transformed version only
            let transformed = self.with_transform(TransformType::default());
            let transformed_label = self.get_short_label();
            options.push(ParameterOption {
                id: format!("transformed::{}", self.channel_name),
                display_label: transformed_label,
                parameter: transformed,
                category: ParameterCategory::Fluorescence,
            });

            // If compensated, add compensated transformed version
            if include_compensated {
                let comp_trans = self
                    .with_state(ParameterProcessing::Compensated)
                    .with_transform(TransformType::default());
                let comp_trans_label = comp_trans.get_display_label();
                options.push(ParameterOption {
                    id: format!("comp_trans::{}", self.channel_name),
                    display_label: comp_trans_label,
                    parameter: comp_trans,
                    category: ParameterCategory::CompensatedTransformed,
                });

                // Add unmixed versions (always transformed)
                let unmix_trans = self
                    .with_state(ParameterProcessing::Unmixed)
                    .with_transform(TransformType::default());
                let unmix_trans_label = unmix_trans.get_display_label();
                options.push(ParameterOption {
                    id: format!("unmix_trans::{}", self.channel_name),
                    display_label: unmix_trans_label,
                    parameter: unmix_trans,
                    category: ParameterCategory::Unmixed,
                });

                // Add combined compensated+unmixed versions (always transformed)
                let comp_unmix_trans = self
                    .with_state(ParameterProcessing::UnmixedCompensated)
                    .with_transform(TransformType::default());
                let comp_unmix_trans_label = comp_unmix_trans.get_display_label();
                options.push(ParameterOption {
                    id: format!("comp_unmix_trans::{}", self.channel_name),
                    display_label: comp_unmix_trans_label,
                    parameter: comp_unmix_trans,
                    category: ParameterCategory::Unmixed,
                });
            }
        } else {
            // For non-fluorescence (scatter/time): include raw parameter
            options.push(ParameterOption {
                id: format!("raw::{}", self.channel_name),
                display_label: self.get_short_label(),
                parameter: self.clone(),
                category: ParameterCategory::Raw,
            });
        }

        options
    }
}
