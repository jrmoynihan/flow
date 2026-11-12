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

use burn::backend::wgpu::Wgpu;
use burn::tensor::{Device as BurnDevice, Tensor};
use cubecl::client::Client;
use std::sync::OnceLock;

/// Shared device for repeated PaCMAP optimize launches.
pub struct GpuWgpuContext {
    /// Burn device handle (for tensor construction; Burn 0.22 API).
    burn_device: BurnDevice,
    /// cubeCL client for the same runtime as tensors on [`burn_device`].
    client: Client,
}

impl GpuWgpuContext {
    fn try_new() -> Result<Self, String> {
        let burn_device = BurnDevice::default();
        let smoke = Tensor::<1>::zeros([1], &burn_device);
        let client = smoke
            .try_into_primitive::<Wgpu>()
            .map_err(|e| format!("Wgpu smoke tensor: {e:?}"))?
            .client
            .clone();
        Ok(Self { burn_device, client })
    }

    pub fn device(&self) -> &BurnDevice {
        &self.burn_device
    }

    pub fn client(&self) -> Client {
        self.client.clone()
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
                let ctx = GpuWgpuContext::try_new()?;
                let client = ctx.client();
                let handle = client.empty(4);
                let _ = client.read_one(handle);
                Ok(ctx)
            }));
            std::panic::set_hook(prev);
            match result {
                Ok(inner) => inner,
                Err(payload) => Err(if let Some(s) = payload.downcast_ref::<&str>() {
                    format!("WGPU adapter init panicked: {s}")
                } else if let Some(s) = payload.downcast_ref::<String>() {
                    format!("WGPU adapter init panicked: {s}")
                } else {
                    "WGPU adapter init panicked".into()
                }),
            }
        })
        .as_ref()
        .map_err(|s| s.clone())
}
