//! FCS integration for TRU-OLS unmixing.
//!
//! This module provides integration between TRU-OLS and the `Fcs` struct from `flow-fcs`.
//! It enables TRU-OLS unmixing directly on FCS file data structures.

#[cfg(feature = "flow-fcs")]
use crate::error::TruOlsError;
#[cfg(feature = "flow-fcs")]
use crate::unmixing::{TruOls, UnmixingStrategy};
#[cfg(feature = "flow-fcs")]
use crate::provenance::UnmixProvenance;
#[cfg(feature = "flow-fcs")]
use faer::{Col, Mat};
#[cfg(feature = "flow-fcs")]
use flow_fcs::Fcs;

/// Custom TEXT keyword identifying an unmixed product and its method.
/// Stored/looked up as `$UNMIXED` because `flow-fcs` write always `$`-prefixes keys.
#[cfg(feature = "flow-fcs")]
pub const UNMIXED_KEYWORD: &str = "$UNMIXED";
/// Pre-`$`-prefix alias (in-memory inserts before a write round-trip).
#[cfg(feature = "flow-fcs")]
pub const UNMIXED_KEYWORD_BARE: &str = "UNMIXED";
/// Value written by the TRU-OLS export path.
#[cfg(feature = "flow-fcs")]
pub const UNMIXED_METHOD_TRU_OLS: &str = "TRU-OLS";
/// Reserved for plain OLS export.
#[cfg(feature = "flow-fcs")]
pub const UNMIXED_METHOD_OLS: &str = "OLS";
/// Explicit non-unmixed marker (absence also means not unmixed).
#[cfg(feature = "flow-fcs")]
pub const UNMIXED_METHOD_FALSE: &str = "FALSE";
/// Default AF abundance `$PnN`.
#[cfg(feature = "flow-fcs")]
pub const DEFAULT_AF_CHANNEL_NAME: &str = "Autofluorescence";

/// Cast TRU-OLS abundances to FCS float32 **without** clamping.
///
/// UCM (and unrestricted OLS) produce signed values around zero for
/// irrelevant / over-extracted endmembers. Clamping to ≥0 piles those events
/// on the plot axes and undoes UCM.
#[cfg(feature = "flow-fcs")]
fn abundance_f64_to_f32(values: &[f64]) -> Vec<f32> {
    values.iter().map(|&x| x as f32).collect()
}

#[cfg(feature = "flow-fcs")]
fn identity_spillover_keyword(channel_names: &[String]) -> flow_fcs::keyword::MixedKeyword {
    let n = channel_names.len();
    let mut matrix_values = vec![0.0_f32; n * n];
    for i in 0..n {
        matrix_values[i * n + i] = 1.0;
    }
    flow_fcs::keyword::MixedKeyword::SPILLOVER {
        n_parameters: n,
        parameter_names: channel_names.to_vec(),
        matrix_values,
    }
}

#[cfg(feature = "flow-fcs")]
fn strip_acquisition_spillover_keywords<S: std::hash::BuildHasher>(
    keywords: &mut std::collections::HashMap<String, flow_fcs::keyword::Keyword, S>,
) {
    for key in ["$SPILLOVER", "$SPILL", "$COMP"] {
        keywords.remove(key);
    }
}

/// True for per-parameter TEXT keys (`$P12N`, `P10DISPLAY`, …), not `$PAR` / `$PROJ` / `$PLATENAME`.
///
/// Cytek often stores display keys without a `$` prefix; both forms must be stripped when
/// rebuilding the parameter list so orphans do not round-trip onto reduced-channel exports.
#[cfg(feature = "flow-fcs")]
pub(crate) fn is_parameter_index_keyword(key: &str) -> bool {
    let rest = key.strip_prefix('$').unwrap_or(key);
    let mut chars = rest.chars();
    if chars.next() != Some('P') {
        return false;
    }
    let mut saw_digit = false;
    for c in chars {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else {
            // `$P12N`, `P10DISPLAY` — digit run then a non-digit suffix.
            return saw_digit;
        }
    }
    false
}

/// Drop `$Pn*` / `Pn*` parameter keywords while keeping sample metadata (`$PROJ`, `$PLATENAME`, …).
#[cfg(feature = "flow-fcs")]
fn strip_parameter_index_keywords<S: std::hash::BuildHasher>(
    keywords: &mut std::collections::HashMap<String, flow_fcs::keyword::Keyword, S>,
) {
    keywords.retain(|k, _| !is_parameter_index_keyword(k));
}

