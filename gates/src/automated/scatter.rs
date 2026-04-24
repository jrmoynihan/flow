//! Automated scatter gating (FSC vs SSC)
//!
//! Provides algorithms for automatically identifying viable cell populations
//! in scatter plots, supporting multi-population detection.

use crate::geometry::{create_ellipse_geometry, create_polygon_geometry};
use crate::polygon::point_in_polygon;
use crate::{Gate, GateError, GateResult, GateStatistics};
use flow_fcs::Fcs;
use flow_plots::contour::contour_paths_at_threshold;
use flow_utils::kde::KernelDensity2D;
use ndarray::Array2;
use std::sync::Arc;

/// Configuration for scatter gating
#[derive(Debug, Clone)]
pub struct ScatterGateConfig {
    /// FSC channel name
    pub fsc_channel: String,
    /// SSC channel name
    pub ssc_channel: String,
    /// Gating method to use
    pub method: ScatterGateMethod,
    /// Minimum number of events required
    pub min_events: usize,
    /// Density threshold (for density-based methods)
    pub density_threshold: Option<f64>,
    /// Cluster epsilon (for DBSCAN)
    pub cluster_eps: Option<f64>,
    /// Minimum samples for clustering
    pub cluster_min_samples: Option<usize>,
}

/// Scatter gating method
#[derive(Debug, Clone)]
pub enum ScatterGateMethod {
    /// Density contour-based gating
    DensityContour { threshold: f64 },
    /// Clustering-based gating
    Clustering { algorithm: ClusterAlgorithm },
    /// Ellipse fitting to main population
    EllipseFit,
}

/// Clustering algorithm for scatter gating
#[derive(Debug, Clone, Copy)]
pub enum ClusterAlgorithm {
    /// K-means clustering
    KMeans,
    /// DBSCAN clustering
    Dbscan,
    /// Gaussian Mixture Model
    Gmm,
}

/// Result of scatter gating
#[derive(Debug, Clone)]
pub struct ScatterGateResult {
    /// Generated gate (if successful)
    pub gate: Option<Gate>,
    /// Population mask (true = inside gate)
    pub population_mask: Vec<bool>,
    /// Statistics about the gated population
    pub statistics: GateStatistics,
    /// Method used for gating
    pub method_used: String,
}

/// Thresholds for flagging scatter gating outcomes that may warrant a fallback (e.g. FSC-only debris).
#[derive(Debug, Clone)]
pub struct ScatterQualityPolicy {
    /// Below this fraction of events kept inside the gate, the result is suspicious (too aggressive or broken).
    pub min_retention_fraction: f64,
    /// Above this fraction kept, the result is suspicious (almost no exclusion).
    pub max_retention_fraction: f64,
}

impl Default for ScatterQualityPolicy {
    fn default() -> Self {
        Self {
            min_retention_fraction: 0.02,
            max_retention_fraction: 0.999,
        }
    }
}

impl ScatterGateResult {
    /// Fraction of events marked inside the gate (`population_mask == true`).
    pub fn retention_fraction(&self) -> f64 {
        let n = self.population_mask.len();
        if n == 0 {
            return 0.0;
        }
        self.population_mask
            .iter()
            .filter(|&&inside| inside)
            .count() as f64
            / n as f64
    }

    /// Whether retention falls outside `[min_retention_fraction, max_retention_fraction]`.
    pub fn is_suspicious(&self, policy: &ScatterQualityPolicy) -> bool {
        let r = self.retention_fraction();
        r < policy.min_retention_fraction || r > policy.max_retention_fraction
    }
}

// GateStatistics is imported from crate::statistics

