//! Kernel launch code for cubeCL kernels

use crate::error::Result;
use cubecl::Device;
use cubecl::bytes::Bytes;
use cubecl::prelude::*;
use cubecl::zspace::{Shape, Strides};
use realfft::num_complex::Complex;

unsafe fn tensor_arg_1d(handle: cubecl::server::Handle, len: usize) -> TensorArg {
    unsafe { TensorArg::from_raw_parts(handle, Strides::new(&[1]), Shape::new([len])) }
}

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

    let client = Device::default().client();

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

    let a_re_bytes = Bytes::from_bytes_vec(bytemuck::cast_slice(&a_re_f32).to_vec());
    let a_im_bytes = Bytes::from_bytes_vec(bytemuck::cast_slice(&a_im_f32).to_vec());
    let b_re_bytes = Bytes::from_bytes_vec(bytemuck::cast_slice(&b_re_f32).to_vec());
    let b_im_bytes = Bytes::from_bytes_vec(bytemuck::cast_slice(&b_im_f32).to_vec());

    let a_re_handle = client.create(a_re_bytes);
    let a_im_handle = client.create(a_im_bytes);
    let b_re_handle = client.create(b_re_bytes);
    let b_im_handle = client.create(b_im_bytes);
    let result_re_handle = client.empty(n * std::mem::size_of::<f32>());
    let result_im_handle = client.empty(n * std::mem::size_of::<f32>());

    unsafe {
        complex_multiply_kernel::launch::<f32>(
            &client,
            CubeCount::Static(1, 1, 1),
            CubeDim::new_1d(n as u32),
            tensor_arg_1d(a_re_handle.clone(), n),
            tensor_arg_1d(a_im_handle.clone(), n),
            tensor_arg_1d(b_re_handle.clone(), n),
            tensor_arg_1d(b_im_handle.clone(), n),
            tensor_arg_1d(result_re_handle.clone(), n),
            tensor_arg_1d(result_im_handle.clone(), n),
            n as u32,
        );
    }

    let result_re_bytes = client
        .read_one(result_re_handle)
        .map_err(|e| crate::error::PeacoQCError::StatsError(format!("cubeCL read: {e}")))?;
    let result_im_bytes = client
        .read_one(result_im_handle)
        .map_err(|e| crate::error::PeacoQCError::StatsError(format!("cubeCL read: {e}")))?;

    let result_re_f32: &[f32] = bytemuck::cast_slice(&*result_re_bytes);
    let result_im_f32: &[f32] = bytemuck::cast_slice(&*result_im_bytes);

    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        result.push(Complex::new(
            result_re_f32[i] as f64,
            result_im_f32[i] as f64,
        ));
    }

    Ok(result)
}
