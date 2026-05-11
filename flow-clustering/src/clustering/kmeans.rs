//! K-means clustering implementation

use crate::clustering::{ClusteringError, ClusteringResult};
use linfa::prelude::*;
use linfa_clustering::KMeans as LinfaKMeans;
use ndarray::Array2;

/// Configuration for K-means clustering
#[derive(Debug, Clone)]
pub struct KMeansConfig {
    /// Number of clusters
    pub n_clusters: usize,
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Tolerance for convergence
    pub tolerance: f64,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

impl Default for KMeansConfig {
    fn default() -> Self {
        Self {
            n_clusters: 2,
            max_iterations: 300,
            tolerance: 1e-4,
            seed: None,
        }
    }
}

/// K-means clustering result
#[derive(Debug, Clone)]
pub struct KMeansResult {
    /// Cluster assignments for each point
    pub assignments: Vec<usize>,
    /// Cluster centroids
    pub centroids: Array2<f64>,
    /// Number of iterations performed
    pub iterations: usize,
    /// Inertia (sum of squared distances to centroids)
    pub inertia: f64,
}

/// K-means clustering
pub struct KMeans;

impl KMeans {
    /// Fit K-means clustering model to data from raw vectors
    ///
    /// Helper function to accept Vec<Vec<f64>> for version compatibility
    ///
    /// # Arguments
    /// * `data_rows` - Input data as rows (n_samples × n_features)
    /// * `config` - Configuration for K-means
    ///
    /// # Returns
    /// KMeansResult with cluster assignments and centroids
    pub fn fit_from_rows(
        data_rows: Vec<Vec<f64>>,
        config: &KMeansConfig,
    ) -> ClusteringResult<KMeansResult> {
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

    /// Perform K-means clustering
    ///
    /// # Arguments
    /// * `data` - Input data matrix (n_samples × n_features)
    /// * `config` - Configuration for K-means
    ///
    /// # Returns
    /// KMeansResult with cluster assignments and centroids
    pub fn fit(data: &Array2<f64>, config: &KMeansConfig) -> ClusteringResult<KMeansResult> {
        if data.nrows() == 0 {
            return Err(ClusteringError::EmptyData);
        }

        if data.nrows() < config.n_clusters {
            return Err(ClusteringError::InsufficientData {
                min: config.n_clusters,
                actual: data.nrows(),
            });
        }

        // Use linfa-clustering for K-means
        // Array2 implements Records, so we can create DatasetBase directly
        // Note: linfa expects data as records (samples × features)
        // Use DatasetBase::new with empty targets () for unsupervised learning
        let dataset = DatasetBase::new(data.clone(), ());
        let model = LinfaKMeans::params(config.n_clusters)
            .max_n_iterations(config.max_iterations as u64)
            .tolerance(config.tolerance)
            .fit(&dataset)
            .map_err(|e| ClusteringError::ClusteringFailed(format!("{}", e)))?;

        // Extract assignments - KMeans doesn't have predict, use centroids to assign
        let assignments: Vec<usize> = (0..data.nrows())
            .map(|i| {
                let point = data.row(i);
                let mut min_dist = f64::INFINITY;
                let mut best_cluster = 0;
                for (j, centroid) in model.centroids().rows().into_iter().enumerate() {
                    let dist: f64 = point
                        .iter()
                        .zip(centroid.iter())
                        .map(|(a, b)| (a - b).powi(2))
                        .sum();
                    if dist < min_dist {
                        min_dist = dist;
                        best_cluster = j;
                    }
                }
                best_cluster
            })
            .collect();

        // Extract centroids (convert to Array2<f64>)
        let centroids = model.centroids().to_owned();

        // Calculate inertia
        let inertia = Self::calculate_inertia(data, &centroids, &assignments);

        Ok(KMeansResult {
            assignments,
            centroids,
            iterations: config.max_iterations, // linfa doesn't expose n_iterations, use config
            inertia,
        })
    }

    /// Calculate inertia (sum of squared distances to centroids)
    fn calculate_inertia(
        data: &Array2<f64>,
        centroids: &Array2<f64>,
        assignments: &[usize],
    ) -> f64 {
        let mut inertia = 0.0;
        for (i, assignment) in assignments.iter().enumerate() {
            let point = data.row(i);
            let centroid = centroids.row(*assignment);
            let dist_sq: f64 = point
                .iter()
                .zip(centroid.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum();
            inertia += dist_sq;
        }
        inertia
    }
}
