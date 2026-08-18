//! Optional unstained cleaning: scatter-match against stained scatter, then PCA outliers.

use crate::config::{CleanConfig, PcaCleanConfig, ScatterCleanConfig};
use crate::error::{AutospectralError, Result};
use flow_dimensional_reduction::Pca;
use flow_knn::AnnIndex;

/// Scatter buffers for [`clean_unstained`].
#[derive(Debug, Clone, Copy)]
pub struct ScatterInput<'a> {
    pub unstained: &'a [f64],
    pub n_unstained: usize,
    pub stained: &'a [f64],
    pub n_stained: usize,
    pub dims: usize,
}

/// Indices of unstained events retained after optional scatter + PCA filters.
#[derive(Debug, Clone)]
pub struct CleanedEvents {
    pub keep: Vec<usize>,
    /// Row-major fluorescence of kept events (`keep.len() × n_detectors`).
    pub fluorescence: Vec<f64>,
}

/// Apply scatter-match then PCA-intrusive filters (each step skipped when unset).
pub fn clean_unstained(
    fluorescence: &[f64],
    n_events: usize,
    n_detectors: usize,
    scatter: Option<ScatterInput<'_>>,
    config: &CleanConfig,
) -> Result<CleanedEvents> {
    if n_events == 0 {
        return Err(AutospectralError::EmptyEvents);
    }
    if n_detectors == 0 || fluorescence.len() != n_events * n_detectors {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_detectors,
            got: fluorescence.len().checked_div(n_events.max(1)).unwrap_or(0),
        });
    }

    let mut keep: Vec<usize> = (0..n_events).collect();

    if let Some(sc) = &config.scatter {
        let scatter = scatter.ok_or_else(|| {
            AutospectralError::InvalidConfig(
                "scatter cleaning enabled but no scatter buffers were provided".into(),
            )
        })?;
        keep = keep_indices_scatter_match(scatter, sc)?;
        if keep.is_empty() {
            return Err(AutospectralError::EmptyEvents);
        }
    }

    if let Some(pc) = &config.pca {
        let subset = gather_rows(fluorescence, n_detectors, &keep);
        let keep_pca = keep_indices_pca_intrusive(&subset, keep.len(), n_detectors, pc)?;
        keep = keep_pca.into_iter().map(|i| keep[i]).collect();
        if keep.is_empty() {
            return Err(AutospectralError::EmptyEvents);
        }
    }

    let gathered = gather_rows(fluorescence, n_detectors, &keep);
    Ok(CleanedEvents {
        keep,
        fluorescence: gathered,
    })
}

fn gather_rows(row_major: &[f64], n_cols: usize, rows: &[usize]) -> Vec<f64> {
    let mut out = Vec::with_capacity(rows.len() * n_cols);
    for &r in rows {
        let start = r * n_cols;
        out.extend_from_slice(&row_major[start..start + n_cols]);
    }
    out
}

fn keep_indices_scatter_match(
    scatter: ScatterInput<'_>,
    config: &ScatterCleanConfig,
) -> Result<Vec<usize>> {
    if scatter.dims == 0 {
        return Err(AutospectralError::InvalidConfig(
            "scatter_dims must be >= 1".into(),
        ));
    }
    if scatter.unstained.len() != scatter.n_unstained * scatter.dims {
        return Err(AutospectralError::DetectorMismatch {
            expected: scatter.dims,
            got: scatter
                .unstained
                .len()
                .checked_div(scatter.n_unstained.max(1))
                .unwrap_or(0),
        });
    }
    if scatter.stained.len() != scatter.n_stained * scatter.dims {
        return Err(AutospectralError::DetectorMismatch {
            expected: scatter.dims,
            got: scatter
                .stained
                .len()
                .checked_div(scatter.n_stained.max(1))
                .unwrap_or(0),
        });
    }
    if scatter.n_stained == 0 {
        return Err(AutospectralError::EmptyEvents);
    }
    if !(0.0..=1.0).contains(&config.keep_percentile) {
        return Err(AutospectralError::InvalidConfig(
            "scatter keep_percentile must be in [0, 1]".into(),
        ));
    }

    let stained_f32: Vec<f32> = scatter.stained.iter().map(|&x| x as f32).collect();
    let index = AnnIndex::build(
        &stained_f32,
        scatter.n_stained,
        scatter.dims,
        &config.knn_method,
        config.metric,
    )
    .map_err(|e| AutospectralError::Knn(e.to_string()))?;

    let queries: Vec<f32> = scatter.unstained.iter().map(|&x| x as f32).collect();
    let nbrs = index
        .search_batch(&queries, scatter.n_unstained, 1)
        .map_err(|e| AutospectralError::Knn(e.to_string()))?;

    let distances: Vec<f64> = nbrs
        .iter()
        .map(|list| list.distances.first().copied().unwrap_or(f32::INFINITY) as f64)
        .collect();
    let threshold = percentile_threshold(&distances, config.keep_percentile);
    Ok(distances
        .iter()
        .enumerate()
        .filter(|(_, d)| **d <= threshold)
        .map(|(i, _)| i)
        .collect())
}

