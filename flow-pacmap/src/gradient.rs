//! PaCMAP loss gradient computation for near-neighbour, mid-near, and further pairs.
//!
//! Loss terms (where d̃_ab = ‖ya − yb‖² + 1):
//!   L_NB  = d̃_ij / (10   + d̃_ij)     attractive — near neighbours
//!   L_MN  = d̃_ik / (10000 + d̃_ik)    attractive — mid-near pairs
//!   L_FP  = 1     / (1    + d̃_il)     repulsive  — further pairs
//!
//! Gradients are accumulated per point into a pre-allocated buffer.

use crate::weights::Weights;
use rayon::prelude::*;

/// Accumulate gradients from one pair list into `grad`.
///
/// `pairs` is a flat `&[[u32; 2]]`.
/// `w` is the weight for this pair type.
/// `denom_const` and `weight_const` are the constants for the loss term:
///   loss   = w · d̃ / (denom_const + d̃)  for attractive
///   grad_i = w · 2 · weight_const · (yi − yj) / (denom_const + d̃)²
/// For further pairs the form is 1/(1+d̃), handled via `is_repulsive`.
fn accumulate_pairs(
    embedding: &[[f32; 2]],
    pairs: &[[u32; 2]],
    w: f32,
    denom: f32,
    weight_const: f32,
    is_repulsive: bool,
    grad: &mut [[f32; 2]],
    loss_acc: &mut f32,
) {
    for pair in pairs {
        let i = pair[0] as usize;
        let j = pair[1] as usize;
        let yi = embedding[i];
        let yj = embedding[j];
        let dx = yi[0] - yj[0];
        let dy = yi[1] - yj[1];
        let d_sq = dx * dx + dy * dy;
        let d_tilde = d_sq + 1.0;

        let (loss, grad_scale) = if is_repulsive {
            // L_FP = 1 / (1 + d̃); grad = w · 2 · (yi − yj) / (1 + d̃)²
            let denom_sq = d_tilde * d_tilde;
            let l = 1.0 / d_tilde;
            let g = w * 2.0 / denom_sq;
            (l, g)
        } else {
            // L_NB/MN = d̃ / (C + d̃); grad = −w · 2 · C · (yi − yj) / (C + d̃)²
            let c_plus_d = denom + d_tilde;
            let l = d_tilde / c_plus_d;
            // Attractive: gradient pushes i toward j (negative of d̃/(C+d̃) wrt yi)
            // ∂L/∂yi = C · 2(yi-yj) / (C+d̃)²  (positive for attractive means move away?)
            // Actually: ∂/∂yi d̃/(C+d̃) = 2(yi-yj)·C/(C+d̃)²
            // We want gradient descent, so we subtract this from yi to attract.
            let g = w * weight_const / (c_plus_d * c_plus_d);
            (l, g)
        };

        *loss_acc += w * loss;

        let gi0 = grad_scale * dx;
        let gi1 = grad_scale * dy;

        if is_repulsive {
            // Repulsive: push i away from j
            grad[i][0] -= gi0;
            grad[i][1] -= gi1;
            grad[j][0] += gi0;
            grad[j][1] += gi1;
        } else {
            // Attractive: pull i toward j
            grad[i][0] += gi0;
            grad[i][1] += gi1;
            grad[j][0] -= gi0;
            grad[j][1] -= gi1;
        }
    }
}

/// Compute the full gradient over all three pair types for the current embedding.
///
/// Returns `(gradient: Vec<[f32; 2]>, total_loss: f32)`.
/// The gradient buffer is reused via `grad_buf` to avoid per-iteration allocation.
///
/// Rayon is used to process chunks of pairs in parallel, then results are summed.
/// Each chunk produces an independent gradient contribution that is added to the
/// shared accumulator — safe because each chunk slice is read-only and additions
/// are commutative.
pub fn compute_gradient(
    embedding: &[[f32; 2]],
    near: &[[u32; 2]],
    mid_near: &[[u32; 2]],
    further: &[[u32; 2]],
    weights: &Weights,
    n: usize,
) -> (Vec<[f32; 2]>, f32) {
    let chunk_size = 128 * 1024;

    // Process each pair type in parallel chunks, accumulate per-chunk gradients,
    // then sum. Each chunk has its own grad buffer to avoid races.
    let process_pairs = |pairs: &[[u32; 2]],
                         w: f32,
                         denom: f32,
                         wc: f32,
                         is_rep: bool|
     -> (Vec<[f32; 2]>, f32) {
        let (grad_sum, loss_sum) = pairs
            .par_chunks(chunk_size)
            .map(|chunk| {
                let mut grad = vec![[0.0_f32; 2]; n];
                let mut loss = 0.0_f32;
                accumulate_pairs(embedding, chunk, w, denom, wc, is_rep, &mut grad, &mut loss);
                (grad, loss)
            })
            .reduce(
                || (vec![[0.0_f32; 2]; n], 0.0_f32),
                |(mut g1, l1), (g2, l2)| {
                    for (a, b) in g1.iter_mut().zip(g2.iter()) {
                        a[0] += b[0];
                        a[1] += b[1];
                    }
                    (g1, l1 + l2)
                },
            );
        (grad_sum, loss_sum)
    };

    // Near: attractive, denom=10, weight_const=20 (= 2 × denom for the C·2/(C+d̃)² form)
    let (g_nb, l_nb) = process_pairs(near, weights.w_nb, 10.0, 20.0, false);
    // Mid-near: attractive, denom=10000
    let (g_mn, l_mn) = process_pairs(mid_near, weights.w_mn, 10000.0, 20000.0, false);
    // Further: repulsive
    let (g_fp, l_fp) = process_pairs(further, weights.w_fp, 1.0, 2.0, true);

    // Sum all gradient contributions
    let mut grad = g_nb;
    for (a, b) in grad.iter_mut().zip(g_mn.iter()) {
        a[0] += b[0];
        a[1] += b[1];
    }
    for (a, b) in grad.iter_mut().zip(g_fp.iter()) {
        a[0] += b[0];
        a[1] += b[1];
    }

    (grad, l_nb + l_mn + l_fp)
}
