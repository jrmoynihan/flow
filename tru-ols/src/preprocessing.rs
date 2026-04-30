//! Preprocessing functions for TRU-OLS algorithm.
//!
//! This module handles the analysis of unstained control data to determine
//! cutoff thresholds and calculate the nonspecific observation.

use crate::error::TruOlsError;
#[cfg(not(feature = "blas"))]
use faer::MatMut;
use faer::{Col, ColRef, MatRef};

/// Solve a linear system Ax = b, using least squares for overdetermined systems.
///
/// For overdetermined systems (nrows > ncols), uses QR-based least squares.
/// For square systems, uses LU decomposition.
///
/// # Arguments
/// * `a` - Coefficient matrix (nrows × ncols)
/// * `b` - Right-hand side vector (length = nrows)
///
/// # Returns
/// Solution vector x (length = ncols)
pub(crate) fn solve_linear_system(
    a: MatRef<'_, f64>,
    b: ColRef<'_, f64>,
) -> Result<Col<f64>, TruOlsError> {
    let nrows = a.nrows();
    let ncols = a.ncols();

    if nrows < ncols {
        return Err(TruOlsError::LinearAlgebra(
            "Underdetermined systems are not supported".to_string(),
        ));
    }

    #[cfg(feature = "blas")]
    {
        use ndarray_linalg::LeastSquaresSvdInto;

        let a_ndarray = ndarray::Array2::from_shape_fn((nrows, ncols), |(i, j)| a[(i, j)]);
        let b_vec: Vec<f64> = (0..nrows).map(|i| b[i]).collect();
        let b_ndarray = ndarray::Array1::from_vec(b_vec);
        let x = a_ndarray
            .least_squares_into(b_ndarray)
            .map_err(|e| TruOlsError::LinearAlgebra(format!("BLAS solve failed: {}", e)))?;
        Ok(Col::from_fn(ncols, |i| x.solution[i]))
    }

    #[cfg(not(feature = "blas"))]
    {
        let mut x_out = vec![0.0_f64; ncols];
        let mut b_rhs = vec![0.0_f64; nrows];
        let mut gram = vec![0.0_f64; ncols * ncols.max(1)];
        let mut rhs_gram = vec![0.0_f64; ncols];
        solve_least_squares_faer_in_place(
            a,
            b,
            &mut b_rhs,
            &mut x_out,
            &mut gram,
            &mut rhs_gram,
            true,
        )?;
        Ok(Col::from_fn(ncols, |i| x_out[i]))
    }
}

/// Least squares / square solve with **`b_rhs_buf`** and **`x_out`** scratch (no per-call `Mat::from_fn` for RHS).
///
/// When `prefer_gram_cholesky` is true and the system is overdetermined, tries normal equations +
/// Cholesky (same pattern as [`run_ols_normal_equations`](crate::batched_ols::run_ols_normal_equations)) and falls back to QR
/// if the Gram matrix is not SPD enough for a stable factorization.
#[cfg(not(feature = "blas"))]
pub(crate) fn solve_least_squares_faer_in_place(
    a: MatRef<'_, f64>,
    b: ColRef<'_, f64>,
    b_rhs_buf: &mut [f64],
    x_out: &mut [f64],
    gram_buf: &mut [f64],
    rhs_gram: &mut [f64],
    prefer_gram_cholesky: bool,
) -> Result<(), TruOlsError> {
    use faer::linalg::solvers::{PartialPivLu, Qr};
    use faer::prelude::{Solve, SolveLstsq};

    let nrows = a.nrows();
    let ncols = a.ncols();

    if nrows < ncols {
        return Err(TruOlsError::LinearAlgebra(
            "Underdetermined systems are not supported".to_string(),
        ));
    }
    if b_rhs_buf.len() < nrows || x_out.len() < ncols || rhs_gram.len() < ncols {
        return Err(TruOlsError::LinearAlgebra(
            "Least-squares scratch buffers are too small".to_string(),
        ));
    }
    for i in 0..nrows {
        b_rhs_buf[i] = b[i];
    }

    if nrows > ncols {
        if prefer_gram_cholesky && gram_buf.len() >= ncols * ncols {
            if try_solve_lstsq_gram_cholesky(a, b, gram_buf, rhs_gram, x_out, nrows, ncols).is_ok()
            {
                return Ok(());
            }
        }
        let b_mat = MatRef::from_column_major_slice(&b_rhs_buf[..nrows], nrows, 1);
        let qr = Qr::new(a);
        let x_faer = qr.solve_lstsq(b_mat);
        for i in 0..ncols {
            x_out[i] = x_faer[(i, 0)];
        }
        return Ok(());
    }

    let b_mat = MatRef::from_column_major_slice(&b_rhs_buf[..nrows], nrows, 1);
    let lu = PartialPivLu::new(a);
    let x_faer = lu.solve(b_mat);
    for i in 0..ncols {
        x_out[i] = x_faer[(i, 0)];
    }
    Ok(())
}

