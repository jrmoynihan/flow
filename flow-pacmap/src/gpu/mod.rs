//! Optional Burn + cubeCL / WGPU path for PaCMAP optimize.
//!
//! Enable with the `cubecl` crate feature. Uses wgpu (Metal / Vulkan / DX12 /
//! CUDA via wgpu). Requires a working WGPU adapter.
//!
//! Strategy (Apple-friendly, zero-copy):
//! - Device / Adam: Burn (`Wgpu` backend + `burn::optim::Adam`)
//! - Pair gradients: raw cubeCL CSR kernel on the same Burn tensor `Handle`s
//!   (one thread per node, no float atomics)
//! - Host sync only when downloading the final embedding


mod csr;
mod kernels;
mod optimize;

pub use optimize::{GpuOptimizeContext, gpu_context_available, optimize_embedding_gpu};

use burn::backend::wgpu::WgpuDevice;
use cubecl::client::ComputeClient;
use cubecl::prelude::Runtime;
use cubecl::wgpu::WgpuRuntime;
use std::sync::OnceLock;

/// Shared WGPU device for repeated PaCMAP optimize launches.
pub struct GpuWgpuContext {
    device: WgpuDevice,
}

impl GpuWgpuContext {
    pub fn new() -> Self {
        Self {
            device: WgpuDevice::default(),
        }
    }

    pub fn device(&self) -> &WgpuDevice {
        &self.device
    }

    pub fn client(&self) -> ComputeClient<WgpuRuntime> {
        WgpuRuntime::client(&self.device)
    }
}

static GPU_TRY_INIT: OnceLock<Result<GpuWgpuContext, String>> = OnceLock::new();

/// Lazily constructs a shared context after a tiny smoke launch.
///
/// Returns `Err` when no WGPU adapter is available (headless CI / sandbox).
/// CubeCL's default device path can panic on adapter lookup — that is caught here.
pub fn try_shared_gpu_context() -> Result<&'static GpuWgpuContext, String> {
    GPU_TRY_INIT
        .get_or_init(|| {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let ctx = GpuWgpuContext::new();
                let client = ctx.client();
                let handle = client.empty(4);
                let _ = client.read_one(handle);
                ctx
            }));
            std::panic::set_hook(prev);
            result.map_err(|payload| {
                if let Some(s) = payload.downcast_ref::<&str>() {
                    format!("WGPU adapter init panicked: {s}")
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    format!("WGPU adapter init panicked: {s}")
                } else {
                    "WGPU adapter init panicked".into()
                }
            })
        })
        .as_ref()
        .map_err(|s| s.clone())
}
