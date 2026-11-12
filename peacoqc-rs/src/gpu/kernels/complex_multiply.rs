//! GPU kernel for complex spectrum multiplication using cubeCL
//!
//! Multiplies two complex arrays element-wise:
//! (a_re + i*a_im) * (b_re + i*b_im) = (a_re*b_re - a_im*b_im) + i*(a_re*b_im + a_im*b_re)

use cubecl::prelude::*;

/// Complex multiplication kernel
///
/// Each GPU thread processes one complex multiplication
#[cube(launch)]
pub fn complex_multiply_kernel<F: Float>(
    a_re: &Tensor<F>,
    a_im: &Tensor<F>,
    b_re: &Tensor<F>,
    b_im: &Tensor<F>,
    result_re: &mut Tensor<F>,
    result_im: &mut Tensor<F>,
    n: u32,
) {
    let idx = ABSOLUTE_POS;
    let n_usz = n as usize;
    if idx < n_usz {
        let a_re_val = a_re[idx];
        let a_im_val = a_im[idx];
        let b_re_val = b_re[idx];
        let b_im_val = b_im[idx];

        result_re[idx] = a_re_val * b_re_val - a_im_val * b_im_val;
        result_im[idx] = a_re_val * b_im_val + a_im_val * b_re_val;
    }
}
