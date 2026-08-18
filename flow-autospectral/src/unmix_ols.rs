//! Standalone OLS helpers used by residual matching and unmixed output.

use crate::error::{AutospectralError, Result};
use crate::library::AfLibrary;
use faer::prelude::SolveLstsq;
use faer::{Mat, MatRef};

/// Append one AF library column to a fluorophore mixing matrix (detectors × fluors).
pub fn swap_af_column(
    fluor_matrix: MatRef<'_, f64>,
    library: &AfLibrary,
    af_index: usize,
) -> Result<Mat<f64>> {
    if af_index >= library.n_signatures() {
        return Err(AutospectralError::AfIndexOutOfRange {
            index: af_index,
            n: library.n_signatures(),
        });
    }
    let n_det = fluor_matrix.nrows();
    if n_det != library.n_detectors() {
        return Err(AutospectralError::DetectorMismatch {
            expected: library.n_detectors(),
            got: n_det,
        });
    }
    let n_fluor = fluor_matrix.ncols();
    let mut m = Mat::<f64>::zeros(n_det, n_fluor + 1);
    for j in 0..n_fluor {
        for i in 0..n_det {
            m[(i, j)] = fluor_matrix[(i, j)];
        }
    }
    for i in 0..n_det {
        m[(i, n_fluor)] = library.signatures[(i, af_index)];
    }
    Ok(m)
}

/// Squared residual ‖y − M α̂‖² for the OLS solution of `M α ≈ y`.
pub fn ols_residual(m: MatRef<'_, f64>, y: &[f64]) -> Result<f64> {
    let n_det = m.nrows();
    let n_em = m.ncols();
    if y.len() != n_det {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_det,
            got: y.len(),
        });
    }
    if n_em == 0 {
        // No columns: residual is ‖y‖².
        return Ok(y.iter().map(|v| v * v).sum());
    }

    let b = Mat::from_fn(n_det, 1, |i, _| y[i]);
    let qr = m.qr();
    let x = qr.solve_lstsq(&b);
    let mut residual = 0.0;
    for i in 0..n_det {
        let mut pred = 0.0;
        for j in 0..n_em {
            pred += m[(i, j)] * x[(j, 0)];
        }
        let e = y[i] - pred;
        residual += e * e;
    }
    Ok(residual)
}

/// Unmix one event with a fixed mixing matrix; returns abundances (length = ncols).
pub fn unmix_event_ols(m: MatRef<'_, f64>, y: &[f64]) -> Result<Vec<f64>> {
    let n_det = m.nrows();
    let n_em = m.ncols();
    if y.len() != n_det {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_det,
            got: y.len(),
        });
    }
    if n_em == 0 {
        return Ok(Vec::new());
    }
    let b = Mat::from_fn(n_det, 1, |i, _| y[i]);
    let qr = m.qr();
    let x = qr.solve_lstsq(&b);
    Ok((0..n_em).map(|j| x[(j, 0)]).collect())
}

/// Unmix many events that share the same mixing matrix (row-major events → row-major abundances).
pub fn unmix_events_ols(
    m: MatRef<'_, f64>,
    events_row_major: &[f64],
    n_events: usize,
) -> Result<Vec<f64>> {
    let n_det = m.nrows();
    let n_em = m.ncols();
    if events_row_major.len() != n_events * n_det {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_det,
            got: events_row_major.len() / n_events.max(1),
        });
    }
    let mut out = vec![0.0; n_events * n_em];
    for e in 0..n_events {
        let y = &events_row_major[e * n_det..(e + 1) * n_det];
        let alpha = unmix_event_ols(m, y)?;
        out[e * n_em..(e + 1) * n_em].copy_from_slice(&alpha);
    }
    Ok(out)
}
