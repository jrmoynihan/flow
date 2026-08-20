//! Self-organizing maps and FlowSOM-style metaclustering.
//!
//! Batch SOM trains a rectangular codebook; [`FlowSom`] then clusters those
//! nodes (k-means) and maps each event through its best-matching unit (BMU).

use crate::clustering::{ClusteringError, ClusteringResult, KMeans, KMeansConfig, KMeansResult};
use ndarray::Array2;

/// Configuration for a rectangular batch SOM.
#[derive(Debug, Clone)]
pub struct SomConfig {
    /// Grid width (nodes along X).
    pub width: usize,
    /// Grid height (nodes along Y).
    pub height: usize,
    /// Full-data assignment / update passes.
    pub n_epochs: usize,
    /// Initial neighborhood radius in grid units. `None` → max(width, height) / 2.
    pub radius: Option<f64>,
    /// RNG seed used only to pick initial codebook rows from the data.
    pub seed: Option<u64>,
}

impl Default for SomConfig {
    fn default() -> Self {
        Self {
            width: 10,
            height: 10,
            n_epochs: 10,
            radius: None,
            seed: Some(42),
        }
    }
}

/// Trained SOM codebook plus per-event BMU indices.
#[derive(Debug, Clone)]
pub struct SomResult {
    /// Codebook: `n_nodes × n_features`.
    pub weights: Array2<f64>,
    /// Best-matching unit index for each input row.
    pub assignments: Vec<usize>,
    pub grid_width: usize,
    pub grid_height: usize,
}

impl SomResult {
    pub fn n_nodes(&self) -> usize {
        self.grid_width.saturating_mul(self.grid_height)
    }
}

/// Batch self-organizing map.
pub struct Som;

impl Som {
    /// Fit a rectangular SOM to `n_samples × n_features` data.
    pub fn fit(data: &Array2<f64>, config: &SomConfig) -> ClusteringResult<SomResult> {
        if data.nrows() == 0 || data.ncols() == 0 {
            return Err(ClusteringError::EmptyData);
        }
        if config.width == 0 || config.height == 0 {
            return Err(ClusteringError::InvalidConfig(
                "SOM width and height must be >= 1".into(),
            ));
        }
        if config.n_epochs == 0 {
            return Err(ClusteringError::InvalidConfig(
                "SOM n_epochs must be >= 1".into(),
            ));
        }

        let d = data.ncols();
        let n_nodes = config.width * config.height;

        let mut weights = init_weights(data, n_nodes, config.seed.unwrap_or(0));
        let sigma0 = config
            .radius
            .unwrap_or((config.width.max(config.height) as f64) / 2.0)
            .max(0.5);
        let width = config.width;

        for epoch in 0..config.n_epochs {
            let frac = (config.n_epochs - epoch) as f64 / config.n_epochs as f64;
            let sigma = (sigma0 * frac).max(0.5);
            let two_sigma2 = 2.0 * sigma * sigma;

            let mut numer = Array2::<f64>::zeros((n_nodes, d));
            let mut denom = vec![0.0_f64; n_nodes];

            for row in data.rows() {
                let bmu = best_matching_unit(&weights, row);
                for j in 0..n_nodes {
                    let h = (-grid_dist2(bmu, j, width) / two_sigma2).exp();
                    if h < 1e-6 {
                        continue;
                    }
                    denom[j] += h;
                    for c in 0..d {
                        numer[(j, c)] += h * row[c];
                    }
                }
            }

            for j in 0..n_nodes {
                if denom[j] > 0.0 {
                    let inv = 1.0 / denom[j];
                    for c in 0..d {
                        weights[(j, c)] = numer[(j, c)] * inv;
                    }
                }
            }
        }

        let assignments: Vec<usize> = data
            .rows()
            .into_iter()
            .map(|row| best_matching_unit(&weights, row))
            .collect();

        Ok(SomResult {
            weights,
            assignments,
            grid_width: config.width,
            grid_height: config.height,
        })
    }
}

/// FlowSOM: SOM codebook followed by k-means metaclustering of the nodes.
#[derive(Debug, Clone)]
pub struct FlowSomConfig {
    pub som: SomConfig,
    /// Number of metaclusters on the codebook (and therefore event labels).
    pub n_metaclusters: usize,
    pub meta_max_iterations: usize,
    pub meta_seed: Option<u64>,
}

impl Default for FlowSomConfig {
    fn default() -> Self {
        Self {
            som: SomConfig::default(),
            n_metaclusters: 10,
            meta_max_iterations: 100,
            meta_seed: Some(42),
        }
    }
}

/// FlowSOM result: SOM plus node/event metacluster labels and event centroids.
#[derive(Debug, Clone)]
pub struct FlowSomResult {
    pub som: SomResult,
    /// Metacluster id for each SOM node.
    pub node_metaclusters: Vec<usize>,
    /// Metacluster id for each event (via BMU).
    pub event_metaclusters: Vec<usize>,
    /// Event-mean centroid per occupied metacluster (`k × d`). Empty clusters
    /// fall back to the mean of their node weights.
    pub metacluster_centroids: Array2<f64>,
}

/// FlowSOM (SOM + codebook k-means).
pub struct FlowSom;

