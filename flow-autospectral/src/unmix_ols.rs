//! Standalone OLS helpers used by residual matching and unmixed output.
//!
//! Batch unmix factors \(M^\top M\) once (Cholesky) and triangular-solves per event,
//! falling back to a single QR of \(M\) when the Gram matrix is not SPD or the
//! system is underdetermined (`n_det < n_em`). Set [`OlsUnmixConfig::reuse_factor`]
//! to `false` for the per-event QR path used as a Criterion A/B baseline.

use crate::config::{OlsUnmixConfig, force_sequential};
use crate::error::{AutospectralError, Result};
use crate::library::AfLibrary;
use faer::linalg::solvers::{Llt, Qr};
use faer::prelude::{Solve, SolveLstsq};
use faer::{Mat, MatRef, Side};
use rayon::prelude::*;

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

/// Squared residual ‖y − M α̂‖² for the OLS solution of `M α ≈ y` (per-call QR).
pub fn ols_residual(m: MatRef<'_, f64>, y: &[f64]) -> Result<f64> {
    residual_from_alpha(m, y, &unmix_event_ols(m, y)?)
}

/// Squared residual after factoring `m` once (Gram Cholesky, QR fallback).
pub fn ols_residual_with_matrix(m: MatRef<'_, f64>, y: &[f64]) -> Result<f64> {
    OlsFactor::from_matrix(m).residual(y)
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
///
/// Uses [`OlsUnmixConfig::default`]: factor once and Rayon above 256 events.
pub fn unmix_events_ols(
    m: MatRef<'_, f64>,
    events_row_major: &[f64],
    n_events: usize,
) -> Result<Vec<f64>> {
    unmix_events_ols_with(m, events_row_major, n_events, &OlsUnmixConfig::default())
}

/// [`unmix_events_ols`] with explicit factor-reuse and parallel thresholds.
pub fn unmix_events_ols_with(
    m: MatRef<'_, f64>,
    events_row_major: &[f64],
    n_events: usize,
    config: &OlsUnmixConfig,
) -> Result<Vec<f64>> {
    let n_det = m.nrows();
    let n_em = m.ncols();
    if events_row_major.len() != n_events * n_det {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_det,
            got: events_row_major.len() / n_events.max(1),
        });
    }
    let parallel = use_parallel_events(n_events, n_det, n_em, config.parallel_event_threshold);
    if config.reuse_factor {
        let factor = OlsFactor::from_matrix(m);
        map_event_abundances(events_row_major, n_events, n_det, n_em, parallel, |y| {
            factor.unmix(y)
        })
    } else {
        map_event_abundances(events_row_major, n_events, n_det, n_em, parallel, |y| {
            unmix_event_ols(m, y)
        })
    }
}

fn use_parallel_events(n_events: usize, n_det: usize, n_em: usize, threshold: usize) -> bool {
    !force_sequential() && n_events >= threshold && n_det > 0 && n_em > 0
}

fn map_event_abundances<F>(
    events_row_major: &[f64],
    n_events: usize,
    n_det: usize,
    n_em: usize,
    parallel: bool,
    unmix_one: F,
) -> Result<Vec<f64>>
where
    F: Fn(&[f64]) -> Result<Vec<f64>> + Sync + Send,
{
    if n_em == 0 {
        return Ok(Vec::new());
    }
    let mut out = vec![0.0; n_events * n_em];
    if parallel {
        out.par_chunks_mut(n_em)
            .zip(events_row_major.par_chunks(n_det))
            .try_for_each(|(chunk, y)| {
                let alpha = unmix_one(y)?;
                chunk.copy_from_slice(&alpha);
                Ok(())
            })?;
    } else {
        for e in 0..n_events {
            let y = &events_row_major[e * n_det..(e + 1) * n_det];
            let alpha = unmix_one(y)?;
            out[e * n_em..(e + 1) * n_em].copy_from_slice(&alpha);
        }
    }
    Ok(out)
}

