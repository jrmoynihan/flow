//! Nearest-centroid assignment backends shared by e-means and Lloyd.

use crate::clustering::{ClusteringError, ClusteringResult};

use super::config::AssignBackend;

/// Assign each point in a contiguous row-major block to its nearest centroid.
///
/// Distances use `‖x − c‖² = ‖x‖² + ‖c‖² − 2⟨x,c⟩` with centroid norms
/// precomputed once per call. Writes into `out` (length = number of points).
pub trait Assign: Send + Sync {
    fn assign(
        &self,
        points: &[f32],
        n: usize,
        d: usize,
        centroids: &[f32],
        k: usize,
        out: &mut [u32],
    ) -> ClusteringResult<()>;
}

/// Rayon over points; hand-written inner loop, no scratch buffer.
#[derive(Debug, Default, Clone, Copy)]
pub struct CpuLoopAssign;

impl Assign for CpuLoopAssign {
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
        use rayon::prelude::*;
        out.par_iter_mut().enumerate().for_each(|(i, slot)| {
            let row = &points[i * d..(i + 1) * d];
            *slot = nearest_centroid(row, centroids, k, d, &c_norm2);
        });
        Ok(())
    }
}

/// Build the assigner selected by config.
pub fn make_assigner(backend: AssignBackend) -> ClusteringResult<Box<dyn Assign>> {
    match backend {
        AssignBackend::CpuLoop => Ok(Box::new(CpuLoopAssign)),
        #[cfg(feature = "emeans-faer")]
        AssignBackend::CpuFaer => Ok(Box::new(super::faer_assign::FaerAssign::default())),
        #[cfg(feature = "emeans-gpu")]
        AssignBackend::Gpu => Ok(Box::new(super::gpu::GpuAssign::try_new()?)),
    }
}

pub(crate) fn validate_shapes(
    points: &[f32],
    n: usize,
    d: usize,
    centroids: &[f32],
    k: usize,
    out: &[u32],
) -> ClusteringResult<()> {
    if d == 0 {
        return Err(ClusteringError::InvalidConfig(
            "feature dimension must be > 0".into(),
        ));
    }
    if points.len() != n.saturating_mul(d) {
        return Err(ClusteringError::InvalidConfig(format!(
            "points length {} != n·d = {n}·{d}",
            points.len()
        )));
    }
    if centroids.len() != k.saturating_mul(d) {
        return Err(ClusteringError::InvalidConfig(format!(
            "centroids length {} != k·d = {k}·{d}",
            centroids.len()
        )));
    }
    if out.len() != n {
        return Err(ClusteringError::InvalidConfig(format!(
            "assignment buffer length {} != n = {n}",
            out.len()
        )));
    }
    Ok(())
}

pub(crate) fn centroid_norm2(centroids: &[f32], k: usize, d: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; k];
    for j in 0..k {
        let row = &centroids[j * d..(j + 1) * d];
        let mut s = 0.0f32;
        for &v in row {
            s += v * v;
        }
        out[j] = s;
    }
    out
}

#[inline]
pub(crate) fn nearest_centroid(
    point: &[f32],
    centroids: &[f32],
    k: usize,
    d: usize,
    c_norm2: &[f32],
) -> u32 {
    let mut x_norm2 = 0.0f32;
    for &v in point {
        x_norm2 += v * v;
    }
    let mut best_j = 0u32;
    let mut best_dist = f32::INFINITY;
    for j in 0..k {
        let c = &centroids[j * d..(j + 1) * d];
        let mut dot = 0.0f32;
        for i in 0..d {
            dot += point[i] * c[i];
        }
        // ‖x−c‖² = ‖x‖² + ‖c‖² − 2⟨x,c⟩; ‖x‖² is constant across j but kept
        // so the absolute value matches a true squared Euclidean distance.
        let dist = x_norm2 + c_norm2[j] - 2.0 * dot;
        if dist < best_dist {
            best_dist = dist;
            best_j = j as u32;
        }
    }
    best_j
}

/// Accumulate per-cluster counts and sums for a batch given its assignments.
pub fn accumulate_batch(
    points: &[f32],
    n: usize,
    d: usize,
    assignments: &[u32],
    k: usize,
    counts: &mut [f64],
    sums: &mut [f64],
) {
    debug_assert_eq!(counts.len(), k);
    debug_assert_eq!(sums.len(), k * d);
    counts.fill(0.0);
    sums.fill(0.0);
    for i in 0..n {
        let j = assignments[i] as usize;
        if j >= k {
            continue;
        }
        counts[j] += 1.0;
        let src = &points[i * d..(i + 1) * d];
        let dst = &mut sums[j * d..(j + 1) * d];
        for t in 0..d {
            dst[t] += src[t] as f64;
        }
    }
}

/// Mean squared error of assignments against centroids.
pub fn inertia(points: &[f32], n: usize, d: usize, centroids: &[f32], assignments: &[u32]) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for i in 0..n {
        let j = assignments[i] as usize;
        let x = &points[i * d..(i + 1) * d];
        let c = &centroids[j * d..(j + 1) * d];
        let mut dist = 0.0f64;
        for t in 0..d {
            let diff = x[t] as f64 - c[t] as f64;
            dist += diff * diff;
        }
        sum += dist;
    }
    sum / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigns_to_exact_centroid() {
        let points = vec![0.0f32, 0.0, 10.0, 10.0];
        let centroids = vec![0.0f32, 0.0, 10.0, 10.0];
        let mut out = vec![0u32; 2];
        CpuLoopAssign
            .assign(&points, 2, 2, &centroids, 2, &mut out)
            .unwrap();
        assert_eq!(out, vec![0, 1]);
    }
}
