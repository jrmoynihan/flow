//! Adam optimizer step for the 2-D embedding.
//!
//! All state operates on `Vec<[f32; 2]>` — no matrix types, no large allocations.
//! Rayon `par_chunks_mut` parallelises the per-point update.

use rayon::prelude::*;

/// Persistent Adam state buffers. Allocated once, live for the full run.
pub struct AdamState {
    pub m: Vec<[f32; 2]>, // first moment
    pub v: Vec<[f32; 2]>, // second moment
}

impl AdamState {
    pub fn new(n: usize) -> Self {
        Self {
            m: vec![[0.0; 2]; n],
            v: vec![[0.0; 2]; n],
        }
    }
}

/// Apply one Adam step: update `embedding` in place.
///
/// `t` is the 1-indexed iteration number (used for bias correction).
pub fn adam_step(
    embedding: &mut [[f32; 2]],
    grad: &[[f32; 2]],
    state: &mut AdamState,
    t: usize,
    lr: f32,
) {
    let beta1 = 0.9_f32;
    let beta2 = 0.999_f32;
    let eps = 1e-7_f32;
    let t = t as f32;

    // Bias-correction scalars (computed once per iteration)
    let bc1 = 1.0 - beta1.powf(t);
    let bc2 = 1.0 - beta2.powf(t);
    let lr_t = lr * bc2.sqrt() / bc1;

    // Parallel per-point update
    embedding
        .par_chunks_mut(4096)
        .zip(grad.par_chunks(4096))
        .zip(state.m.par_chunks_mut(4096))
        .zip(state.v.par_chunks_mut(4096))
        .for_each(|(((y_chunk, g_chunk), m_chunk), v_chunk)| {
            for (((y, g), m), v) in y_chunk
                .iter_mut()
                .zip(g_chunk)
                .zip(m_chunk.iter_mut())
                .zip(v_chunk.iter_mut())
            {
                for dim in 0..2 {
                    m[dim] = beta1 * m[dim] + (1.0 - beta1) * g[dim];
                    v[dim] = beta2 * v[dim] + (1.0 - beta2) * g[dim] * g[dim];
                    y[dim] -= lr_t * m[dim] / (v[dim].sqrt() + eps);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adam_moves_in_gradient_direction() {
        let n = 4;
        let mut emb = vec![[0.0_f32; 2]; n];
        let grad = vec![[1.0_f32, -1.0]; n]; // constant gradient
        let mut state = AdamState::new(n);

        adam_step(&mut emb, &grad, &mut state, 1, 1.0);

        // After one step, each point should have moved in the -gradient direction
        for y in &emb {
            assert!(y[0] < 0.0, "should move in -x direction");
            assert!(y[1] > 0.0, "should move in +y direction");
        }
    }
}
