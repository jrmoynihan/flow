//! GPU-accelerated FFT operations for KDE
//!
//! Uses GPU for complex multiplication in frequency domain (FFT convolution step).
//! CPU FFT is used for the actual transforms (burn doesn't expose FFT directly).
//! cubeCL custom kernels are available as an optional optimization.

use burn::backend::wgpu::WgpuDevice;
use burn::tensor::Tensor;
use crate::error::{PeacoQCError, Result};
use crate::gpu::is_gpu_available;
use realfft::num_complex::Complex;

type Backend = burn::backend::wgpu::Wgpu;

/// GPU-accelerated FFT-based KDE
///
/// Uses GPU for convolution multiplication and other operations,
/// while using CPU FFT for the actual transforms.
pub fn kde_fft_gpu(
    data: &[f64],
    grid: &[f64],
    bandwidth: f64,
    n: f64,
) -> Result<Vec<f64>> {
    let m = grid.len();
    if m < 2 {
        return Err(PeacoQCError::StatsError("Grid must have at least 2 points".to_string()));
    }

    let grid_min = grid[0];
    let grid_max = grid[m - 1];
    let grid_spacing = (grid_max - grid_min) / (m - 1) as f64;

    // Step 1: Bin data onto grid (CPU - too small for GPU overhead)
    let mut binned = vec![0.0; m];
    for &x in data {
        let idx = ((x - grid_min) / grid_spacing).floor() as isize;
        if idx >= 0 && (idx as usize) < m {
            binned[idx as usize] += 1.0;
        }
    }

    // Step 2: Create kernel values on grid (CPU - small computation)
    let kernel_center = (m - 1) as f64 / 2.0;
    let mut kernel = Vec::with_capacity(m);
    for i in 0..m {
        let grid_pos = (i as f64 - kernel_center) * grid_spacing;
        let u = grid_pos / bandwidth;
        kernel.push(gaussian_kernel(u));
    }

    // Step 3: Zero-pad to avoid circular convolution
    let fft_size = (2 * m).next_power_of_two();

    // Use CPU FFT for now (burn doesn't expose FFT directly)
    // We'll use GPU for the multiplication step if beneficial
    use realfft::RealFftPlanner;
    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(fft_size);
    let c2r = planner.plan_fft_inverse(fft_size);

    // Prepare padded arrays
    let mut binned_padded = vec![0.0; fft_size];
    binned_padded[..m].copy_from_slice(&binned);

    let mut kernel_padded = vec![0.0; fft_size];
    let kernel_start = (fft_size - m) / 2;
    let first_half = (m + 1) / 2;
    kernel_padded[kernel_start..kernel_start + first_half].copy_from_slice(&kernel[m / 2..]);
    let second_half = m / 2;
    if second_half > 0 {
        kernel_padded[..second_half].copy_from_slice(&kernel[..second_half]);
    }

    // Forward FFT (CPU)
    let mut binned_spectrum = r2c.make_output_vec();
    r2c.process(&mut binned_padded, &mut binned_spectrum)
        .map_err(|e| PeacoQCError::StatsError(format!("FFT forward failed: {}", e)))?;

    let mut kernel_spectrum = r2c.make_output_vec();
    r2c.process(&mut kernel_padded, &mut kernel_spectrum)
        .map_err(|e| PeacoQCError::StatsError(format!("FFT forward failed: {}", e)))?;

    // Step 4: Multiply in frequency domain (GPU if available)
    // Batched operations provide speedup even for smaller datasets
    #[cfg(feature = "gpu")]
    let conv_spectrum = if is_gpu_available() {
        // Try cubeCL kernel first (if available), then fall back to burn tensors
        #[cfg(feature = "cubecl")]
        {
            if let Ok(result) = crate::gpu::kernels::multiply_spectra_cubecl(&binned_spectrum, &kernel_spectrum) {
                result
            } else {
                // Fall through to burn tensor implementation if cubeCL fails
                multiply_spectra_gpu(&binned_spectrum, &kernel_spectrum)?
            }
        }
        #[cfg(not(feature = "cubecl"))]
        {
            // Use GPU for multiplication (burn tensor operations)
            multiply_spectra_gpu(&binned_spectrum, &kernel_spectrum)?
        }
    } else {
        // CPU fallback
        binned_spectrum
            .iter()
            .zip(kernel_spectrum.iter())
            .map(|(a, b)| a * b)
            .collect()
    };
    
    #[cfg(not(feature = "gpu"))]
    let conv_spectrum = binned_spectrum
        .iter()
        .zip(kernel_spectrum.iter())
        .map(|(a, b)| a * b)
        .collect();

    // Step 5: Inverse FFT (CPU)
    let mut conv_result = c2r.make_output_vec();
    let mut conv_spectrum_mut = conv_spectrum;
    c2r.process(&mut conv_spectrum_mut, &mut conv_result)
        .map_err(|e| PeacoQCError::StatsError(format!("FFT inverse failed: {}", e)))?;

    // Step 6: Extract relevant portion and normalize
    let kernel_start = (fft_size - m) / 2;
    let mut density = Vec::with_capacity(m);
    for i in 0..m {
        let idx = (kernel_start + i) % fft_size;
        density.push(conv_result[idx]);
    }
    
    // Normalize
    let density: Vec<f64> = density
        .iter()
        .map(|&val| val / (fft_size as f64 * n * bandwidth))
        .collect();

    Ok(density)
}

