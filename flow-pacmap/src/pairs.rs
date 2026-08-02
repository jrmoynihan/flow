//! Near, mid-near, and further pair construction from Algorithm 1.
//!
//! All pair indices are `u32`; the entry guard in `lib.rs` ensures n ≤ u32::MAX.
//! All multiplications use `checked_mul` before any allocation.

use crate::error::PaCMAPError;
use crate::knn::NeighborList;
use rand::{RngExt, SeedableRng, rngs::SmallRng};
use rayon::prelude::*;

/// Pre-allocated pair storage for all three pair types.
pub struct Pairs {
    /// Near pairs: (point_i, near_neighbour_j) — shape [n * n_nb, 2]
    pub near: Vec<[u32; 2]>,
    /// Mid-near pairs — shape [n * n_mn, 2]
    pub mid_near: Vec<[u32; 2]>,
    /// Further pairs — shape [n * n_fp, 2]
    pub further: Vec<[u32; 2]>,
}

/// Build all three pair types from pre-computed KNN results.
///
/// # Overflow safety
/// All `n * k` multiplications use `checked_mul`. `n` is guaranteed ≤ u32::MAX
/// by the entry guard in `lib.rs`.
pub fn build_pairs(
    knn: &[NeighborList],
    data: &[f32],
    n: usize,
    d: usize,
    n_nb: usize,
    n_mn: usize,
    n_fp: usize,
    seed: Option<u64>,
) -> Result<Pairs, PaCMAPError> {
    let cap_nb = n
        .checked_mul(n_nb)
        .ok_or(PaCMAPError::PairCountOverflow { n, k: n_nb })?;
    let cap_mn = n
        .checked_mul(n_mn)
        .ok_or(PaCMAPError::PairCountOverflow { n, k: n_mn })?;
    let cap_fp = n
        .checked_mul(n_fp)
        .ok_or(PaCMAPError::PairCountOverflow { n, k: n_fp })?;

    // ── Near pairs ─────────────────────────────────────────────────────────
    // For each i, compute scaled distance d²_select = ‖xi−xj‖² / (σi·σj)
    // and keep the top n_nb by scaled distance from the candidate set.

    // σi = average distance to 4th–6th Euclidean neighbours
    let sigma: Vec<f32> = knn
        .par_iter()
        .map(|nl| {
            let start = 3.min(nl.distances.len());
            let end = 6.min(nl.distances.len());
            if start >= end {
                // Fallback for very small n: use available distances
                if nl.distances.is_empty() {
                    1.0
                } else {
                    nl.distances.iter().sum::<f32>() / nl.distances.len() as f32
                }
            } else {
                nl.distances[start..end].iter().sum::<f32>() / (end - start) as f32
            }
        })
        .collect();

    let mut near: Vec<[u32; 2]> = Vec::with_capacity(cap_nb);

    // Build near pairs sequentially per point (scaled distance reranking is cheap)
    for (i, nl) in knn.iter().enumerate() {
        let row_i = &data[i * d..(i + 1) * d];
        let sigma_i = sigma[i].max(f32::EPSILON);

        // Compute scaled distance for each candidate
        let mut scaled: Vec<(f32, u32)> = nl
            .indices
            .iter()
            .map(|&j| {
                let row_j = &data[j as usize * d..(j as usize + 1) * d];
                let l2sq: f32 = row_i
                    .iter()
                    .zip(row_j)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum();
                let sigma_j = sigma[j as usize].max(f32::EPSILON);
                let d_scaled = l2sq / (sigma_i * sigma_j);
                (d_scaled, j)
            })
            .collect();

        // Keep top n_nb by scaled distance
        scaled.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (_, j) in scaled.iter().take(n_nb) {
            near.push([i as u32, *j]);
        }
    }

    // ── Mid-near pairs ──────────────────────────────────────────────────────
    // For each i: sample 6 random points, use the 2nd closest as mid-near partner.
    // Repeat n_mn times.
    let base_seed = seed.unwrap_or(42);

    let mut mid_near: Vec<[u32; 2]> = Vec::with_capacity(cap_mn);
    // Sequential per-point to avoid RNG sharing across threads
    for i in 0..n {
        let row_i = &data[i * d..(i + 1) * d];
        let mut rng = SmallRng::seed_from_u64(base_seed.wrapping_add(i as u64));
        for _ in 0..n_mn {
            let candidates = sample_6_excluding(&mut rng, n as u32, i as u32);
            // Find 2nd closest candidate (index 1 in sorted order)
            let mut dists: Vec<(f32, u32)> = candidates
                .iter()
                .map(|&j| {
                    let row_j = &data[j as usize * d..(j as usize + 1) * d];
                    let d2: f32 = row_i
                        .iter()
                        .zip(row_j)
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum();
                    (d2, j)
                })
                .collect();
            dists.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            if let Some(second) = dists.get(1) {
                mid_near.push([i as u32, second.1]);
            } else if let Some(first) = dists.first() {
                mid_near.push([i as u32, first.1]);
            }
        }
    }

    // ── Further pairs ───────────────────────────────────────────────────────
    // For each i: sample n_fp random non-neighbour points.
    // Build a set of near-neighbour indices per point for rejection sampling.
    let mut further: Vec<[u32; 2]> = Vec::with_capacity(cap_fp);
    for (i, _) in knn.iter().enumerate().take(n) {
        let mut rng = SmallRng::seed_from_u64(
            base_seed
                .wrapping_add(i as u64)
                .wrapping_add(0xdeadbeef_cafebabe),
        );
        let near_set: std::collections::HashSet<u32> = knn[i].indices.iter().copied().collect();
        let mut count = 0usize;
        let mut attempts = 0usize;
        while count < n_fp && attempts < n_fp * 100 {
            let j = rng.random_range(0..n as u32);
            if j != i as u32 && !near_set.contains(&j) {
                further.push([i as u32, j]);
                count += 1;
            }
            attempts += 1;
        }
        // If rejection sampling exhausted, pad with whatever we have
        // (only happens for pathologically small n)
        if count < n_fp {
            for j in 0..n as u32 {
                if j != i as u32 && !near_set.contains(&j) && count < n_fp {
                    further.push([i as u32, j]);
                    count += 1;
                }
            }
        }
    }

    Ok(Pairs {
        near,
        mid_near,
        further,
    })
}

