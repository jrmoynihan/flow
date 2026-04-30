//! Optional cubeCL / WGPU helpers for batched linear algebra used in OLS-style workloads.
//!
//! Enable with the `cubecl` crate feature. Requires a working WGPU adapter (may fail in headless CI).

mod launch;
mod matmul_kernel;

pub use launch::launch_obs_times_mixing_f32;

use crate::error::TruOlsError;
use cubecl::client::ComputeClient;
use cubecl::prelude::Runtime;
use cubecl::wgpu::{WgpuDevice, WgpuRuntime};
use faer::linalg::solvers::Llt;
use faer::prelude::*;
use faer::{Mat, MatRef, Side};
use std::sync::OnceLock;

/// Holds a WGPU device for repeated cubeCL launches.
pub struct GpuWgpuContext {
    device: WgpuDevice,
}

impl GpuWgpuContext {
    pub fn new() -> Self {
        Self {
            device: WgpuDevice::default(),
        }
    }

    pub fn client(&self) -> ComputeClient<WgpuRuntime> {
        WgpuRuntime::client(&self.device)
    }
}

static GPU_TRY_INIT: OnceLock<Result<GpuWgpuContext, String>> = OnceLock::new();

/// Lazily constructs a shared [`GpuWgpuContext`] after a 1×1 GEMM smoke test on the GPU.
pub fn try_shared_gpu_context() -> Result<&'static GpuWgpuContext, TruOlsError> {
    let res = GPU_TRY_INIT.get_or_init(|| {
        let ctx = GpuWgpuContext::new();
        let client = ctx.client();
        let mut out = [0.0f32];
        match launch_obs_times_mixing_f32(&client, &[2.0], &[3.0], &mut out, 1, 1, 1) {
            Ok(()) if (out[0] - 6.0).abs() < 0.01 => Ok(ctx),
            Ok(()) => Err("GPU GEMM smoke test: unexpected result".into()),
            Err(e) => Err(format!("{e}")),
        }
    });
    res.as_ref()
        .map_err(|s| TruOlsError::LinearAlgebra(format!("GPU unavailable: {s}")))
}

/// OLS via normal equations with the `observations @ mixing` RHS block computed in `f32` on the GPU.
///
/// Cholesky of \(M^\top M\) and triangular solves stay on the CPU in `f64` for stability.
/// Numerically matches [`crate::run_ols_normal_equations`] only within `f32` contraction error; compare
/// abundances to [`crate::benchmark::run_ols`] with a tolerance when validating.
pub fn run_ols_normal_equations_gpu_rhs(
    observations: MatRef<'_, f64>,
    mixing_matrix: MatRef<'_, f64>,
    gpu: &GpuWgpuContext,
) -> Result<Mat<f64>, TruOlsError> {
    let n_events = observations.nrows();
    let n_det = observations.ncols();
    let n_em = mixing_matrix.ncols();

    if mixing_matrix.nrows() != n_det {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_det,
            actual: mixing_matrix.nrows(),
        });
    }
    if n_det < n_em {
        return Err(TruOlsError::LinearAlgebra(
            "Underdetermined systems are not supported".into(),
        ));
    }

    let obs_f32: Vec<f32> = (0..n_events)
        .flat_map(|e| (0..n_det).map(move |d| observations[(e, d)] as f32))
        .collect();
    let mix_f32: Vec<f32> = (0..n_det)
        .flat_map(|d| (0..n_em).map(move |j| mixing_matrix[(d, j)] as f32))
        .collect();
    let mut rhs_f32 = vec![0.0f32; n_events * n_em];

    let client = gpu.client();
    launch_obs_times_mixing_f32(
        &client,
        &obs_f32,
        &mix_f32,
        &mut rhs_f32,
        n_events,
        n_det,
        n_em,
    )?;

    let mt = mixing_matrix.transpose().to_owned();
    let gram: Mat<f64> = &mt * mixing_matrix;
    let llt = Llt::new(gram.as_ref(), Side::Lower).map_err(|e| {
        TruOlsError::LinearAlgebra(format!(
            "Cholesky of Gram matrix failed (matrix may be rank-deficient or ill-conditioned): {e}"
        ))
    })?;

    let mut result = Mat::zeros(n_events, n_em);
    if crate::use_parallel_independent_events(n_events) {
        use rayon::prelude::*;
        let rows: Vec<(usize, Vec<f64>)> = (0..n_events)
            .into_par_iter()
            .map(|ev| {
                let rhs_mat = Mat::from_fn(n_em, 1, |i, _| rhs_f32[ev * n_em + i] as f64);
                let x = llt.solve(rhs_mat.as_ref());
                let row: Vec<f64> = (0..n_em).map(|j| x[(j, 0)]).collect();
                (ev, row)
            })
            .collect();
        let mut sorted = rows;
        sorted.sort_by_key(|(ev, _)| *ev);
        for (ev, row) in sorted {
            for j in 0..n_em {
                result[(ev, j)] = row[j];
            }
        }
    } else {
        for ev in 0..n_events {
            let rhs_mat = Mat::from_fn(n_em, 1, |i, _| rhs_f32[ev * n_em + i] as f64);
            let x = llt.solve(rhs_mat.as_ref());
            for j in 0..n_em {
                result[(ev, j)] = x[(j, 0)];
            }
        }
    }

    Ok(result)
}

/// Returns true if [`try_shared_gpu_context`] succeeded at least once in this process.
pub fn gpu_context_available() -> bool {
    try_shared_gpu_context().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmark::run_ols;
    use crate::run_ols_normal_equations;
    use faer::mat;

    #[test]
    #[ignore = "requires WGPU adapter"]
    fn gpu_rhs_matches_cpu_normal_equations_tight_panel() {
        let mixing = mat![[1.0, 0.2, 0.0], [0.0, 1.0, 0.2], [0.0, 0.0, 1.0]];
        let observations = Mat::from_fn(200, 3, |ev, det| (ev + det * 7) as f64 * 0.01 + 1.0);
        let ctx = GpuWgpuContext::new();
        let cpu = run_ols_normal_equations(observations.as_ref(), mixing.as_ref()).unwrap();
        let gpu_m =
            run_ols_normal_equations_gpu_rhs(observations.as_ref(), mixing.as_ref(), &ctx).unwrap();
        let reference = run_ols(observations.as_ref(), mixing.as_ref()).unwrap();
        for i in 0..cpu.nrows() {
            for j in 0..cpu.ncols() {
                let r = reference[(i, j)];
                let g = gpu_m[(i, j)];
                assert!(
                    (r - g).abs() < 5e-4,
                    "ref vs gpu mismatch at ({i},{j}): {r} vs {g}"
                );
            }
        }
    }
}