/// Forms \(M^\top M\) and \(M^\top b\), then Cholesky solve; returns `Ok(())` on success.
#[cfg(not(feature = "blas"))]
fn try_solve_lstsq_gram_cholesky(
    m: MatRef<'_, f64>,
    b: ColRef<'_, f64>,
    gram_buf: &mut [f64],
    rhs_gram: &mut [f64],
    x_out: &mut [f64],
    nrows: usize,
    k: usize,
) -> Result<(), ()> {
    use faer::Side;
    use faer::linalg::solvers::Llt;
    use faer::prelude::Solve;

    let gram_slice = &mut gram_buf[..k * k];
    let mut gram_mut = MatMut::from_column_major_slice_mut(gram_slice, k, k);
    for c in 0..k {
        for r in 0..=c {
            let mut s = 0.0_f64;
            for i in 0..nrows {
                s += m[(i, r)] * m[(i, c)];
            }
            gram_mut[(r, c)] = s;
            gram_mut[(c, r)] = s;
        }
    }
    for c in 0..k {
        let mut s = 0.0_f64;
        for i in 0..nrows {
            s += m[(i, c)] * b[i];
        }
        rhs_gram[c] = s;
    }

    let gram_ref = gram_mut.as_ref();
    let llt = Llt::new(gram_ref, Side::Lower).map_err(|_| ())?;
    let rhs_mat = MatRef::from_column_major_slice(&rhs_gram[..k], k, 1);
    let x_mat = llt.solve(rhs_mat);
    for i in 0..k {
        x_out[i] = x_mat[(i, 0)];
    }
    Ok(())
}

#[cfg(feature = "blas")]
#[cfg_attr(all(feature = "blas", feature = "unmix-cache"), allow(dead_code))]
pub(crate) fn solve_least_squares_blas_in_place(
    a: MatRef<'_, f64>,
    b: ColRef<'_, f64>,
    x_out: &mut [f64],
) -> Result<(), TruOlsError> {
    use ndarray_linalg::LeastSquaresSvdInto;

    let nrows = a.nrows();
    let ncols = a.ncols();
    if nrows < ncols {
        return Err(TruOlsError::LinearAlgebra(
            "Underdetermined systems are not supported".to_string(),
        ));
    }
    if x_out.len() < ncols {
        return Err(TruOlsError::LinearAlgebra(
            "Output buffer too small".to_string(),
        ));
    }
    let a_ndarray = ndarray::Array2::from_shape_fn((nrows, ncols), |(i, j)| a[(i, j)]);
    let b_vec: Vec<f64> = (0..nrows).map(|i| b[i]).collect();
    let b_ndarray = ndarray::Array1::from_vec(b_vec);
    let x = a_ndarray
        .least_squares_into(b_ndarray)
        .map_err(|e| TruOlsError::LinearAlgebra(format!("BLAS solve failed: {}", e)))?;
    for i in 0..ncols {
        x_out[i] = x.solution[i];
    }
    Ok(())
}

/// Calculates cutoff thresholds for each endmember based on unstained control data.
pub struct CutoffCalculator {
    cutoffs: Col<f64>,
}

