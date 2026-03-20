//! FCS integration for TRU-OLS unmixing.
//!
//! This module provides integration between TRU-OLS and the `Fcs` struct from `flow-fcs`.
//! It enables TRU-OLS unmixing directly on FCS file data structures.

#[cfg(feature = "flow-fcs")]
use crate::error::TruOlsError;
#[cfg(feature = "flow-fcs")]
use crate::unmixing::{TruOls, UnmixingStrategy};
#[cfg(feature = "flow-fcs")]
use faer::Mat;
#[cfg(feature = "flow-fcs")]
use flow_fcs::Fcs;
#[cfg(feature = "flow-fcs")]

/// Extract detector data from Fcs DataFrame and convert to Mat<f64>.
///
/// This function extracts detector channel data from an Fcs struct and converts
/// it from f32 to f64 for use with TRU-OLS algorithm.
///
/// # Arguments
/// * `fcs` - The Fcs struct containing the data
/// * `detector_names` - Names of detector channels to extract (must exist in FCS file)
///
/// # Returns
/// Matrix of shape (events × detectors) with f64 values
///
/// # Errors
/// Returns error if any detector name is not found or data cannot be extracted
#[cfg(feature = "flow-fcs")]
pub fn extract_detector_data(fcs: &Fcs, detector_names: &[&str]) -> Result<Mat<f64>, TruOlsError> {
    let n_events = fcs.get_event_count_from_dataframe();
    let n_detectors = detector_names.len();

    if n_detectors == 0 {
        return Err(TruOlsError::InsufficientData(
            "At least one detector must be specified".to_string(),
        ));
    }

    // Extract data for each detector
    let mut detector_data: Vec<Vec<f64>> = Vec::with_capacity(n_detectors);
    for &detector_name in detector_names {
        let f32_slice = fcs.get_parameter_events_slice(detector_name).map_err(|e| {
            TruOlsError::InsufficientData(format!("Detector '{}' not found: {}", detector_name, e))
        })?;

        // Convert f32 to f64 efficiently
        let f64_vec: Vec<f64> = f32_slice.iter().map(|&x| x as f64).collect();
        detector_data.push(f64_vec);
    }

    // Build Mat from column vectors (transpose to get events × detectors)
    let result = Mat::from_fn(n_events, n_detectors, |event_idx, detector_idx| {
        detector_data[detector_idx][event_idx]
    });

    Ok(result)
}

/// Derive a short display name from an endmember stem (e.g. control filename without extension).
/// Strips "Reference Group_<well> " and " (Beads)_*" so marker names that contain underscores
/// (e.g. CD14_CD19_dump) are kept whole instead of taking only the last segment.
#[cfg(feature = "flow-fcs")]
fn short_name_from_endmember_stem(stem: &str) -> String {
    let s = stem
        .strip_prefix("Reference Group_")
        .or_else(|| stem.strip_prefix("Reference group_"))
        .unwrap_or(stem);
    let s = if let Some(space_pos) = s.find(' ') {
        s[space_pos + 1..].trim()
    } else {
        s.trim()
    };
    let s = if let Some(beads) = s.find(" (Beads)_") {
        s[..beads].trim()
    } else {
        s
    };
    if s.is_empty() {
        stem.to_string()
    } else {
        s.to_string()
    }
}

/// Extension trait for Fcs to enable TRU-OLS unmixing.
#[cfg(feature = "flow-fcs")]
pub trait TruOlsUnmixing {
    /// Apply TRU-OLS unmixing to FCS data.
    ///
    /// This method performs TRU-OLS unmixing on the FCS data, returning a new
    /// DataFrame with unmixed endmember abundances. Only performs unmixing -
    /// no compensation or transformation is applied.
    ///
    /// # Arguments
    /// * `unstained_control` - Fcs struct containing unstained control data
    /// * `mixing_matrix` - Mixing matrix (detectors × endmembers) as f64
    /// * `detector_names` - Names of detector channels in the mixing matrix (filtered to stained file)
    /// * `endmember_names` - Names of endmembers (dyes) in the mixing matrix
    /// * `autofluorescence_name` - Name of the autofluorescence endmember
    /// * `strategy` - Optional strategy for handling irrelevant abundances (default: Zero)
    /// * `primary_detector_names` - Primary detector names from controls (one per endmember, for naming unmixed columns)
    /// * `primary_detector_pn_names` - $PnN values extracted from primary detectors in control files
    /// * `primary_detector_pn_labels` - $PnS values extracted from primary detectors in control files
    /// * `selected_marker_names` - User-selected marker names from interactive prompt (e.g., "HLA-DR_DQ", "CD4")
    /// * `selected_fluor_names` - User-selected fluor names (dye labels like "RB705", "BV421")
    ///
    /// # Returns
    /// New Fcs struct with original parameters plus unmixed endmember abundance columns
    ///
    /// # Errors
    /// Returns error if data cannot be extracted, mixing matrix dimensions don't match,
    /// or unmixing fails
    fn apply_tru_ols_unmixing(
        &self,
        unstained_control: &Fcs,
        mixing_matrix: Mat<f64>,
        detector_names: &[&str],
        endmember_names: &[&str],
        autofluorescence_name: &str,
        strategy: Option<UnmixingStrategy>,
        primary_detector_names: &[Option<String>],
        primary_detector_pn_names: &[Option<String>],
        primary_detector_pn_labels: &[Option<String>],
        selected_marker_names: &[Option<String>],
        selected_fluor_names: &[Option<String>],
    ) -> Result<Fcs, TruOlsError>;
}

