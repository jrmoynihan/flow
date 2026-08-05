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

pub mod state;

use state::{Fitted, Unfitted};

mod sealed {
    pub trait Sealed {}
}

/// State of a [`Pca`]. Sealed: downstream crates cannot add states, so the set
/// of valid transitions stays a fact local to this module.
pub trait PcaComponent: sealed::Sealed + Sized + std::fmt::Debug {
    /// Requested component count on [`state::Unfitted`]; actual (clamped)
    /// count on [`state::Fitted`].
    fn n_components(&self) -> usize;
}

/// Principal Component Analysis, state-aware via type parameter.
///
/// Fit with [`Pca::fit`], then project new data with [`Pca::transform`].
///
/// `transform` and the basis accessors exist only on `Pca<Fitted>`,
/// so projecting before fitting is a compile error rather than a runtime one:
///
/// ```compile_fail,E0599
/// use flow_dimensional_reduction::Pca;
/// let data = vec![1.0_f32, 2.0, 3.0, 4.0];
/// // `transform` does not exist on an unfitted model.
/// let _ = Pca::new(1).transform(&data, 2, 2);
/// ```
///
/// The default type parameter keeps `Pca::new(k)` working without a turbofish.
#[derive(Debug, Clone)]
pub struct Pca<C: PcaComponent = Unfitted> {
    state: C,
}

impl Pca<Unfitted> {
    /// Create an unfitted PCA requesting `n_components` components.
    ///
    /// The actual count is clamped to the feature count `d` during [`Pca::fit`].
    #[must_use]
    pub fn new(n_components: usize) -> Self {
        Pca { state: Unfitted { n_components } }
    }

    /// Fit to `n × d` row-major data, consuming the unfitted model.
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
    pub fn fit(self, data: &[f32], n: usize, d: usize) -> PcaResult<Pca<Fitted>> {
        if n == 0 || d == 0 {
            return Err(PcaError::EmptyData);
        }
        let expected_len = n
            .checked_mul(d)
            .ok_or(PcaError::DimensionMismatch { len: data.len(), n, d })?;
        if data.len() != expected_len {
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

        let k = self.state.n_components.min(d);

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

        // Row i of `components` is the i-th principal axis (column i of U),
        // stored flat row-major: axis i occupies [i*d .. (i+1)*d].
        let mut components = vec![0.0_f32; k * d];
        for i in 0..k {
            for j in 0..d {
                components[i * d + j] = *u.get(j, i) as f32;
            }
        }

        // For a covariance matrix the singular values ARE the variances.
        let total: f64 = sigma.iter().sum();
        let explained_variance_ratio: Vec<f32> = if total > 0.0 {
            sigma.iter().take(k).map(|&s| (s / total) as f32).collect()
        } else {
            vec![0.0; k]
        };

        let mean = mean64.into_iter().map(|m| m as f32).collect();

        Ok(Pca { state: Fitted { n_components: k, components, explained_variance_ratio, mean } })
    }
}

impl Pca<Fitted> {
    /// Project `n × d` row-major data onto the fitted axes.
    ///
    /// Returns `n × n_components` row-major.
    ///
    /// # Errors
    /// [`PcaError::DimensionMismatch`] if `data.len() != n * d`;
    /// [`PcaError::FeatureMismatch`] if `d` differs from the fitted feature count.
    pub fn transform(&self, data: &[f32], n: usize, d: usize) -> PcaResult<Vec<f32>> {
        let expected_len = n
            .checked_mul(d)
            .ok_or(PcaError::DimensionMismatch { len: data.len(), n, d })?;
        if data.len() != expected_len {
            return Err(PcaError::DimensionMismatch { len: data.len(), n, d });
        }
        if d != self.state.mean.len() {
            return Err(PcaError::FeatureMismatch { fitted: self.state.mean.len(), actual: d });
        }

        let k = self.state.n_components;
        // `k <= d` (clamped in `fit`) and `n * d` did not overflow above, so
        // `n * k` cannot overflow either.
        let mut out = Vec::with_capacity(n * k);
        for row in data.chunks_exact(d) {
            for i in 0..k {
                let mut acc = 0.0_f32;
                for (j, (&x, &m)) in row.iter().zip(self.state.mean.iter()).enumerate() {
                    acc += (x - m) * self.state.components[i * d + j];
                }
                out.push(acc);
            }
        }
        Ok(out)
    }

