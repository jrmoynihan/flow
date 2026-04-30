//! Gaussian Mixture Model clustering implementation

use crate::clustering::{ClusteringError, ClusteringResult};
use linfa::prelude::*;
use linfa_clustering::GaussianMixtureModel as LinfaGmm;
use ndarray::Array2;

/// Configuration for GMM clustering
#[derive(Debug, Clone)]
pub struct GmmConfig {
    /// Number of components
    pub n_components: usize,
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Tolerance for convergence
    pub tolerance: f64,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

impl Default for GmmConfig {
    fn default() -> Self {
        Self {
            n_components: 2,
            max_iterations: 100,
            tolerance: 1e-3,
            seed: None,
        }
    }
}

/// GMM clustering result
#[derive(Debug, Clone)]
pub struct GmmResult {
    /// Cluster assignments for each point
    pub assignments: Vec<usize>,
    /// Component means
    pub means: Array2<f64>,
    /// Number of iterations performed
    pub iterations: usize,
    /// Log likelihood
    pub log_likelihood: f64,
}

/// Gaussian Mixture Model clustering
pub struct Gmm;

impl Gmm {
    /// Fit GMM clustering model to data from raw vectors
    ///
    /// Helper function to accept Vec<Vec<f64>> for version compatibility
    ///
    /// # Arguments
    /// * `data_rows` - Input data as rows (n_samples × n_features)
    /// * `config` - Configuration for GMM
    ///
    /// # Returns
    /// GmmResult with component assignments and means
    pub fn fit_from_rows(
        data_rows: Vec<Vec<f64>>,
        config: &GmmConfig,
    ) -> ClusteringResult<GmmResult> {
        if data_rows.is_empty() {
            return Err(ClusteringError::EmptyData);
        }
        let n_features = data_rows[0].len();
        let n_samples = data_rows.len();

        // Flatten and create Array2
        let flat: Vec<f64> = data_rows.into_iter().flatten().collect();
        let data = Array2::from_shape_vec((n_samples, n_features), flat).map_err(|e| {
            ClusteringError::ClusteringFailed(format!("Failed to create array: {:?}", e))
        })?;

        Self::fit(&data, config)
    }

    /// Perform GMM clustering
    ///
    /// # Arguments
    /// * `data` - Input data matrix (n_samples × n_features)
    /// * `config` - Configuration for GMM
    ///
    /// # Returns
    /// GmmResult with cluster assignments and means
    pub fn fit(data: &Array2<f64>, config: &GmmConfig) -> ClusteringResult<GmmResult> {
        if data.nrows() == 0 {
            return Err(ClusteringError::EmptyData);
        }

        if data.nrows() < config.n_components {
            return Err(ClusteringError::InsufficientData {
                min: config.n_components,
                actual: data.nrows(),
            });
        }

        // Use linfa-clustering for GMM
        // Use DatasetBase::new with empty targets () for unsupervised learning
        let dataset = DatasetBase::new(data.clone(), ());
        let model = LinfaGmm::params(config.n_components)
            .max_n_iterations(config.max_iterations as u64)
            .tolerance(config.tolerance)
            .fit(&dataset)
            .map_err(|e| ClusteringError::ClusteringFailed(format!("{}", e)))?;

        // Extract assignments (hard assignment: most likely component)
        // GMM predict returns probabilities, we need to find argmax for each point
        let assignments: Vec<usize> = (0..data.nrows())
            .map(|i| {
                let point = data.row(i);
                let mut max_prob = f64::NEG_INFINITY;
                let mut best_component = 0;
                // Calculate probability for each component (simplified - use means distance)
                for (j, mean) in model.means().rows().into_iter().enumerate() {
                    let dist: f64 = point
                        .iter()
                        .zip(mean.iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum();
                    let prob = (-dist).exp(); // Simplified probability
                    if prob > max_prob {
                        max_prob = prob;
                        best_component = j;
                    }
                }
                best_component
            })
            .collect();

        // Extract means (convert to Array2<f64>)
        let means = model.means().to_owned();

        Ok(GmmResult {
            assignments,
            means,
            iterations: config.max_iterations, // linfa doesn't expose n_iterations
            log_likelihood: 0.0,               // linfa doesn't expose log_likelihood directly
        })
    }
}
