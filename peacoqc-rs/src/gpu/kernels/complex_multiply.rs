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
    a_re: &Array<Line<F>>,      // Real part of first complex array
    a_im: &Array<Line<F>>,       // Imaginary part of first complex array
    b_re: &Array<Line<F>>,      // Real part of second complex array
    b_im: &Array<Line<F>>,       // Imaginary part of second complex array
    result_re: &mut Array<Line<F>>,  // Output: real part
    result_im: &mut Array<Line<F>>,   // Output: imaginary part
) {
    let idx = ABSOLUTE_POS;
    
    // Bounds check
    if idx < a_re.len() {
        let a_re_val = a_re[idx];
        let a_im_val = a_im[idx];
        let b_re_val = b_re[idx];
        let b_im_val = b_im[idx];
        
        // Complex multiplication: (a_re + i*a_im) * (b_re + i*b_im)
        // = (a_re*b_re - a_im*b_im) + i*(a_re*b_im + a_im*b_re)
        result_re[idx] = a_re_val * b_re_val - a_im_val * b_im_val;
        result_im[idx] = a_re_val * b_im_val + a_im_val * b_re_val;
    }
}
