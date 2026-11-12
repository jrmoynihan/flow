//! Optional faer matrix-product assignment kernel (`emeans-faer`).
//!
//! Cross-term for a chunk of points against all centroids as one `f32`
//! product, then a row-wise minimum. Kept behind a feature until Criterion
//! A/B against [`super::CpuLoopAssign`] decides the default.

use faer::MatRef;
use rayon::prelude::*;

use crate::clustering::ClusteringResult;

use super::assign::{Assign, centroid_norm2, validate_shapes};

/// Chunk width aimed at L2-resident scratch (`chunk × k` f32).
const DEFAULT_CHUNK: usize = 64;

#[derive(Debug, Default, Clone, Copy)]
pub struct FaerAssign;

impl Assign for FaerAssign {
    fn assign(
        &self,
        points: &[f32],
        n: usize,
        d: usize,
        centroids: &[f32],
        k: usize,
        out: &mut [u32],
    ) -> ClusteringResult<()> {
        validate_shapes(points, n, d, centroids, k, out)?;
        if n == 0 {
            return Ok(());
        }
        let c_norm2 = centroid_norm2(centroids, k, d);
        // Centroids as d × k column-major for a points(n×d) · centroids(d×k) product.
        // Our centroids are row-major k×d; build a column-major d×k view by transpose copy.
        let mut cents_cm = vec![0.0f32; d * k];
        for j in 0..k {
            for t in 0..d {
                cents_cm[t + j * d] = centroids[j * d + t];
            }
        }
        let cents = MatRef::from_column_major_slice(&cents_cm, d, k);

        let chunk = DEFAULT_CHUNK.min(n).max(1);
        out.par_chunks_mut(chunk)
            .enumerate()
            .for_each(|(chunk_idx, out_chunk)| {
                let start = chunk_idx * chunk;
                let rows = out_chunk.len();
                let pts_slice = &points[start * d..(start + rows) * d];
                // Row-major n×d as MatRef.
                let pts = MatRef::from_row_major_slice(pts_slice, rows, d);
                let mut product = faer::Mat::<f32>::zeros(rows, k);
                // product = pts * cents  => (rows×d)·(d×k)
                faer::linalg::matmul::matmul(
                    product.as_mut(),
                    faer::Accum::Replace,
                    pts,
                    cents,
                    1.0,
                    faer::Par::Seq,
                );
                for i in 0..rows {
                    let row = &points[(start + i) * d..(start + i + 1) * d];
                    let mut x_norm2 = 0.0f32;
                    for &v in row {
                        x_norm2 += v * v;
                    }
                    let mut best_j = 0u32;
                    let mut best = f32::INFINITY;
                    for j in 0..k {
                        let dist = x_norm2 + c_norm2[j] - 2.0 * product[(i, j)];
                        if dist < best {
                            best = dist;
                            best_j = j as u32;
                        }
                    }
                    out_chunk[i] = best_j;
                }
            });
        Ok(())
    }
}