/// Multiply two complex spectra on GPU
fn multiply_spectra_gpu(
    a: &[Complex<f64>],
    b: &[Complex<f64>],
) -> Result<Vec<Complex<f64>>> {
    let device = WgpuDevice::default();
    let n = a.len();

    // Convert complex arrays to real arrays (interleaved: [real, imag, real, imag, ...])
    let mut a_real = Vec::with_capacity(n * 2);
    let mut a_imag = Vec::with_capacity(n * 2);
    let mut b_real = Vec::with_capacity(n * 2);
    let mut b_imag = Vec::with_capacity(n * 2);

    for &c in a {
        a_real.push(c.re);
        a_imag.push(c.im);
    }
    for &c in b {
        b_real.push(c.re);
        b_imag.push(c.im);
    }

    // Create tensors - burn 0.20 API
    // Use TensorData and from_data
    use burn::tensor::TensorData;
    
    // Convert f64 to bytes for TensorData
    let a_re_bytes: Vec<u8> = a_real.iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let a_re_data = TensorData::new(a_re_bytes.into(), vec![n]);
    let a_re_tensor = Tensor::<Backend, 1, burn::tensor::Float>::from_data(a_re_data, &device);
    
    let a_im_bytes: Vec<u8> = a_imag.iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let a_im_data = TensorData::new(a_im_bytes.into(), vec![n]);
    let a_im_tensor = Tensor::<Backend, 1, burn::tensor::Float>::from_data(a_im_data, &device);
    
    let b_re_bytes: Vec<u8> = b_real.iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let b_re_data = TensorData::new(b_re_bytes.into(), vec![n]);
    let b_re_tensor = Tensor::<Backend, 1, burn::tensor::Float>::from_data(b_re_data, &device);
    
    let b_im_bytes: Vec<u8> = b_imag.iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let b_im_data = TensorData::new(b_im_bytes.into(), vec![n]);
    let b_im_tensor = Tensor::<Backend, 1, burn::tensor::Float>::from_data(b_im_data, &device);

    // Complex multiplication: (a_re + i*a_im) * (b_re + i*b_im)
    // = (a_re*b_re - a_im*b_im) + i*(a_re*b_im + a_im*b_re)
    let re_result = a_re_tensor.clone().mul(b_re_tensor.clone())
        .sub(a_im_tensor.clone().mul(b_im_tensor.clone()));
    let im_result = a_re_tensor.clone().mul(b_im_tensor.clone())
        .add(a_im_tensor.clone().mul(b_re_tensor.clone()));

    // Convert back to complex - burn 0.20 API
    let re_data = re_result.to_data();
    let im_data = im_result.to_data();
    let re_values: Vec<f64> = re_data.as_slice::<f64>().unwrap().to_vec();
    let im_values: Vec<f64> = im_data.as_slice::<f64>().unwrap().to_vec();

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        result.push(Complex::new(re_values[i], im_values[i]));
    }

    Ok(result)
}

/// Gaussian kernel function
#[inline]
fn gaussian_kernel(u: f64) -> f64 {
    const INV_SQRT_2PI: f64 = 0.3989422804014327; // 1/sqrt(2*pi)
    INV_SQRT_2PI * (-0.5 * u * u).exp()
}
