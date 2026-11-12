//! Host launch for nearest-centroid assignment.

use super::kernel::assign_nearest_centroid;
use crate::clustering::ClusteringError;
use cubecl::bytes::Bytes;
use cubecl::calculate_cube_count_elemwise;
use cubecl::prelude::*;
use cubecl::wgpu::WgpuRuntime;

/// Assign each of `n` points to the nearest of `k` centroids on the GPU.
pub fn launch_assign_f32(
    client: &ComputeClient<WgpuRuntime>,
    points: &[f32],
    n: usize,
    d: usize,
    centroids: &[f32],
    k: usize,
    out: &mut [u32],
) -> Result<(), ClusteringError> {
    let expected_pts = n
        .checked_mul(d)
        .ok_or_else(|| ClusteringError::InvalidConfig("n·d overflow".into()))?;
    let expected_c = k
        .checked_mul(d)
        .ok_or_else(|| ClusteringError::InvalidConfig("k·d overflow".into()))?;
    if points.len() != expected_pts || centroids.len() != expected_c || out.len() != n {
        return Err(ClusteringError::InvalidConfig(
            "assign buffer length mismatch".into(),
        ));
    }
    if n == 0 {
        return Ok(());
    }

    let pts = Bytes::from_bytes_vec(bytemuck::cast_slice(points).to_vec());
    let cents = Bytes::from_bytes_vec(bytemuck::cast_slice(centroids).to_vec());
    let pts_h = client.create(pts);
    let cents_h = client.create(cents);
    let out_h = client.empty(n * core::mem::size_of::<u32>());

    let cube_dim = CubeDim::new_1d(256);
    let cube_count = calculate_cube_count_elemwise(client, n, cube_dim);

    unsafe {
        assign_nearest_centroid::launch::<WgpuRuntime>(
            client,
            cube_count,
            cube_dim,
            ArrayArg::from_raw_parts(pts_h.clone(), expected_pts),
            ArrayArg::from_raw_parts(cents_h.clone(), expected_c),
            ArrayArg::from_raw_parts(out_h.clone(), n),
            n as u32,
            d as u32,
            k as u32,
        );
    }

    let out_bytes = client
        .read_one(out_h)
        .map_err(|e| ClusteringError::GpuUnavailable(format!("cubeCL read failed: {e}")))?;
    let slice: &[u32] = bytemuck::cast_slice(&*out_bytes);
    out.copy_from_slice(slice);
    Ok(())
}