/// Create automated scatter gate
///
/// # Arguments
/// * `fcs` - FCS file data
/// * `config` - Scatter gate configuration
///
/// # Returns
/// ScatterGateResult with gate and statistics
pub fn create_scatter_gate(fcs: &Fcs, config: &ScatterGateConfig) -> GateResult<ScatterGateResult> {
    // Extract FSC/SSC data (NO transformation - raw values)
    // Fcs API returns f32 slices, convert to f64 for processing
    let fsc_data_f32 = fcs
        .get_parameter_events_slice(&config.fsc_channel)
        .map_err(|e| GateError::Other {
            message: format!("Failed to get FSC channel {}: {}", config.fsc_channel, e),
            source: None, // anyhow::Error doesn't implement StdError, use message only
        })?;
    let ssc_data_f32 = fcs
        .get_parameter_events_slice(&config.ssc_channel)
        .map_err(|e| GateError::Other {
            message: format!("Failed to get SSC channel {}: {}", config.ssc_channel, e),
            source: None, // anyhow::Error doesn't implement StdError, use message only
        })?;

    // Convert to f64 for processing
    let fsc_data: Vec<f64> = fsc_data_f32.iter().map(|&x| x as f64).collect();
    let ssc_data: Vec<f64> = ssc_data_f32.iter().map(|&x| x as f64).collect();

    if fsc_data.len() != ssc_data.len() {
        return Err(GateError::Other {
            message: format!(
                "FSC and SSC channels have different lengths: {} vs {}",
                fsc_data.len(),
                ssc_data.len()
            ),
            source: None,
        });
    }

    if fsc_data.len() < config.min_events {
        return Err(GateError::Other {
            message: format!(
                "Insufficient data: need at least {} events, got {}",
                config.min_events,
                fsc_data.len()
            ),
            source: None,
        });
    }

    // Create 2D data matrix (n_samples × 2 features)
    let n_samples = fsc_data.len();
    let mut data = Array2::<f64>::zeros((n_samples, 2));
    for (i, (&fsc, &ssc)) in fsc_data.iter().zip(ssc_data.iter()).enumerate() {
        data[[i, 0]] = fsc;
        data[[i, 1]] = ssc;
    }

    // Apply gating method
    let (gate, mask, method_name) = match &config.method {
        ScatterGateMethod::DensityContour { threshold } => {
            create_density_contour_gate(&data, config, *threshold)?
        }
        ScatterGateMethod::Clustering { algorithm } => {
            create_clustering_gate(&data, config, *algorithm)?
        }
        ScatterGateMethod::EllipseFit => create_ellipse_fit_gate(&data, config)?,
    };

    // Calculate statistics using GateStatistics
    // Note: GateStatistics::calculate requires a gate
    let statistics = if let Some(ref gate) = gate {
        GateStatistics::calculate(fcs, gate).unwrap_or_else(|_| {
            // Create empty statistics manually if calculation fails
            GateStatistics {
                event_count: 0,
                percentage: 0.0,
                centroid: (0.0, 0.0),
                x_stats: crate::statistics::ParameterStatistics {
                    parameter: config.fsc_channel.clone(),
                    mean: 0.0,
                    geometric_mean: 0.0,
                    median: 0.0,
                    std_dev: 0.0,
                    min: 0.0,
                    max: 0.0,
                    q1: 0.0,
                    q3: 0.0,
                    cv: 0.0,
                },
                y_stats: crate::statistics::ParameterStatistics {
                    parameter: config.ssc_channel.clone(),
                    mean: 0.0,
                    geometric_mean: 0.0,
                    median: 0.0,
                    std_dev: 0.0,
                    min: 0.0,
                    max: 0.0,
                    q1: 0.0,
                    q3: 0.0,
                    cv: 0.0,
                },
            }
        })
    } else {
        // Create empty statistics if no gate
        GateStatistics {
            event_count: 0,
            percentage: 0.0,
            centroid: (0.0, 0.0),
            x_stats: crate::statistics::ParameterStatistics {
                parameter: config.fsc_channel.clone(),
                mean: 0.0,
                geometric_mean: 0.0,
                median: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                q1: 0.0,
                q3: 0.0,
                cv: 0.0,
            },
            y_stats: crate::statistics::ParameterStatistics {
                parameter: config.ssc_channel.clone(),
                mean: 0.0,
                geometric_mean: 0.0,
                median: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                q1: 0.0,
                q3: 0.0,
                cv: 0.0,
            },
        }
    };

    Ok(ScatterGateResult {
        gate,
        population_mask: mask,
        statistics,
        method_used: method_name,
    })
}

