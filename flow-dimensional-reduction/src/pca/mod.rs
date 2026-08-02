//! Principal Component Analysis via the covariance method.

use faer::{Mat, linalg::solvers::Svd};
use thiserror::Error;

/// Error type for PCA operations.
#[derive(Error, Debug)]
pub enum PcaError {
    #[error("Empty data")]
    EmptyData,
    #[error("Insufficient data: need at least {min} points, got {actual}")]
    InsufficientData { min: usize, actual: usize },
    #[error("Dimension mismatch: slice length {len} != n*d ({n}*{d})")]
    DimensionMismatch { len: usize, n: usize, d: usize },
    #[error("Feature count mismatch: model was fitted on {fitted} features, got {actual}")]
    FeatureMismatch { fitted: usize, actual: usize },
    #[error("SVD decomposition failed: {0}")]
    SvdFailed(String),
}

pub type PcaResult<T> = Result<T, PcaError>;

/// Principal Component Analysis.
///
/// Fit with [`Pca::fit`], then project new data with [`Pca::transform`].
#[derive(Debug, Clone)]
pub struct Pca {
    /// Requested count before fitting; actual (clamped) count after.
    n_components: usize,
    /// `n_components × d`, row `i` is the i-th principal axis. Empty until fitted.
    components: Mat<f32>,
    /// Fraction of total variance per retained component, descending.
    explained_variance_ratio: Vec<f32>,
    /// Column means of the training data, length `d`. Empty until fitted.
    mean: Vec<f32>,
}

impl Pca {
    /// Create an unfitted PCA requesting `n_components` components.
    ///
    /// The actual count is clamped to the feature count `d` during [`Pca::fit`].
    #[must_use]
    pub fn new(n_components: usize) -> Self {
        Self {
            n_components,
            components: Mat::zeros(0, 0),
            explained_variance_ratio: Vec::new(),
            mean: Vec::new(),
        }
    }

    /// Fit to `n × d` row-major data.
    ///
    /// Means and the covariance matrix are accumulated in `f64` regardless of
    /// the `f32` input, then downcast — `n` can be large enough that `f32`
    /// accumulation loses significant precision.
    ///
    /// # Errors
    /// [`PcaError::EmptyData`] if `n == 0` or `d == 0`;
    /// [`PcaError::InsufficientData`] if `n < 2`;
    /// [`PcaError::DimensionMismatch`] if `data.len() != n * d`;
    /// [`PcaError::SvdFailed`] if the decomposition fails.
    pub fn fit(mut self, data: &[f32], n: usize, d: usize) -> PcaResult<Self> {
        if n == 0 || d == 0 {
            return Err(PcaError::EmptyData);
        }
        if data.len() != n * d {
            return Err(PcaError::DimensionMismatch { len: data.len(), n, d });
        }
        if n < 2 {
            return Err(PcaError::InsufficientData { min: 2, actual: n });
        }

        // Column means, accumulated in f64.
        let mut mean64 = vec![0.0_f64; d];
        for row in data.chunks_exact(d) {
            for (j, &v) in row.iter().enumerate() {
                mean64[j] += f64::from(v);
            }
        }
        let inv_n = 1.0_f64 / n as f64;
        for m in &mut mean64 {
            *m *= inv_n;
        }

        // Covariance C = (X - mean)^T (X - mean) / n, symmetric.
        // Only the upper triangle is accumulated, then mirrored.
        // The 1/n scaling affects neither the eigenvectors nor the variance
        // ratios, but is applied so the matrix is a true covariance.
        let mut cov = Mat::<f64>::zeros(d, d);
        for row in data.chunks_exact(d) {
            for i in 0..d {
                let xi = f64::from(row[i]) - mean64[i];
                for j in i..d {
                    let xj = f64::from(row[j]) - mean64[j];
                    cov[(i, j)] += xi * xj;
                }
            }
        }
        for i in 0..d {
            for j in i..d {
                cov[(i, j)] *= inv_n;
                if i != j {
                    cov[(j, i)] = cov[(i, j)];
                }
            }
        }

        let k = self.n_components.min(d);

        // One decomposition: U and S come from the same Svd object, so they
        // are guaranteed to correspond (sigma[i] <-> column i of U) — unlike
        // pairing a standalone `singular_values()` call with a separate
        // `Svd::new` call, which relies on an unstated ordering invariant
        // between two independent decompositions.
        let svd = Svd::<f64>::new(cov.as_ref())
            .map_err(|e| PcaError::SvdFailed(format!("{e:?}")))?;
        let u = svd.U();
        // `S()` returns a `DiagRef`, not a slice/Vec; go through its column
        // vector view to iterate the singular values in decomposition order.
        let sigma: Vec<f64> = svd.S().column_vector().iter().copied().collect();

        // Row i of `components` is the i-th principal axis (column i of U).
        let mut components = Mat::<f32>::zeros(k, d);
        for i in 0..k {
            for j in 0..d {
                components[(i, j)] = *u.get(j, i) as f32;
            }
        }

        // For a covariance matrix the singular values ARE the variances.
        let total: f64 = sigma.iter().sum();
        let explained_variance_ratio: Vec<f32> = if total > 0.0 {
            sigma.iter().take(k).map(|&s| (s / total) as f32).collect()
        } else {
            vec![0.0; k]
        };

        self.n_components = k;
        self.components = components;
        self.explained_variance_ratio = explained_variance_ratio;
        self.mean = mean64.into_iter().map(|m| m as f32).collect();

        Ok(self)
    }