    /// Principal axes, `k * d` row-major: axis `i` occupies `[i*d .. (i+1)*d]`.
    #[must_use]
    pub fn components(&self) -> &[f32] {
        &self.state.components
    }

    /// Shape of [`Self::components`] as `(k, d)`.
    #[must_use]
    pub fn components_shape(&self) -> (usize, usize) {
        (self.state.n_components, self.state.mean.len())
    }

    /// Fraction of total variance per component, descending.
    #[must_use]
    pub fn explained_variance_ratio(&self) -> &[f32] {
        &self.state.explained_variance_ratio
    }

    /// Column means of the training data.
    #[must_use]
    pub fn mean(&self) -> &[f32] {
        &self.state.mean
    }
}

/// Available in every state — delegates to the trait method, since a generic
/// `C` cannot be pattern-matched.
impl<C: PcaComponent> Pca<C> {
    /// Component count: requested before fitting, actual (clamped) after.
    #[must_use]
    pub fn n_components(&self) -> usize {
        self.state.n_components()
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

    /// Non-degenerate 3-D data: points scaled along the direction `(1, 2, 3)`.
    /// The covariance has a single nonzero eigenvalue, so PC1 is pinned up to
    /// sign to `(1, 2, 3) / sqrt(14)` — no SVD sign-convention ambiguity
    /// beyond an overall flip, and no degenerate subspace to hide a bug in.
    ///
    /// A 2-D fixture cannot do this job: for *any* 2x2 orthogonal matrix,
    /// `|U(0,1)| == |U(1,0)|` is a structural identity (both rows and
    /// columns of an orthogonal matrix are orthonormal), so comparing
    /// `.abs()` values is blind to swapping `u.get(j, i)` for `u.get(i, j)`.
    fn diagonal_fixture() -> (Vec<f32>, usize, usize) {
        let dir = [1.0_f32, 2.0, 3.0];
        let mut data = Vec::new();
        for t in [-2.0_f32, -1.0, 0.0, 1.0, 2.0] {
            for &c in &dir {
                data.push(t * c);
            }
        }
        (data, 5, 3)
    }

    #[test]
    fn u_column_maps_to_matching_principal_axis() {
        // Pins `components[(i, j)] = u.get(j, i)`. Transposing to
        // `u.get(i, j)` pulls components 1 and 2 from the *degenerate*
        // zero-eigenvalue subspace of U's first row, which will not equal
        // these values, so the mutation is caught.
        let (data, n, d) = diagonal_fixture();
        let pca = Pca::new(1).fit(&data, n, d).expect("fit");
        let norm = 14.0_f32.sqrt();
        assert_eq!(pca.components_shape(), (1, 3), "k=1, d=3");
        // k == 1, so axis 0 occupies the entire flat slice: indices 0..d.
        let c = pca.components();
        approx::assert_abs_diff_eq!(c[0].abs(), 1.0 / norm, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(c[1].abs(), 2.0 / norm, epsilon = 1e-4);
        approx::assert_abs_diff_eq!(c[2].abs(), 3.0 / norm, epsilon = 1e-4);
    }

    #[test]
    fn transform_centers_training_mean_to_zero() {
        // Pins the `- mean[j]` term in `transform`: projecting the training
        // centroid itself must land at the origin on every axis. Dropping
        // the subtraction shifts every projection by the same constant,
        // which the cluster-separation test cannot see (it only compares
        // `left - right`), but this test does.
        let (data, n, d) = fixture();
        let pca = Pca::new(2).fit(&data, n, d).expect("fit");
        let mean_row = pca.mean().to_vec();
        let out = pca.transform(&mean_row, 1, d).expect("transform");
        for v in out {
            approx::assert_abs_diff_eq!(v, 0.0_f32, epsilon = 1e-4);
        }
    }

    #[test]
    fn fit_rejects_length_that_would_overflow_n_times_d() {
        // n * d wraps to 0 in release mode; checked_mul must reject this
        // instead of accepting an empty slice as a valid `usize::MAX x 2`
        // input.
        assert!(matches!(
            Pca::new(1).fit(&[], usize::MAX, 2),
            Err(PcaError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn transform_rejects_length_that_would_overflow_n_times_d() {
        let (data, n, d) = fixture();
        let pca = Pca::new(1).fit(&data, n, d).expect("fit");
        assert!(matches!(
            pca.transform(&[], usize::MAX, 2),
            Err(PcaError::DimensionMismatch { .. })
        ));
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
