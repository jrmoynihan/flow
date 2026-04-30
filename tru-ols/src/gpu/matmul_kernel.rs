//! cubeCL kernel: `out = observations @ mixing` with row-major packed matrices.
//!
//! `observations` is `n_row × k`, `mixing` is `k × n_col`, `out` is `n_row × n_col`.

use cubecl::prelude::*;

#[cube(launch)]
pub fn matmul_obs_times_mixing<F: Float>(
    observations: &Array<F>,
    mixing: &Array<F>,
    out: &mut Array<F>,
    n_row: usize,
    k: usize,
    n_col: usize,
) {
    let linear = ABSOLUTE_POS;
    let n_out = n_row * n_col;
    if linear < n_out {
        let i = linear / n_col;
        let j = linear % n_col;
        let mut acc = F::new(0.0);
        for t in 0..k {
            let a_idx = i * k + t;
            let b_idx = t * n_col + j;
            acc += observations[a_idx] * mixing[b_idx];
        }
        out[linear] = acc;
    }
}