impl FlowSom {
    /// Train a SOM then metacluster the codebook with k-means.
    pub fn fit(data: &Array2<f64>, config: &FlowSomConfig) -> ClusteringResult<FlowSomResult> {
        if config.n_metaclusters == 0 {
            return Err(ClusteringError::InvalidConfig(
                "n_metaclusters must be >= 1".into(),
            ));
        }
        let som = Som::fit(data, &config.som)?;
        let n_nodes = som.n_nodes();
        let k = config.n_metaclusters.min(n_nodes).min(data.nrows()).max(1);

        let km_cfg = KMeansConfig {
            n_clusters: k,
            max_iterations: config.meta_max_iterations,
            tolerance: 1e-4,
            seed: config.meta_seed,
        };
        let KMeansResult {
            assignments: node_metaclusters,
            ..
        } = KMeans::fit(&som.weights, &km_cfg)?;

        let d = data.ncols();
        let event_metaclusters: Vec<usize> = som
            .assignments
            .iter()
            .map(|&bmu| node_metaclusters[bmu])
            .collect();

        let mut sums = Array2::<f64>::zeros((k, d));
        let mut counts = vec![0.0_f64; k];
        for (row, &mc) in data.rows().into_iter().zip(event_metaclusters.iter()) {
            if mc >= k {
                continue;
            }
            counts[mc] += 1.0;
            for c in 0..d {
                sums[(mc, c)] += row[c];
            }
        }

        // Empty metaclusters: mean of their codebook nodes.
        let mut node_sums = Array2::<f64>::zeros((k, d));
        let mut node_counts = vec![0.0_f64; k];
        for (j, &mc) in node_metaclusters.iter().enumerate() {
            if mc >= k {
                continue;
            }
            node_counts[mc] += 1.0;
            for c in 0..d {
                node_sums[(mc, c)] += som.weights[(j, c)];
            }
        }

        let mut metacluster_centroids = Array2::<f64>::zeros((k, d));
        for mc in 0..k {
            if counts[mc] > 0.0 {
                let inv = 1.0 / counts[mc];
                for c in 0..d {
                    metacluster_centroids[(mc, c)] = sums[(mc, c)] * inv;
                }
            } else if node_counts[mc] > 0.0 {
                let inv = 1.0 / node_counts[mc];
                for c in 0..d {
                    metacluster_centroids[(mc, c)] = node_sums[(mc, c)] * inv;
                }
            }
        }

        Ok(FlowSomResult {
            som,
            node_metaclusters,
            event_metaclusters,
            metacluster_centroids,
        })
    }
}

fn init_weights(data: &Array2<f64>, n_nodes: usize, seed: u64) -> Array2<f64> {
    let n = data.nrows();
    let d = data.ncols();
    let mut weights = Array2::<f64>::zeros((n_nodes, d));
    for i in 0..n_nodes {
        let row = pick_row(n, i, seed);
        weights.row_mut(i).assign(&data.row(row));
    }
    weights
}

fn pick_row(n: usize, i: usize, seed: u64) -> usize {
    let hash = seed
        .wrapping_add(i as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (hash as usize) % n.max(1)
}

fn best_matching_unit(weights: &Array2<f64>, x: ndarray::ArrayView1<'_, f64>) -> usize {
    let d = weights.ncols();
    let mut best = 0usize;
    let mut best_dist = f64::INFINITY;
    for j in 0..weights.nrows() {
        let mut dist = 0.0;
        for c in 0..d {
            let diff = weights[(j, c)] - x[c];
            dist += diff * diff;
        }
        if dist < best_dist {
            best_dist = dist;
            best = j;
        }
    }
    best
}

fn grid_dist2(i: usize, j: usize, width: usize) -> f64 {
    let (ri, ci) = (i / width, i % width);
    let (rj, cj) = (j / width, j % width);
    let dr = ri as f64 - rj as f64;
    let dc = ci as f64 - cj as f64;
    dr * dr + dc * dc
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    fn two_blobs() -> Array2<f64> {
        let mut data = Array2::<f64>::zeros((80, 2));
        for i in 0..40 {
            data[(i, 0)] = 0.0;
            data[(i, 1)] = 0.0;
        }
        for i in 40..80 {
            data[(i, 0)] = 10.0;
            data[(i, 1)] = 10.0;
        }
        data
    }

    #[test]
    fn som_maps_separated_blobs_to_distinct_nodes() {
        let data = two_blobs();
        let cfg = SomConfig {
            width: 4,
            height: 4,
            n_epochs: 8,
            radius: Some(2.0),
            seed: Some(1),
        };
        let som = Som::fit(&data, &cfg).expect("som");
        assert_eq!(som.assignments.len(), 80);
        let a = som.assignments[0];
        let b = som.assignments[40];
        assert_ne!(a, b, "well-separated blobs should not share a BMU");
    }

    #[test]
    fn flowsom_recovers_two_metaclusters() {
        let data = two_blobs();
        let cfg = FlowSomConfig {
            som: SomConfig {
                width: 4,
                height: 4,
                n_epochs: 8,
                radius: Some(2.0),
                seed: Some(3),
            },
            n_metaclusters: 2,
            meta_max_iterations: 50,
            meta_seed: Some(3),
        };
        let fs = FlowSom::fit(&data, &cfg).expect("flowsom");
        assert_eq!(fs.event_metaclusters.len(), 80);
        assert_eq!(fs.metacluster_centroids.nrows(), 2);
        let left = fs.event_metaclusters[0];
        let right = fs.event_metaclusters[40];
        assert_ne!(left, right);
        let majority_left = fs.event_metaclusters[..40]
            .iter()
            .filter(|c| **c == left)
            .count();
        let majority_right = fs.event_metaclusters[40..]
            .iter()
            .filter(|c| **c == right)
            .count();
        assert!(majority_left >= 30, "left blob mixed: {majority_left}/40");
        assert!(
            majority_right >= 30,
            "right blob mixed: {majority_right}/40"
        );
    }

    #[test]
    fn som_rejects_zero_grid() {
        let data = Array2::<f64>::zeros((4, 2));
        let cfg = SomConfig {
            width: 0,
            height: 2,
            ..SomConfig::default()
        };
        assert!(Som::fit(&data, &cfg).is_err());
    }
}
