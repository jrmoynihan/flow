//! GPU-accelerated statistical operations
//!
//! Note: Median and percentile operations fall back to CPU (GPU sorting not implemented).
//! Standard deviation uses GPU but may have overhead for small datasets.

use burn::backend::wgpu::WgpuDevice;
use burn::tensor::Tensor;
use crate::error::{PeacoQCError, Result};

type Backend = burn::backend::wgpu::Wgpu;

/// Calculate standard deviation on GPU
///
/// Uses GPU tensor operations. May have overhead for small datasets.
pub fn standard_deviation_gpu(data: &[f64]) -> Result<f64> {
    if data.is_empty() {
        return Err(PeacoQCError::StatsError("Empty data".to_string()));
    }

    let device = WgpuDevice::default();
    
    // Convert to tensor - burn 0.20 API
    use burn::tensor::TensorData;
    
    // Convert f64 to bytes for TensorData
    let data_bytes: Vec<u8> = data.iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let data_tensor_data = TensorData::new(data_bytes.into(), vec![data.len()]);
    let data_tensor = Tensor::<Backend, 1, burn::tensor::Float>::from_data(data_tensor_data, &device);

    // Calculate mean
    let mean = data_tensor.clone().mean();
    let mean_data = mean.to_data();
    let mean_value = mean_data.as_slice::<f64>().unwrap()[0];

    // Calculate variance: mean((x - mean)^2)
    let diff = data_tensor - mean_value;
    let diff_squared = diff.powf_scalar(2.0);
    let variance = diff_squared.mean();
    let variance_data = variance.to_data();
    let variance_value = variance_data.as_slice::<f64>().unwrap()[0];

    Ok(variance_value.sqrt())
}

/// Calculate median on GPU
///
/// Currently falls back to CPU implementation (GPU sorting not implemented).
pub fn median_gpu(data: &[f64]) -> Result<f64> {
    if data.is_empty() {
        return Err(PeacoQCError::StatsError("Empty data".to_string()));
    }
    // Fall back to CPU implementation (GPU sorting not implemented)
    crate::stats::median(data)
}

/// Calculate percentile on GPU
///
/// Currently falls back to CPU implementation (GPU sorting not implemented).
pub fn percentile_gpu(data: &[f64], p: f64) -> Result<f64> {
    if data.is_empty() {
        return Err(PeacoQCError::StatsError("Empty data".to_string()));
    }

    if !(0.0..=1.0).contains(&p) {
        return Err(PeacoQCError::StatsError(
            format!("Percentile must be between 0 and 1, got {}", p)
        ));
    }

    // Fall back to CPU implementation (GPU sorting not implemented)
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let n = sorted.len();
    let idx = (p * (n - 1) as f64).floor() as usize;
    let idx = idx.min(n - 1);

    Ok(sorted[idx])
}
