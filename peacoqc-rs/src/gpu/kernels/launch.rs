//! Kernel launch code for cubeCL kernels

use cubecl::prelude::*;
use cubecl::wgpu::WgpuRuntime;
use cubecl::bytes::Bytes;
use crate::error::Result;
use realfft::num_complex::Complex;

/// Launch complex multiplication kernel using cubeCL
#[cfg(feature = "cubecl")]
pub fn multiply_spectra_cubecl(
    a: &[Complex<f64>],
    b: &[Complex<f64>],
) -> Result<Vec<Complex<f64>>> {
    use crate::gpu::kernels::complex_multiply::complex_multiply_kernel;
    
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    
    // Create cubeCL runtime client
    let cubecl_device = cubecl::wgpu::WgpuDevice::default();
    let client = WgpuRuntime::client(&cubecl_device);
    
    // Extract real and imaginary parts, convert to f32
    // cubeCL works with f32, so we convert f64 -> f32 (may lose precision but faster)
    let mut a_re_f32 = Vec::with_capacity(n);
    let mut a_im_f32 = Vec::with_capacity(n);
    let mut b_re_f32 = Vec::with_capacity(n);
    let mut b_im_f32 = Vec::with_capacity(n);
    
    for &c in a {
        a_re_f32.push(c.re as f32);
        a_im_f32.push(c.im as f32);
    }
    for &c in b {
        b_re_f32.push(c.re as f32);
        b_im_f32.push(c.im as f32);
    }
    
    // Create GPU buffers using cubeCL API
    // Convert f32 slices to Bytes using f32::as_bytes helper
    let a_re_bytes = Bytes::from_bytes_vec(f32::as_bytes(&a_re_f32).to_vec());
    let a_im_bytes = Bytes::from_bytes_vec(f32::as_bytes(&a_im_f32).to_vec());
    let b_re_bytes = Bytes::from_bytes_vec(f32::as_bytes(&b_re_f32).to_vec());
    let b_im_bytes = Bytes::from_bytes_vec(f32::as_bytes(&b_im_f32).to_vec());
    
    // Upload to GPU and allocate output buffers
    let a_re_handle = client.create(a_re_bytes);
    let a_im_handle = client.create(a_im_bytes);
    let b_re_handle = client.create(b_re_bytes);
    let b_im_handle = client.create(b_im_bytes);
    let result_re_handle = client.empty(n * std::mem::size_of::<f32>());
    let result_im_handle = client.empty(n * std::mem::size_of::<f32>());
    
    // Launch kernel - each thread processes one complex multiplication
    unsafe {
        let _ = complex_multiply_kernel::launch::<f32, WgpuRuntime>(
            &client,
            CubeCount::Static(1, 1, 1),
            CubeDim::new_1d(n as u32),
            ArrayArg::from_raw_parts::<f32>(&a_re_handle, n, 1),
            ArrayArg::from_raw_parts::<f32>(&a_im_handle, n, 1),
            ArrayArg::from_raw_parts::<f32>(&b_re_handle, n, 1),
            ArrayArg::from_raw_parts::<f32>(&b_im_handle, n, 1),
            ArrayArg::from_raw_parts::<f32>(&result_re_handle, n, 1),
            ArrayArg::from_raw_parts::<f32>(&result_im_handle, n, 1),
        );
    }
    
    // Read results back from GPU
    let result_re_bytes = client.read_one(result_re_handle);
    let result_im_bytes = client.read_one(result_im_handle);
    
    // Convert bytes back to f32 using bytemuck
    let result_re_f32: &[f32] = bytemuck::cast_slice(&result_re_bytes);
    let result_im_f32: &[f32] = bytemuck::cast_slice(&result_im_bytes);
    
    // Convert back to f64 and Complex
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        result.push(Complex::new(result_re_f32[i] as f64, result_im_f32[i] as f64));
    }
    
    Ok(result)
}