/// Create gate using density contour method
fn create_density_contour_gate(
    data: &Array2<f64>,
    config: &ScatterGateConfig,
    threshold: f64,
) -> GateResult<(Option<Gate>, Vec<bool>, String)> {
    // Extract FSC and SSC values
    let fsc_values: Vec<f64> = data.column(0).iter().copied().collect();
    let ssc_values: Vec<f64> = data.column(1).iter().copied().collect();

    // Use 2D KDE for better density estimation
    let kde2d = KernelDensity2D::estimate(&fsc_values, &ssc_values, 1.0, 128).map_err(|e| {
        GateError::Other {
            message: format!("2D KDE failed: {:?}", e),
            source: None,
        }
    })?;

    // Ordered closed contour path(s) via marching squares (flow-plots)
    let paths = contour_paths_at_threshold(&kde2d, threshold);
    let valid_paths: Vec<_> = paths.into_iter().filter(|p| p.len() >= 3).collect();

    if valid_paths.is_empty() {
        return create_ellipse_fit_gate(data, config);
    }

    // When multiple paths (e.g. disconnected regions), choose the one containing the most events
    let best_path = if valid_paths.len() == 1 {
        valid_paths.into_iter().next().unwrap()
    } else {
        let polygon_f32 = |path: &[(f64, f64)]| {
            path.iter()
                .map(|(x, y)| (*x as f32, *y as f32))
                .collect::<Vec<_>>()
        };
        valid_paths
            .into_iter()
            .max_by_key(|path| {
                let poly = polygon_f32(path);
                (0..data.nrows())
                    .filter(|&i| point_in_polygon(data[[i, 0]] as f32, data[[i, 1]] as f32, &poly))
                    .count()
            })
            .unwrap()
    };

    // Filter out non-finite points (marching squares can produce -inf in edge cases)
    let coords: Vec<(f32, f32)> = best_path
        .iter()
        .map(|(x, y)| (*x as f32, *y as f32))
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect();

    if coords.len() < 3 {
        return create_ellipse_fit_gate(data, config);
    }

    let geometry = create_polygon_geometry(coords.clone(), &config.fsc_channel, &config.ssc_channel)?;

    let gate = Gate::new(
        "scatter-gate",
        "Automated Scatter Gate (Density Contour)",
        geometry,
        Arc::from(config.fsc_channel.as_str()),
        Arc::from(config.ssc_channel.as_str()),
    );

    // Build mask by point-in-polygon against the chosen contour. Using the density
    // threshold here would also keep any dense region (e.g. a debris centroid) outside
    // the selected polygon, which is exactly what the polygon was chosen to exclude.
    let mask: Vec<bool> = (0..data.nrows())
        .map(|i| point_in_polygon(data[[i, 0]] as f32, data[[i, 1]] as f32, &coords))
        .collect();

    Ok((Some(gate), mask, "DensityContour".to_string()))
}

