//! Host-side GPU optimize loop: cubeCL CSR gradients + Burn Adam (zero-copy).
//!
//! Embedding and gradient live as Burn `Wgpu` tensors whose storage is a cubeCL
//! `Handle`. The CSR kernel launches on those same handles; Adam updates stay on
//! device. Host sync happens only on final download.

use super::csr::PacmapCsr;
use super::kernels::pacmap_grad_accum;
use super::{GpuWgpuContext, try_shared_gpu_context};
use crate::error::PaCMAPError;
use crate::weights::{Weights, weights_at};
use burn::backend::wgpu::{CubeTensor, Wgpu};
use burn::optim::{Adam, AdamConfig, AdamState, SimpleOptimizer};
use burn::tensor::{Tensor, TensorData};
use cubecl::bytes::Bytes;
use cubecl::calculate_cube_count_elemwise;
use cubecl::prelude::*;
use cubecl::server::Handle;
use cubecl::wgpu::WgpuRuntime;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

type Backend = Wgpu<f32, i32>;

fn as_cube(tensor: Tensor<Backend, 2>) -> CubeTensor<WgpuRuntime> {
    tensor.into_primitive().tensor()
}

/// Reusable GPU buffers for a PaCMAP optimize run.
pub struct GpuOptimizeContext {
    #[allow(dead_code)]
    wgpu: &'static GpuWgpuContext,
    client: ComputeClient<WgpuRuntime>,
    adam: Adam,
    adam_state: Option<AdamState<Backend, 2>>,
    embd_tensor: Tensor<Backend, 2>,
    grad_tensor: Tensor<Backend, 2>,
    n: usize,
    n2: usize,
    offsets: Handle,
    others: Handle,
    kinds: Handle,
    n_edges: usize,
}

impl GpuOptimizeContext {
    /// Upload embedding + CSR; allocate device-resident grad for CSR writes.
    pub fn new(
        embedding: &[[f32; 2]],
        near: &[[u32; 2]],
        mid_near: &[[u32; 2]],
        further: &[[u32; 2]],
    ) -> Result<Self, PaCMAPError> {
        let wgpu = try_shared_gpu_context()
            .map_err(|e| PaCMAPError::Gpu(format!("WGPU adapter unavailable: {e}")))?;
        let client = wgpu.client();
        let device = wgpu.device().clone();
        let n = embedding.len();
        let n2 = n * 2;
        let csr = PacmapCsr::from_pair_lists(n, near, mid_near, further);

        let flat: Vec<f32> = embedding.iter().flat_map(|p| [p[0], p[1]]).collect();
        let embd_tensor =
            Tensor::<Backend, 2>::from_data(TensorData::new(flat, [n, 2]), &device);
        // Grad buffer stays on device for the whole run (CSR writes; Adam reads).
        let grad_tensor = Tensor::<Backend, 2>::zeros([n, 2], &device);

        let offsets = client.create(Bytes::from_bytes_vec(
            bytemuck::cast_slice(&csr.offsets).to_vec(),
        ));
        let others = client.create(Bytes::from_bytes_vec(
            bytemuck::cast_slice(&csr.others).to_vec(),
        ));
        let kinds = client.create(Bytes::from_bytes_vec(
            bytemuck::cast_slice(&csr.kinds).to_vec(),
        ));

        // Match CPU PaCMAP Adam (epsilon 1e-7); Burn's default is 1e-5.
        let adam = AdamConfig::new()
            .with_beta_1(0.9)
            .with_beta_2(0.999)
            .with_epsilon(1e-7)
            .build();

        Ok(Self {
            wgpu,
            client,
            adam,
            adam_state: None,
            embd_tensor,
            grad_tensor,
            n,
            n2,
            offsets,
            others,
            kinds,
            n_edges: csr.others.len(),
        })
    }