/// Sample 6 distinct random indices in [0, max) excluding `exclude`.
fn sample_6_excluding(rng: &mut SmallRng, max: u32, exclude: u32) -> Vec<u32> {
    let mut result = Vec::with_capacity(6);
    let mut attempts = 0u32;
    while result.len() < 6 && attempts < 1000 {
        let j = rng.random_range(0..max);
        if j != exclude && !result.contains(&j) {
            result.push(j);
        }
        attempts += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knn::NeighborList;

    fn make_knn(n: usize, k: usize) -> Vec<NeighborList> {
        (0..n)
            .map(|i| NeighborList {
                indices: (0..k).map(|t| ((i + t + 1) % n) as u32).collect(),
                distances: vec![1.0; k],
            })
            .collect()
    }

    #[test]
    fn pair_counts_match_expected() {
        let n = 20;
        let n_nb = 5;
        let n_mn = 2;
        let n_fp = 4;
        let data: Vec<f32> = (0..n * 3).map(|i| i as f32).collect();
        let knn = make_knn(n, 10);
        let pairs = build_pairs(&knn, &data, n, 3, n_nb, n_mn, n_fp, Some(0)).unwrap();
        assert_eq!(pairs.near.len(), n * n_nb);
        assert_eq!(pairs.mid_near.len(), n * n_mn);
        assert_eq!(pairs.further.len(), n * n_fp);
    }

    #[test]
    fn near_pairs_no_self_loops() {
        let n = 20;
        let data: Vec<f32> = (0..n * 3).map(|i| i as f32).collect();
        let knn = make_knn(n, 10);
        let pairs = build_pairs(&knn, &data, n, 3, 5, 2, 4, Some(0)).unwrap();
        for p in &pairs.near {
            assert_ne!(p[0], p[1], "near pair should not be a self-loop");
        }
    }
}