fn keep_indices_pca_intrusive(
    fluorescence: &[f64],
    n_events: usize,
    n_detectors: usize,
    config: &PcaCleanConfig,
) -> Result<Vec<usize>> {
    if !(0.0..=1.0).contains(&config.keep_percentile) {
        return Err(AutospectralError::InvalidConfig(
            "PCA keep_percentile must be in [0, 1]".into(),
        ));
    }
    if n_events < 2 {
        return Ok((0..n_events).collect());
    }
    let k = config.n_components.max(1).min(n_detectors);
    let data_f32: Vec<f32> = fluorescence.iter().map(|&x| x as f32).collect();
    let pca = Pca::new(k)
        .fit(&data_f32, n_events, n_detectors)
        .map_err(|e| AutospectralError::Pca(e.to_string()))?;
    let scores = pca
        .transform(&data_f32, n_events, n_detectors)
        .map_err(|e| AutospectralError::Pca(e.to_string()))?;
    let radii: Vec<f64> = scores
        .chunks_exact(k)
        .map(|row| row.iter().map(|v| f64::from(*v) * f64::from(*v)).sum())
        .collect();
    let threshold = percentile_threshold(&radii, config.keep_percentile);
    Ok(radii
        .iter()
        .enumerate()
        .filter(|(_, r)| **r <= threshold)
        .map(|(i, _)| i)
        .collect())
}

fn percentile_threshold(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((sorted.len() - 1) as f64 * p.clamp(0.0, 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PcaCleanConfig;

    #[test]
    fn pca_clean_drops_far_outlier() {
        let mut fluor = Vec::new();
        for _ in 0..40 {
            fluor.extend_from_slice(&[1.0, 1.0]);
        }
        fluor.extend_from_slice(&[100.0, 100.0]);
        let cleaned = clean_unstained(
            &fluor,
            41,
            2,
            None,
            &CleanConfig {
                scatter: None,
                pca: Some(PcaCleanConfig {
                    n_components: 1,
                    keep_percentile: 0.95,
                }),
            },
        )
        .expect("clean");
        assert!(cleaned.keep.len() < 41);
        assert!(!cleaned.keep.contains(&40));
    }

    #[test]
    fn scatter_match_keeps_overlapping_cloud() {
        let unstained_sc = vec![0.0, 0.0, 0.1, 0.1, 50.0, 50.0];
        let stained_sc = vec![0.0, 0.0, 0.05, 0.05, 0.2, 0.0];
        let fluor = vec![1.0, 2.0, 1.1, 2.1, 9.0, 9.0];
        let cleaned = clean_unstained(
            &fluor,
            3,
            2,
            Some(ScatterInput {
                unstained: &unstained_sc,
                n_unstained: 3,
                stained: &stained_sc,
                n_stained: 3,
                dims: 2,
            }),
            &CleanConfig {
                scatter: Some(ScatterCleanConfig {
                    keep_percentile: 0.6,
                    ..ScatterCleanConfig::default()
                }),
                pca: None,
            },
        )
        .expect("clean");
        assert!(cleaned.keep.contains(&0));
        assert!(!cleaned.keep.contains(&2));
    }
}