/// Create gate using clustering method
fn create_clustering_gate(
    data: &Array2<f64>,
    config: &ScatterGateConfig,
    algorithm: ClusterAlgorithm,
) -> GateResult<(Option<Gate>, Vec<bool>, String)> {
    use flow_utils::clustering::{Gmm, GmmConfig, KMeans, KMeansConfig};

    match algorithm {
        ClusterAlgorithm::KMeans => {
            // Use K-means to identify main population
            // Convert ndarray 0.17 to Vec<Vec<f64>> for version compatibility
            let data_rows: Vec<Vec<f64>> = (0..data.nrows())
                .map(|i| data.row(i).iter().copied().collect())
                .collect();

            let kmeans_config = KMeansConfig {
                n_clusters: 2, // Main population + debris/noise
                max_iterations: 100,
                tolerance: 1e-4,
                seed: None,
            };

            let result =
                KMeans::fit_from_rows(data_rows, &kmeans_config).map_err(|e| GateError::Other {
                    message: format!("K-means clustering failed: {:?}", e),
                    source: None,
                })?;

            // Find largest cluster (main population)
            let mut cluster_counts = vec![0; result.centroids.nrows()];
            for &assignment in &result.assignments {
                cluster_counts[assignment] += 1;
            }

            let main_cluster = cluster_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.cmp(b))
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            // Create mask for main cluster
            let mask: Vec<bool> = result
                .assignments
                .iter()
                .map(|&cluster| cluster == main_cluster)
                .collect();

            // Create ellipse gate around main cluster
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut count = 0;
            for (i, &in_cluster) in mask.iter().enumerate() {
                if in_cluster {
                    sum_x += data[[i, 0]];
                    sum_y += data[[i, 1]];
                    count += 1;
                }
            }

            if count == 0 {
                return create_ellipse_fit_gate(data, config);
            }

            let center_x = sum_x / count as f64;
            let center_y = sum_y / count as f64;

            // Calculate spread
            let mut sum_dist_x = 0.0;
            let mut sum_dist_y = 0.0;
            for (i, &in_cluster) in mask.iter().enumerate() {
                if in_cluster {
                    sum_dist_x += (data[[i, 0]] - center_x).abs();
                    sum_dist_y += (data[[i, 1]] - center_y).abs();
                }
            }

            let radius_x = sum_dist_x / count as f64 * 2.0;
            let radius_y = sum_dist_y / count as f64 * 2.0;

            // Create ellipse gate
            let center = (center_x as f32, center_y as f32);
            let right = ((center_x + radius_x) as f32, center_y as f32);
            let top = (center_x as f32, (center_y + radius_y) as f32);
            let left = ((center_x - radius_x) as f32, center_y as f32);
            let bottom = (center_x as f32, (center_y - radius_y) as f32);
            let coords = vec![center, right, top, left, bottom];

            let geometry =
                create_ellipse_geometry(coords, &config.fsc_channel, &config.ssc_channel)?;

            let gate = Gate::new(
                "scatter-gate",
                "Automated Scatter Gate (K-means)",
                geometry,
                Arc::from(config.fsc_channel.as_str()),
                Arc::from(config.ssc_channel.as_str()),
            );

            Ok((Some(gate), mask, "Clustering(KMeans)".to_string()))
        }
        ClusterAlgorithm::Gmm => {
            // Use GMM to identify main population
            // Convert ndarray 0.17 to Vec<Vec<f64>> for version compatibility
            let data_rows: Vec<Vec<f64>> = (0..data.nrows())
                .map(|i| data.row(i).iter().copied().collect())
                .collect();

            let gmm_config = GmmConfig {
                n_components: 2,
                max_iterations: 100,
                tolerance: 1e-3,
                seed: None,
            };

            let result =
                Gmm::fit_from_rows(data_rows, &gmm_config).map_err(|e| GateError::Other {
                    message: format!("GMM clustering failed: {:?}", e),
                    source: None,
                })?;

            // Find largest component (main population)
            let mut component_counts = vec![0; result.means.nrows()];
            for &assignment in &result.assignments {
                component_counts[assignment] += 1;
            }

            let main_component = component_counts
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.cmp(b))
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            // Create mask for main component
            let mask: Vec<bool> = result
                .assignments
                .iter()
                .map(|&component| component == main_component)
                .collect();

            // Create ellipse gate around main component (similar to K-means)
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut count = 0;
            for (i, &in_component) in mask.iter().enumerate() {
                if in_component {
                    sum_x += data[[i, 0]];
                    sum_y += data[[i, 1]];
                    count += 1;
                }
            }

            if count == 0 {
                return create_ellipse_fit_gate(data, config);
            }

            let center_x = sum_x / count as f64;
            let center_y = sum_y / count as f64;

            let mut sum_dist_x = 0.0;
            let mut sum_dist_y = 0.0;
            for (i, &in_component) in mask.iter().enumerate() {
                if in_component {
                    sum_dist_x += (data[[i, 0]] - center_x).abs();
                    sum_dist_y += (data[[i, 1]] - center_y).abs();
                }
            }

            let radius_x = sum_dist_x / count as f64 * 2.0;
            let radius_y = sum_dist_y / count as f64 * 2.0;

            let center = (center_x as f32, center_y as f32);
            let right = ((center_x + radius_x) as f32, center_y as f32);
            let top = (center_x as f32, (center_y + radius_y) as f32);
            let left = ((center_x - radius_x) as f32, center_y as f32);
            let bottom = (center_x as f32, (center_y - radius_y) as f32);
            let coords = vec![center, right, top, left, bottom];

            let geometry =
                create_ellipse_geometry(coords, &config.fsc_channel, &config.ssc_channel)?;

            let gate = Gate::new(
                "scatter-gate",
                "Automated Scatter Gate (GMM)",
                geometry,
                Arc::from(config.fsc_channel.as_str()),
                Arc::from(config.ssc_channel.as_str()),
            );

            Ok((Some(gate), mask, "Clustering(GMM)".to_string()))
        }
        ClusterAlgorithm::Dbscan => {
            // DBSCAN is temporarily disabled
            Err(GateError::Other {
                message: "DBSCAN clustering is temporarily unavailable. Please use K-means or GMM."
                    .to_string(),
                source: None,
            })
        }
    }
}