    fn launch_grad(
        &self,
        embd: &CubeTensor<WgpuRuntime>,
        grad: &CubeTensor<WgpuRuntime>,
        w: Weights,
    ) -> Result<(), PaCMAPError> {
        let cube_dim = CubeDim::new_1d(256);
        let cube_count = calculate_cube_count_elemwise(&self.client, self.n, cube_dim);
        let n_off = self.n + 1;
        let n_edges = self.n_edges.max(1);
        unsafe {
            pacmap_grad_accum::launch::<WgpuRuntime>(
                &self.client,
                cube_count,
                cube_dim,
                ArrayArg::from_raw_parts(embd.handle.clone(), self.n2),
                ArrayArg::from_raw_parts(self.offsets.clone(), n_off),
                ArrayArg::from_raw_parts(self.others.clone(), n_edges),
                ArrayArg::from_raw_parts(self.kinds.clone(), n_edges),
                ArrayArg::from_raw_parts(grad.handle.clone(), self.n2),
                self.n as u32,
                w.w_nb,
                w.w_mn,
                w.w_fp,
            );
        }
        Ok(())
    }

    /// One CSR grad + Burn Adam step with no host round-trip.
    fn step(&mut self, w: Weights, lr: f32) -> Result<(), PaCMAPError> {
        // Borrow cubeCL handles from Burn storage, then drop wrappers so Adam
        // can take exclusive ownership of the tensors.
        {
            let embd_c = as_cube(self.embd_tensor.clone());
            let grad_c = as_cube(self.grad_tensor.clone());
            self.launch_grad(&embd_c, &grad_c, w)?;
        }

        let embd = self.embd_tensor.clone();
        let grad = self.grad_tensor.clone();
        let (new_embd, new_state) =
            self.adam
                .step(f64::from(lr), embd, grad, self.adam_state.take());
        self.embd_tensor = new_embd;
        self.adam_state = new_state;
        Ok(())
    }

    /// Download embedding into `out` (`len == n`). Only host sync of the run.
    pub fn download_embedding(&self, out: &mut [[f32; 2]]) -> Result<(), PaCMAPError> {
        if out.len() != self.n {
            return Err(PaCMAPError::Gpu(format!(
                "download size mismatch: out={}, n={}",
                out.len(),
                self.n
            )));
        }
        let data = self.embd_tensor.to_data();
        let flat = data
            .as_slice::<f32>()
            .map_err(|e| PaCMAPError::Gpu(format!("embd download: {e}")))?;
        if flat.len() != self.n2 {
            return Err(PaCMAPError::Gpu(format!(
                "unexpected embd tensor len {}",
                flat.len()
            )));
        }
        for (i, p) in out.iter_mut().enumerate() {
            *p = [flat[2 * i], flat[2 * i + 1]];
        }
        Ok(())
    }
}

/// Whether a shared GPU context initialized successfully in this process.
pub fn gpu_context_available() -> bool {
    try_shared_gpu_context().is_ok()
}

/// Run the full Adam / pair-gradient optimize loop on the GPU.
///
/// CSR pair gradients use raw cubeCL on Burn tensor storage; Adam uses Burn on
/// the same device buffers (zero-copy). Loss is not computed on GPU. Embedding
/// is updated in place at the end.
pub fn optimize_embedding_gpu(
    embedding: &mut [[f32; 2]],
    near: &[[u32; 2]],
    mid_near: &[[u32; 2]],
    further: &[[u32; 2]],
    phase_iters: &[usize; 3],
    learning_rate: f32,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(), PaCMAPError> {
    let mut ctx = GpuOptimizeContext::new(embedding, near, mid_near, further)?;
    let mut global_iter = 0usize;

    for &phase_len in phase_iters {
        for _ in 0..phase_len {
            if let Some(ref cancel) = cancel
                && cancel.load(Ordering::Relaxed)
            {
                return Err(PaCMAPError::Cancelled);
            }
            global_iter += 1;
            let w = weights_at(global_iter, phase_iters);
            ctx.step(w, learning_rate)?;
        }
    }

    ctx.download_embedding(embedding)
}
