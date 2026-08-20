//! Optional adapters that feed selected AF columns into `flow-tru-ols`.

use crate::error::{AutospectralError, Result};
use crate::library::AfLibrary;
use faer::{Mat, MatRef};
use flow_tru_ols::{
    CutoffCalculator, MixingMatrix, MixingMatrixBuilder, NonspecificObservation, TruOls,
};

/// Options for assembling a TRU-OLS mixing matrix from fluorophores + one AF signature.
#[derive(Debug, Clone, Copy)]
pub struct MixingMatrixAfOptions<'a> {
    pub detector_names: &'a [String],
    pub fluor_names: &'a [String],
    pub fluor_matrix: MatRef<'a, f64>,
    pub library: &'a AfLibrary,
    pub af_index: usize,
    /// Name stored on the AF endmember column (builder default is `"Autofluorescence"`).
    pub af_endmember_name: &'a str,
    /// When true, subtract the selected AF vector from each fluor column before
    /// unit-peak normalization (raw positive-median inputs). Leave false when
    /// `fluor_matrix` is already AF-corrected / unit-peak spectra.
    pub af_correction: bool,
}

/// TRU-OLS instance built from a selected AF column plus precomputed cutoffs.
pub struct SelectedAfTruOls {
    pub mixing: MixingMatrix,
    pub tru_ols: TruOls,
    /// Column index of the AF endmember in [`SelectedAfTruOls::mixing`] (always last).
    pub autofluorescence_idx: usize,
}

/// Build a [`MixingMatrix`] with fluorophore columns plus one selected AF signature.
///
/// Fluor columns are added as endmembers; the AF library column is stored via
/// [`MixingMatrixBuilder::set_autofluorescence`] so it is appended last after
/// optional AF subtraction. The last [`MixingMatrix::endmember_names`] entry is
/// overwritten with `af_endmember_name`.
pub fn mixing_matrix_with_selected_af(opts: MixingMatrixAfOptions<'_>) -> Result<MixingMatrix> {
    let MixingMatrixAfOptions {
        detector_names,
        fluor_names,
        fluor_matrix,
        library,
        af_index,
        af_endmember_name,
        af_correction,
    } = opts;

    if fluor_matrix.nrows() != detector_names.len() {
        return Err(AutospectralError::DetectorMismatch {
            expected: detector_names.len(),
            got: fluor_matrix.nrows(),
        });
    }
    if fluor_matrix.ncols() != fluor_names.len() {
        return Err(AutospectralError::InvalidConfig(format!(
            "fluor_names len {} != fluor columns {}",
            fluor_names.len(),
            fluor_matrix.ncols()
        )));
    }
    if detector_names.len() != library.n_detectors() {
        return Err(AutospectralError::DetectorMismatch {
            expected: library.n_detectors(),
            got: detector_names.len(),
        });
    }
    if fluor_names.is_empty() {
        return Err(AutospectralError::InvalidConfig(
            "MixingMatrixBuilder requires at least one fluorophore endmember".into(),
        ));
    }
    if af_endmember_name.is_empty() {
        return Err(AutospectralError::InvalidConfig(
            "af_endmember_name must be non-empty".into(),
        ));
    }

    let af_col = library.column_slice(af_index)?;
    let mut builder = MixingMatrixBuilder::new(detector_names.to_vec());
    builder.set_autofluorescence(af_col);

    for (j, name) in fluor_names.iter().enumerate() {
        let mut col = Vec::with_capacity(fluor_matrix.nrows());
        for i in 0..fluor_matrix.nrows() {
            col.push(fluor_matrix[(i, j)]);
        }
        builder.add_endmember(name.clone(), col, af_correction);
    }

    let mut mixing = builder
        .build()
        .map_err(|e| AutospectralError::TruOls(e.to_string()))?;
    if let Some(last) = mixing.endmember_names.last_mut() {
        *last = af_endmember_name.to_string();
    }
    Ok(mixing)
}

