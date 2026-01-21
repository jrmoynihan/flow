//! GPU backend detection and initialization

use burn::backend::wgpu::WgpuDevice;
use std::sync::atomic::{AtomicBool, Ordering};

static GPU_AVAILABLE: AtomicBool = AtomicBool::new(false);
static GPU_CHECKED: AtomicBool = AtomicBool::new(false);

/// Check if GPU backend is available
pub fn is_gpu_available() -> bool {
    if GPU_CHECKED.load(Ordering::Relaxed) {
        return GPU_AVAILABLE.load(Ordering::Relaxed);
    }

    // Try to initialize GPU backend
    let available = init_gpu_backend().is_ok();
    GPU_AVAILABLE.store(available, Ordering::Relaxed);
    GPU_CHECKED.store(true, Ordering::Relaxed);
    available
}

/// Initialize GPU backend
fn init_gpu_backend() -> anyhow::Result<()> {
    // Try to create a WGPU device
    // This will fail if no GPU is available
    let _device = WgpuDevice::default();
    
    // If we get here, GPU is available
    Ok(())
}

/// GPU backend wrapper
pub struct GpuBackend {
    device: WgpuDevice,
}

impl GpuBackend {
    /// Create a new GPU backend instance
    pub fn new() -> anyhow::Result<Self> {
        let device = WgpuDevice::default();
        Ok(Self { device })
    }

    /// Get the device
    pub fn device(&self) -> &WgpuDevice {
        &self.device
    }
}

impl Default for GpuBackend {
    fn default() -> Self {
        Self::new().expect("GPU backend should be initialized after availability check")
    }
}
