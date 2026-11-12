//! cubeCL nearest-centroid assignment: one invocation per point.

use cubecl::prelude::*;

/// For each point `i`, write `out[i] = argmin_j ‖x_i − c_j‖²`.
///
/// `points` is row-major `n × d`, `centroids` is row-major `k × d`, `out` is length `n`.
#[cube(launch)]
pub fn assign_nearest_centroid(
    points: &Array<f32>,
    centroids: &Array<f32>,
    out: &mut Array<u32>,
    n: u32,
    d: u32,
    k: u32,
) {
    let i = ABSOLUTE_POS;
    let n_usz = n as usize;
    if i >= n_usz {
        terminate!();
    }

    let d_usz = d as usize;
    let k_usz = k as usize;
    let base = i * d_usz;
    let mut best_j = 0u32;
    let mut best_dist = 1.0e30f32;
    let mut j = 0usize;
    while j < k_usz {
        let cbase = j * d_usz;
        let mut dist = 0.0f32;
        let mut t = 0usize;
        while t < d_usz {
            let diff = points[base + t] - centroids[cbase + t];
            dist += diff * diff;
            t += 1;
        }
        if dist < best_dist {
            best_dist = dist;
            best_j = j as u32;
        }
        j += 1;
    }
    out[i] = best_j;
}