/// Assemble a mixing matrix for `af_index`, preprocess unstained controls, then
/// construct [`TruOls`] via [`TruOls::from_preprocessed`].
///
/// `unstained_control` is events × detectors (same layout as `TruOls::new`).
pub fn tru_ols_from_selected_af(
    opts: MixingMatrixAfOptions<'_>,
    unstained_control: MatRef<'_, f64>,
    cutoff_percentile: f64,
) -> Result<SelectedAfTruOls> {
    let mixing = mixing_matrix_with_selected_af(opts)?;
    let n_det = mixing.matrix.nrows();
    if unstained_control.ncols() != n_det {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_det,
            got: unstained_control.ncols(),
        });
    }
    let af_idx = mixing
        .matrix
        .ncols()
        .checked_sub(1)
        .ok_or_else(|| AutospectralError::InvalidConfig("empty mixing matrix".into()))?;

    let cutoffs =
        CutoffCalculator::calculate(mixing.matrix.as_ref(), unstained_control, cutoff_percentile)
            .map_err(|e| AutospectralError::TruOls(e.to_string()))?;
    let nonspecific =
        NonspecificObservation::calculate(mixing.matrix.as_ref(), unstained_control, af_idx)
            .map_err(|e| AutospectralError::TruOls(e.to_string()))?;

    let unstained_owned = Mat::from_fn(
        unstained_control.nrows(),
        unstained_control.ncols(),
        |i, j| unstained_control[(i, j)],
    );
    let tru_ols = TruOls::from_preprocessed(
        mixing.matrix.clone(),
        unstained_owned,
        cutoffs.cutoffs().clone(),
        nonspecific.observation().clone(),
        af_idx,
    )
    .map_err(|e| AutospectralError::TruOls(e.to_string()))?;

    Ok(SelectedAfTruOls {
        mixing,
        tru_ols,
        autofluorescence_idx: af_idx,
    })
}

/// Copy row-major `n_events × n_detectors` into a faer events × detectors matrix.
pub fn events_row_major_to_mat(
    events_row_major: &[f64],
    n_events: usize,
    n_detectors: usize,
) -> Result<Mat<f64>> {
    if n_events == 0 {
        return Err(AutospectralError::EmptyEvents);
    }
    if events_row_major.len() != n_events * n_detectors {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_detectors,
            got: events_row_major
                .len()
                .checked_div(n_events.max(1))
                .unwrap_or(0),
        });
    }
    Ok(Mat::from_fn(n_events, n_detectors, |i, j| {
        events_row_major[i * n_detectors + j]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DiscoverConfig, DiscoveryBackend};
    use crate::discover::discover_af_library;
    use faer::Mat;

    fn tiny_library() -> AfLibrary {
        let mut events = Vec::new();
        for _ in 0..30 {
            events.extend_from_slice(&[10.0, 1.0]);
        }
        for _ in 0..30 {
            events.extend_from_slice(&[1.0, 10.0]);
        }
        let names = vec!["D1".into(), "D2".into()];
        let cfg = DiscoverConfig {
            backend: DiscoveryBackend::KMeans,
            fixed_k: Some(2),
            seed: Some(1),
            ..DiscoverConfig::default()
        };
        discover_af_library(&events, 60, 2, &names, &cfg).unwrap()
    }

    #[test]
    fn mixing_matrix_appends_named_af_column() {
        let lib = tiny_library();
        let detectors = lib.detector_names.clone();
        let fluor_names = vec!["FITC".into()];
        let fluor = Mat::from_fn(2, 1, |i, _| if i == 0 { 1.0 } else { 0.1 });
        let mixing = mixing_matrix_with_selected_af(MixingMatrixAfOptions {
            detector_names: &detectors,
            fluor_names: &fluor_names,
            fluor_matrix: fluor.as_ref(),
            library: &lib,
            af_index: 0,
            af_endmember_name: "AF_0",
            af_correction: false,
        })
        .unwrap();
        assert_eq!(mixing.matrix.ncols(), 2);
        assert_eq!(
            mixing.endmember_names.last().map(String::as_str),
            Some("AF_0")
        );
    }

    #[test]
    fn from_preprocessed_matches_detector_count() {
        let lib = tiny_library();
        let detectors = lib.detector_names.clone();
        let fluor_names = vec!["FITC".into()];
        let fluor = Mat::from_fn(2, 1, |i, _| if i == 0 { 1.0 } else { 0.1 });
        let mut unstained = Vec::new();
        for _ in 0..20 {
            unstained.extend_from_slice(&[0.5, 0.4]);
        }
        let u = events_row_major_to_mat(&unstained, 20, 2).unwrap();
        let selected = tru_ols_from_selected_af(
            MixingMatrixAfOptions {
                detector_names: &detectors,
                fluor_names: &fluor_names,
                fluor_matrix: fluor.as_ref(),
                library: &lib,
                af_index: 0,
                af_endmember_name: "AF_0",
                af_correction: false,
            },
            u.as_ref(),
            0.995,
        )
        .unwrap();
        assert_eq!(selected.autofluorescence_idx, 1);
        assert_eq!(selected.mixing.matrix.nrows(), 2);
    }

    #[test]
    fn rejects_empty_fluor_panel() {
        let lib = tiny_library();
        let detectors = lib.detector_names.clone();
        let fluor = Mat::<f64>::zeros(2, 0);
        let err = mixing_matrix_with_selected_af(MixingMatrixAfOptions {
            detector_names: &detectors,
            fluor_names: &[],
            fluor_matrix: fluor.as_ref(),
            library: &lib,
            af_index: 0,
            af_endmember_name: "AF",
            af_correction: false,
        });
        assert!(err.is_err());
    }
}