impl CutoffCalculator {
    /// Calculate cutoff thresholds from unstained control data.
    ///
    /// # Arguments
    /// * `mixing_matrix` - The full mixing matrix (detectors × endmembers)
    /// * `unstained_control` - Unstained control observations (events × detectors)
    /// * `percentile` - Percentile to use for cutoff (e.g., 0.995 for 99.5th percentile)
    ///
    /// # Returns
    /// Vector of cutoff values, one per endmember
    pub fn calculate(
        mixing_matrix: MatRef<'_, f64>,
        unstained_control: MatRef<'_, f64>,
        percentile: f64,
    ) -> Result<Self, TruOlsError> {
        if !(0.0..=1.0).contains(&percentile) {
            return Err(TruOlsError::InvalidPercentile(percentile));
        }

        let n_detectors = mixing_matrix.nrows();
        let n_endmembers = mixing_matrix.ncols();
        let n_events = unstained_control.nrows();

        if unstained_control.ncols() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: unstained_control.ncols(),
            });
        }

        if n_events == 0 {
            return Err(TruOlsError::InsufficientData(
                "Unstained control must contain at least one event".to_string(),
            ));
        }

        let unmixed_abundances: Vec<Vec<f64>> = if crate::use_parallel_independent_events(n_events)
        {
            use rayon::prelude::*;
            let mut rows: Vec<(usize, Vec<f64>)> = (0..n_events)
                .into_par_iter()
                .map(|event_idx| {
                    let observation =
                        Col::from_fn(n_detectors, |i| unstained_control[(event_idx, i)]);
                    let abundances = solve_linear_system(mixing_matrix, observation.as_ref())
                        .map_err(|e| {
                            TruOlsError::LinearAlgebra(format!("Failed to solve: {}", e))
                        })?;
                    let v: Vec<f64> = (0..abundances.nrows()).map(|i| abundances[i]).collect();
                    Ok((event_idx, v))
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.sort_by_key(|(i, _)| *i);
            rows.into_iter().map(|(_, v)| v).collect()
        } else {
            let mut out = Vec::with_capacity(n_events);
            for event_idx in 0..n_events {
                let observation = Col::from_fn(n_detectors, |i| unstained_control[(event_idx, i)]);
                let abundances = solve_linear_system(mixing_matrix, observation.as_ref())
                    .map_err(|e| TruOlsError::LinearAlgebra(format!("Failed to solve: {}", e)))?;
                out.push((0..abundances.nrows()).map(|i| abundances[i]).collect());
            }
            out
        };

        // Calculate percentile for each endmember
        let mut cutoffs = Vec::with_capacity(n_endmembers);
        for endmember_idx in 0..n_endmembers {
            let mut values: Vec<f64> = unmixed_abundances
                .iter()
                .map(|abundances| abundances[endmember_idx])
                .collect();

            // NaN-safe total order (partial_cmp returns None if any operand is NaN).
            values.sort_by(|a, b| a.total_cmp(b));

            let percentile_idx = ((values.len() - 1) as f64 * percentile).round() as usize;
            let cutoff = values[percentile_idx.min(values.len() - 1)];
            cutoffs.push(cutoff);
        }

        Ok(Self {
            cutoffs: Col::from_fn(n_endmembers, |i| cutoffs[i]),
        })
    }

    /// Get the cutoff value for a specific endmember.
    pub fn get_cutoff(&self, endmember_idx: usize) -> f64 {
        self.cutoffs[endmember_idx]
    }

    /// Get all cutoff values.
    pub fn cutoffs(&self) -> &Col<f64> {
        &self.cutoffs
    }
}

/// Calculates the nonspecific observation from unstained control data.
pub struct NonspecificObservation {
    observation: Col<f64>,
}

