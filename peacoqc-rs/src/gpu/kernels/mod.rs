//! Custom cubeCL kernels for GPU-accelerated operations

#[cfg(feature = "cubecl")]
mod complex_multiply;
#[cfg(feature = "cubecl")]
mod launch;

#[cfg(feature = "cubecl")]
pub use launch::multiply_spectra_cubecl;