fn residual_from_alpha(m: MatRef<'_, f64>, y: &[f64], alpha: &[f64]) -> Result<f64> {
    let n_det = m.nrows();
    let n_em = m.ncols();
    if y.len() != n_det {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_det,
            got: y.len(),
        });
    }
    if n_em == 0 {
        return Ok(y.iter().map(|v| v * v).sum());
    }
    let mut residual = 0.0;
    for i in 0..n_det {
        let mut pred = 0.0;
        for (j, a) in alpha.iter().enumerate() {
            pred += m[(i, j)] * a;
        }
        let e = y[i] - pred;
        residual += e * e;
    }
    Ok(residual)
}

/// Cached mixing matrix plus Gram Cholesky or QR. `Send + Sync` for event-level Rayon.
pub(crate) enum OlsFactor {
    Empty { n_det: usize },
    Cholesky { m: Mat<f64>, llt: Llt<f64> },
    Qr { m: Mat<f64>, qr: Qr<f64> },
}

impl OlsFactor {
    pub(crate) fn from_matrix(m: MatRef<'_, f64>) -> Self {
        Self::from_owned(m.to_owned())
    }

    pub(crate) fn from_owned(m: Mat<f64>) -> Self {
        let n_det = m.nrows();
        let n_em = m.ncols();
        if n_em == 0 {
            return Self::Empty { n_det };
        }
        if n_det >= n_em {
            let mt = m.transpose().to_owned();
            let gram: Mat<f64> = &mt * &m;
            if let Ok(llt) = Llt::new(gram.as_ref(), Side::Lower) {
                return Self::Cholesky { m, llt };
            }
        }
        let qr = m.qr();
        Self::Qr { m, qr }
    }

    fn n_det(&self) -> usize {
        match self {
            Self::Empty { n_det } => *n_det,
            Self::Cholesky { m, .. } | Self::Qr { m, .. } => m.nrows(),
        }
    }

    fn n_em(&self) -> usize {
        match self {
            Self::Empty { .. } => 0,
            Self::Cholesky { m, .. } | Self::Qr { m, .. } => m.ncols(),
        }
    }

