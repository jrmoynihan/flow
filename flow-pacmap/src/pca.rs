//! PCA initialisation for PaCMAP.
//!
//! A two-component specialization of [`flow_dimensional_reduction::Pca`], which
//! uses the covariance method: O(n·d²) to build a d×d matrix plus O(d³) to
//! decompose it, rather than an O(n²)-scale decomposition of the data matrix.

use crate::error::PaCMAPError;
use flow_dimensional_reduction::Pca;

/// Project `n × d` row-major data onto its top 2 principal components.
///
/// Returns `n` (PC1, PC2) score pairs.
///
/// # Errors
/// Returns [`PaCMAPError::Pca`] if the decomposition fails or the inputs are
/// inconsistent.
pub fn pca_init(data: &[f32], n: usize, d: usize) -> Result<Vec<[f32; 2]>, PaCMAPError> {
    debug_assert_eq!(data.len(), n * d);

    let pca = Pca::new(2).fit(data, n, d)?;
    let flat = pca.transform(data, n, d)?;
    let k = pca.n_components();

    // `k` is 1 when d == 1; pad the second axis with zeros to keep the
    // [f32; 2] contract that callers rely on.
    Ok(flat
        .chunks(k)
        .map(|c| [c[0], if k >= 2 { c[1] } else { 0.0 }])
        .collect())
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
