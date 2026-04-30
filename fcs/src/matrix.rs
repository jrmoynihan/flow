//! Matrix operations for flow cytometry compensation
//!
//! Provides CPU-based matrix operations for compensation calculations.

use anyhow::Result;
use faer::{Mat, MatRef};

/// Matrix operations for compensation
pub struct MatrixOps;

impl MatrixOps {
    /// Invert matrix on CPU.
    /// Uses faer (pure Rust) by default, or ndarray-linalg with system BLAS when `blas` feature is enabled.
    pub fn invert_matrix(matrix: MatRef<'_, f32>) -> Result<Mat<f32>> {
        #[cfg(feature = "blas")]
        {
            use faer_ext::IntoNdarray;
            use ndarray_linalg::Inverse;

            let a_ndarray = matrix.into_ndarray().to_owned();
            let inv_ndarray = a_ndarray
                .inv()
                .map_err(|e| anyhow::anyhow!("BLAS inverse failed: {}", e))?;
            Ok(Mat::from_fn(
                inv_ndarray.nrows(),
                inv_ndarray.ncols(),
                |i, j| inv_ndarray[[i, j]],
            ))
        }

        #[cfg(not(feature = "blas"))]
        {
            use faer::linalg::solvers::{DenseSolveCore, PartialPivLu};

            let lu = PartialPivLu::new(matrix);
            Ok(lu.inverse())
        }
    }

    /// Batch matrix-vector multiplication on CPU
    /// Input: matrix [n×n], channel_data [n_channels × n_events]
    /// Output: compensated_data [n_channels × n_events]
    pub fn batch_matvec(
        matrix: MatRef<'_, f32>,
        channel_data: &[Vec<f32>],
    ) -> Result<Vec<Vec<f32>>> {
        let n_channels = channel_data.len();
        let n_events = channel_data.first().map(|v| v.len()).unwrap_or(0);

        if n_events == 0 {
            return Ok(vec![]);
        }

        if matrix.nrows() != n_channels || matrix.ncols() != n_channels {
            return Err(anyhow::anyhow!(
                "Matrix dimensions ({}, {}) don't match channel count ({})",
                matrix.nrows(),
                matrix.ncols(),
                n_channels
            ));
        }

        // Build data matrix: [n_channels × n_events]
        let data_matrix = Mat::from_fn(n_channels, n_events, |i, j| channel_data[i][j]);

        // Result: matrix @ data_matrix -> [n_channels × n_events]
        let mut result = Mat::zeros(n_channels, n_events);
        faer::linalg::matmul::matmul(
            result.as_mut(),
            faer::Accum::Replace,
            matrix,
            data_matrix.as_ref(),
            1.0_f32,
            faer::Par::rayon(0),
        );

        // Convert back to Vec<Vec<f32>>
        let mut out = Vec::with_capacity(n_channels);
        for i in 0..n_channels {
            let channel_result: Vec<f32> = (0..n_events).map(|j| result[(i, j)]).collect();
            out.push(channel_result);
        }

        Ok(out)
    }

    /// Compensate parameters on CPU
    pub fn compensate_parameters(
        comp_matrix: MatRef<'_, f32>,
        channel_data: &[Vec<f32>],
    ) -> Result<Vec<Vec<f32>>> {
        let comp_inv = Self::invert_matrix(comp_matrix)?;
        Self::batch_matvec(comp_inv.as_ref(), channel_data)
    }
}
