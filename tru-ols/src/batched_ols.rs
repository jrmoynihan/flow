//! Batched ordinary least squares via normal equations (Gram matrix + Cholesky).
//!
//! For a fixed mixing matrix \(M\) (detectors × endmembers) and many observation rows,
//! this path factors \(M^\top M\) once, forms all RHS rows as **observations × M** (stacked
//! \(M^\top b_i\)), then triangular-solves per event (parallel when event count is high).
//! It matches the algebraic solution of least squares when \(M\) has full column rank and
//! \(M^\top M\) is well-conditioned; use [`crate::benchmark::run_ols`] (QR/BLAS) when
//! that is in doubt. A future GPU implementation would typically accelerate the same pattern.

use crate::error::TruOlsError;
use faer::linalg::solvers::Llt;
use faer::prelude::*;
use faer::{Mat, MatRef, Side};

/// OLS unmixing using one Cholesky factorization of \(M^\top M\) and triangular solves per event.
///
/// Returns the same shape as [`crate::benchmark::run_ols`]: (events × endmembers).
pub fn run_ols_normal_equations(
    observations: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
) -> Result<Mat<f64>, TruOlsError> {
    let n_events = observations.nrows();
    let n_det = observations.ncols();
    let n_em = mixing_matrix.ncols();

    if mixing_matrix.nrows() != n_det {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_det,
            actual: mixing_matrix.nrows(),
        });
    }

    if n_det < n_em {
        return Err(TruOlsError::LinearAlgebra(
            "Underdetermined systems are not supported".to_string(),
        ));
    }

    let mt = mixing_matrix.transpose().to_owned();
    let gram: Mat<f64> = &mt * mixing_matrix;

    let llt = Llt::new(gram.as_ref(), Side::Lower).map_err(|e| {
        TruOlsError::LinearAlgebra(format!(
            "Cholesky of Gram matrix failed (matrix may be rank-deficient or ill-conditioned): {}",
            e
        ))
    })?;

    // Stacked RHS: row i is M^T b_i — same as (observations * mixing_matrix)[i, :].
    let rhs_all: Mat<f64> = observations * mixing_matrix;

    let mut result = Mat::zeros(n_events, n_em);
    if crate::use_parallel_independent_events(n_events) {
        use rayon::prelude::*;
        let rows: Vec<(usize, Vec<f64>)> = (0..n_events)
            .into_par_iter()
            .map(|ev| {
                let rhs_mat = Mat::from_fn(n_em, 1, |i, _| rhs_all[(ev, i)]);
                let x = llt.solve(rhs_mat.as_ref());
                let row: Vec<f64> = (0..n_em).map(|j| x[(j, 0)]).collect();
                (ev, row)
            })
            .collect();
        let mut sorted = rows;
        sorted.sort_by_key(|(ev, _)| *ev);
        for (ev, row) in sorted {
            for j in 0..n_em {
                result[(ev, j)] = row[j];
            }
        }
    } else {
        for ev in 0..n_events {
            let rhs_mat = Mat::from_fn(n_em, 1, |i, _| rhs_all[(ev, i)]);
            let x = llt.solve(rhs_mat.as_ref());
            for j in 0..n_em {
                result[(ev, j)] = x[(j, 0)];
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::run_ols;
    use faer::mat;

    #[test]
    fn normal_equations_matches_run_ols_well_conditioned() {
        let mixing = mat![[1.0, 0.2, 0.0], [0.0, 1.0, 0.2], [0.0, 0.0, 1.0]];
        let observations = Mat::from_fn(50, 3, |ev, det| (ev + det * 7) as f64 * 0.01 + 1.0);
        let ref_ab = run_ols(observations.as_ref(), mixing.as_ref()).unwrap();
        let ne_ab = run_ols_normal_equations(observations.as_ref(), mixing.as_ref()).unwrap();
        for i in 0..ref_ab.nrows() {
            for j in 0..ref_ab.ncols() {
                let a = ref_ab[(i, j)];
                let b = ne_ab[(i, j)];
                assert!(
                    (a - b).abs() < 1e-9,
                    "mismatch at ({}, {}): {} vs {}",
                    i,
                    j,
                    a,
                    b
                );
            }
        }
    }

    #[test]
    fn normal_equations_batched_rhs_matches_run_ols_many_events() {
        let mixing = mat![[1.0, 0.2, 0.0], [0.0, 1.0, 0.2], [0.0, 0.0, 1.0]];
        let observations = Mat::from_fn(500, 3, |ev, det| (ev + det * 7) as f64 * 0.001 + 1.0);
        let ref_ab = run_ols(observations.as_ref(), mixing.as_ref()).unwrap();
        let ne_ab = run_ols_normal_equations(observations.as_ref(), mixing.as_ref()).unwrap();
        for i in 0..ref_ab.nrows() {
            for j in 0..ref_ab.ncols() {
                let a = ref_ab[(i, j)];
                let b = ne_ab[(i, j)];
                assert!(
                    (a - b).abs() < 1e-9,
                    "mismatch at ({}, {}): {} vs {}",
                    i,
                    j,
                    a,
                    b
                );
            }
        }
    }
}