/// The `$GUID` a source file identifies itself by, if it has one.
///
/// Checked under both spellings: the writer `$`-prefixes every key, but a file
/// assembled in memory may still hold the bare form.
#[cfg(feature = "flow-fcs")]
fn source_guid(fcs: &Fcs) -> Option<String> {
    for key in ["$GUID", "GUID"] {
        if let Some(value) = fcs.metadata.keywords.get(key).and_then(|k| k.value_str()) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Free text for `$UNSTAINEDINFO` naming the control the background came from.
///
/// FCS 3.2 defines this keyword for exactly this: describing how unstained /
/// autofluorescence information was obtained. A GUID alone is not human
/// readable, so this records the control's filename too.
#[cfg(feature = "flow-fcs")]
fn describe_unstained_control(unstained: &Fcs, autofluorescence_name: &str) -> Option<String> {
    let name = unstained
        .metadata
        .keywords
        .get("$FIL")
        .and_then(|k| k.value_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::path::Path::new(&unstained.file_access.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })?;

    Some(format!(
        "Autofluorescence endmember '{autofluorescence_name}' derived from unstained control '{name}'"
    ))
}

/// Replace any cloned source GUID with a fresh identity for the unmixed product.
#[cfg(feature = "flow-fcs")]
pub(crate) fn mint_unmixed_file_guid(fcs: &mut Fcs) {
    fcs.metadata.keywords.remove("GUID");
    fcs.metadata.keywords.remove("$GUID");
    fcs.metadata.validate_guid();
}

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

#[cfg(feature = "flow-fcs")]
#[allow(clippy::too_many_arguments)]
fn build_unmixed_fcs_from_unmixed_abundances(
    stained_fcs: &Fcs,
    unmixed_abundances: &Mat<f64>,
    endmember_names: &[&str],
    autofluorescence_name: &str,
    _endmember_to_detector: &std::collections::HashMap<&str, &str>,
    _primary_detector_names: &[Option<String>],
    _primary_detector_pn_names: &[Option<String>],
    _primary_detector_pn_labels: &[Option<String>],
    selected_marker_names: &[Option<String>],
    selected_fluor_names: &[Option<String>],
    af_channel_name: &str,
    provenance: &UnmixProvenance,
) -> Result<Fcs, TruOlsError> {
    use flow_fcs::keyword::Keyword;
    use polars::prelude::Column;
    use std::sync::Arc;

    // Create a new FCS struct with fresh parameters
    let mut output_fcs = stained_fcs.clone();

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

    for param_name in stained_fcs.get_parameter_names_from_dataframe() {
        if is_scatter_or_time_param(&param_name) {
            scatter_time_params.push(param_name.clone());

            // Get the column data
            if let Ok(values) = stained_fcs.get_parameter_events_slice(&param_name) {
                let column =
                    polars::prelude::Column::new(param_name.clone().into(), values.to_vec());
                scatter_time_columns.push(column);
            }
        }
    }

    // Clear parameters and rebuild with only scatter/time
    output_fcs.parameters.clear();

    // Drop only `$Pn*` / `Pn*` parameter keys (keep `$PROJ`, `$PLATENAME`, `$PAR`, …).
    strip_parameter_index_keywords(&mut output_fcs.metadata.keywords);

    // Re-add scatter/time parameters
    let mut param_num = 1;
    for scatter_param_name in &scatter_time_params {
        if let Some(orig_param) = stained_fcs.parameters.get(scatter_param_name.as_str()) {
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
    let mut abundance_channel_names: Vec<String> = Vec::new();

    // Process fluorophore endmembers (skip autofluorescence)
    for (endmember_idx, &endmember_name) in endmember_names.iter().enumerate() {
        // Skip autofluorescence for now - handle separately at the end
        if endmember_name == autofluorescence_name {
            continue;
        }

        // Extract column from unmixed abundances and convert to f32.
        // Preserve signed values: UCM maps irrelevant endmembers onto the unstained
        // noise distribution (often straddling zero). Clamping to ≥0 collapses that
        // cloud onto the axis and defeats UCM.
        let f64_values: Vec<f64> = (0..n_events)
            .map(|event_idx| unmixed_abundances[(event_idx, endmember_idx)])
            .collect();
        let f32_values: Vec<f32> = abundance_f64_to_f32(&f64_values);

        // Product contract (Select Controls / unmix export):
        //   $PnN = fluor (no Unmixed_ prefix; UNMIXED keyword IDs the file)
        //   $PnS = target / marker
        // Never write hardware detector ids (UV1-A, B10-A, …) into either keyword.
        let fluor = selected_fluor_names
            .get(endmember_idx)
            .and_then(|opt| opt.as_ref())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| short_name_from_endmember_stem(endmember_name));
        let target = selected_marker_names
            .get(endmember_idx)
            .and_then(|opt| opt.as_ref())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_default();

        let unmixed_col_name = fluor;
        let column = Column::new(unmixed_col_name.clone().into(), f32_values.clone());

        // Collect column for DataFrame creation
        result_df_columns.push(column);
        abundance_channel_names.push(unmixed_col_name.clone());

        // Add parameter metadata for this new column
        use flow_fcs::{Parameter, TransformType};
        let param_num = starting_param_num + result_df_columns.len() - scatter_time_count - 1;

        let param = Parameter::new(
            &param_num,
            &unmixed_col_name,
            &target,
            &TransformType::Linear,
        );
        output_fcs
            .parameters
            .insert(unmixed_col_name.clone().into(), param);

        // Add FCS TEXT segment keywords for this parameter
        use flow_fcs::keyword::{IntegerKeyword, MixedKeyword, StringKeyword};

        output_fcs.metadata.keywords.insert(
            format!("$P{}N", param_num),
            Keyword::String(StringKeyword::PnN(Arc::from(unmixed_col_name.clone()))),
        );

        output_fcs.metadata.keywords.insert(
            format!("$P{}S", param_num),
            Keyword::String(StringKeyword::PnS(Arc::from(target))),
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
        let f32_values: Vec<f32> = abundance_f64_to_f32(&f64_values);

        let af_pn = {
            let trimmed = af_channel_name.trim();
            if trimmed.is_empty() {
                DEFAULT_AF_CHANNEL_NAME.to_string()
            } else {
                trimmed.to_string()
            }
        };
        let column = Column::new(af_pn.clone().into(), f32_values);
        result_df_columns.push(column);
        abundance_channel_names.push(af_pn.clone());

        // Add parameter metadata for autofluorescence
        use flow_fcs::{Parameter, TransformType};
        let param_num = starting_param_num + result_df_columns.len() - scatter_time_count - 1;
        let param = Parameter::new(
            &param_num,
            &af_pn,
            "",
            &TransformType::Linear,
        );
        output_fcs.parameters.insert(af_pn.clone().into(), param);

        // Add FCS keywords for autofluorescence
        use flow_fcs::keyword::{IntegerKeyword, MixedKeyword, StringKeyword};
        output_fcs.metadata.keywords.insert(
            format!("$P{}N", param_num),
            Keyword::String(StringKeyword::PnN(Arc::from(af_pn))),
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

    // Strip acquisition spillover (detectors removed) and write identity over abundances.
    strip_acquisition_spillover_keywords(&mut output_fcs.metadata.keywords);
    if !abundance_channel_names.is_empty() {
        use flow_fcs::keyword::Keyword;
        output_fcs.metadata.keywords.insert(
            "$SPILLOVER".to_string(),
            Keyword::Mixed(identity_spillover_keyword(&abundance_channel_names)),
        );
    }

    // Every export path reaches this function, so stamping here - rather than in
    // each caller - is what makes the provenance guarantee hold on all of them.
    // `stamp_onto` also mints a product `$GUID` (the clone above inherited the
    // source's) and moves the file off a space TEXT delimiter, which free-text
    // provenance values would otherwise corrupt.
    provenance.stamp_onto(&mut output_fcs);

    // Unmixed products are emitted as FCS 3.2 - the version that has native
    // keywords for what provenance needs to say ($ORIGINALITY, $UNSTAINEDINFO,
    // $LAST_MODIFIED). This is scoped to derived files on purpose: raw
    // passthrough writes elsewhere in the workspace keep their source version,
    // because re-declaring a vendor file as 3.2 would assert conformance we
    // have not checked.
    output_fcs.header.version = flow_fcs::upgrade::stamp_v3_2(&mut output_fcs.metadata);

    Ok(output_fcs)
}

#[cfg(feature = "flow-fcs")]
enum UnmixMode {
    Fresh,
    Preprocessed {
        cutoffs: Col<f64>,
        nonspecific: Col<f64>,
    },
    #[cfg(feature = "unmix-cache")]
    PreprocessedShared {
        cutoffs: Col<f64>,
        nonspecific: Col<f64>,
        factor_cache: crate::unmixing::SharedMaskFactorCache,
    },
}

#[cfg(feature = "flow-fcs")]
#[allow(clippy::too_many_arguments)]
fn tru_ols_unmix_fcs_impl(
    stained: &Fcs,
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
    af_channel_name: &str,
    unmixed_method: &str,
    mode: UnmixMode,
) -> Result<Fcs, TruOlsError> {
    let endmember_to_detector: std::collections::HashMap<&str, &str> = endmember_names
        .iter()
        .zip(detector_names.iter())
        .map(|(&em, &det)| (em, det))
        .collect();

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

    let autofluorescence_idx = endmember_names
        .iter()
        .position(|&name| name == autofluorescence_name)
        .ok_or_else(|| {
            TruOlsError::InsufficientData(format!(
                "Autofluorescence endmember '{}' not found in endmember names",
                autofluorescence_name
            ))
        })?;

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

    // Snapshot the transform before `mode` consumes the matrix. Everything the
    // caller told us is captured here; `strategy` is overwritten below with the
    // value that actually ran, since `None` means "the constructor default".
    let mut provenance = UnmixProvenance::from_matrix(
        {
            let trimmed = unmixed_method.trim();
            if trimmed.is_empty() {
                UNMIXED_METHOD_TRU_OLS
            } else {
                trimmed
            }
        },
        detector_names.iter().map(|s| s.to_string()).collect(),
        endmember_names.iter().map(|s| s.to_string()).collect(),
        mixing_matrix.as_ref(),
    );
    provenance.af_endmember_index = Some(autofluorescence_idx);
    provenance.raw_datasource_guid = source_guid(stained);
    provenance.unstained_datasource_guid = source_guid(unstained_control);
    provenance.unstained_info = describe_unstained_control(unstained_control, autofluorescence_name);

    let stained_data = extract_detector_data(stained, detector_names)?;
    let unstained_data = extract_detector_data(unstained_control, detector_names)?;

    let mut tru_ols = match mode {
        UnmixMode::Fresh => TruOls::new(mixing_matrix, unstained_data, autofluorescence_idx)?,
        UnmixMode::Preprocessed {
            cutoffs,
            nonspecific,
        } => TruOls::from_preprocessed(
            mixing_matrix,
            unstained_data,
            cutoffs,
            nonspecific,
            autofluorescence_idx,
        )?,
        #[cfg(feature = "unmix-cache")]
        UnmixMode::PreprocessedShared {
            cutoffs,
            nonspecific,
            factor_cache,
        } => TruOls::from_preprocessed_with_factor_cache(
            mixing_matrix,
            unstained_data,
            cutoffs,
            nonspecific,
            autofluorescence_idx,
            factor_cache,
        )?,
    };

    if let Some(s) = strategy {
        tru_ols.set_strategy(s);
    }
    provenance.strategy = Some(tru_ols.strategy());

    let unmixed_abundances = tru_ols.unmix(stained_data.as_ref())?;

    build_unmixed_fcs_from_unmixed_abundances(
        stained,
        &unmixed_abundances,
        endmember_names,
        autofluorescence_name,
        &endmember_to_detector,
        primary_detector_names,
        primary_detector_pn_names,
        primary_detector_pn_labels,
        selected_marker_names,
        selected_fluor_names,
        af_channel_name,
        &provenance,
    )
}

/// Batch path: reuse precomputed cutoffs and nonspecific observation (full panel), with per-file
/// filtered mixing matrix and unstained columns.
#[cfg(feature = "flow-fcs")]
#[allow(clippy::too_many_arguments)]
pub fn apply_tru_ols_unmixing_from_preprocessed(
    stained: &Fcs,
    unstained_control: &Fcs,
    mixing_matrix: Mat<f64>,
    detector_names: &[&str],
    endmember_names: &[&str],
    autofluorescence_name: &str,
    strategy: Option<UnmixingStrategy>,
    cutoffs: Col<f64>,
    nonspecific_observation: Col<f64>,
    primary_detector_names: &[Option<String>],
    primary_detector_pn_names: &[Option<String>],
    primary_detector_pn_labels: &[Option<String>],
    selected_marker_names: &[Option<String>],
    selected_fluor_names: &[Option<String>],
    af_channel_name: &str,
    unmixed_method: &str,
) -> Result<Fcs, TruOlsError> {
    tru_ols_unmix_fcs_impl(
        stained,
        unstained_control,
        mixing_matrix,
        detector_names,
        endmember_names,
        autofluorescence_name,
        strategy,
        primary_detector_names,
        primary_detector_pn_names,
        primary_detector_pn_labels,
        selected_marker_names,
        selected_fluor_names,
        af_channel_name,
        unmixed_method,
        UnmixMode::Preprocessed {
            cutoffs,
            nonspecific: nonspecific_observation,
        },
    )
}

/// Same as [`apply_tru_ols_unmixing_from_preprocessed`], but shares one mask-factor cache across files.
#[cfg(all(feature = "flow-fcs", feature = "unmix-cache"))]
#[allow(clippy::too_many_arguments)]
pub fn apply_tru_ols_unmixing_from_preprocessed_with_shared_factor_cache(
    stained: &Fcs,
    unstained_control: &Fcs,
    mixing_matrix: Mat<f64>,
    detector_names: &[&str],
    endmember_names: &[&str],
    autofluorescence_name: &str,
    strategy: Option<UnmixingStrategy>,
    cutoffs: Col<f64>,
    nonspecific_observation: Col<f64>,
    factor_cache: crate::unmixing::SharedMaskFactorCache,
    primary_detector_names: &[Option<String>],
    primary_detector_pn_names: &[Option<String>],
    primary_detector_pn_labels: &[Option<String>],
    selected_marker_names: &[Option<String>],
    selected_fluor_names: &[Option<String>],
    af_channel_name: &str,
    unmixed_method: &str,
) -> Result<Fcs, TruOlsError> {
    tru_ols_unmix_fcs_impl(
        stained,
        unstained_control,
        mixing_matrix,
        detector_names,
        endmember_names,
        autofluorescence_name,
        strategy,
        primary_detector_names,
        primary_detector_pn_names,
        primary_detector_pn_labels,
        selected_marker_names,
        selected_fluor_names,
        af_channel_name,
        unmixed_method,
        UnmixMode::PreprocessedShared {
            cutoffs,
            nonspecific: nonspecific_observation,
            factor_cache,
        },
    )
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
    /// * `strategy` - Optional strategy for handling irrelevant abundances
    ///   (default: [`UnmixingStrategy::UnstainedControlMapping`])
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
        tru_ols_unmix_fcs_impl(
            self,
            unstained_control,
            mixing_matrix,
            detector_names,
            endmember_names,
            autofluorescence_name,
            strategy,
            primary_detector_names,
            primary_detector_pn_names,
            primary_detector_pn_labels,
            selected_marker_names,
            selected_fluor_names,
            DEFAULT_AF_CHANNEL_NAME,
            UNMIXED_METHOD_TRU_OLS,
            UnmixMode::Fresh,
        )
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
            dataset_start: 0,
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
            col_names.contains(&"Dye1".to_string()),
            "Should have Dye1 abundance column"
        );
        assert!(
            col_names.contains(&"Autofluorescence".to_string()),
            "Should have Autofluorescence column"
        );
        assert!(
            !col_names.iter().any(|c| c.starts_with("Unmixed_")),
            "Should not use Unmixed_ prefix"
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
            dataset_start: 0,
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
            col_names.contains(&"Dye1".to_string()),
            "Output should contain Dye1"
        );
        assert!(
            col_names.contains(&"Autofluorescence".to_string()),
            "Output should contain Autofluorescence"
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
        // Expected: FSC-A, SSC-A, Time (3 scatter/time) + Dye + Autofluorescence = 5 total
        assert_eq!(
            col_names.len(),
            5,
            "Output should have 5 parameters (3 scatter/time + 2 unmixed), got: {:?}",
            col_names
        );
    }

    #[test]
    fn test_unmixed_export_roundtrip_readable_without_acquisition_detectors() {
        // Write → reopen → read abundance columns. Regresses the plot path that
        // used to fetch UV1-A / FL1-A from reduced-channel files.
        use flow_fcs::write_fcs_file;
        use std::fs::File;
        use std::io::Write;

        let src_path = std::env::temp_dir().join("test_tru_ols_roundtrip_src.tmp");
        {
            let mut f = File::create(&src_path).expect("create src stub");
            f.write_all(b"test").expect("write stub");
        }

        let mut columns = Vec::new();
        columns.push(Column::new(
            "FSC-A".into(),
            vec![100.0f32, 200.0, 300.0, 400.0, 500.0],
        ));
        columns.push(Column::new(
            "SSC-A".into(),
            vec![50.0f32, 100.0, 150.0, 200.0, 250.0],
        ));
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
        let df = DataFrame::new_infer_height(columns).expect("df");

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
            "FL1-A".into(),
            Parameter::new(&3, "FL1-A", "BUV615", &TransformType::Linear),
        );
        params.insert(
            "FL2-A".into(),
            Parameter::new(&4, "FL2-A", "LD", &TransformType::Linear),
        );
        params.insert(
            "FL3-A".into(),
            Parameter::new(&5, "FL3-A", "AF", &TransformType::Linear),
        );

        let mut stained_fcs = Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(src_path.to_str().unwrap_or(""))
                .expect("AccessWrapper"),
            dataset_start: 0,
        };
        // Space delimiter corrupts keywords whose values contain spaces ($PROJ, …).
        stained_fcs.metadata.delimiter = '\u{000c}';
        // Seed sample metadata that must survive export (regression: `$P` strip dropped these).
        {
            use flow_fcs::keyword::{Keyword, MixedKeyword, StringKeyword};
            stained_fcs.metadata.keywords.insert(
                "$PROJ".into(),
                Keyword::String(StringKeyword::Other(Arc::from("Baseline Phenotyping"))),
            );
            stained_fcs.metadata.keywords.insert(
                "$PLATENAME".into(),
                Keyword::String(StringKeyword::Other(Arc::from("Plate_001"))),
            );
            // FCS 3.2 requires `$CYT`; the product inherits it from the source.
            stained_fcs.metadata.keywords.insert(
                "$CYT".into(),
                Keyword::String(StringKeyword::Other(Arc::from("Aurora 5L"))),
            );
            // 3.1 spellings the 3.2 stamp has to migrate.
            stained_fcs.metadata.keywords.insert(
                "$DATE".into(),
                Keyword::String(StringKeyword::Other(Arc::from("01-JAN-2024"))),
            );
            stained_fcs.metadata.keywords.insert(
                "$BTIM".into(),
                Keyword::String(StringKeyword::Other(Arc::from("14:30:00"))),
            );
            stained_fcs.metadata.keywords.insert(
                "$WELLID".into(),
                Keyword::String(StringKeyword::Other(Arc::from("B04"))),
            );
            stained_fcs.metadata.keywords.insert(
                "TUBENAME".into(),
                Keyword::String(StringKeyword::Other(Arc::from("Full Stain"))),
            );
            // Orphan Cytek display key for a removed detector — must not round-trip.
            stained_fcs.metadata.keywords.insert(
                "P71DISPLAY".into(),
                Keyword::String(StringKeyword::Other(Arc::from("LOG"))),
            );
            stained_fcs.metadata.keywords.insert(
                "$GUID".into(),
                Keyword::String(StringKeyword::Other(Arc::from(
                    "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
                ))),
            );
            stained_fcs.metadata.keywords.insert(
                "$SPILLOVER".into(),
                Keyword::Mixed(MixedKeyword::SPILLOVER {
                    n_parameters: 2,
                    parameter_names: vec!["FL1-A".into(), "FL2-A".into()],
                    matrix_values: vec![1.0, 0.1, 0.1, 1.0],
                }),
            );
        }
        let mut unstained_fcs = stained_fcs.clone();
        // Distinct identity so provenance can't pass by accidentally recording the
        // stained GUID in both source slots.
        unstained_fcs.metadata.keywords.insert(
            "$GUID".into(),
            flow_fcs::keyword::Keyword::String(flow_fcs::keyword::StringKeyword::Other(
                Arc::from("11111111-2222-3333-4444-555555555555"),
            )),
        );

        let mixing_matrix = mat![[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]];
        let detector_names = &["FL1-A", "FL2-A", "FL3-A"];
        let endmember_names = &["BUV615", "Autofluorescence"];
        let empty_pn: Vec<Option<String>> = vec![None; endmember_names.len()];
        let fluor_names = vec![Some("BUV615".into()), None];
        let target_names = vec![Some("CD19".into()), None];

        let unmixed = stained_fcs
            .apply_tru_ols_unmixing(
                &unstained_fcs,
                mixing_matrix,
                detector_names,
                endmember_names,
                "Autofluorescence",
                None,
                &empty_pn,
                &empty_pn,
                &empty_pn,
                &target_names,
                &fluor_names,
            )
            .expect("unmix");

        // Deliberately no manual `set_raw_datasource_guid` / `mint_unmixed_file_guid`
        // here: the point of this assertion is that the *trait* path stamps its own
        // provenance. It used to be the caller's job, which is how the trait path
        // came to emit files that inherited the raw file's `$GUID`.
        assert!(
            crate::provenance::UnmixProvenance::read_from(&unmixed).is_some(),
            "trait path must stamp provenance without caller help; keys={:?}",
            unmixed.metadata.keywords.keys().collect::<Vec<_>>()
        );

        assert!(
            unmixed.metadata.keywords.contains_key(UNMIXED_KEYWORD)
                || unmixed.metadata.keywords.contains_key(UNMIXED_KEYWORD_BARE),
            "UNMIXED must be set before write; keys={:?}",
            unmixed.metadata.keywords.keys().collect::<Vec<_>>()
        );

        let out_path = std::env::temp_dir().join("test_tru_ols_roundtrip_out.fcs");
        write_fcs_file(unmixed, &out_path).expect("write unmixed FCS");

        let reopened = Fcs::open(out_path.to_str().expect("utf8 path")).expect("reopen unmixed");

        let method = reopened
            .get_keyword_string_value(UNMIXED_KEYWORD)
            .or_else(|_| reopened.get_keyword_string_value(UNMIXED_KEYWORD_BARE))
            .unwrap_or_else(|e| {
                panic!(
                    "UNMIXED keyword missing after reopen ({e}); keys={:?}",
                    reopened.metadata.keywords.keys().collect::<Vec<_>>()
                )
            });
        assert_eq!(method.as_ref(), UNMIXED_METHOD_TRU_OLS);

        // Sample keywords carried from raw (write always `$`-prefixes).
        let proj = reopened
            .get_keyword_string_value("$PROJ")
            .expect("$PROJ carried");
        assert_eq!(proj.as_ref(), "Baseline Phenotyping");
        let plate = reopened
            .get_keyword_string_value("$PLATENAME")
            .expect("$PLATENAME carried");
        assert_eq!(plate.as_ref(), "Plate_001");
        let tube = reopened
            .get_keyword_string_value("$TUBENAME")
            .or_else(|_| reopened.get_keyword_string_value("TUBENAME"))
            .expect("TUBENAME carried");
        assert_eq!(tube.as_ref(), "Full Stain");
        assert!(
            reopened.metadata.keywords.get("$P71DISPLAY").is_none()
                && reopened.metadata.keywords.get("P71DISPLAY").is_none(),
            "orphaned PnDISPLAY must be stripped"
        );

        let spill = reopened
            .get_spillover_matrix()
            .expect("spillover parse")
            .expect("identity $SPILLOVER present");
        assert_eq!(spill.1.len(), 2, "abundance channels in spillover: {:?}", spill.1);
        assert!((spill.0[(0, 0)] - 1.0).abs() < 1e-5);
        assert!(spill.0[(0, 1)].abs() < 1e-5);

        // The product declares FCS 3.2 and actually speaks it: the required
        // `$CYT` is present, the 3.1 spellings have been migrated, and the
        // originals are still there for a 3.1 reader.
        assert_eq!(
            reopened.header.version,
            flow_fcs::Version::V3_2,
            "unmixed products are emitted as 3.2"
        );
        assert_eq!(
            reopened
                .get_keyword_string_value("$CYT")
                .expect("$CYT required by 3.2")
                .as_ref(),
            "Aurora 5L"
        );
        assert_eq!(
            reopened
                .get_keyword_string_value(crate::provenance::ORIGINALITY_KEYWORD)
                .expect("$ORIGINALITY")
                .as_ref(),
            crate::provenance::ORIGINALITY_DATA_MODIFIED
        );
        assert_eq!(
            reopened
                .get_keyword_string_value("$BEGINDATETIME")
                .expect("$DATE + $BTIM migrated")
                .as_ref(),
            "2024-01-01T14:30:00"
        );
        assert_eq!(
            reopened
                .get_keyword_string_value("$LOCATIONID")
                .expect("$WELLID migrated")
                .as_ref(),
            "B04"
        );
        assert_eq!(
            reopened
                .get_keyword_string_value("$CARRIERTYPE")
                .expect("$PLATENAME migrated")
                .as_ref(),
            "Plate_001"
        );
        assert!(
            reopened.get_keyword_string_value("$DATE").is_ok(),
            "deprecated originals stay for 3.1 readers"
        );

        // Full provenance survives the write/reopen boundary, not just the two GUIDs.
        let recovered = crate::provenance::UnmixProvenance::read_from(&reopened)
            .expect("provenance recovered after reopen");
        assert_eq!(recovered.method, UNMIXED_METHOD_TRU_OLS);
        assert_eq!(recovered.detector_names, detector_names);
        assert_eq!(recovered.endmember_names, endmember_names);
        assert_eq!(
            recovered.raw_datasource_guid.as_deref(),
            Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        );
        assert_eq!(
            recovered.unstained_datasource_guid.as_deref(),
            Some("11111111-2222-3333-4444-555555555555")
        );
        assert_eq!(
            recovered.af_endmember_index,
            Some(1),
            "Autofluorescence is endmember 1"
        );
        assert_eq!(
            recovered.strategy,
            Some(UnmixingStrategy::UnstainedControlMapping),
            "the effective strategy, not the caller's `None`"
        );
        assert!(
            recovered
                .unstained_info
                .as_deref()
                .is_some_and(|s| s.contains("Autofluorescence")),
            "unstained info describes the AF endmember: {:?}",
            recovered.unstained_info
        );
        // f32 storage of an f64 source matrix, so compare with tolerance rather than
        // by equality; the ASCII round-trip itself is exact (`f32::to_string` is
        // shortest-round-trip).
        let expected_row_major = [0.9_f64, 0.1, 0.1, 0.9, 0.05, 0.05];
        assert_eq!(recovered.mixing_matrix.len(), expected_row_major.len());
        for (got, want) in recovered.mixing_matrix.iter().zip(expected_row_major) {
            assert!(
                (f64::from(*got) - want).abs() < 1e-6,
                "matrix {:?} != {expected_row_major:?}",
                recovered.mixing_matrix
            );
        }

        let product_guid = reopened
            .metadata
            .keywords
            .get("$GUID")
            .or_else(|| reopened.metadata.keywords.get("GUID"))
            .expect("product GUID");
        let product_guid_str = match product_guid {
            flow_fcs::keyword::Keyword::String(s) => {
                use flow_fcs::keyword::StringableKeyword;
                s.get_str().to_string()
            }
            _ => panic!("GUID not string"),
        };
        assert_ne!(
            product_guid_str, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "unmixed product must mint a new GUID"
        );
        assert_ne!(
            product_guid_str, "11111111-2222-3333-4444-555555555555",
            "…and it must not be the unstained control's either"
        );

        let names: Vec<String> = reopened
            .get_parameter_names_from_dataframe()
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(names.contains(&"BUV615".to_string()), "abundance $PnN present");
        assert!(
            names.contains(&"Autofluorescence".to_string()),
            "AF abundance present"
        );
        assert!(
            !names.iter().any(|n| n == "FL1-A" || n == "FL2-A" || n == "UV1-A"),
            "acquisition detectors must be absent: {names:?}"
        );

        // Plot path equivalent: read only selected abundance axes (no matrix expand).
        let buv = reopened
            .data_frame
            .column("BUV615")
            .expect("read BUV615 for plot");
        assert_eq!(buv.len(), 5);
        assert!(
            reopened.data_frame.column("FL1-A").is_err(),
            "FL1-A must not be fetchable on unmixed product"
        );

        let _ = std::fs::remove_file(&src_path);
        let _ = std::fs::remove_file(&out_path);
    }

    #[test]
    fn parameter_index_keyword_detection() {
        assert!(is_parameter_index_keyword("$P1N"));
        assert!(is_parameter_index_keyword("$P12S"));
        assert!(is_parameter_index_keyword("$P10DISPLAY"));
        assert!(is_parameter_index_keyword("P71DISPLAY"));
        assert!(!is_parameter_index_keyword("$PAR"));
        assert!(!is_parameter_index_keyword("$PROJ"));
        assert!(!is_parameter_index_keyword("$PLATENAME"));
        assert!(!is_parameter_index_keyword("$PLATECOLS"));
        assert!(!is_parameter_index_keyword("$PK1N"));
        assert!(!is_parameter_index_keyword("$SPILLOVER"));
        assert!(!is_parameter_index_keyword("TUBENAME"));
    }

    #[test]
    fn abundance_export_preserves_signed_ucm_values() {
        // REGRESSION: clamping to ≥0 piled UCM noise on the plot axes.
        let signed = abundance_f64_to_f32(&[-12.5, -0.001, 0.0, 3.25]);
        assert_eq!(signed[0], -12.5);
        assert_eq!(signed[1], -0.001);
        assert_eq!(signed[2], 0.0);
        assert_eq!(signed[3], 3.25);
    }
}