    /// Project `n × d` row-major data onto the fitted axes.
    ///
    /// Returns `n × n_components` row-major.
    ///
    /// # Errors
    /// [`PcaError::DimensionMismatch`] if `data.len() != n * d`;
    /// [`PcaError::FeatureMismatch`] if `d` differs from the fitted feature count.
    pub fn transform(&self, data: &[f32], n: usize, d: usize) -> PcaResult<Vec<f32>> {
        if data.len() != n * d {
            return Err(PcaError::DimensionMismatch { len: data.len(), n, d });
        }
        if d != self.mean.len() {
            return Err(PcaError::FeatureMismatch { fitted: self.mean.len(), actual: d });
        }

        let k = self.n_components;
        let mut out = Vec::with_capacity(n * k);
        for row in data.chunks_exact(d) {
            for i in 0..k {
                let mut acc = 0.0_f32;
                for (j, (&x, &m)) in row.iter().zip(self.mean.iter()).enumerate() {
                    acc += (x - m) * self.components[(i, j)];
                }
                out.push(acc);
            }
        }
        Ok(out)
    }

    /// Principal axes, `n_components × d`.
    #[must_use]
    pub fn components(&self) -> &Mat<f32> {
        &self.components
    }

    /// Fraction of total variance per component, descending.
    #[must_use]
    pub fn explained_variance_ratio(&self) -> &[f32] {
        &self.explained_variance_ratio
    }

    /// Column means of the training data.
    #[must_use]
    pub fn mean(&self) -> &[f32] {
        &self.mean
    }

