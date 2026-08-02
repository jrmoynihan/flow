//! cubeCL kernel for PaCMAP CSR pair-gradient accumulation.
//!
//! Adam runs on Burn (`burn::optim::Adam`); only the irregular sparse gradient
//! stays as a hand-written cubeCL launch.

use cubecl::prelude::*;

/// One thread per node: walk CSR edges, write `grad[node*2 .. node*2+2]`.
///
/// Pair kinds: 0 = near (attractive, C=10), 1 = mid-near (C=10000), 2 = far (repulsive).
#[cube(launch)]
pub fn pacmap_grad_accum(
    embd: &Array<f32>,
    offsets: &Array<u32>,
    others: &Array<u32>,
    kinds: &Array<u32>,
    grad: &mut Array<f32>,
    n: u32,
    w_nb: f32,
    w_mn: f32,
    w_fp: f32,
) {
    let node = ABSOLUTE_POS;
    let n_usz = n as usize;
    if node >= n_usz {
        terminate!();
    }

    let base = node * 2;
    let yi0 = embd[base];
    let yi1 = embd[base + 1];

    let mut g0 = 0.0f32;
    let mut g1 = 0.0f32;

    let start = offsets[node];
    let end = offsets[node + 1];
    let mut pos = start;
    while pos < end {
        let other = others[pos as usize] as usize;
        let kind = kinds[pos as usize];
        let oj = other * 2;
        let yj0 = embd[oj];
        let yj1 = embd[oj + 1];
        let dx = yi0 - yj0;
        let dy = yi1 - yj1;
        let d_sq = dx * dx + dy * dy;
        let d_tilde = d_sq + 1.0f32;

        if kind == 2 {
            let denom_sq = d_tilde * d_tilde;
            let g = w_fp * 2.0f32 / denom_sq;
            g0 -= g * dx;
            g1 -= g * dy;
        } else if kind == 0 {
            let c = 10.0f32;
            let c_plus_d = c + d_tilde;
            let g = w_nb * (2.0f32 * c) / (c_plus_d * c_plus_d);
            g0 += g * dx;
            g1 += g * dy;
        } else {
            let c = 10000.0f32;
            let c_plus_d = c + d_tilde;
            let g = w_mn * (2.0f32 * c) / (c_plus_d * c_plus_d);
            g0 += g * dx;
            g1 += g * dy;
        }
        pos += 1;
    }

    grad[base] = g0;
    grad[base + 1] = g1;
}
