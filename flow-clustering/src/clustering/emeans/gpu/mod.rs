//! cubeCL assignment backend for e-means / Lloyd.
//!
//! Written against the workspace cubeCL pin (0.10 today; migrate with the
//! workspace 0.11 upgrade). Only the assignment kernel runs on the GPU;
//! EMA, revival and finalisation stay on the CPU.

mod kernel;
mod launch;

use std::sync::OnceLock;

use cubecl::client::ComputeClient;
use cubecl::prelude::Runtime;
use cubecl::wgpu::{WgpuDevice, WgpuRuntime};

use crate::clustering::{ClusteringError, ClusteringResult};

use super::assign::Assign;
use launch::launch_assign_f32;

struct GpuCtx {
    device: WgpuDevice,
}

impl GpuCtx {
    fn client(&self) -> ComputeClient<WgpuRuntime> {
        WgpuRuntime::client(&self.device)
    }
}

static GPU_TRY_INIT: OnceLock<Result<GpuCtx, String>> = OnceLock::new();

fn try_shared_gpu() -> ClusteringResult<&'static GpuCtx> {
    let res = GPU_TRY_INIT.get_or_init(|| {
        let ctx = GpuCtx {
            device: WgpuDevice::default(),
        };
        let client = ctx.client();
        // Smoke: one point, one centroid at the origin → assignment 0.
        let points = [0.0f32, 0.0];
        let centroids = [0.0f32, 0.0];
        let mut out = [u32::MAX];
        match launch_assign_f32(&client, &points, 1, 2, &centroids, 1, &mut out) {
            Ok(()) if out[0] == 0 => Ok(ctx),
            Ok(()) => Err(format!("GPU assign smoke test: unexpected {}", out[0])),
            Err(e) => Err(format!("{e}")),
        }
    });
    res.as_ref()
        .map_err(|s| ClusteringError::GpuUnavailable(s.clone()))
}

/// [`Assign`] implementation that launches the cubeCL kernel per call.
pub struct GpuAssign;

impl GpuAssign {
    pub fn try_new() -> ClusteringResult<Self> {
        let _ = try_shared_gpu()?;
        Ok(Self)
    }
}

impl Assign for GpuAssign {
    fn assign(
        &self,
        points: &[f32],
        n: usize,
        d: usize,
        centroids: &[f32],
        k: usize,
        out: &mut [u32],
    ) -> ClusteringResult<()> {
        let ctx = try_shared_gpu()?;
        let client = ctx.client();
        launch_assign_f32(&client, points, n, d, centroids, k, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clustering::emeans::assign::CpuLoopAssign;

    #[test]
    #[ignore = "requires WGPU adapter"]
    fn gpu_matches_cpu_on_small() {
        let points = vec![0.0f32, 0.0, 10.0, 10.0, 0.1, 0.0, 9.9, 10.1];
        let centroids = vec![0.0f32, 0.0, 10.0, 10.0];
        let mut cpu = vec![0u32; 4];
        let mut gpu = vec![0u32; 4];
        CpuLoopAssign
            .assign(&points, 4, 2, &centroids, 2, &mut cpu)
            .unwrap();
        let g = GpuAssign::try_new().expect("gpu");
        g.assign(&points, 4, 2, &centroids, 2, &mut gpu)
            .expect("assign");
        assert_eq!(cpu, gpu);
    }
}
