//! Principal Component Analysis (PCA) module

use linfa_linalg::svd::SVD;
use ndarray::{Array2, Axis, s};
use thiserror::Error;

/// Error type for PCA operations
#[derive(Error, Debug)]
pub enum PcaError {
    #[error("Empty data")]
    EmptyData,
    #[error("Insufficient data: need at least {min} points, got {actual}")]
    InsufficientData { min: usize, actual: usize },
    #[error("Invalid number of components: {0}")]
    InvalidComponents(String),
    #[error("SVD decomposition failed: {0}")]
    SvdFailed(String),
}

pub type PcaResult<T> = Result<T, PcaError>;

/// Principal Component Analysis
#[derive(Debug)]
pub struct Pca {
    /// Number of components
    n_components: usize,
    /// Principal components (eigenvectors)
    components: Array2<f64>,
    /// Explained variance ratio for each component
    explained_variance_ratio: Vec<f64>,
    /// Mean of the input data (for centering)
    mean: ndarray::Array1<f64>,
}

impl Pca {
    /// Create a new PCA instance
    ///
    /// # Arguments
    /// * `n_components` - Number of components to keep
    pub fn new(n_components: usize) -> Self {
        Self {
            n_components,
            components: Array2::zeros((0, 0)),
            explained_variance_ratio: Vec::new(),
            mean: ndarray::Array1::zeros(0),
        }
    }

    /// Fit PCA to data
    ///
    /// # Arguments
    /// * `data` - Input data matrix (n_samples × n_features)
    ///
    /// # Returns
    /// Fitted Pca instance
    pub fn fit(mut self, data: &Array2<f64>) -> PcaResult<Self> {
        if data.nrows() == 0 {
            return Err(PcaError::EmptyData);
        }

        let n_samples = data.nrows();
        let n_features = data.ncols();

        if n_samples < 2 {
            return Err(PcaError::InsufficientData {
                min: 2,
                actual: n_samples,
            });
        }

        // Center the data
        let mean = data
            .mean_axis(Axis(0))
            .ok_or_else(|| PcaError::SvdFailed("Failed to calculate mean".to_string()))?;
        let mut centered = data.clone();
        for mut row in centered.rows_mut() {
            row -= &mean;
        }

        // Perform SVD using linfa-linalg (compatible with ndarray 0.16)
        // SVD returns (Option<U>, S, Option<Vt>) tuple
        let (u_opt, s, vt_opt) = centered
            .svd(true, true)
            .map_err(|e| PcaError::SvdFailed(format!("SVD failed: {:?}", e)))?;

        let _u = u_opt.ok_or_else(|| PcaError::SvdFailed("U matrix not available".to_string()))?;
        let vt =
            vt_opt.ok_or_else(|| PcaError::SvdFailed("Vt matrix not available".to_string()))?;

        // Extract components (right singular vectors, transposed)
        // vt is already an Array2, not an Option
        let components = vt;

        // Calculate explained variance ratio
        let s_squared: Vec<f64> = s.iter().map(|&val| val * val).collect();
        let total_variance: f64 = s_squared.iter().sum();
        let explained_variance_ratio: Vec<f64> =
            s_squared.iter().map(|&val| val / total_variance).collect();

        // Limit to n_components
        let n_components = self.n_components.min(n_features);
        let components = components.slice(s![..n_components, ..]).to_owned();
        let explained_variance_ratio = explained_variance_ratio[..n_components].to_vec();

        self.n_components = n_components;
        self.components = components;
        self.explained_variance_ratio = explained_variance_ratio;
        self.mean = mean;

        Ok(self)
    }

    /// Transform data to principal component space
    ///
    /// # Arguments
    /// * `data` - Input data matrix (n_samples × n_features)
    ///
    /// # Returns
    /// Transformed data (n_samples × n_components)
    pub fn transform(&self, data: &Array2<f64>) -> PcaResult<Array2<f64>> {
        if data.nrows() == 0 {
            return Err(PcaError::EmptyData);
        }

        // Center the data
        let mut centered = data.clone();
        for mut row in centered.rows_mut() {
            row -= &self.mean;
        }

        // Project onto principal components
        let transformed = centered.dot(&self.components.t());

        Ok(transformed)
    }

    /// Get principal components
    pub fn components(&self) -> &Array2<f64> {
        &self.components
    }

    /// Get explained variance ratio
    pub fn explained_variance_ratio(&self) -> &[f64] {
        &self.explained_variance_ratio
    }

    /// Get mean of training data
    pub fn mean(&self) -> &ndarray::Array1<f64> {
        &self.mean
    }
}