    /// Component count: requested before fitting, actual (clamped) after.
    #[must_use]
    pub fn n_components(&self) -> usize {
        self.n_components
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two strongly correlated columns; PC1 must capture nearly all variance.
    /// Row-major, n=5, d=2.
    fn fixture() -> (Vec<f32>, usize, usize) {
        let data = vec![
            1.0, 2.0,
            2.0, 4.1,
            3.0, 5.9,
            4.0, 8.2,
            5.0, 9.8,
        ];
        (data, 5, 2)
    }

    #[test]
    fn fit_then_transform_projects_to_n_components() {
        let (data, n, d) = fixture();
        let pca = Pca::new(1).fit(&data, n, d).expect("fit");
        let out = pca.transform(&data, n, d).expect("transform");
        assert_eq!(out.len(), n, "output must be n x n_components row-major");
    }

    #[test]
    fn first_component_dominates_variance() {
        let (data, n, d) = fixture();
        let pca = Pca::new(2).fit(&data, n, d).expect("fit");
        assert!(
            pca.explained_variance_ratio()[0] > 0.99,
            "PC1 ratio was {}",
            pca.explained_variance_ratio()[0]
        );
    }

    #[test]
    fn explained_variance_ratio_sums_to_one_and_descends() {
        let (data, n, d) = fixture();
        let pca = Pca::new(2).fit(&data, n, d).expect("fit");
        let r = pca.explained_variance_ratio();
        let total: f32 = r.iter().sum();
        assert!((total - 1.0).abs() < 1e-4, "ratios summed to {total}");
        assert!(r[0] >= r[1], "ratios must descend: {r:?}");
    }

    #[test]
    fn n_components_clamped_to_d() {
        let (data, n, d) = fixture();
        let pca = Pca::new(10).fit(&data, n, d).expect("fit");
        assert_eq!(pca.n_components(), 2, "must clamp to d");
    }

    #[test]
    fn mean_is_column_mean() {
        let (data, n, d) = fixture();
        let pca = Pca::new(1).fit(&data, n, d).expect("fit");
        // column 0 mean = (1+2+3+4+5)/5 = 3.0
        approx::assert_abs_diff_eq!(pca.mean()[0], 3.0_f32, epsilon = 1e-5);
    }

    #[test]
    fn separates_axis_aligned_clusters() {
        // Mirrors the guard test in flow-pacmap/src/pca.rs:84 so Task 3 can
        // rely on identical behaviour.
        let mut data: Vec<f32> = Vec::new();
        for _ in 0..50 {
            data.extend_from_slice(&[0.0_f32, 0.0]);
        }
        for _ in 0..50 {
            data.extend_from_slice(&[10.0_f32, 0.0]);
        }
        let pca = Pca::new(2).fit(&data, 100, 2).expect("fit");
        let out = pca.transform(&data, 100, 2).expect("transform");
        let left: f32 = out.chunks_exact(2).take(50).map(|c| c[0]).sum::<f32>() / 50.0;
        let right: f32 = out.chunks_exact(2).skip(50).map(|c| c[0]).sum::<f32>() / 50.0;
        assert!((left - right).abs() > 1.0, "PC1 must separate the clusters");
    }

    #[test]
    fn fit_empty_data_errors() {
        assert!(matches!(Pca::new(1).fit(&[], 0, 0), Err(PcaError::EmptyData)));
    }

    #[test]
    fn fit_single_row_errors() {
        let data = vec![1.0_f32, 2.0];
        assert!(matches!(
            Pca::new(1).fit(&data, 1, 2),
            Err(PcaError::InsufficientData { min: 2, actual: 1 })
        ));
    }

    #[test]
    fn fit_length_mismatch_errors() {
        let data = vec![1.0_f32, 2.0, 3.0];
        assert!(matches!(
            Pca::new(1).fit(&data, 2, 2),
            Err(PcaError::DimensionMismatch { len: 3, n: 2, d: 2 })
        ));
    }

    #[test]
    fn transform_length_mismatch_errors() {
        let (data, n, d) = fixture();
        let pca = Pca::new(1).fit(&data, n, d).expect("fit");
        let bad = vec![1.0_f32; 7];
        assert!(matches!(
            pca.transform(&bad, 3, 2),
            Err(PcaError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn transform_rejects_wrong_feature_count() {
        let (data, n, d) = fixture();
        let pca = Pca::new(1).fit(&data, n, d).expect("fit");
        let three_wide = vec![1.0_f32; 6];
        assert!(
            pca.transform(&three_wide, 2, 3).is_err(),
            "transform must reject d != fitted d"
        );
    }
}
