//! Host launch for [`super::matmul_kernel`].

use super::matmul_kernel::matmul_obs_times_mixing;
use crate::error::TruOlsError;
use cubecl::bytes::Bytes;
use cubecl::calculate_cube_count_elemwise;
use cubecl::prelude::*;
use cubecl::wgpu::WgpuRuntime;

/// Row-major GEMM: `out = observations @ mixing` with `observations` (`n_row × k`) and `mixing` (`k × n_col`).
///
/// All buffers are `f32`. `out` length must be `n_row * n_col`.
pub fn launch_obs_times_mixing_f32(
    client: &ComputeClient<WgpuRuntime>,
    observations: &[f32],
    mixing: &[f32],
    out: &mut [f32],
    n_row: usize,
    k: usize,
    n_col: usize,
) -> Result<(), TruOlsError> {
    let expected_obs = n_row
        .checked_mul(k)
        .ok_or_else(|| TruOlsError::LinearAlgebra("GEMM dimension overflow".into()))?;
    let expected_mix = k
        .checked_mul(n_col)
        .ok_or_else(|| TruOlsError::LinearAlgebra("GEMM dimension overflow".into()))?;
    let expected_out = n_row
        .checked_mul(n_col)
        .ok_or_else(|| TruOlsError::LinearAlgebra("GEMM dimension overflow".into()))?;

    if observations.len() != expected_obs
        || mixing.len() != expected_mix
        || out.len() != expected_out
    {
        return Err(TruOlsError::DimensionMismatch {
            expected: expected_obs,
            actual: observations.len(),
        });
    }

    let obs_bytes = Bytes::from_bytes_vec(bytemuck::cast_slice(observations).to_vec());
    let mix_bytes = Bytes::from_bytes_vec(bytemuck::cast_slice(mixing).to_vec());
    let obs_handle = client.create(obs_bytes);
    let mix_handle = client.create(mix_bytes);
    let out_handle = client.empty(expected_out * core::mem::size_of::<f32>());

    let cube_dim = CubeDim::new_1d(256);
    let cube_count = calculate_cube_count_elemwise(client, expected_out, cube_dim);

    unsafe {
        matmul_obs_times_mixing::launch::<f32, WgpuRuntime>(
            client,
            cube_count,
            cube_dim,
            ArrayArg::from_raw_parts(obs_handle.clone(), expected_obs),
            ArrayArg::from_raw_parts(mix_handle.clone(), expected_mix),
            ArrayArg::from_raw_parts(out_handle.clone(), expected_out),
            n_row,
            k,
            n_col,
        );
    }

    let out_bytes = client
        .read_one(out_handle)
        .map_err(|e| TruOlsError::LinearAlgebra(format!("cubeCL read failed: {e}")))?;
    let slice: &[f32] = bytemuck::cast_slice(&*out_bytes);
    out.copy_from_slice(slice);
    Ok(())
}