#[cfg(feature = "flow-fcs")]
impl TruOlsUnmixing for Fcs {
    fn apply_tru_ols_unmixing(
        &self,
        unstained_control: &Fcs,
        mixing_matrix: Mat<f64>,
        detector_names: &[&str],
        endmember_names: &[&str],
        autofluorescence_name: &str,
        strategy: Option<UnmixingStrategy>,
        primary_detector_names: &[Option<String>],
        primary_detector_pn_names: &[Option<String>],
        primary_detector_pn_labels: &[Option<String>],
        selected_marker_names: &[Option<String>],
        selected_fluor_names: &[Option<String>],
    ) -> Result<Fcs, TruOlsError> {
        // Create a mapping from endmember names to detector names
        // This helps us retrieve the original channel label for each endmember
        let endmember_to_detector: std::collections::HashMap<&str, &str> = endmember_names
            .iter()
            .zip(detector_names.iter())
            .map(|(&em, &det)| (em, det))
            .collect();
        use flow_fcs::keyword::Keyword;
        use polars::prelude::Column;
        use std::sync::Arc;

        // Validate dimensions
        let n_detectors = mixing_matrix.nrows();
        let n_endmembers = mixing_matrix.ncols();

        if detector_names.len() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: detector_names.len(),
            });
        }

        if endmember_names.len() != n_endmembers {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_endmembers,
                actual: endmember_names.len(),
            });
        }

        // Find autofluorescence index
        let autofluorescence_idx = endmember_names
            .iter()
            .position(|&name| name == autofluorescence_name)
            .ok_or_else(|| {
                TruOlsError::InsufficientData(format!(
                    "Autofluorescence endmember '{}' not found in endmember names",
                    autofluorescence_name
                ))
            })?;

        // Validate primary detector metadata vectors length (if provided)
        if !primary_detector_pn_names.is_empty()
            && primary_detector_pn_names.len() != endmember_names.len()
        {
            return Err(TruOlsError::InsufficientData(format!(
                "primary_detector_pn_names length ({}) does not match endmember count ({})",
                primary_detector_pn_names.len(),
                endmember_names.len()
            )));
        }

        if !primary_detector_pn_labels.is_empty()
            && primary_detector_pn_labels.len() != endmember_names.len()
        {
            return Err(TruOlsError::InsufficientData(format!(
                "primary_detector_pn_labels length ({}) does not match endmember count ({})",
                primary_detector_pn_labels.len(),
                endmember_names.len()
            )));
        }

        // Extract detector data from both files
        let stained_data = extract_detector_data(self, detector_names)?;
        let unstained_data = extract_detector_data(unstained_control, detector_names)?;

        // Create TRU-OLS instance
        let mut tru_ols = TruOls::new(mixing_matrix, unstained_data, autofluorescence_idx)?;

        // Set strategy if provided
        if let Some(s) = strategy {
            tru_ols.set_strategy(s);
        }

        // Perform unmixing
        let unmixed_abundances = tru_ols.unmix(stained_data.as_ref())?;

        // Create a new FCS struct with fresh parameters
        let mut output_fcs = self.clone();

        // Helper function to identify scatter/time parameters
        fn is_scatter_or_time_param(name: &str) -> bool {
            let upper = name.to_uppercase();
            upper.contains("FSC")
                || upper.contains("SSC")
                || upper.contains("TIME")
                || upper.contains("TIME ")
        }

        // Step 1: Preserve scatter/time parameters from original
        let mut scatter_time_params: Vec<String> = Vec::new();
        let mut scatter_time_columns: Vec<polars::prelude::Column> = Vec::new();

        for param_name in self.get_parameter_names_from_dataframe() {
            if is_scatter_or_time_param(&param_name) {
                scatter_time_params.push(param_name.clone());

                // Get the column data
                if let Ok(values) = self.get_parameter_events_slice(&param_name) {
                    let column =
                        polars::prelude::Column::new(param_name.clone().into(), values.to_vec());
                    scatter_time_columns.push(column);
                }
            }
        }

        // Clear parameters and rebuild with only scatter/time
        output_fcs.parameters.clear();

        // Also clear old parameter keywords to rebuild them
        output_fcs
            .metadata
            .keywords
            .retain(|k, _| !k.starts_with("$P"));

        // Re-add scatter/time parameters
        let mut param_num = 1;
        for scatter_param_name in &scatter_time_params {
            if let Some(orig_param) = self.parameters.get(scatter_param_name.as_str()) {
                // Add the parameter
                output_fcs
                    .parameters
                    .insert(scatter_param_name.clone().into(), orig_param.clone());

                // Also ensure FCS keywords for this parameter are preserved
                use flow_fcs::keyword::{IntegerKeyword, MixedKeyword, StringKeyword};

                output_fcs.metadata.keywords.insert(
                    format!("$P{}N", param_num),
                    Keyword::String(StringKeyword::PnN(Arc::from(
                        orig_param.channel_name.as_ref().to_string(),
                    ))),
                );

                output_fcs.metadata.keywords.insert(
                    format!("$P{}S", param_num),
                    Keyword::String(StringKeyword::PnS(Arc::from(""))),
                );

                output_fcs.metadata.keywords.insert(
                    format!("$P{}B", param_num),
                    Keyword::Int(IntegerKeyword::PnB(32)),
                );

                output_fcs.metadata.keywords.insert(
                    format!("$P{}R", param_num),
                    Keyword::Int(IntegerKeyword::PnR(262144)),
                );

                output_fcs.metadata.keywords.insert(
                    format!("$P{}E", param_num),
                    Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
                );

                param_num += 1;
            }
        }

        // Step 2: Build unmixed columns
        let scatter_time_count = scatter_time_columns.len();
        let mut result_df_columns: Vec<polars::prelude::Column> = scatter_time_columns;
        let starting_param_num = param_num;

        // Add unmixed endmember columns
        let n_events = unmixed_abundances.nrows();

        // Process fluorophore endmembers (skip autofluorescence)
        for (endmember_idx, &endmember_name) in endmember_names.iter().enumerate() {
            // Skip autofluorescence for now - handle separately at the end
            if endmember_name == autofluorescence_name {
                continue;
            }

            // Extract column from unmixed abundances and convert to f32
            let f64_values: Vec<f64> = (0..n_events)
                .map(|event_idx| unmixed_abundances[(event_idx, endmember_idx)])
                .collect();

            // Convert to f32 and clamp to non-negative to avoid large negative display from overextraction
            let f32_values: Vec<f32> = f64_values
                .iter()
                .map(|&x| (x as f32).max(0.0))
                .collect();

            // Determine naming and labels for this unmixed column
            // Look up which detector this endmember maps to in the stained file
            let original_detector = endmember_to_detector.get(endmember_name);

            // Extract fully-stained parameter metadata if available
            let mut fs_pn: Option<String> = None;
            let mut fs_ps: Option<String> = None;
            if let Some(det_name) = original_detector {
                if let Some(param) = self.parameters.get(&Arc::from(*det_name)) {
                    if !param.channel_name.is_empty() {
                        fs_pn = Some(param.channel_name.to_string());
                    }
                    if !param.label_name.is_empty() {
                        fs_ps = Some(param.label_name.to_string());
                    }
                }
            }

            let control_pn = primary_detector_pn_names
                .get(endmember_idx)
                .and_then(|o| o.clone());
            let control_ps = primary_detector_pn_labels
                .get(endmember_idx)
                .and_then(|o| o.clone());

            // Get primary detector name from controls
            let primary_detector_name = primary_detector_names
                .get(endmember_idx)
                .and_then(|o| o.clone());

            let detector_id = original_detector
                .map(|s| s.to_string())
                .unwrap_or_else(|| endmember_name.to_string());
            // Use the user-selected marker name if available, otherwise fall back to extraction
            let endmember_short_name = selected_marker_names
                .get(endmember_idx)
                .and_then(|opt| opt.clone())
                .unwrap_or_else(|| {
                    if endmember_name.contains("Autofluorescence")
                        || endmember_name.eq("Autofluorescence")
                    {
                        "Autofluorescence".to_string()
                    } else {
                        short_name_from_endmember_stem(endmember_name)
                    }
                }); // Robust precedence for unmixed column naming:
            // 1) User-selected marker name from interactive prompt (most explicit from user)
            // 2) Extracted endmember marker name (from filename - most robust and unique)
            // 3) Control file's $PnS (parameter label/short label) - from FCS metadata but can have duplicates
            // 4) Control file's $PnN (parameter name) - from FCS metadata but can have duplicates
            // 5) Primary detector identifier (e.g., "V7-A", "R4-A")
            // 6) Fully-stained file's $PnN (if exists)
            // 7) Fully-stained file's $PnS (if exists)
            // 8) Detector ID from stained file
            // 9) Endmember name
            //
            // Rationale: We prioritize the user-selected marker name because:
            // - User has explicitly confirmed it's correct
            // - It's unique (each control file represents a different marker/fluorophore)
            // - It's human-readable (CD4, CD19, HLA-DR_DQ, etc.)
            // - It doesn't depend on detector mapping (multiple markers can use the same detector)
            // FCS metadata is used as fallback when no user selection was made

            let chosen_pn = Some(endmember_short_name.clone())
                .filter(|n| n != "Autofluorescence" && !n.is_empty() && n.len() > 2)
                .or_else(|| control_ps.clone())
                .or_else(|| control_pn.clone())
                .or_else(|| primary_detector_name.clone())
                .or(fs_pn.clone())
                .or(fs_ps.clone())
                .unwrap_or(detector_id.clone());

            // Choose $PnS (label) with same precedence
            let chosen_ps = Some(endmember_short_name.clone())
                .filter(|n| n != "Autofluorescence" && !n.is_empty() && n.len() > 2)
                .or_else(|| control_ps.clone())
                .or_else(|| control_pn.clone())
                .or_else(|| primary_detector_name.clone())
                .or(fs_pn.clone())
                .or(fs_ps.clone())
                .unwrap_or(detector_id.clone());

            // Final unmixed column name keeps the Unmixed_ prefix
            let unmixed_col_name = format!("Unmixed_{}", chosen_pn);
            let column = Column::new(unmixed_col_name.clone().into(), f32_values.clone());

            // Collect column for DataFrame creation
            result_df_columns.push(column);

            // Add parameter metadata for this new column
            use flow_fcs::{Parameter, TransformType};
            let param_num = starting_param_num + result_df_columns.len() - scatter_time_count - 1;

            let param = Parameter::new(
                &param_num,
                &unmixed_col_name,
                &chosen_ps,
                &TransformType::Linear,
            );
            output_fcs
                .parameters
                .insert(unmixed_col_name.clone().into(), param);

            // Add FCS TEXT segment keywords for this parameter
            use flow_fcs::keyword::{IntegerKeyword, MixedKeyword, StringKeyword};

            // Get the original detector name and its label for this endmember
            let original_detector = endmember_to_detector.get(endmember_name);

            // Decide on the short label ($PnS) to use for this unmixed parameter:
            // 1. Prefer user-selected fluor/dye name from interactive prompt (most explicit from user)
            // 2. Otherwise use marker/dye label from the control file's $PnS (e.g., "RB705", "FITC")
            // 3. Otherwise use the original stained file's parameter label
            // 4. Otherwise fall back to the endmember name or detector ID
            // This ensures $PnS reflects the marker/dye, not the detector hardware name.
            let marker_label = selected_fluor_names
                .get(endmember_idx)
                .and_then(|opt| opt.clone())
                .or_else(|| control_ps.clone())
                .or_else(|| {
                    original_detector.and_then(|det_name| {
                        self.parameters
                            .get(&Arc::from(*det_name))
                            .and_then(|param| {
                                if !param.label_name.is_empty() {
                                    Some(param.label_name.as_ref().to_string())
                                } else {
                                    None
                                }
                            })
                    })
                })
                .unwrap_or_else(|| endmember_name.to_string());

            // $P{i}N - Parameter name: use "Unmixed_" + fluorophore name, fall back to detector ID
            // Prefer the fluorophore name extracted from endmember filename (e.g., "CD56", "TIM3")
            // over detector name (e.g., "B10-A")
            let pn_name = format!(
                "Unmixed_{}",
                Some(endmember_short_name.clone())
                    .filter(|n| n != "Autofluorescence" && !n.is_empty() && n.len() > 2)
                    .unwrap_or_else(|| detector_id.clone())
            );
            output_fcs.metadata.keywords.insert(
                format!("$P{}N", param_num),
                Keyword::String(StringKeyword::PnN(Arc::from(pn_name))),
            );

            // $P{i}S - Parameter short name (use marker/dye label from control file)
            output_fcs.metadata.keywords.insert(
                format!("$P{}S", param_num),
                Keyword::String(StringKeyword::PnS(Arc::from(marker_label))),
            );

            // $P{i}B - Bits per parameter (32 for float32)
            output_fcs.metadata.keywords.insert(
                format!("$P{}B", param_num),
                Keyword::Int(IntegerKeyword::PnB(32)),
            );

            // $P{i}R - Range (max value, use large value for abundances)
            output_fcs.metadata.keywords.insert(
                format!("$P{}R", param_num),
                Keyword::Int(IntegerKeyword::PnR(262144)),
            );

            // $P{i}E - Amplification (0,0 for linear)
            output_fcs.metadata.keywords.insert(
                format!("$P{}E", param_num),
                Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
            );
        }

        // Step 3: Create synthetic autofluorescence channel
        if let Some(af_idx) = endmember_names
            .iter()
            .position(|&name| name == autofluorescence_name)
        {
            let f64_values: Vec<f64> = (0..n_events)
                .map(|event_idx| unmixed_abundances[(event_idx, af_idx)])
                .collect();
            let f32_values: Vec<f32> = f64_values
                .iter()
                .map(|&x| (x as f32).max(0.0))
                .collect();

            let af_column_name = "Unmixed_Autofluorescence";
            let column = Column::new(af_column_name.into(), f32_values);
            result_df_columns.push(column);

            // Add parameter metadata for autofluorescence
            use flow_fcs::{Parameter, TransformType};
            let param_num = starting_param_num + result_df_columns.len() - scatter_time_count - 1;
            let param = Parameter::new(
                &param_num,
                af_column_name,
                af_column_name,
                &TransformType::Linear,
            );
            output_fcs.parameters.insert(af_column_name.into(), param);

            // Add FCS keywords for autofluorescence
            use flow_fcs::keyword::{IntegerKeyword, MixedKeyword, StringKeyword};
            output_fcs.metadata.keywords.insert(
                format!("$P{}N", param_num),
                Keyword::String(StringKeyword::PnN(Arc::from(af_column_name))),
            );

            // Leave $PnS blank for autofluorescence
            output_fcs.metadata.keywords.insert(
                format!("$P{}S", param_num),
                Keyword::String(StringKeyword::PnS(Arc::from(""))),
            );

            output_fcs.metadata.keywords.insert(
                format!("$P{}B", param_num),
                Keyword::Int(IntegerKeyword::PnB(32)),
            );

            output_fcs.metadata.keywords.insert(
                format!("$P{}R", param_num),
                Keyword::Int(IntegerKeyword::PnR(262144)),
            );

            output_fcs.metadata.keywords.insert(
                format!("$P{}E", param_num),
                Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
            );
        }

        // Create new DataFrame from unmixed columns only (not the original detectors)
        let result_df = polars::frame::DataFrame::new_infer_height(result_df_columns).map_err(|e| {
            TruOlsError::InsufficientData(format!(
                "Failed to create DataFrame from unmixed columns: {}",
                e
            ))
        })?;

        // Update the DataFrame in the output FCS
        output_fcs.data_frame = Arc::new(result_df);

        // Update $PAR to reflect new parameter count
        let new_param_count = output_fcs.parameters.len();
        output_fcs.metadata.keywords.insert(
            "$PAR".to_string(),
            Keyword::Int(flow_fcs::keyword::IntegerKeyword::PAR(new_param_count)),
        );

        Ok(output_fcs)
    }
}