/// Create gate using ellipse fitting
fn create_ellipse_fit_gate(
    data: &Array2<f64>,
    config: &ScatterGateConfig,
) -> GateResult<(Option<Gate>, Vec<bool>, String)> {
    // Calculate center (mean)
    let center_x = data.column(0).iter().sum::<f64>() / data.nrows() as f64;
    let center_y = data.column(1).iter().sum::<f64>() / data.nrows() as f64;

    // Calculate standard deviations for ellipse radii
    let var_x: f64 = data
        .column(0)
        .iter()
        .map(|x| (x - center_x).powi(2))
        .sum::<f64>()
        / data.nrows() as f64;
    let var_y: f64 = data
        .column(1)
        .iter()
        .map(|y| (y - center_y).powi(2))
        .sum::<f64>()
        / data.nrows() as f64;

    // Use 2 standard deviations for ellipse (covers ~95% of data)
    let radius_x = var_x.sqrt() * 2.0;
    let radius_y = var_y.sqrt() * 2.0;

    // Create ellipse gate
    // create_ellipse_geometry expects Vec<(f32, f32)> coordinates
    // Create coordinates: center, right, top, left, bottom
    let center = (center_x as f32, center_y as f32);
    let right = ((center_x + radius_x) as f32, center_y as f32);
    let top = (center_x as f32, (center_y + radius_y) as f32);
    let left = ((center_x - radius_x) as f32, center_y as f32);
    let bottom = (center_x as f32, (center_y - radius_y) as f32);
    let coords = vec![center, right, top, left, bottom];

    let geometry = create_ellipse_geometry(coords, &config.fsc_channel, &config.ssc_channel)?;

    let gate = Gate::new(
        "scatter-gate",
        "Automated Scatter Gate (Ellipse Fit)",
        geometry,
        Arc::from(config.fsc_channel.as_str()),
        Arc::from(config.ssc_channel.as_str()),
    );

    // Create mask
    let mask: Vec<bool> = (0..data.nrows())
        .map(|i| {
            let dx = (data[[i, 0]] - center_x) / radius_x;
            let dy = (data[[i, 1]] - center_y) / radius_y;
            dx * dx + dy * dy <= 1.0
        })
        .collect();

    Ok((Some(gate), mask, "EllipseFit".to_string()))
}
