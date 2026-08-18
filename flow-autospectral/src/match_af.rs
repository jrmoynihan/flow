//! Match stained events to AF library signatures.

use crate::config::{force_sequential, MatchConfig, MatchStrategy};
use crate::error::{AutospectralError, Result};
use crate::library::AfLibrary;
use crate::unmix_ols::{ols_residual, swap_af_column};
use faer::{Mat, MatRef};
use flow_knn::AnnIndex;
use rayon::prelude::*;

/// Per-event AF assignment and optional residual / NN distance.
#[derive(Debug, Clone)]
pub struct AfMatchResult {
    pub af_indices: Vec<usize>,
    pub residuals: Vec<f64>,
    pub nn_distances: Vec<f64>,
}

/// Match row-major stained events (`n × detectors`) to an AF library.
///
/// `fluor_matrix` is detectors × fluorophores (no AF column). When using
/// [`MatchStrategy::ResidualOls`], each candidate AF column is appended and the
/// OLS residual against the event spectrum is compared.
pub fn match_events(
    events_row_major: &[f64],
    n_events: usize,
    library: &AfLibrary,
    fluor_matrix: MatRef<'_, f64>,
    config: &MatchConfig,
) -> Result<AfMatchResult> {
    if n_events == 0 {
        return Err(AutospectralError::EmptyEvents);
    }
    let d = library.n_detectors();
    if events_row_major.len() != n_events * d {
        return Err(AutospectralError::DetectorMismatch {
            expected: d,
            got: events_row_major.len() / n_events.max(1),
        });
    }
    if fluor_matrix.nrows() != d {
        return Err(AutospectralError::DetectorMismatch {
            expected: d,
            got: fluor_matrix.nrows(),
        });
    }
    if library.n_signatures() == 0 {
        return Err(AutospectralError::EmptyLibrary);
    }

    match config.strategy {
        MatchStrategy::NearestNeighbor => match_nn(events_row_major, n_events, library, config),
        MatchStrategy::ResidualOls => {
            match_residual(events_row_major, n_events, library, fluor_matrix, config)
        }
    }
}

fn match_nn(
    events_row_major: &[f64],
    n_events: usize,
    library: &AfLibrary,
    config: &MatchConfig,
) -> Result<AfMatchResult> {
    let d = library.n_detectors();
    let lib_f32 = library.signatures_row_major_f32();
    let index = AnnIndex::build(
        &lib_f32,
        library.n_signatures(),
        d,
        &config.knn_method,
        config.metric,
    )
    .map_err(|e| AutospectralError::Knn(e.to_string()))?;

    let queries: Vec<f32> = events_row_major.iter().map(|&x| x as f32).collect();
    let nbrs = index
        .search_batch(&queries, n_events, 1)
        .map_err(|e| AutospectralError::Knn(e.to_string()))?;

    let mut af_indices = Vec::with_capacity(n_events);
    let mut nn_distances = Vec::with_capacity(n_events);
    for list in &nbrs {
        let idx = list.indices.first().copied().unwrap_or(0) as usize;
        let dist = list.distances.first().copied().unwrap_or(f32::INFINITY) as f64;
        af_indices.push(idx);
        nn_distances.push(dist);
    }
    Ok(AfMatchResult {
        af_indices,
        residuals: vec![f64::NAN; n_events],
        nn_distances,
    })
}

