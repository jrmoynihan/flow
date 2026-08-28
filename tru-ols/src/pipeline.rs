//! Shared helpers for assembling / applying TRU-OLS outside a specific UI shell.

use crate::TruOlsError;
use faer::Mat;
use std::path::{Path, PathBuf};

/// Resolve an AF endmember index/name, appending a unit `AF` column when missing.
///
/// `flat_matrix` is row-major detectors × endmembers and is extended in place when AF is appended.
pub fn resolve_or_append_af_endmember(
    endmember_names: &mut Vec<String>,
    flat_matrix: &mut Vec<f64>,
    n_detectors: usize,
    af_endmember_index: Option<usize>,
) -> Result<(usize, String), TruOlsError> {
    if let Some(i) = af_endmember_index {
        if i >= endmember_names.len() {
            return Err(TruOlsError::InsufficientData(format!(
                "af_endmember_index {i} out of range for {} endmembers",
                endmember_names.len()
            )));
        }
        return Ok((i, endmember_names[i].clone()));
    }
    if let Some(i) = endmember_names.iter().position(|n| {
        let lower = n.to_ascii_lowercase();
        lower == "af" || lower.contains("autofluor")
    }) {
        return Ok((i, endmember_names[i].clone()));
    }
    let af_name = "AF".to_string();
    for _ in 0..n_detectors {
        flat_matrix.push(1.0);
    }
    endmember_names.push(af_name.clone());
    Ok((endmember_names.len() - 1, af_name))
}

/// Build a detectors × endmembers matrix from a row-major flat buffer.
pub fn matrix_from_row_major_flat(
    flat: &[f64],
    n_detectors: usize,
    n_endmembers: usize,
) -> Result<Mat<f64>, TruOlsError> {
    if flat.len() != n_detectors * n_endmembers {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors * n_endmembers,
            actual: flat.len(),
        });
    }
    Ok(Mat::from_fn(n_detectors, n_endmembers, |i, j| {
        flat[i * n_endmembers + j]
    }))
}

/// Display-oriented output path: `{stem}_unmixed.fcs` under `out_dir` (not `$FIL` prefix).
pub fn unmixed_output_path(src: &Path, out_dir: &Path) -> PathBuf {
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("sample");
    out_dir.join(format!("{stem}_unmixed.fcs"))
}

#[cfg(feature = "flow-fcs")]
pub use fcs_export::{
    PathBExportRequest, UnmixExportRequest, UnmixExportResult, FitMetricsSummary,
    RAW_DATASOURCE_GUID_KEYWORD, export_unmixed_fcs, export_unmixed_fcs_path_b,
    set_raw_datasource_guid,
};

#[cfg(feature = "flow-fcs")]
mod fcs_export {
    use super::{matrix_from_row_major_flat, resolve_or_append_af_endmember, unmixed_output_path};
    use crate::TruOlsError;
    use crate::af_match::{
        AfMatchLibrary, AfMatchOptions, AfMatchResult, PreparedAfLibrary, build_af_match_library,
        match_and_subtract,
    };
    use crate::benchmark::run_ols;
    use crate::fcs_integration::{
        AF_ASSIGNMENT_KEYWORD, AF_CONFIDENCE_KEYWORD, DEFAULT_AF_CHANNEL_NAME,
        UNMIXED_METHOD_OLS_MULTI_AF, UNMIXED_METHOD_TRU_OLS, UNMIXED_METHOD_TRU_OLS_MULTI_AF,
        apply_tru_ols_unmixing_from_preprocessed, build_unmixed_fcs_from_unmixed_abundances,
        extract_detector_data,
    };
    use crate::metrics::{FitMetrics, compute_fit_metrics};
    use crate::preprocessing::{CutoffCalculator, NonspecificObservation};
    use crate::provenance::{UnmixFitProvenance, UnmixProvenance};
    use crate::unmixing::{TruOls, UnmixingStrategy};
    use faer::{Mat, MatRef};
    use flow_fcs::Fcs;
    use flow_fcs::keyword::{Keyword, StringKeyword};
    use flow_fcs::write_fcs_file;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// Provenance keyword written onto exported unmixed FCS files.
    ///
    /// Re-exported from [`crate::provenance`], which owns the full keyword set.
    /// Previously spelled without the `$`; the writer prefixes every key, so
    /// files on disk always carried the `$` form and readers tolerate both.
    pub use crate::provenance::RAW_DATASOURCE_GUID_KEYWORD;

    /// Compact fit metrics suitable for IPC / CLI summaries.
    #[derive(Debug, Clone)]
    pub struct FitMetricsSummary {
        pub r_squared_mean: f64,
        pub r_squared_median: f64,
        pub residual_abs_mean: f64,
        pub residual_abs_median: f64,
        pub residual_abs_max: f64,
    }

    impl From<&FitMetrics> for FitMetricsSummary {
        fn from(fit: &FitMetrics) -> Self {
            Self {
                r_squared_mean: fit.r_squared_mean,
                r_squared_median: fit.r_squared_median,
                residual_abs_mean: fit.residual_abs_mean,
                residual_abs_median: fit.residual_abs_median,
                residual_abs_max: fit.residual_abs_max,
            }
        }
    }

