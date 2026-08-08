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
    UnmixExportRequest, UnmixExportResult, FitMetricsSummary, RAW_DATASOURCE_GUID_KEYWORD,
    export_unmixed_fcs, set_raw_datasource_guid,
};

#[cfg(feature = "flow-fcs")]
mod fcs_export {
    use super::{matrix_from_row_major_flat, resolve_or_append_af_endmember, unmixed_output_path};
    use crate::TruOlsError;
    use crate::fcs_integration::{
        apply_tru_ols_unmixing_from_preprocessed, extract_detector_data, DEFAULT_AF_CHANNEL_NAME,
        UNMIXED_METHOD_TRU_OLS,
    };
    use crate::metrics::{FitMetrics, compute_fit_metrics};
    use crate::preprocessing::{CutoffCalculator, NonspecificObservation};
    use crate::provenance::UnmixFitProvenance;
    use crate::unmixing::{TruOls, UnmixingStrategy};
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
