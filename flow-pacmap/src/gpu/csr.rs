//! Build undirected CSR graphs from PaCMAP pair lists.

/// Kind tags stored per directed CSR edge.
pub const KIND_NEAR: u32 = 0;
pub const KIND_MID: u32 = 1;
pub const KIND_FAR: u32 = 2;

/// CSR over both directions of each undirected pair.
#[derive(Debug, Clone)]
pub struct PacmapCsr {
    /// Length `n + 1`.
    pub offsets: Vec<u32>,
    /// Other endpoint for each directed edge.
    pub others: Vec<u32>,
    /// [`KIND_NEAR`] / [`KIND_MID`] / [`KIND_FAR`].
    pub kinds: Vec<u32>,
}

impl PacmapCsr {
    /// Build a CSR listing every undirected pair under both endpoints.
    pub fn from_pair_lists(
        n: usize,
        near: &[[u32; 2]],
        mid_near: &[[u32; 2]],
        further: &[[u32; 2]],
    ) -> Self {
        let mut deg = vec![0u32; n];
        for p in near.iter().chain(mid_near.iter()).chain(further.iter()) {
            let i = p[0] as usize;
            let j = p[1] as usize;
            if i < n {
                deg[i] += 1;
            }
            if j < n {
                deg[j] += 1;
            }
        }

        let mut offsets = vec![0u32; n + 1];
        for i in 0..n {
            offsets[i + 1] = offsets[i] + deg[i];
        }
        let n_edges = offsets[n] as usize;
        let mut others = vec![0u32; n_edges];
        let mut kinds = vec![0u32; n_edges];
        let mut cursor = offsets[..n].to_vec();

        let mut push = |a: u32, b: u32, kind: u32| {
            let ia = a as usize;
            if ia >= n {
                return;
            }
            let slot = cursor[ia] as usize;
            others[slot] = b;
            kinds[slot] = kind;
            cursor[ia] += 1;
        };

        for p in near {
            push(p[0], p[1], KIND_NEAR);
            push(p[1], p[0], KIND_NEAR);
        }
        for p in mid_near {
            push(p[0], p[1], KIND_MID);
            push(p[1], p[0], KIND_MID);
        }
        for p in further {
            push(p[0], p[1], KIND_FAR);
            push(p[1], p[0], KIND_FAR);
        }

        Self {
            offsets,
            others,
            kinds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undirected_pair_appears_twice() {
        let csr = PacmapCsr::from_pair_lists(3, &[[0, 1]], &[], &[[1, 2]]);
        assert_eq!(csr.offsets[3], 4); // 2 undirected → 4 directed
        assert_eq!(csr.others.len(), 4);
    }

    /// Host-side CSR walk mirrors the GPU kernel; must match undirected `accumulate_pairs`.
    #[test]
    fn csr_walk_matches_undirected_cpu_gradient() {
        let emb = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 2.0]];
        let near = [[0u32, 1]];
        let mid: [[u32; 2]; 0] = [];
        let far = [[1u32, 2]];
        let w_nb = 2.0f32;
        let w_mn = 0.0f32;
        let w_fp = 1.0f32;

        let mut grad_list = [[0.0f32; 2]; 3];
        // near attractive
        {
            let i = 0usize;
            let j = 1usize;
            let dx = emb[i][0] - emb[j][0];
            let dy = emb[i][1] - emb[j][1];
            let d_tilde = dx * dx + dy * dy + 1.0;
            let c = 10.0f32;
            let g = w_nb * (2.0 * c) / (c + d_tilde).powi(2);
            grad_list[i][0] += g * dx;
            grad_list[i][1] += g * dy;
            grad_list[j][0] -= g * dx;
            grad_list[j][1] -= g * dy;
        }
        // far repulsive
        {
            let i = 1usize;
            let j = 2usize;
            let dx = emb[i][0] - emb[j][0];
            let dy = emb[i][1] - emb[j][1];
            let d_tilde = dx * dx + dy * dy + 1.0;
            let g = w_fp * 2.0 / (d_tilde * d_tilde);
            grad_list[i][0] -= g * dx;
            grad_list[i][1] -= g * dy;
            grad_list[j][0] += g * dx;
            grad_list[j][1] += g * dy;
        }

        let csr = PacmapCsr::from_pair_lists(3, &near, &mid, &far);
        let mut grad_csr = [[0.0f32; 2]; 3];
        for node in 0..3 {
            let start = csr.offsets[node] as usize;
            let end = csr.offsets[node + 1] as usize;
            let yi = emb[node];
            for e in start..end {
                let other = csr.others[e] as usize;
                let kind = csr.kinds[e];
                let yj = emb[other];
                let dx = yi[0] - yj[0];
                let dy = yi[1] - yj[1];
                let d_tilde = dx * dx + dy * dy + 1.0;
                if kind == KIND_FAR {
                    let g = w_fp * 2.0 / (d_tilde * d_tilde);
                    grad_csr[node][0] -= g * dx;
                    grad_csr[node][1] -= g * dy;
                } else {
                    let (w, c) = if kind == KIND_NEAR {
                        (w_nb, 10.0f32)
                    } else {
                        (w_mn, 10000.0f32)
                    };
                    let g = w * (2.0 * c) / (c + d_tilde).powi(2);
                    grad_csr[node][0] += g * dx;
                    grad_csr[node][1] += g * dy;
                }
            }
        }

        for i in 0..3 {
            assert!(
                (grad_list[i][0] - grad_csr[i][0]).abs() < 1e-5
                    && (grad_list[i][1] - grad_csr[i][1]).abs() < 1e-5,
                "node {i}: list={:?} csr={:?}",
                grad_list[i],
                grad_csr[i]
            );
        }
    }
}