    /// Inputs for exporting one or more reduced-channel unmixed FCS files.
    pub struct UnmixExportRequest<'a> {
        pub stained: &'a Fcs,
        pub unstained: &'a Fcs,
        pub detector_names: &'a [String],
        pub endmember_names: Vec<String>,
        pub flat_matrix: Vec<f64>,
        pub af_endmember_index: Option<usize>,
        pub cutoff_percentile: f64,
        pub strategy: UnmixingStrategy,
        pub output_dir: &'a Path,
        pub raw_datasource_guid: &'a str,
        /// When true, also compute fit metrics for this stained file.
        pub compute_fit: bool,
        /// Parallel to input `endmember_names` (before AF append): fluor → `$PnN`.
        pub endmember_fluor_names: Vec<Option<String>>,
        /// Parallel to input `endmember_names` (before AF append): target/marker → `$PnS`.
        pub endmember_target_names: Vec<Option<String>>,
        /// AF abundance `$PnN` (default `Autofluorescence`).
        pub af_channel_name: String,
    }

    #[derive(Debug, Clone)]
    pub struct UnmixExportResult {
        pub output_path: PathBuf,
        pub endmember_names: Vec<String>,
        pub fit_metrics: Option<FitMetricsSummary>,
    }

    /// Sets `$RAW_DATASOURCE_GUID` in isolation.
    ///
    /// Retained for callers that stamp a source pointer onto a file they built
    /// themselves. The export paths no longer need it: provenance is written as
    /// one record by [`crate::provenance::UnmixProvenance::stamp_onto`], which
    /// is reached from the single builder both paths share.
    pub fn set_raw_datasource_guid(fcs: &mut Fcs, guid: &str) {
        fcs.metadata.keywords.insert(
            RAW_DATASOURCE_GUID_KEYWORD.into(),
            Keyword::String(StringKeyword::Other(Arc::from(guid))),
        );
    }

    /// Unmix one stained file against a shared unstained control and write reduced-channel FCS.
    pub fn export_unmixed_fcs(req: UnmixExportRequest<'_>) -> Result<UnmixExportResult, TruOlsError> {
        let n_det = req.detector_names.len();
        if n_det == 0 {
            return Err(TruOlsError::InsufficientData(
                "detector_names must be non-empty".into(),
            ));
        }
        let mut endmember_names = req.endmember_names;
        if endmember_names.is_empty() {
            return Err(TruOlsError::InsufficientData(
                "endmember_names must be non-empty".into(),
            ));
        }
        let mut flat_matrix = req.flat_matrix;
        let (af_idx, af_name) = resolve_or_append_af_endmember(
            &mut endmember_names,
            &mut flat_matrix,
            n_det,
            req.af_endmember_index,
        )?;
        let n_em = endmember_names.len();
        let matrix = matrix_from_row_major_flat(&flat_matrix, n_det, n_em)?;

        let det_refs: Vec<&str> = req.detector_names.iter().map(|s| s.as_str()).collect();
        let em_refs: Vec<&str> = endmember_names.iter().map(|s| s.as_str()).collect();
        let primary_opts: Vec<Option<String>> = vec![None; n_em];

        // Pad label arrays to final endmember count (AF may have been appended).
        let mut fluor_names = req.endmember_fluor_names;
        let mut target_names = req.endmember_target_names;
        fluor_names.resize(n_em, None);
        target_names.resize(n_em, None);

        let unstained_data = extract_detector_data(req.unstained, &det_refs)?;
        let cutoff = if req.cutoff_percentile.is_finite() && req.cutoff_percentile > 0.0 {
            req.cutoff_percentile
        } else {
            0.995
        };
        let cutoffs =
            CutoffCalculator::calculate(matrix.as_ref(), unstained_data.as_ref(), cutoff)?;
        let nonspecific = NonspecificObservation::calculate(
            matrix.as_ref(),
            unstained_data.as_ref(),
            af_idx,
        )?;

        let fit_metrics = if req.compute_fit {
            let stained_data = extract_detector_data(req.stained, &det_refs)?;
            let mut tru_ols = TruOls::from_preprocessed(
                matrix.clone(),
                unstained_data.clone(),
                cutoffs.cutoffs().clone(),
                nonspecific.observation().clone(),
                af_idx,
            )?;
            tru_ols.set_strategy(req.strategy);
            let abundances = tru_ols.unmix(stained_data.as_ref())?;
            let fit = compute_fit_metrics(stained_data.as_ref(), abundances.as_ref(), matrix.as_ref());
            Some(FitMetricsSummary::from(&fit))
        } else {
            None
        };

        let af_pn = {
            let trimmed = req.af_channel_name.trim();
            if trimmed.is_empty() {
                DEFAULT_AF_CHANNEL_NAME.to_string()
            } else {
                trimmed.to_string()
            }
        };

        let mut unmixed = apply_tru_ols_unmixing_from_preprocessed(
            req.stained,
            req.unstained,
            matrix,
            &det_refs,
            &em_refs,
            &af_name,
            Some(req.strategy),
            cutoffs.cutoffs().clone(),
            nonspecific.observation().clone(),
            &primary_opts,
            &[],
            &[],
            // selected_marker_names → $PnS (target)
            &target_names,
            // selected_fluor_names → $PnN (fluor)
            &fluor_names,
            &af_pn,
            UNMIXED_METHOD_TRU_OLS,
        )?;
        // The builder already stamped the transform, both source GUIDs, the
        // strategy and a fresh product identity. Enrich that record with the two
        // things only this path knows - the cutoff percentile it computed and
        // the fit metrics it was asked for - plus a caller-supplied source
        // pointer, which may name something other than the stained file's own
        // `$GUID` (a LIMS id, say).
        //
        // `write_to` rather than `stamp_onto`: the identity minted above must
        // not churn.
        match crate::provenance::UnmixProvenance::read_from(&unmixed) {
            Some(mut provenance) => {
                provenance.cutoff_percentile = Some(cutoff);
                provenance.fit = fit_metrics.as_ref().map(|f| UnmixFitProvenance {
                    r_squared_mean: f.r_squared_mean,
                    r_squared_median: f.r_squared_median,
                    residual_abs_mean: f.residual_abs_mean,
                    residual_abs_median: f.residual_abs_median,
                    residual_abs_max: f.residual_abs_max,
                });
                if !req.raw_datasource_guid.trim().is_empty() {
                    provenance.raw_datasource_guid = Some(req.raw_datasource_guid.to_string());
                }
                provenance.write_to(&mut unmixed);
            }
            // Unreachable via the builder, but a silent skip here would drop the
            // caller's source pointer without trace, so fall back to the shim.
            None => {
                tracing::warn!(
                    "unmixed file carries no provenance record; writing the source GUID alone"
                );
                set_raw_datasource_guid(&mut unmixed, req.raw_datasource_guid);
                crate::fcs_integration::mint_unmixed_file_guid(&mut unmixed);
            }
        }

        let out_path = unmixed_output_path(&req.stained.file_access.path, req.output_dir);
        write_fcs_file(unmixed, &out_path).map_err(|e| {
            TruOlsError::InsufficientData(format!("write {}: {e}", out_path.display()))
        })?;

        Ok(UnmixExportResult {
            output_path: out_path,
            endmember_names,
            fit_metrics,
        })
    }

    /// Inputs for a **Path B** export: autofluorescence is matched per event against a
    /// catalog instead of arriving as a single mixing-matrix column from the caller.
    ///
    /// The shape difference from [`UnmixExportRequest`] is deliberate and load-bearing:
    /// `endmember_names` and `flat_matrix` here are **fluorophore-only**. OLS Path B
    /// subtracts matched AF then solves fluors; TRU Path B appends the argmax catalog
    /// signature as a protected AF column for the TRU-OLS solve.
    pub struct PathBExportRequest<'a> {
        pub stained: &'a Fcs,
        pub unstained: &'a Fcs,
        pub detector_names: &'a [String],
        /// Fluorophore endmembers only — an AF-looking name here is rejected.
        pub endmember_names: Vec<String>,
        /// Row-major detectors × fluorophore endmembers. No AF column.
        pub flat_matrix: Vec<f64>,
        /// AF signatures with their FSC/SSC morphology, expressed over `detector_names`.
        pub af_library: AfMatchLibrary,
        /// `$PnN` of the forward-scatter channel feeding the morphological prior.
        pub fsc_channel: &'a str,
        /// `$PnN` of the side-scatter channel feeding the morphological prior.
        pub ssc_channel: &'a str,
        /// `true` → TRU-OLS with each event's argmax catalog signature as a protected AF
        /// column; `false` → plain OLS ([`run_ols`]) on the AF-subtracted residual.
        pub truncate: bool,
        pub cutoff_percentile: f64,
        pub strategy: UnmixingStrategy,
        pub output_dir: &'a Path,
        pub raw_datasource_guid: &'a str,
        pub compute_fit: bool,
        /// Parallel to `endmember_names`: fluor → `$PnN`.
        pub endmember_fluor_names: Vec<Option<String>>,
        /// Parallel to `endmember_names`: target/marker → `$PnS`.
        pub endmember_target_names: Vec<Option<String>>,
        /// `$PnN` of the single blended AF abundance channel (default `Autofluorescence`).
        pub af_channel_name: String,
        /// Overrides for AF-catalog preparation; defaults are usually right.
        pub af_match_options: AfMatchOptions,
        /// Unstained control `$FIL` stamped as `$AF_SOURCE` (empty → omit).
        pub af_source_fil: Option<String>,
        /// Assigned AF profile `source_file_guid` stamped as `$AF_SOURCE_GUID`.
        pub af_source_guid: Option<String>,
    }

    /// Unmix one stained file with per-event AF catalog matching (Path B).
    ///
    /// 1. Extract detectors plus FSC/SSC from the stained file and detectors from the control.
    /// 2. Compute a **fluorophore-only** nonspecific observation from the control (matcher baseline).
    /// 3. Match each event against the AF catalog (assignment + blended `k*` abundance).
    /// 4. Solve fluors:
    ///    - OLS (`truncate == false`): subtract soft-blend AF, then fluor-only OLS on the residual.
    ///    - TRU (`truncate == true`): append the argmax signature as a protected AF column and
    ///      run TRU-OLS on the **original** observations (not the AF-subtracted residual).
    /// 5. Write fluorophore abundances plus one matcher `Autofluorescence` channel.
    pub fn export_unmixed_fcs_path_b(
        req: PathBExportRequest<'_>,
    ) -> Result<UnmixExportResult, TruOlsError> {
        let n_det = req.detector_names.len();
        if n_det == 0 {
            return Err(TruOlsError::InsufficientData(
                "detector_names must be non-empty".into(),
            ));
        }
        let fluor_names = req.endmember_names;
        if fluor_names.is_empty() {
            return Err(TruOlsError::InsufficientData(
                "endmember_names must be non-empty".into(),
            ));
        }
        // An AF column in the *fluorophore* matrix would be wrong for OLS Path B
        // (soft-blend subtract then fluor-only solve). TRU Path B appends the catalog
        // signature itself — never a caller-supplied AF endmember name here.
        if let Some(offender) = fluor_names.iter().find(|n| looks_like_autofluorescence(n)) {
            return Err(TruOlsError::InsufficientData(format!(
                "Path B endmembers must be fluorophore-only, but '{offender}' names an \
                 autofluorescence endmember. Autofluorescence comes from `af_library`; \
                 drop that column or use `export_unmixed_fcs` (Path A) instead."
            )));
        }
        if req.fsc_channel.trim().is_empty() || req.ssc_channel.trim().is_empty() {
            return Err(TruOlsError::InsufficientData(
                "Path B needs both an FSC and an SSC channel for the morphological prior".into(),
            ));
        }

        let n_fluor = fluor_names.len();
        let fluor_matrix = matrix_from_row_major_flat(&req.flat_matrix, n_det, n_fluor)?;

        let det_refs: Vec<&str> = req.detector_names.iter().map(|s| s.as_str()).collect();
        let stained_data = extract_detector_data(req.stained, &det_refs)?;
        let unstained_data = extract_detector_data(req.unstained, &det_refs)?;
        let fsc_ssc = extract_detector_data(req.stained, &[req.fsc_channel, req.ssc_channel])?;

        let cutoff = if req.cutoff_percentile.is_finite() && req.cutoff_percentile > 0.0 {
            req.cutoff_percentile
        } else {
            0.995
        };

        // Fluorophore-only: no AF index to exclude, so every dye contributes to the
        // background estimate.
        let nonspecific = NonspecificObservation::calculate_fluor_only(
            fluor_matrix.as_ref(),
            unstained_data.as_ref(),
        )?;
        let baseline: Vec<f64> = (0..n_det)
            .map(|i| nonspecific.observation()[i])
            .collect();

        let prepared = build_af_match_library(
            req.af_library,
            fluor_matrix.as_ref(),
            req.af_match_options,
        )?;
        let af = match_and_subtract(
            stained_data.as_ref(),
            fsc_ssc.as_ref(),
            Some(&baseline),
            &prepared,
        )?;

        // Match still runs (assignment + exported Autofluorescence = matcher k*). When
        // truncate is on, TRU-OLS solves on original observations with the argmax catalog
        // signature as a protected AF column — never subtract-then-fluor-only TRU.
        let fluor_abundances = if req.truncate {
            path_b_tru_with_protected_af(
                fluor_matrix.as_ref(),
                stained_data.as_ref(),
                unstained_data.as_ref(),
                &prepared,
                &af.assignment_1based,
                cutoff,
                req.strategy,
            )?
        } else {
            run_ols(af.residual_observations.as_ref(), fluor_matrix.as_ref())?
        };

        let fit_metrics = if req.compute_fit {
            let fit = compute_fit_metrics(
                af.residual_observations.as_ref(),
                fluor_abundances.as_ref(),
                fluor_matrix.as_ref(),
            );
            Some(FitMetricsSummary::from(&fit))
        } else {
            None
        };

        let af_pn = {
            let trimmed = req.af_channel_name.trim();
            if trimmed.is_empty() {
                DEFAULT_AF_CHANNEL_NAME.to_string()
            } else {
                trimmed.to_string()
            }
        };

        // The builder emits one channel per name, so AF joins the list here only.
        let mut export_names = fluor_names.clone();
        export_names.push(af_pn.clone());
        let n_events = fluor_abundances.nrows();
        let abundances = Mat::from_fn(n_events, n_fluor + 1, |ev, col| {
            if col < n_fluor {
                fluor_abundances[(ev, col)]
            } else {
                af.af_abundance[ev]
            }
        });

        let method = if req.truncate {
            UNMIXED_METHOD_TRU_OLS_MULTI_AF
        } else {
            UNMIXED_METHOD_OLS_MULTI_AF
        };

        // Recorded transform stays fluorophore-only (no single AF spectrum). When TRU ran,
        // `af_endmember_index = n_fluor` records that a protected catalog-signature column
        // sat at that index during the solve; `$UNMIXED` / `$UNSTAINEDINFO` explain the
        // exported Autofluorescence channel.
        let mut provenance = UnmixProvenance::from_matrix(
            method,
            req.detector_names.to_vec(),
            fluor_names.clone(),
            fluor_matrix.as_ref(),
        );
        provenance.strategy = req.truncate.then_some(req.strategy);
        provenance.cutoff_percentile = req.truncate.then_some(cutoff);
        provenance.af_endmember_index = req.truncate.then_some(n_fluor);
        provenance.raw_datasource_guid =
            crate::fcs_integration::source_guid(req.stained);
        provenance.unstained_datasource_guid =
            crate::fcs_integration::source_guid(req.unstained);
        provenance.unstained_info = Some(format!(
            "Autofluorescence channel '{af_pn}' assigned per event from a {}-entry AF catalog \
             (Path B multi-AF matching over {}/{}); fluorophore background from the unstained control",
            prepared.len(),
            req.fsc_channel,
            req.ssc_channel
        ));
        provenance.fit = fit_metrics.as_ref().map(|f| UnmixFitProvenance {
            r_squared_mean: f.r_squared_mean,
            r_squared_median: f.r_squared_median,
            residual_abs_mean: f.residual_abs_mean,
            residual_abs_median: f.residual_abs_median,
            residual_abs_max: f.residual_abs_max,
        });
        if !req.raw_datasource_guid.trim().is_empty() {
            provenance.raw_datasource_guid = Some(req.raw_datasource_guid.to_string());
        }

        let mut fluor_pn_names = req.endmember_fluor_names;
        let mut target_names = req.endmember_target_names;
        fluor_pn_names.resize(export_names.len(), None);
        target_names.resize(export_names.len(), None);

        let export_refs: Vec<&str> = export_names.iter().map(|s| s.as_str()).collect();
        let mut unmixed = build_unmixed_fcs_from_unmixed_abundances(
            req.stained,
            &abundances,
            &export_refs,
            &af_pn,
            &std::collections::HashMap::new(),
            &vec![None; export_names.len()],
            &[],
            &[],
            &target_names,
            &fluor_pn_names,
            &af_pn,
            &provenance,
        )?;

        set_af_match_keywords(&mut unmixed, &af, &prepared);
        append_af_match_annotation_channels(&mut unmixed, &af)?;
        set_af_source_keywords(
            &mut unmixed,
            req.af_source_fil.as_deref(),
            req.af_source_guid.as_deref(),
        );

        let out_path = unmixed_output_path(&req.stained.file_access.path, req.output_dir);
        write_fcs_file(unmixed, &out_path).map_err(|e| {
            TruOlsError::InsufficientData(format!("write {}: {e}", out_path.display()))
        })?;

        Ok(UnmixExportResult {
            output_path: out_path,
            endmember_names: export_names,
            fit_metrics,
        })
    }

    /// Matches [`resolve_or_append_af_endmember`]'s notion of an AF endmember name.
    fn looks_like_autofluorescence(name: &str) -> bool {
        let lower = name.trim().to_ascii_lowercase();
        lower == "af" || lower.contains("autofluor")
    }

    /// Append one AF signature column to a fluorophore-only mixing matrix.
    fn hstack_fluor_and_signature(
        fluor_matrix: MatRef<'_, f64>,
        signature: &[f64],
    ) -> Result<Mat<f64>, TruOlsError> {
        let n_det = fluor_matrix.nrows();
        let n_fluor = fluor_matrix.ncols();
        if signature.len() != n_det {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_det,
                actual: signature.len(),
            });
        }
        Ok(Mat::from_fn(n_det, n_fluor + 1, |r, c| {
            if c < n_fluor {
                fluor_matrix[(r, c)]
            } else {
                signature[r]
            }
        }))
    }

    /// Path B TRU: group events by argmax catalog assignment and unmix each group with
    /// `[fluors | a_k]` and `autofluorescence_idx = n_fluor`. Observations are the
    /// original stained detectors (TruOls subtracts nonspecific itself); exported AF
    /// abundance stays the matcher `k*` and is not taken from these solves.
    fn path_b_tru_with_protected_af(
        fluor_matrix: MatRef<'_, f64>,
        stained_data: MatRef<'_, f64>,
        unstained_data: MatRef<'_, f64>,
        prepared: &PreparedAfLibrary,
        assignment_1based: &[u32],
        cutoff_percentile: f64,
        strategy: UnmixingStrategy,
    ) -> Result<Mat<f64>, TruOlsError> {
        let n_det = fluor_matrix.nrows();
        let n_fluor = fluor_matrix.ncols();
        let n_events = stained_data.nrows();
        if stained_data.ncols() != n_det {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_det,
                actual: stained_data.ncols(),
            });
        }
        if assignment_1based.len() != n_events {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_events,
                actual: assignment_1based.len(),
            });
        }
        if prepared.is_empty() {
            return Err(TruOlsError::InsufficientData(
                "prepared AF library has no usable entries".into(),
            ));
        }

        let mut fluor_abundances = Mat::<f64>::zeros(n_events, n_fluor);
        let n_groups = prepared.len();

        for k in 0..n_groups {
            let signature = prepared.entry_signature(k).ok_or_else(|| {
                TruOlsError::InsufficientData(format!(
                    "AF catalog assignment {k} is out of range for {} prepared entries",
                    prepared.len()
                ))
            })?;
            let matrix_with_af = hstack_fluor_and_signature(fluor_matrix, signature)?;
            let cutoffs = CutoffCalculator::calculate(
                matrix_with_af.as_ref(),
                unstained_data,
                cutoff_percentile,
            )?;
            let nonspecific_with_af = NonspecificObservation::calculate(
                matrix_with_af.as_ref(),
                unstained_data,
                n_fluor,
            )?;
            let mut tru = TruOls::from_preprocessed(
                matrix_with_af,
                unstained_data.to_owned(),
                cutoffs.cutoffs().clone(),
                nonspecific_with_af.observation().clone(),
                n_fluor,
            )?;
            tru.set_strategy(strategy);

            let event_indices: Vec<usize> = (0..n_events)
                .filter(|&ev| (assignment_1based[ev] as usize).saturating_sub(1) == k)
                .collect();
            if event_indices.is_empty() {
                continue;
            }

            let group_obs = Mat::from_fn(event_indices.len(), n_det, |i, d| {
                stained_data[(event_indices[i], d)]
            });
            let group_abundances = tru.unmix(group_obs.as_ref())?;
            for (i, &ev) in event_indices.iter().enumerate() {
                for col in 0..n_fluor {
                    fluor_abundances[(ev, col)] = group_abundances[(i, col)];
                }
            }
        }

        Ok(fluor_abundances)
    }

    fn set_af_match_keywords(
        fcs: &mut Fcs,
        result: &AfMatchResult,
        library: &PreparedAfLibrary,
    ) {
        let ids = library.entry_ids();
        let n_events = result.assignment_1based.len();
        let mut counts = vec![0_usize; ids.len()];
        for &assignment in &result.assignment_1based {
            // `assignment_1based` is produced by `match_and_subtract` against this same
            // library, so the index is in range; skip rather than panic if it is not.
            if let Some(slot) = counts.get_mut((assignment as usize).saturating_sub(1)) {
                *slot += 1;
            }
        }
        let divisor = n_events.max(1) as f64;
        let assignment = ids
            .iter()
            .zip(&counts)
            .enumerate()
            .map(|(i, (id, &count))| {
                format!("{}={}:{:.4}", i + 1, id, count as f64 / divisor)
            })
            .collect::<Vec<_>>()
            .join("|");

        let mut sorted = result.confidence.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let mean = if sorted.is_empty() {
            0.0
        } else {
            sorted.iter().sum::<f64>() / sorted.len() as f64
        };
        let median = if sorted.is_empty() {
            0.0
        } else {
            sorted[sorted.len() / 2]
        };
        let min = sorted.first().copied().unwrap_or(0.0);
        let confidence = format!("mean={mean:.4};median={median:.4};min={min:.4}");

        for (key, value) in [
            (AF_ASSIGNMENT_KEYWORD, assignment),
            (AF_CONFIDENCE_KEYWORD, confidence),
        ] {
            fcs.metadata.keywords.insert(
                key.to_string(),
                Keyword::String(StringKeyword::Other(Arc::from(value.as_str()))),
            );
        }
    }

    fn append_af_match_annotation_channels(
        fcs: &mut Fcs,
        result: &AfMatchResult,
    ) -> Result<(), TruOlsError> {
        use crate::fcs_integration::{
            AF_CONFIDENCE_CHANNEL_NAME, AF_SIGNATURE_CHANNEL_NAME, append_linear_parameter_column,
        };
        let signature: Vec<f64> = result
            .assignment_1based
            .iter()
            .map(|&v| f64::from(v))
            .collect();
        append_linear_parameter_column(
            fcs,
            AF_SIGNATURE_CHANNEL_NAME,
            AF_SIGNATURE_CHANNEL_NAME,
            &signature,
        )?;
        append_linear_parameter_column(
            fcs,
            AF_CONFIDENCE_CHANNEL_NAME,
            AF_CONFIDENCE_CHANNEL_NAME,
            &result.confidence,
        )?;
        Ok(())
    }

    fn set_af_source_keywords(
        fcs: &mut Fcs,
        af_source_fil: Option<&str>,
        af_source_guid: Option<&str>,
    ) {
        use crate::fcs_integration::{AF_SOURCE_GUID_KEYWORD, AF_SOURCE_KEYWORD};
        for (key, value) in [
            (AF_SOURCE_KEYWORD, af_source_fil),
            (AF_SOURCE_GUID_KEYWORD, af_source_guid),
        ] {
            let Some(raw) = value.map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            fcs.metadata.keywords.insert(
                key.to_string(),
                Keyword::String(StringKeyword::Other(Arc::from(raw))),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_af_when_missing() {
        let mut names = vec!["FITC".into(), "PE".into()];
        let mut flat = vec![1.0, 0.0, 0.0, 1.0];
        let (idx, name) =
            resolve_or_append_af_endmember(&mut names, &mut flat, 2, None).unwrap();
        assert_eq!(name, "AF");
        assert_eq!(idx, 2);
        assert_eq!(names.len(), 3);
        assert_eq!(flat.len(), 6);
    }

    #[test]
    fn matrix_from_flat_roundtrip_shape() {
        let flat = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let m = matrix_from_row_major_flat(&flat, 2, 3).unwrap();
        assert_eq!(m.nrows(), 2);
        assert_eq!(m.ncols(), 3);
        assert!((m[(1, 2)] - 6.0).abs() < 1e-12);
    }
}

#[cfg(all(test, feature = "flow-fcs"))]
mod path_b_tests {
    use super::{PathBExportRequest, export_unmixed_fcs_path_b};
    use crate::af_match::{AfCatalogEntry, AfMatchLibrary, AfMatchOptions};
    use crate::error::TruOlsError;
    use crate::fcs_integration::{
        AF_ASSIGNMENT_KEYWORD, AF_CONFIDENCE_KEYWORD, UNMIXED_KEYWORD,
        UNMIXED_METHOD_OLS_MULTI_AF, UNMIXED_METHOD_TRU_OLS_MULTI_AF,
    };
    use crate::unmixing::UnmixingStrategy;
    use flow_fcs::keyword::{Keyword, StringKeyword};
    use flow_fcs::{
        Fcs, Header, Metadata, Parameter, TransformType, file::AccessWrapper,
        parameter::ParameterMap,
    };
    use polars::prelude::{Column, DataFrame};
    use std::sync::Arc;

    const DETECTORS: [&str; 4] = ["D1-A", "D2-A", "D3-A", "D4-A"];
    /// Fluorophores confined to the first two detectors, so the AF signatures below have
    /// a genuinely fluorophore-orthogonal component and the expected answers are exact.
    const FLUOR_MATRIX_ROW_MAJOR: [f64; 8] = [
        1.00, 0.20, //
        0.15, 1.00, //
        0.00, 0.00, //
        0.00, 0.00,
    ];
    const AF_SMALL: [f64; 4] = [0.10, 0.05, 1.00, 0.20];
    const AF_LARGE: [f64; 4] = [0.05, 0.10, 0.20, 1.00];

    fn af_library() -> AfMatchLibrary {
        AfMatchLibrary {
            entries: vec![
                AfCatalogEntry {
                    id: "af_small".into(),
                    values: AF_SMALL.to_vec(),
                    fsc_ssc_mean: [200.0, 150.0],
                    fsc_ssc_cov: [[400.0, 0.0], [0.0, 400.0]],
                    mixing_proportion: 0.5,
                },
                AfCatalogEntry {
                    id: "af_large".into(),
                    values: AF_LARGE.to_vec(),
                    fsc_ssc_mean: [800.0, 700.0],
                    fsc_ssc_cov: [[400.0, 0.0], [0.0, 400.0]],
                    mixing_proportion: 0.5,
                },
            ],
            beta: 2.0,
            detector_names: DETECTORS.iter().map(|d| d.to_string()).collect(),
        }
    }

    /// `fluor · M + af_amount · signature`, matching the forward model the export inverts.
    fn synthesize(fluor: [f64; 2], signature: &[f64; 4], af_amount: f64) -> [f32; 4] {
        let mut out = [0.0_f32; 4];
        for i in 0..4 {
            let value = FLUOR_MATRIX_ROW_MAJOR[i * 2] * fluor[0]
                + FLUOR_MATRIX_ROW_MAJOR[i * 2 + 1] * fluor[1]
                + signature[i] * af_amount;
            out[i] = value as f32;
        }
        out
    }

    fn build_fcs(path: &std::path::Path, rows: &[([f32; 4], [f32; 2])]) -> Fcs {
        std::fs::write(path, b"stub").expect("create fixture placeholder");

        let mut columns = vec![
            Column::new(
                "FSC-A".into(),
                rows.iter().map(|(_, s)| s[0]).collect::<Vec<f32>>(),
            ),
            Column::new(
                "SSC-A".into(),
                rows.iter().map(|(_, s)| s[1]).collect::<Vec<f32>>(),
            ),
        ];
        for (d, name) in DETECTORS.iter().enumerate() {
            columns.push(Column::new(
                (*name).into(),
                rows.iter().map(|(v, _)| v[d]).collect::<Vec<f32>>(),
            ));
        }
        let df = DataFrame::new_infer_height(columns).expect("build fixture frame");

        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );
        params.insert(
            "SSC-A".into(),
            Parameter::new(&2, "SSC-A", "SSC-A", &TransformType::Linear),
        );
        for (d, name) in DETECTORS.iter().enumerate() {
            params.insert(
                (*name).into(),
                Parameter::new(&(d + 3), name, name, &TransformType::Linear),
            );
        }

        let mut fcs = Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            Arc::new(df),
            AccessWrapper::new(path.to_str().unwrap_or("")).expect("access wrapper"),
        );
        // FCS 3.2 requires `$CYT`, and the export stamps 3.2.
        fcs.metadata.keywords.insert(
            "$CYT".into(),
            Keyword::String(StringKeyword::Other(Arc::from("Aurora 5L"))),
        );
        fcs
    }

    /// Alternating small-cell / large-cell events with known fluor and AF amounts.
    fn stained_rows() -> Vec<([f32; 4], [f32; 2])> {
        (0..8)
            .map(|e| {
                let af_amount = 20.0 + e as f64;
                if e % 2 == 0 {
                    (
                        synthesize([120.0, 45.0], &AF_SMALL, af_amount),
                        [200.0_f32, 150.0],
                    )
                } else {
                    (
                        synthesize([80.0, 200.0], &AF_LARGE, af_amount),
                        [800.0_f32, 700.0],
                    )
                }
            })
            .collect()
    }

    fn unstained_rows() -> Vec<([f32; 4], [f32; 2])> {
        (0..6)
            .map(|e| {
                let jitter = e as f64 * 0.5;
                (
                    synthesize([1.0 + jitter, 0.8 + jitter], &AF_SMALL, 2.0 + jitter),
                    [200.0_f32, 150.0],
                )
            })
            .collect()
    }

    fn request<'a>(
        stained: &'a Fcs,
        unstained: &'a Fcs,
        detector_names: &'a [String],
        output_dir: &'a std::path::Path,
        truncate: bool,
    ) -> PathBExportRequest<'a> {
        PathBExportRequest {
            stained,
            unstained,
            detector_names,
            endmember_names: vec!["FITC".into(), "PE".into()],
            flat_matrix: FLUOR_MATRIX_ROW_MAJOR.to_vec(),
            af_library: af_library(),
            fsc_channel: "FSC-A",
            ssc_channel: "SSC-A",
            truncate,
            cutoff_percentile: 0.995,
            strategy: UnmixingStrategy::Zero,
            output_dir,
            raw_datasource_guid: "",
            compute_fit: true,
            endmember_fluor_names: vec![Some("FITC".into()), Some("PE".into())],
            endmember_target_names: vec![Some("CD3".into()), Some("CD19".into())],
            af_channel_name: "Autofluorescence".into(),
            af_match_options: AfMatchOptions::default(),
            af_source_fil: Some("Unstained_Control".into()),
            af_source_guid: Some("unstained-guid".into()),
        }
    }

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("truols-path-b-{name}"));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn plain_ols_path_b_export_writes_fluor_channels_plus_one_af_channel() {
        let dir = scratch_dir("ols");
        let stained = build_fcs(&dir.join("stained.fcs"), &stained_rows());
        let unstained = build_fcs(&dir.join("unstained.fcs"), &unstained_rows());
        let detector_names: Vec<String> = DETECTORS.iter().map(|d| d.to_string()).collect();

        let result = export_unmixed_fcs_path_b(request(
            &stained,
            &unstained,
            &detector_names,
            &dir,
            false,
        ))
        .expect("path B export succeeds");

        assert_eq!(
            result.endmember_names,
            vec![
                "FITC".to_string(),
                "PE".to_string(),
                "Autofluorescence".to_string()
            ],
            "AF is appended for export only, never solved for"
        );

        let reopened = Fcs::open(result.output_path.to_str().expect("utf-8 path"))
            .expect("reopen path B export");

        let names: Vec<String> = reopened
            .get_parameter_names_from_dataframe()
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(names.contains(&"FITC".to_string()));
        assert!(names.contains(&"PE".to_string()));
        assert!(names.contains(&"Autofluorescence".to_string()));
        assert!(
            names.contains(&"AF_Signature".to_string()),
            "Path B must expose per-event signature as $PnN: {names:?}"
        );
        assert!(
            names.contains(&"AF_Confidence".to_string()),
            "Path B must expose per-event confidence as $PnN: {names:?}"
        );
        assert!(names.contains(&"FSC-A".to_string()), "scatter survives");
        assert!(
            !names.iter().any(|n| DETECTORS.contains(&n.as_str())),
            "acquisition detectors must be gone: {names:?}"
        );

        let spill = reopened
            .get_keyword_string_value("$SPILLOVER")
            .unwrap_or_default();
        assert!(
            !spill.contains("AF_Signature") && !spill.contains("AF_Confidence"),
            "annotation channels must not appear in $SPILLOVER: {spill}"
        );

        let method = reopened
            .get_keyword_string_value(UNMIXED_KEYWORD)
            .expect("$UNMIXED present");
        assert_eq!(method.as_ref(), UNMIXED_METHOD_OLS_MULTI_AF);
        assert!(
            method.contains("MULTI_AF"),
            "the method string must mark the AF channel as catalog-matched"
        );

        assert_eq!(
            reopened
                .get_keyword_string_value("$AF_SOURCE")
                .expect("$AF_SOURCE")
                .as_ref(),
            "Unstained_Control"
        );
        assert_eq!(
            reopened
                .get_keyword_string_value("$AF_SOURCE_GUID")
                .expect("$AF_SOURCE_GUID")
                .as_ref(),
            "unstained-guid"
        );

        let sig = reopened
            .get_parameter_events_slice("AF_Signature")
            .expect("AF_Signature column");
        let conf = reopened
            .get_parameter_events_slice("AF_Confidence")
            .expect("AF_Confidence column");
        assert_eq!(sig.len(), conf.len());
        assert!(sig.iter().all(|&v| v >= 1.0), "assignments are 1-based");
        assert!(conf.iter().all(|&v| (0.0..=1.0).contains(&v)));

        // Every abundance is measured above the nonspecific background, which the export
        // removes before matching, so the absolute values sit a little below the amounts
        // that generated them. Differences cancel that constant exactly, which is what
        // makes them the thing worth asserting on.
        let fitc = reopened
            .get_parameter_events_slice("FITC")
            .expect("read FITC");
        let pe = reopened.get_parameter_events_slice("PE").expect("read PE");
        let af = reopened
            .get_parameter_events_slice("Autofluorescence")
            .expect("read AF");

        for event in 0..8 {
            let (generated_fitc, generated_pe) = if event % 2 == 0 {
                (120.0_f32, 45.0)
            } else {
                (80.0, 200.0)
            };
            assert!(
                fitc[event] < generated_fitc && generated_fitc - fitc[event] < 10.0,
                "event {event} FITC {} should sit just under {generated_fitc}",
                fitc[event]
            );
            assert!(
                pe[event] < generated_pe && generated_pe - pe[event] < 10.0,
                "event {event} PE {} should sit just under {generated_pe}",
                pe[event]
            );
        }

        for event in (0..8).step_by(2) {
            assert!(
                (fitc[event] - fitc[event + 1] - 40.0).abs() < 1e-2,
                "the 120/80 FITC separation must survive: {} vs {}",
                fitc[event],
                fitc[event + 1]
            );
            assert!(
                (pe[event + 1] - pe[event] - 155.0).abs() < 1e-2,
                "the 45/200 PE separation must survive: {} vs {}",
                pe[event],
                pe[event + 1]
            );
        }

        // AF was generated as `20 + event`, so two events from the same population
        // differ by exactly 2.
        for event in 0..6 {
            assert!(
                (af[event + 2] - af[event] - 2.0).abs() < 1e-2,
                "AF spacing at event {event}: {} vs {}",
                af[event],
                af[event + 2]
            );
        }

        let assignment = reopened
            .get_keyword_string_value(AF_ASSIGNMENT_KEYWORD)
            .expect("$AF_ASSIGNMENT present");
        assert!(
            assignment.contains("1=af_small:0.5000") && assignment.contains("2=af_large:0.5000"),
            "assignment summary should show the 50/50 split: {assignment}"
        );
        let confidence = reopened
            .get_keyword_string_value(AF_CONFIDENCE_KEYWORD)
            .expect("$AF_CONFIDENCE present");
        assert!(
            confidence.starts_with("mean=") && confidence.contains("median="),
            "confidence summary shape: {confidence}"
        );

        // The recorded transform is fluorophore-only, with no AF column to point at.
        let provenance = crate::provenance::UnmixProvenance::read_from(&reopened)
            .expect("provenance recovered");
        assert_eq!(provenance.endmember_names, vec!["FITC", "PE"]);
        assert_eq!(provenance.mixing_matrix.len(), 8);
        assert_eq!(provenance.af_endmember_index, None);
        assert!(
            provenance
                .unstained_info
                .as_deref()
                .is_some_and(|s| s.contains("AF catalog")),
            "unstained info must explain the AF channel: {:?}",
            provenance.unstained_info
        );
        assert!(provenance.fit.is_some(), "compute_fit was requested");
    }

    #[test]
    fn truncated_path_b_export_records_the_strategy_and_cutoff() {
        let dir = scratch_dir("truncated");
        let stained = build_fcs(&dir.join("stained.fcs"), &stained_rows());
        let unstained = build_fcs(&dir.join("unstained.fcs"), &unstained_rows());
        let detector_names: Vec<String> = DETECTORS.iter().map(|d| d.to_string()).collect();

        let result = export_unmixed_fcs_path_b(request(
            &stained,
            &unstained,
            &detector_names,
            &dir,
            true,
        ))
        .expect("truncated path B export succeeds");

        let reopened = Fcs::open(result.output_path.to_str().expect("utf-8 path"))
            .expect("reopen path B export");
        assert_eq!(
            reopened
                .get_keyword_string_value(UNMIXED_KEYWORD)
                .expect("$UNMIXED")
                .as_ref(),
            UNMIXED_METHOD_TRU_OLS_MULTI_AF
        );

        let provenance = crate::provenance::UnmixProvenance::read_from(&reopened)
            .expect("provenance recovered");
        assert_eq!(provenance.strategy, Some(UnmixingStrategy::Zero));
        assert_eq!(provenance.cutoff_percentile, Some(0.995));
        assert_eq!(
            provenance.af_endmember_index,
            Some(2),
            "Path B TRU protects a per-event AF column at index n_fluor"
        );

        // Truncation removes fluorophore columns, never the AF channel: the AF amounts
        // are decided before the solve and carry the same `20 + event` spacing here as on
        // the plain-OLS path.
        let af = reopened
            .get_parameter_events_slice("Autofluorescence")
            .expect("read AF");
        for event in 0..6 {
            assert!(
                (af[event + 2] - af[event] - 2.0).abs() < 1e-2,
                "AF spacing at event {event}: {} vs {}",
                af[event],
                af[event + 2]
            );
        }
    }

    /// Truncation may drop a weak fluorophore, but the AF endmember stays protected and
    /// the exported Autofluorescence channel remains the matcher \(k^*\) (not a
    /// truncated-to-zero / UCM-mapped TRU AF abundance).
    #[test]
    fn tru_path_b_never_drops_the_af_endmember() {
        // Three fluors on the first three detectors; AF lives in detector 4 so it is
        // distinguishable. Unstained events are bright in fluor 0 so its cutoff sits
        // well above the stained event's fluor-0 abundance.
        const FLUOR3: [f64; 12] = [
            1.00, 0.00, 0.00, //
            0.00, 1.00, 0.00, //
            0.00, 0.00, 1.00, //
            0.00, 0.00, 0.00,
        ];
        const AF_SIG: [f64; 4] = [0.05, 0.05, 0.05, 1.00];

        let dir = scratch_dir("protect-af");
        // Unstained: fluor 0 ~ 50 → cutoff for endmember 0 is high.
        let unstained_rows: Vec<([f32; 4], [f32; 2])> = (0..8)
            .map(|e| {
                let j = e as f32 * 0.1;
                (
                    [50.0 + j, 0.5 + j, 0.5 + j, 2.0 + j],
                    [200.0_f32, 150.0],
                )
            })
            .collect();
        // One stained event: fluor 0 under cutoff (~5), fluors 1–2 strong, AF present.
        let stained_row = {
            let mut det = [0.0_f32; 4];
            let fluor = [5.0_f64, 80.0, 90.0];
            let af_amount = 40.0_f64;
            for i in 0..4 {
                det[i] = (FLUOR3[i * 3] * fluor[0]
                    + FLUOR3[i * 3 + 1] * fluor[1]
                    + FLUOR3[i * 3 + 2] * fluor[2]
                    + AF_SIG[i] * af_amount) as f32;
            }
            (det, [200.0_f32, 150.0])
        };

        let stained = build_fcs(&dir.join("stained.fcs"), &[stained_row]);
        let unstained = build_fcs(&dir.join("unstained.fcs"), &unstained_rows);
        let detector_names: Vec<String> = DETECTORS.iter().map(|d| d.to_string()).collect();

        let mut req = request(&stained, &unstained, &detector_names, &dir, true);
        req.endmember_names = vec!["A".into(), "B".into(), "C".into()];
        req.flat_matrix = FLUOR3.to_vec();
        req.endmember_fluor_names = vec![Some("A".into()), Some("B".into()), Some("C".into())];
        req.endmember_target_names = vec![None, None, None];
        req.af_library = AfMatchLibrary {
            entries: vec![AfCatalogEntry {
                id: "af_only".into(),
                values: AF_SIG.to_vec(),
                fsc_ssc_mean: [200.0, 150.0],
                fsc_ssc_cov: [[400.0, 0.0], [0.0, 400.0]],
                mixing_proportion: 1.0,
            }],
            beta: 2.0,
            detector_names: DETECTORS.iter().map(|d| d.to_string()).collect(),
        };
        req.strategy = UnmixingStrategy::Zero;

        let result = export_unmixed_fcs_path_b(req).expect("protected-AF Path B TRU export");
        let reopened = Fcs::open(result.output_path.to_str().expect("utf-8 path"))
            .expect("reopen export");

        let provenance = crate::provenance::UnmixProvenance::read_from(&reopened)
            .expect("provenance recovered");
        assert_eq!(
            provenance.af_endmember_index,
            Some(3),
            "autofluorescence_idx must be Some(n_fluor); fluor-only TRU leaves it None"
        );
        assert_eq!(
            reopened
                .get_keyword_string_value(UNMIXED_KEYWORD)
                .expect("$UNMIXED")
                .as_ref(),
            UNMIXED_METHOD_TRU_OLS_MULTI_AF
        );

        let a = reopened
            .get_parameter_events_slice("A")
            .expect("read A");
        assert!(
            a[0].abs() < 1e-3,
            "fluor A is under its cutoff and should truncate: {}",
            a[0]
        );

        let af = reopened
            .get_parameter_events_slice("Autofluorescence")
            .expect("read AF");
        assert!(
            af[0] > 1.0,
            "exported Autofluorescence must stay matcher k*, not truncated-to-zero: {}",
            af[0]
        );
    }

    #[test]
    fn an_af_endmember_in_the_fluor_matrix_is_rejected() {
        let dir = scratch_dir("reject-af");
        let stained = build_fcs(&dir.join("stained.fcs"), &stained_rows());
        let unstained = build_fcs(&dir.join("unstained.fcs"), &unstained_rows());
        let detector_names: Vec<String> = DETECTORS.iter().map(|d| d.to_string()).collect();

        let mut req = request(&stained, &unstained, &detector_names, &dir, false);
        req.endmember_names = vec!["FITC".into(), "Autofluorescence".into()];

        let err = export_unmixed_fcs_path_b(req).expect_err("AF column must be refused");
        assert!(
            matches!(err, TruOlsError::InsufficientData(ref msg) if msg.contains("fluorophore-only")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn a_missing_scatter_channel_is_reported() {
        let dir = scratch_dir("missing-scatter");
        let stained = build_fcs(&dir.join("stained.fcs"), &stained_rows());
        let unstained = build_fcs(&dir.join("unstained.fcs"), &unstained_rows());
        let detector_names: Vec<String> = DETECTORS.iter().map(|d| d.to_string()).collect();

        let mut req = request(&stained, &unstained, &detector_names, &dir, false);
        req.ssc_channel = "";
        assert!(matches!(
            export_unmixed_fcs_path_b(req),
            Err(TruOlsError::InsufficientData(_))
        ));

        let mut req = request(&stained, &unstained, &detector_names, &dir, false);
        req.ssc_channel = "SSC-Z";
        let err = export_unmixed_fcs_path_b(req).expect_err("unknown channel");
        assert!(
            matches!(err, TruOlsError::InsufficientData(ref msg) if msg.contains("SSC-Z")),
            "unexpected error: {err}"
        );
    }
}
