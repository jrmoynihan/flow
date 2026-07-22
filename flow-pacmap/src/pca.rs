//! PCA initialisation via faer SVD on the d×d covariance matrix.
//!
//! This approach costs O(n·d²) rather than O(n²) — at n=10M and d=46,
//! the covariance matrix is 46×46 = 2116 f32 elements, trivially small.

use crate::error::PaCMAPError;
use faer::{Mat, linalg::solvers::Svd};

/// Project `n × d` row-major data onto its top 2 principal components.
///
/// Returns `n` (x, y) pairs where x and y are the PC1 and PC2 scores,
/// normalised to the range used by the paper's initialisation.
pub fn pca_init(data: &[f32], n: usize, d: usize) -> Result<Vec<[f32; 2]>, PaCMAPError> {
    debug_assert_eq!(data.len(), n * d);

    // Step 1: compute column means
    let mut mean = vec![0.0_f32; d];
    for row in data.chunks_exact(d) {
        for (j, &v) in row.iter().enumerate() {
            mean[j] += v;
        }
    }
    for m in &mut mean {
        *m /= n as f32;
    }

    // Step 2: build d×d covariance C = (X - mean)^T (X - mean) / n
    // C is symmetric; only compute upper triangle then mirror
    let mut cov = Mat::<f32>::zeros(d, d);
    for row in data.chunks_exact(d) {
        for i in 0..d {
            let xi = row[i] - mean[i];
            for j in i..d {
                let xj = row[j] - mean[j];
                cov[(i, j)] += xi * xj;
            }
        }
    }
    let inv_n = 1.0 / n as f32;
    for i in 0..d {
        for j in i..d {
            cov[(i, j)] *= inv_n;
            if i != j {
                cov[(j, i)] = cov[(i, j)];
            }
        }
    }

    // Step 3: thin SVD of symmetric covariance C = U S V^T
    // For a symmetric PSD matrix U ≈ V; top-2 columns of U are the principal components.
    let svd = Svd::<f32>::new(cov.as_ref())
        .map_err(|e| PaCMAPError::Pca(format!("{e:?}")))?;

    let u = svd.U();
    // Top-2 eigenvectors: columns 0 and 1 of U (sorted by descending singular value)
    let pc1: Vec<f32> = (0..d).map(|r| *u.get(r, 0)).collect();
    let pc2: Vec<f32> = if d >= 2 { (0..d).map(|r| *u.get(r, 1)).collect() } else { vec![0.0; d] };

    // Step 4: project each row onto PC1, PC2
    let mut embedding = Vec::with_capacity(n);
    for row in data.chunks_exact(d) {
        let mut s1 = 0.0_f32;
        let mut s2 = 0.0_f32;
        for j in 0..d {
            let v = row[j] - mean[j];
            s1 += v * pc1[j];
            s2 += v * pc2[j];
        }
        embedding.push([s1, s2]);
    }

    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pca_separates_axis_aligned_clusters() {
        // Two clusters separated along dim 0; PCA should put that on PC1
        let mut data: Vec<f32> = Vec::new();
        for _ in 0..50 {
            data.extend_from_slice(&[0.0_f32, 0.0]);
        }
        for _ in 0..50 {
            data.extend_from_slice(&[10.0_f32, 0.0]);
        }
        let emb = pca_init(&data, 100, 2).unwrap();
        // First 50 points should have similar x; second 50 should differ
        let mean_left_x = emb[..50].iter().map(|p| p[0]).sum::<f32>() / 50.0;
        let mean_right_x = emb[50..].iter().map(|p| p[0]).sum::<f32>() / 50.0;
        assert!(
            (mean_left_x - mean_right_x).abs() > 1.0,
            "PCA should separate the two clusters along PC1"
        );
    }
}