    fn mixing(&self) -> Option<MatRef<'_, f64>> {
        match self {
            Self::Empty { .. } => None,
            Self::Cholesky { m, .. } | Self::Qr { m, .. } => Some(m.as_ref()),
        }
    }

    fn unmix(&self, y: &[f64]) -> Result<Vec<f64>> {
        let n_det = self.n_det();
        if y.len() != n_det {
            return Err(AutospectralError::DetectorMismatch {
                expected: n_det,
                got: y.len(),
            });
        }
        match self {
            Self::Empty { .. } => Ok(Vec::new()),
            Self::Cholesky { m, llt } => {
                let n_em = m.ncols();
                let rhs = Mat::from_fn(n_em, 1, |j, _| {
                    let mut s = 0.0;
                    for i in 0..n_det {
                        s += m[(i, j)] * y[i];
                    }
                    s
                });
                let x = llt.solve(rhs.as_ref());
                Ok((0..n_em).map(|j| x[(j, 0)]).collect())
            }
            Self::Qr { qr, .. } => {
                let n_em = self.n_em();
                let b = Mat::from_fn(n_det, 1, |i, _| y[i]);
                let x = qr.solve_lstsq(&b);
                Ok((0..n_em).map(|j| x[(j, 0)]).collect())
            }
        }
    }

    pub(crate) fn residual(&self, y: &[f64]) -> Result<f64> {
        let alpha = self.unmix(y)?;
        match self.mixing() {
            None => Ok(y.iter().map(|v| v * v).sum()),
            Some(m) => residual_from_alpha(m, y, &alpha),
        }
    }

    #[cfg(test)]
    fn is_cholesky(&self) -> bool {
        matches!(self, Self::Cholesky { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::AfLibrary;
    use faer::Mat;

    fn well_conditioned_m() -> Mat<f64> {
        Mat::from_fn(3, 3, |i, j| {
            if i == j {
                1.0
            } else if i + 1 == j {
                0.2
            } else {
                0.0
            }
        })
    }

    fn synthetic_events(n_events: usize, n_det: usize) -> Vec<f64> {
        (0..n_events * n_det)
            .map(|k| (k % 7) as f64 * 0.1 + 1.0)
            .collect()
    }

    fn assert_close(a: &[f64], b: &[f64], tol: f64) {
        assert_eq!(a.len(), b.len());
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(
                (x - y).abs() < tol,
                "mismatch at {i}: {x} vs {y} (tol {tol})"
            );
        }
    }

    #[test]
    fn residual_of_exact_column_is_near_zero() {
        let m = Mat::from_fn(2, 1, |i, _| if i == 0 { 1.0 } else { 0.0 });
        let y = [2.0, 0.0];
        let r = ols_residual(m.as_ref(), &y).unwrap();
        assert!(r < 1e-12, "residual {r}");
        let r_factored = ols_residual_with_matrix(m.as_ref(), &y).unwrap();
        assert!(r_factored < 1e-12, "factored residual {r_factored}");
    }

    #[test]
    fn unmix_recovers_scale() {
        let m = Mat::from_fn(2, 1, |i, _| if i == 0 { 1.0 } else { 0.0 });
        let y = [4.0, 0.0];
        let a = unmix_event_ols(m.as_ref(), &y).unwrap();
        assert_eq!(a.len(), 1);
        assert!((a[0] - 4.0).abs() < 1e-9);
    }

    #[test]
    fn swap_af_appends_library_column() {
        let fluor = Mat::<f64>::zeros(2, 0);
        let mut sig = Mat::<f64>::zeros(2, 1);
        sig[(0, 0)] = 1.0;
        sig[(1, 0)] = 0.2;
        let lib = AfLibrary {
            signatures: sig,
            names: vec!["AF_0".into()],
            detector_names: vec!["D1".into(), "D2".into()],
            provenance: "test".into(),
        };
        let m = swap_af_column(fluor.as_ref(), &lib, 0).unwrap();
        assert_eq!(m.ncols(), 1);
        assert!((m[(0, 0)] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn well_conditioned_uses_cholesky() {
        let factor = OlsFactor::from_owned(well_conditioned_m());
        assert!(factor.is_cholesky());
    }

    #[test]
    fn underdetermined_falls_back_to_qr() {
        let m = Mat::from_fn(2, 3, |i, j| if i == j { 1.0 } else { 0.1 });
        let factor = OlsFactor::from_owned(m);
        assert!(!factor.is_cholesky());
    }

    #[test]
    fn rank_deficient_falls_back_to_qr() {
        // Columns are linearly dependent: col1 = 2 * col0.
        let m = Mat::from_fn(2, 2, |i, j| ((i + 1) * (j + 1)) as f64);
        let factor = OlsFactor::from_owned(m);
        assert!(!factor.is_cholesky());
    }

    #[test]
    fn sequential_vs_parallel_unmix_agrees() {
        let m = well_conditioned_m();
        let events = synthetic_events(32, 3);
        let seq = unmix_events_ols_with(
            m.as_ref(),
            &events,
            32,
            &OlsUnmixConfig {
                parallel_event_threshold: usize::MAX,
                reuse_factor: true,
            },
        )
        .unwrap();
        let par = unmix_events_ols_with(
            m.as_ref(),
            &events,
            32,
            &OlsUnmixConfig {
                parallel_event_threshold: 0,
                reuse_factor: true,
            },
        )
        .unwrap();
        assert_close(&seq, &par, 1e-12);
    }

    #[test]
    fn factor_once_vs_naive_unmix_agrees() {
        let m = well_conditioned_m();
        let events = synthetic_events(16, 3);
        let factored = unmix_events_ols_with(
            m.as_ref(),
            &events,
            16,
            &OlsUnmixConfig {
                parallel_event_threshold: usize::MAX,
                reuse_factor: true,
            },
        )
        .unwrap();
        let naive = unmix_events_ols_with(
            m.as_ref(),
            &events,
            16,
            &OlsUnmixConfig {
                parallel_event_threshold: usize::MAX,
                reuse_factor: false,
            },
        )
        .unwrap();
        assert_close(&factored, &naive, 1e-9);
    }

    #[test]
    fn ols_factor_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OlsFactor>();
    }
}
