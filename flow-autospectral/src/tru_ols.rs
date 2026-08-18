//! Optional adapters that feed selected AF columns into `flow-tru-ols`.

use crate::error::{AutospectralError, Result};
use crate::library::AfLibrary;
use crate::unmix_ols::swap_af_column;
use faer::MatRef;
use flow_tru_ols::{MixingMatrix, MixingMatrixBuilder};

/// Build a [`MixingMatrix`] with fluorophore columns plus one selected AF signature.
pub fn mixing_matrix_with_selected_af(
    detector_names: &[String],
    fluor_names: &[String],
    fluor_matrix: MatRef<'_, f64>,
    library: &AfLibrary,
    af_index: usize,
    af_endmember_name: &str,
) -> Result<MixingMatrix> {
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

    let m = swap_af_column(fluor_matrix, library, af_index)?;
    let mut builder = MixingMatrixBuilder::new(detector_names.to_vec());
    let af_col = library.column_slice(af_index)?;
    builder.set_autofluorescence(af_col);

    for (j, name) in fluor_names.iter().enumerate() {
        let mut col = Vec::with_capacity(m.nrows());
        for i in 0..m.nrows() {
            col.push(m[(i, j)]);
        }
        // AF already stored separately; MixingMatrixBuilder appends AF on build.
        // Pass fluor columns without AF correction here (already cleaned upstream).
        builder.add_endmember(name.clone(), col, false);
    }

    // MixingMatrixBuilder expects AF via set_autofluorescence and may re-normalize.
    // Prefer constructing from the already-assembled matrix when possible — for now
    // rebuild through the builder path for QC metadata (condition / hotspot).
    let _ = af_endmember_name;
    builder
        .build()
        .map_err(|e| AutospectralError::Linalg(e.to_string()))
}
