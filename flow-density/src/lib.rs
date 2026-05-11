//! FFT-accelerated kernel density estimation for flow cytometry.
pub mod common;
pub mod kde;

pub use kde::{KdeError, KdeResult, KernelDensity, KernelDensity2D};
#[cfg(feature = "gpu")]
pub use kde::kde_fft_gpu;
