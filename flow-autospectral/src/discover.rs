//! Build an AF library from unstained (cleaned) fluorescence events.

use crate::config::{DiscoverConfig, DiscoveryBackend};
use crate::error::{AutospectralError, Result};
use crate::library::{merge_near_duplicates, normalize_unit_peak, AfLibrary};
use faer::Mat;
use flow_clustering::{Gmm, GmmConfig, KMeans, KMeansConfig, silhouette_scores_sampled};
use ndarray::Array2;

/// Discover AF signatures from row-major `n_events × n_detectors` fluorescence.
pub fn discover_af_library(
    events_row_major: &[f64],
    n_events: usize,
    n_detectors: usize,
    detector_names: &[String],
    config: &DiscoverConfig,
) -> Result<AfLibrary> {
    if n_events == 0 {
        return Err(AutospectralError::EmptyEvents);
    }
    if n_detectors == 0 || events_row_major.len() != n_events * n_detectors {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_detectors,
            got: events_row_major.len().checked_div(n_events.max(1)).unwrap_or(0),
        });
    }
    if detector_names.len() != n_detectors {
        return Err(AutospectralError::InvalidConfig(format!(
            "detector_names len {} != n_detectors {n_detectors}",
            detector_names.len()
        )));
    }
    if config.k_min == 0 || config.k_max < config.k_min {
        return Err(AutospectralError::InvalidConfig(
            "k_min must be >= 1 and k_max >= k_min".into(),
        ));
    }

    let data = Array2::from_shape_vec((n_events, n_detectors), events_row_major.to_vec())
        .map_err(|e| AutospectralError::InvalidConfig(e.to_string()))?;

    let k = choose_k(&data, config)?;
    let means = fit_means(&data, k, config)?;
    let library = means_to_library(means, detector_names, config, k)?;
    Ok(merge_near_duplicates(library, config.merge_cosine))
}

fn choose_k(data: &Array2<f64>, config: &DiscoverConfig) -> Result<usize> {
    if let Some(k) = config.fixed_k {
        return Ok(k.max(1).min(data.nrows()));
    }
    let k_max = config.k_max.min(data.nrows()).max(1);
    let k_min = config.k_min.min(k_max).max(1);
    if k_min == k_max {
        return Ok(k_min);
    }

    let mut best_k = k_min;
    let mut best_score = f64::NEG_INFINITY;
    for k in k_min..=k_max {
        let means = fit_means(data, k, config)?;
        let assignments = assign_nearest(data, &means);
        // Silhouette needs at least 2 clusters with members; skip degenerate.
        if assignments.iter().copied().max().unwrap_or(0) == 0 {
            continue;
        }
        let rows: Vec<Vec<f64>> = data.rows().into_iter().map(|r| r.to_vec()).collect();
        let scores = silhouette_scores_sampled(&rows, &assignments, 512)
            .map_err(|e| AutospectralError::Clustering(e.to_string()))?;
        let mean = scores.mean_score;
        if mean > best_score {
            best_score = mean;
            best_k = k;
        }
    }
    Ok(best_k)
}

fn fit_means(data: &Array2<f64>, k: usize, config: &DiscoverConfig) -> Result<Array2<f64>> {
    match config.backend {
        DiscoveryBackend::Gmm => {
            let gmm_cfg = GmmConfig {
                n_components: k,
                max_iterations: config.max_iterations,
                tolerance: 1e-3,
                seed: config.seed,
            };
            let result = Gmm::fit(data, &gmm_cfg)
                .map_err(|e| AutospectralError::Clustering(e.to_string()))?;
            Ok(result.means)
        }
        DiscoveryBackend::KMeans => {
            let km_cfg = KMeansConfig {
                n_clusters: k,
                max_iterations: config.max_iterations,
                tolerance: 1e-4,
                seed: config.seed,
            };
            let result = KMeans::fit(data, &km_cfg)
                .map_err(|e| AutospectralError::Clustering(e.to_string()))?;
            Ok(result.centroids)
        }
    }
}

fn assign_nearest(data: &Array2<f64>, means: &Array2<f64>) -> Vec<usize> {
    let k = means.nrows();
    let d = means.ncols();
    data.rows()
        .into_iter()
        .map(|row| {
            let mut best = 0usize;
            let mut best_dist = f64::INFINITY;
            for j in 0..k {
                let mut dist = 0.0;
                for c in 0..d {
                    let diff = row[c] - means[(j, c)];
                    dist += diff * diff;
                }
                if dist < best_dist {
                    best_dist = dist;
                    best = j;
                }
            }
            best
        })
        .collect()
}

fn means_to_library(
    means: Array2<f64>,
    detector_names: &[String],
    config: &DiscoverConfig,
    k: usize,
) -> Result<AfLibrary> {
    let n_det = means.ncols();
    let n_sig = means.nrows();
    if n_sig == 0 {
        return Err(AutospectralError::EmptyLibrary);
    }
    let mut signatures = Mat::<f64>::zeros(n_det, n_sig);
    let mut names = Vec::with_capacity(n_sig);
    for j in 0..n_sig {
        let mut col: Vec<f64> = (0..n_det).map(|i| means[(j, i)]).collect();
        normalize_unit_peak(&mut col);
        for i in 0..n_det {
            signatures[(i, j)] = col[i];
        }
        names.push(format!("AF_{j}"));
    }
    let backend = match config.backend {
        DiscoveryBackend::Gmm => "gmm",
        DiscoveryBackend::KMeans => "kmeans",
    };
    Ok(AfLibrary {
        signatures,
        names,
        detector_names: detector_names.to_vec(),
        provenance: format!("{backend} k={k}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DiscoverConfig;

    #[test]
    fn gmm_recovers_two_well_separated_af_shapes() {
        // Two detector panels; two AF populations near (10,1) and (1,10).
        let mut events = Vec::new();
        for _ in 0..40 {
            events.extend_from_slice(&[10.0, 1.0]);
        }
        for _ in 0..40 {
            events.extend_from_slice(&[1.0, 10.0]);
        }
        let names = vec!["D1".into(), "D2".into()];
        let cfg = DiscoverConfig {
            backend: DiscoveryBackend::Gmm,
            fixed_k: Some(2),
            seed: Some(7),
            ..DiscoverConfig::default()
        };
        let lib = discover_af_library(&events, 80, 2, &names, &cfg).expect("discover");
        assert_eq!(lib.n_signatures(), 2);
        assert_eq!(lib.n_detectors(), 2);
        // After unit-peak normalize, each column should peak on a different detector.
        let c0 = lib.column_slice(0).unwrap();
        let c1 = lib.column_slice(1).unwrap();
        let peak0 = if c0[0] >= c0[1] { 0 } else { 1 };
        let peak1 = if c1[0] >= c1[1] { 0 } else { 1 };
        assert_ne!(peak0, peak1);
    }
}