impl NonspecificObservation {
    /// Calculate the nonspecific observation.
    ///
    /// This represents the expected "background" signal from nonspecific binding/noise.
    /// It is calculated as: `o⃗NS = M · E[α⃗c-NoAuto]`
    ///
    /// # Arguments
    /// * `mixing_matrix` - The full mixing matrix (detectors × endmembers)
    /// * `unstained_control` - Unstained control observations (events × detectors)
    /// * `autofluorescence_idx` - Index of the autofluorescence endmember (excluded from mean)
    pub fn calculate(
        mixing_matrix: MatRef<'_, f64>,
        unstained_control: MatRef<'_, f64>,
        autofluorescence_idx: usize,
    ) -> Result<Self, TruOlsError> {
        let n_detectors = mixing_matrix.nrows();
        let n_endmembers = mixing_matrix.ncols();

        if autofluorescence_idx >= n_endmembers {
            return Err(TruOlsError::NoAutofluorescenceEndmember);
        }

        if unstained_control.ncols() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: unstained_control.ncols(),
            });
        }

        let n_events = unstained_control.nrows();
        if n_events == 0 {
            return Err(TruOlsError::InsufficientData(
                "Unstained control must contain at least one event".to_string(),
            ));
        }

        let partial_rows: Vec<Vec<f64>> = if crate::use_parallel_independent_events(n_events) {
            use rayon::prelude::*;
            (0..n_events)
                .into_par_iter()
                .map(|event_idx| {
                    let observation =
                        Col::from_fn(n_detectors, |i| unstained_control[(event_idx, i)]);
                    let abundances = solve_linear_system(mixing_matrix, observation.as_ref())
                        .map_err(|e| {
                            TruOlsError::LinearAlgebra(format!("Failed to solve: {}", e))
                        })?;
                    let mut row = vec![0.0; n_endmembers];
                    for idx in 0..abundances.nrows() {
                        if idx != autofluorescence_idx {
                            row[idx] = abundances[idx];
                        }
                    }
                    Ok(row)
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let mut out = Vec::with_capacity(n_events);
            for event_idx in 0..n_events {
                let observation = Col::from_fn(n_detectors, |i| unstained_control[(event_idx, i)]);
                let abundances = solve_linear_system(mixing_matrix, observation.as_ref())
                    .map_err(|e| TruOlsError::LinearAlgebra(format!("Failed to solve: {}", e)))?;
                let mut row = vec![0.0; n_endmembers];
                for idx in 0..abundances.nrows() {
                    if idx != autofluorescence_idx {
                        row[idx] = abundances[idx];
                    }
                }
                out.push(row);
            }
            out
        };

        let mut mean_abundances = vec![0.0; n_endmembers];
        for row in partial_rows {
            for idx in 0..n_endmembers {
                mean_abundances[idx] += row[idx];
            }
        }

        // Calculate mean (excluding autofluorescence)
        for x in mean_abundances.iter_mut() {
            *x /= n_events as f64;
        }
        mean_abundances[autofluorescence_idx] = 0.0; // Ensure AF is zero

        let mean_col = Col::from_fn(n_endmembers, |i| mean_abundances[i]);

        // Calculate nonspecific observation: M · mean_abundances
        let observation = mixing_matrix * &mean_col;

        Ok(Self {
            observation: Col::from_fn(n_detectors, |i| observation[i]),
        })
    }

    /// Get the nonspecific observation vector.
    pub fn observation(&self) -> &Col<f64> {
        &self.observation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    #[test]
    fn test_cutoff_calculation() {
        // Simple 2x2 mixing matrix
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        // Two unstained events
        let unstained = mat![[0.0, 0.0], [0.1, 0.1]];

        let calculator =
            CutoffCalculator::calculate(mixing_matrix.as_ref(), unstained.as_ref(), 0.995).unwrap();
        assert_eq!(calculator.cutoffs().nrows(), 2);
    }

    #[test]
    fn test_nonspecific_observation() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let unstained = mat![[0.0, 0.0], [0.1, 0.1]];

        let nonspecific =
            NonspecificObservation::calculate(mixing_matrix.as_ref(), unstained.as_ref(), 0)
                .unwrap();
        assert_eq!(nonspecific.observation().nrows(), 2);
    }
}
