//! Matrix operations for flow cytometry compensation
//!
//! Provides CPU-based matrix operations for compensation calculations.

use anyhow::Result;
use ndarray::Array2;

/// Matrix operations for compensation
pub struct MatrixOps;

impl MatrixOps {
    /// Invert matrix on CPU using ndarray-linalg
    pub fn invert_matrix(matrix: &Array2<f32>) -> Result<Array2<f32>> {
        use ndarray_linalg::Inverse;
        matrix
            .inv()
            .map_err(|e| anyhow::anyhow!("Failed to invert matrix: {:?}", e))
    }

    /// Batch matrix-vector multiplication on CPU
    /// Input: matrix [n×n], channel_data [n_channels × n_events]
    /// Output: compensated_data [n_channels × n_events]
    pub fn batch_matvec(matrix: &Array2<f32>, channel_data: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
        let n_channels = channel_data.len();
        let n_events = channel_data.first().map(|v| v.len()).unwrap_or(0);

        if n_events == 0 {
            return Ok(vec![]);
        }

        // Convert channel_data to matrix format: [n_channels × n_events]
        let data_matrix = {
            let mut mat = Array2::<f32>::zeros((n_channels, n_events));
            for (i, channel) in channel_data.iter().enumerate() {
                for (j, &value) in channel.iter().enumerate() {
                    mat[[i, j]] = value;
                }
            }
            mat
        };

        // Matrix multiplication: matrix @ data_matrix
        // Result: [n_channels × n_events]
        let result_matrix = matrix.dot(&data_matrix);

        // Convert back to Vec<Vec<f32>>
        let mut result = Vec::with_capacity(n_channels);
        for i in 0..n_channels {
            let mut channel_result = Vec::with_capacity(n_events);
            for j in 0..n_events {
                channel_result.push(result_matrix[[i, j]]);
            }
            result.push(channel_result);
        }

        Ok(result)
    }

    /// Compensate parameters on CPU
    pub fn compensate_parameters(
        comp_matrix: &Array2<f32>,
        channel_data: &[Vec<f32>],
    ) -> Result<Vec<Vec<f32>>> {
        // Invert matrix
        let comp_inv = Self::invert_matrix(comp_matrix)?;

        // Batch matrix-vector multiplication
        Self::batch_matvec(&comp_inv, channel_data)
    }
}