fn match_residual(
    events_row_major: &[f64],
    n_events: usize,
    library: &AfLibrary,
    fluor_matrix: MatRef<'_, f64>,
    config: &MatchConfig,
) -> Result<AfMatchResult> {
    let k_lib = library.n_signatures();
    let d = library.n_detectors();
    let use_shortlist = k_lib > config.exhaustive_residual_max_k;

    let shortlists: Option<Vec<Vec<usize>>> = if use_shortlist {
        let lib_f32 = library.signatures_row_major_f32();
        let index = AnnIndex::build(
            &lib_f32,
            k_lib,
            d,
            &config.knn_method,
            config.metric,
        )
        .map_err(|e| AutospectralError::Knn(e.to_string()))?;
        let queries: Vec<f32> = events_row_major.iter().map(|&x| x as f32).collect();
        let k_cand = config.ann_candidates.min(k_lib).max(1);
        let nbrs = index
            .search_batch(&queries, n_events, k_cand)
            .map_err(|e| AutospectralError::Knn(e.to_string()))?;
        Some(
            nbrs.into_iter()
                .map(|list| list.indices.into_iter().map(|i| i as usize).collect())
                .collect(),
        )
    } else {
        None
    };

    let candidates_all: Vec<usize> = (0..k_lib).collect();
    let parallel =
        !force_sequential() && n_events >= config.parallel_event_threshold;

    let evaluate = |event_i: usize| -> Result<(usize, f64)> {
        let y = &events_row_major[event_i * d..(event_i + 1) * d];
        let candidates = shortlists
            .as_ref()
            .map(|s| s[event_i].as_slice())
            .unwrap_or(candidates_all.as_slice());
        let mut best_idx = candidates[0];
        let mut best_res = f64::INFINITY;
        for &af_idx in candidates {
            let m = swap_af_column(fluor_matrix, library, af_idx)?;
            let res = ols_residual(m.as_ref(), y)?;
            if res < best_res {
                best_res = res;
                best_idx = af_idx;
            }
        }
        Ok((best_idx, best_res))
    };

    let results: Result<Vec<(usize, f64)>> = if parallel {
        (0..n_events).into_par_iter().map(evaluate).collect()
    } else {
        (0..n_events).map(evaluate).collect()
    };

    let pairs = results?;
    let mut af_indices = Vec::with_capacity(n_events);
    let mut residuals = Vec::with_capacity(n_events);
    for (idx, res) in pairs {
        af_indices.push(idx);
        residuals.push(res);
    }
    Ok(AfMatchResult {
        af_indices,
        residuals,
        nn_distances: vec![f64::NAN; n_events],
    })
}

/// Partition event indices by assigned AF signature.
pub fn group_events_by_af(match_result: &AfMatchResult) -> Vec<(usize, Vec<usize>)> {
    let mut buckets: Vec<Vec<usize>> = Vec::new();
    for (event_i, &af) in match_result.af_indices.iter().enumerate() {
        if af >= buckets.len() {
            buckets.resize_with(af + 1, Vec::new);
        }
        buckets[af].push(event_i);
    }
    buckets
        .into_iter()
        .enumerate()
        .filter(|(_, evs)| !evs.is_empty())
        .collect()
}

/// Build detectors × (fluors + 1 AF) mixing matrices for each used AF index.
pub fn mixing_matrices_by_af(
    fluor_matrix: MatRef<'_, f64>,
    library: &AfLibrary,
    groups: &[(usize, Vec<usize>)],
) -> Result<Vec<(usize, Mat<f64>)>> {
    let mut out = Vec::with_capacity(groups.len());
    for &(af_idx, _) in groups {
        out.push((af_idx, swap_af_column(fluor_matrix, library, af_idx)?));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DiscoverConfig, DiscoveryBackend};
    use crate::discover::discover_af_library;
    use faer::Mat;

    #[test]
    fn residual_match_picks_matching_af() {
        let mut events = Vec::new();
        for _ in 0..30 {
            events.extend_from_slice(&[10.0, 1.0]);
        }
        for _ in 0..30 {
            events.extend_from_slice(&[1.0, 10.0]);
        }
        let names = vec!["D1".into(), "D2".into()];
        let cfg = DiscoverConfig {
            backend: DiscoveryBackend::KMeans,
            fixed_k: Some(2),
            seed: Some(1),
            ..DiscoverConfig::default()
        };
        let lib = discover_af_library(&events, 60, 2, &names, &cfg).unwrap();
        // No fluorophores: residual = fit of AF alone.
        let fluor = Mat::<f64>::zeros(2, 0);
        let stained = vec![9.5_f64, 1.2, 1.1, 9.0];
        let matched = match_events(
            &stained,
            2,
            &lib,
            fluor.as_ref(),
            &MatchConfig {
                parallel_event_threshold: usize::MAX,
                ..MatchConfig::default()
            },
        )
        .unwrap();
        assert_eq!(matched.af_indices.len(), 2);
        assert_ne!(matched.af_indices[0], matched.af_indices[1]);
    }
}