#[cfg(test)]
#[cfg(feature = "flow-fcs")]
mod tests {
    use super::*;
    use faer::mat;
    use flow_fcs::{
        Header, Metadata, Parameter, TransformType, file::AccessWrapper, parameter::ParameterMap,
    };
    use polars::{frame::DataFrame, prelude::Column};
    use std::sync::Arc;

    /// Helper function to create a test Fcs struct with detector data
    fn create_test_fcs() -> Result<Fcs, Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;

        // Create a temporary file for testing
        let temp_path = std::env::temp_dir().join("test_tru_ols_fcs.tmp");
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(b"test")?;
        }

        // Create test DataFrame with detector channels
        let mut columns = Vec::new();
        columns.push(Column::new(
            "FL1-A".into(),
            vec![100.0f32, 200.0, 300.0, 400.0, 500.0],
        ));
        columns.push(Column::new(
            "FL2-A".into(),
            vec![50.0f32, 150.0, 250.0, 350.0, 450.0],
        ));
        columns.push(Column::new(
            "FL3-A".into(),
            vec![10.0f32, 20.0, 30.0, 40.0, 50.0],
        ));

        let df = DataFrame::new_infer_height(columns).expect("Failed to create test DataFrame");

        // Create parameter map
        let mut params = ParameterMap::default();
        params.insert(
            "FL1-A".into(),
            Parameter::new(&1, "FL1-A", "FL1-A", &TransformType::Linear),
        );
        params.insert(
            "FL2-A".into(),
            Parameter::new(&2, "FL2-A", "FL2-A", &TransformType::Linear),
        );
        params.insert(
            "FL3-A".into(),
            Parameter::new(&3, "FL3-A", "FL3-A", &TransformType::Linear),
        );

        Ok(Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        })
    }

    #[test]
    fn test_extract_detector_data_success() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let detector_names = &["FL1-A", "FL2-A", "FL3-A"];
        let result = extract_detector_data(&fcs, detector_names);

        assert!(result.is_ok(), "Should successfully extract detector data");
        let data = result.unwrap();

        // Check dimensions: 5 events × 3 detectors
        assert_eq!(data.nrows(), 5, "Should have 5 events");
        assert_eq!(data.ncols(), 3, "Should have 3 detectors");

        // Check first event values (Mat is row, col)
        assert!(
            (data[(0, 0)] - 100.0).abs() < 1e-6,
            "First detector, first event should be 100.0"
        );
        assert!(
            (data[(0, 1)] - 50.0).abs() < 1e-6,
            "Second detector, first event should be 50.0"
        );
        assert!(
            (data[(0, 2)] - 10.0).abs() < 1e-6,
            "Third detector, first event should be 10.0"
        );

        // Check last event values
        assert!(
            (data[(4, 0)] - 500.0).abs() < 1e-6,
            "First detector, last event should be 500.0"
        );
        assert!(
            (data[(4, 1)] - 450.0).abs() < 1e-6,
            "Second detector, last event should be 450.0"
        );
        assert!(
            (data[(4, 2)] - 50.0).abs() < 1e-6,
            "Third detector, last event should be 50.0"
        );
    }

    #[test]
    fn test_extract_detector_data_missing_detector() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let detector_names = &["FL1-A", "NonExistent"];
        let result = extract_detector_data(&fcs, detector_names);

        assert!(result.is_err(), "Should error on missing detector");
        match result.unwrap_err() {
            TruOlsError::InsufficientData(msg) => {
                assert!(
                    msg.contains("NonExistent"),
                    "Error message should mention missing detector"
                );
            }
            _ => panic!("Should return InsufficientData error"),
        }
    }

    #[test]
    fn test_extract_detector_data_empty_list() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let detector_names: &[&str] = &[];
        let result = extract_detector_data(&fcs, detector_names);

        assert!(result.is_err(), "Should error on empty detector list");
        match result.unwrap_err() {
            TruOlsError::InsufficientData(msg) => {
                assert!(
                    msg.contains("At least one detector"),
                    "Error message should mention requirement"
                );
            }
            _ => panic!("Should return InsufficientData error"),
        }
    }

    #[test]
    fn test_extract_detector_data_subset() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Extract only first two detectors
        let detector_names = &["FL1-A", "FL2-A"];
        let result = extract_detector_data(&fcs, detector_names);

        assert!(
            result.is_ok(),
            "Should successfully extract subset of detectors"
        );
        let data = result.unwrap();

        assert_eq!(data.nrows(), 5, "Should have 5 events");
        assert_eq!(data.ncols(), 2, "Should have 2 detectors");

        // Verify values
        assert!((data[(0, 0)] - 100.0).abs() < 1e-6);
        assert!((data[(0, 1)] - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_f32_to_f64_conversion_precision() {
        // Test that f32 to f64 conversion preserves precision
        let f32_data = vec![1.0f32, 2.5, 3.14159, -1.0, 0.0, 1e-6];
        let f64_data: Vec<f64> = f32_data.iter().map(|&x| x as f64).collect();

        assert_eq!(f64_data.len(), f32_data.len());
        for (f32_val, f64_val) in f32_data.iter().zip(f64_data.iter()) {
            // f32 to f64 conversion should be exact
            assert_eq!(*f64_val, *f32_val as f64);
        }
    }

    #[test]
    fn test_extract_detector_data_ordering() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Extract detectors in different order
        let detector_names = &["FL3-A", "FL1-A", "FL2-A"];
        let result = extract_detector_data(&fcs, detector_names);

        assert!(
            result.is_ok(),
            "Should successfully extract detectors in different order"
        );
        let data = result.unwrap();

        // Check that ordering is preserved (FL3-A should be first column)
        assert!(
            (data[(0, 0)] - 10.0).abs() < 1e-6,
            "First column should be FL3-A (10.0)"
        );
        assert!(
            (data[(0, 1)] - 100.0).abs() < 1e-6,
            "Second column should be FL1-A (100.0)"
        );
        assert!(
            (data[(0, 2)] - 50.0).abs() < 1e-6,
            "Third column should be FL2-A (50.0)"
        );
    }

    #[test]
    fn test_apply_tru_ols_unmixing_basic() {
        let stained_fcs = create_test_fcs().expect("Failed to create stained FCS");
        let unstained_fcs = create_test_fcs().expect("Failed to create unstained FCS");

        // Create a simple mixing matrix: 3 detectors × 2 endmembers
        // Identity-like matrix for simplicity
        let mixing_matrix = mat![[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]];

        let detector_names = &["FL1-A", "FL2-A", "FL3-A"];
        let endmember_names = &["Dye1", "Autofluorescence"];
        let autofluorescence = "Autofluorescence";

        let empty_pn: Vec<Option<String>> = vec![None; endmember_names.len()];
        let result = stained_fcs.apply_tru_ols_unmixing(
            &unstained_fcs,
            mixing_matrix,
            detector_names,
            endmember_names,
            autofluorescence,
            None, // Use default strategy
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
        );

        assert!(result.is_ok(), "Should successfully apply TRU-OLS unmixing");
        let unmixed_fcs = result.unwrap();

        // Check that DataFrame has correct number of columns (endmembers)
        assert_eq!(
            unmixed_fcs.data_frame.width(),
            2,
            "Should have 2 endmember columns"
        );
        assert_eq!(unmixed_fcs.data_frame.height(), 5, "Should have 5 events");

        // Check that columns exist
        let col_names: Vec<String> = unmixed_fcs
            .get_parameter_names_from_dataframe()
            .iter()
            .map(|s: &String| s.to_string())
            .collect();
        assert!(
            col_names.contains(&"Unmixed_Dye1".to_string()),
            "Should have Unmixed_Dye1 column"
        );
        assert!(
            col_names.contains(&"Unmixed_Autofluorescence".to_string()),
            "Should have Unmixed_Autofluorescence column"
        );
    }

    #[test]
    fn test_apply_tru_ols_unmixing_dimension_mismatch() {
        let stained_fcs = create_test_fcs().expect("Failed to create stained FCS");
        let unstained_fcs = create_test_fcs().expect("Failed to create unstained FCS");

        // Create mixing matrix with wrong dimensions
        let mixing_matrix = mat![[0.9, 0.1], [0.1, 0.9]]; // 2×2 matrix, but we have 3 detectors

        let detector_names = &["FL1-A", "FL2-A", "FL3-A"]; // 3 detectors
        let endmember_names = &["Dye1", "Autofluorescence"];
        let autofluorescence = "Autofluorescence";

        let empty_pn: Vec<Option<String>> = vec![None; endmember_names.len()];
        let result = stained_fcs.apply_tru_ols_unmixing(
            &unstained_fcs,
            mixing_matrix,
            detector_names,
            endmember_names,
            autofluorescence,
            None,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
        );

        assert!(result.is_err(), "Should error on dimension mismatch");
        match result.unwrap_err() {
            TruOlsError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 2, "Expected 2 rows in mixing matrix");
                assert_eq!(actual, 3, "Actual 3 detectors provided");
            }
            _ => panic!("Should return DimensionMismatch error"),
        }
    }

    #[test]
    fn test_apply_tru_ols_unmixing_missing_autofluorescence() {
        let stained_fcs = create_test_fcs().expect("Failed to create stained FCS");
        let unstained_fcs = create_test_fcs().expect("Failed to create unstained FCS");

        let mixing_matrix = mat![[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]];

        let detector_names = &["FL1-A", "FL2-A", "FL3-A"];
        let endmember_names = &["Dye1", "Dye2"]; // No Autofluorescence!
        let autofluorescence = "Autofluorescence";

        let empty_pn: Vec<Option<String>> = vec![None; endmember_names.len()];
        let result = stained_fcs.apply_tru_ols_unmixing(
            &unstained_fcs,
            mixing_matrix,
            detector_names,
            endmember_names,
            autofluorescence,
            None,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
        );

        assert!(result.is_err(), "Should error on missing autofluorescence");
        match result.unwrap_err() {
            TruOlsError::InsufficientData(msg) => {
                assert!(
                    msg.contains("Autofluorescence"),
                    "Error message should mention autofluorescence"
                );
            }
            _ => panic!("Should return InsufficientData error"),
        }
    }

    #[test]
    fn test_unmixing_excludes_original_fluorescent_detectors() {
        // Create test FCS with scatter, time, and fluorescent channels
        use std::fs::File;
        use std::io::Write;

        let temp_path = std::env::temp_dir().join("test_tru_ols_exclude_detectors.tmp");
        {
            let mut f = File::create(&temp_path).expect("Failed to create temp file");
            f.write_all(b"test").expect("Failed to write temp file");
        }

        let mut columns = Vec::new();
        // Scatter/time parameters
        columns.push(Column::new(
            "FSC-A".into(),
            vec![100.0f32, 200.0, 300.0, 400.0, 500.0],
        ));
        columns.push(Column::new(
            "SSC-A".into(),
            vec![50.0f32, 100.0, 150.0, 200.0, 250.0],
        ));
        columns.push(Column::new("Time".into(), vec![1.0f32, 2.0, 3.0, 4.0, 5.0]));
        // Fluorescent parameters (should be excluded from output)
        columns.push(Column::new(
            "FL1-A".into(),
            vec![100.0f32, 200.0, 300.0, 400.0, 500.0],
        ));
        columns.push(Column::new(
            "FL2-A".into(),
            vec![50.0f32, 150.0, 250.0, 350.0, 450.0],
        ));
        columns.push(Column::new(
            "FL3-A".into(),
            vec![10.0f32, 20.0, 30.0, 40.0, 50.0],
        ));

        let df = DataFrame::new_infer_height(columns).expect("Failed to create test DataFrame");

        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );
        params.insert(
            "SSC-A".into(),
            Parameter::new(&2, "SSC-A", "SSC-A", &TransformType::Linear),
        );
        params.insert(
            "Time".into(),
            Parameter::new(&3, "Time", "Time", &TransformType::Linear),
        );
        params.insert(
            "FL1-A".into(),
            Parameter::new(&4, "FL1-A", "Dye1", &TransformType::Linear),
        );
        params.insert(
            "FL2-A".into(),
            Parameter::new(&5, "FL2-A", "Dye2", &TransformType::Linear),
        );
        params.insert(
            "FL3-A".into(),
            Parameter::new(&6, "FL3-A", "Dye3", &TransformType::Linear),
        );

        let stained_fcs = Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))
                .expect("Failed to create AccessWrapper"),
        };

        let unstained_fcs = stained_fcs.clone();

        // Create mixing matrix for 3 detectors × 2 endmembers
        let mixing_matrix = mat![[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]];

        let detector_names = &["FL1-A", "FL2-A", "FL3-A"];
        let endmember_names = &["Dye1", "Autofluorescence"];
        let autofluorescence = "Autofluorescence";

        let empty_pn: Vec<Option<String>> = vec![None; endmember_names.len()];
        let result = stained_fcs.apply_tru_ols_unmixing(
            &unstained_fcs,
            mixing_matrix,
            detector_names,
            endmember_names,
            autofluorescence,
            None,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
            &empty_pn,
        );

        assert!(result.is_ok(), "Unmixing should succeed");
        let unmixed_fcs = result.unwrap();

        // Get column names from output
        let col_names: Vec<String> = unmixed_fcs
            .get_parameter_names_from_dataframe()
            .iter()
            .map(|s: &String| s.to_string())
            .collect();

        // Verify scatter/time parameters are preserved
        assert!(
            col_names.contains(&"FSC-A".to_string()),
            "Output should contain FSC-A"
        );
        assert!(
            col_names.contains(&"SSC-A".to_string()),
            "Output should contain SSC-A"
        );
        assert!(
            col_names.contains(&"Time".to_string()),
            "Output should contain Time"
        );

        // Verify unmixed columns exist
        assert!(
            col_names.contains(&"Unmixed_Dye1".to_string()),
            "Output should contain Unmixed_Dye1"
        );
        assert!(
            col_names.contains(&"Unmixed_Autofluorescence".to_string()),
            "Output should contain Unmixed_Autofluorescence"
        );

        // Verify original fluorescent detectors are NOT in output
        assert!(
            !col_names.contains(&"FL1-A".to_string()),
            "Output should NOT contain original FL1-A"
        );
        assert!(
            !col_names.contains(&"FL2-A".to_string()),
            "Output should NOT contain original FL2-A"
        );
        assert!(
            !col_names.contains(&"FL3-A".to_string()),
            "Output should NOT contain original FL3-A"
        );

        // Verify parameter count
        // Expected: FSC-A, SSC-A, Time (3 scatter/time) + Unmixed_FL1-A + Unmixed_Autofluorescence (2 unmixed) = 5 total
        assert_eq!(
            col_names.len(),
            5,
            "Output should have 5 parameters (3 scatter/time + 2 unmixed), got: {:?}",
            col_names
        );
    }
}
