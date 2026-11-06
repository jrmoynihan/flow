//! Truncation fence estimators for TRU-OLS (percentile vs asinh/rSD).
//!
//! The shipped product fence remains a linear nearest-rank percentile of
//! unstained OLS abundances. Experimental Preview labs can swap in asinh rSD
//! and optionally restrict the unstained rows used (AF-signature adaptive).

use crate::error::TruOlsError;
use crate::preprocessing::solve_linear_system;
use crate::spreading::{DEFAULT_ASINH_COFACTOR, asinh_transform, robust_rsd_asinh};
use faer::{Col, MatRef};

/// Minimum unstained events in a masked subset before falling back to global.
pub const MIN_FENCE_EVENTS: usize = 30;

/// How to turn unstained OLS abundances into per-endmember cutoffs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FenceKind {
    /// Linear nearest-rank percentile (shipped default: `q = 0.995`).
    Percentile { q: f64 },
    /// Median + `k * rSD` in asinh space, mapped back to linear abundance.
    AsinhRsd { k: f64, cofactor: f64 },
}

impl Default for FenceKind {
    fn default() -> Self {
        Self::Percentile { q: 0.995 }
    }
}

/// Request for [`estimate_cutoffs`].
#[derive(Debug, Clone)]
pub struct FenceRequest {
    pub kind: FenceKind,
    /// When set, only these unstained rows contribute (length = unstained nrows).
    pub unstained_row_mask: Option<Vec<bool>>,
}

impl FenceRequest {
    pub fn percentile(q: f64) -> Self {
        Self {
            kind: FenceKind::Percentile { q },
            unstained_row_mask: None,
        }
    }

    pub fn asinh_rsd(k: f64, cofactor: f64) -> Self {
        Self {
            kind: FenceKind::AsinhRsd { k, cofactor },
            unstained_row_mask: None,
        }
    }

    pub fn with_mask(mut self, mask: Option<Vec<bool>>) -> Self {
        self.unstained_row_mask = mask;
        self
    }
}

/// OLS-unmix unstained events, then summarize each endmember column into a cutoff.
pub fn estimate_cutoffs(
    mixing_matrix: MatRef<'_, f64>,
    unstained: MatRef<'_, f64>,
    req: &FenceRequest,
) -> Result<Col<f64>, TruOlsError> {
    match req.kind {
        FenceKind::Percentile { q } => {
            if !(0.0..=1.0).contains(&q) {
                return Err(TruOlsError::InvalidPercentile(q));
            }
        }
        FenceKind::AsinhRsd { k, cofactor } => {
            if !k.is_finite() || k <= 0.0 {
                return Err(TruOlsError::InsufficientData(format!(
                    "asinh-rSD fence k must be finite and > 0, got {k}"
                )));
            }
            if !cofactor.is_finite() || cofactor <= 0.0 {
                return Err(TruOlsError::InsufficientData(format!(
                    "asinh-rSD cofactor must be finite and > 0, got {cofactor}"
                )));
            }
        }
    }

    let n_detectors = mixing_matrix.nrows();
    let n_events = unstained.nrows();

    if unstained.ncols() != n_detectors {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors,
            actual: unstained.ncols(),
        });
    }
    if n_events == 0 {
        return Err(TruOlsError::InsufficientData(
            "Unstained control must contain at least one event".into(),
        ));
    }
    if let Some(mask) = &req.unstained_row_mask {
        if mask.len() != n_events {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_events,
                actual: mask.len(),
            });
        }
        let kept = mask.iter().filter(|&&m| m).count();
        if kept < MIN_FENCE_EVENTS {
            return Err(TruOlsError::InsufficientData(format!(
                "fence mask keeps {kept} events; need at least {MIN_FENCE_EVENTS}"
            )));
        }
    }

    let columns =
        unstained_abundance_columns(mixing_matrix, unstained, req.unstained_row_mask.as_deref())?;
    let cutoffs = cutoffs_from_abundance_columns(&columns, req.kind)?;
    Ok(Col::from_fn(cutoffs.len(), |i| cutoffs[i]))
}

/// Summarize already-unmixed unstained abundance columns into per-endmember cutoffs.
pub fn cutoffs_from_abundance_columns(
    columns: &[Vec<f64>],
    kind: FenceKind,
) -> Result<Vec<f64>, TruOlsError> {
    match kind {
        FenceKind::Percentile { q } => {
            if !(0.0..=1.0).contains(&q) {
                return Err(TruOlsError::InvalidPercentile(q));
            }
        }
        FenceKind::AsinhRsd { k, cofactor } => {
            if !k.is_finite() || k <= 0.0 {
                return Err(TruOlsError::InsufficientData(format!(
                    "asinh-rSD fence k must be finite and > 0, got {k}"
                )));
            }
            if !cofactor.is_finite() || cofactor <= 0.0 {
                return Err(TruOlsError::InsufficientData(format!(
                    "asinh-rSD cofactor must be finite and > 0, got {cofactor}"
                )));
            }
        }
    }

    let mut cutoffs = Vec::with_capacity(columns.len());
    for (endmember_idx, values) in columns.iter().enumerate() {
        let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
        if finite.is_empty() {
            return Err(TruOlsError::InsufficientData(format!(
                "no finite unstained abundances for endmember {endmember_idx}"
            )));
        }
        let cutoff = match kind {
            FenceKind::Percentile { q } => percentile_nearest_rank(&finite, q),
            FenceKind::AsinhRsd { k, cofactor } => asinh_rsd_cutoff(&finite, k, cofactor)?,
        };
        cutoffs.push(cutoff);
    }
    Ok(cutoffs)
}

/// Estimate cutoffs; on a too-small mask, retry without the mask (global fence).
pub fn estimate_cutoffs_with_fallback(
    mixing_matrix: MatRef<'_, f64>,
    unstained: MatRef<'_, f64>,
    req: &FenceRequest,
) -> Result<(Col<f64>, bool), TruOlsError> {
    match estimate_cutoffs(mixing_matrix, unstained, req) {
        Ok(cutoffs) => Ok((cutoffs, false)),
        Err(e) if req.unstained_row_mask.is_some() => {
            let global = FenceRequest {
                kind: req.kind,
                unstained_row_mask: None,
            };
            let cutoffs = estimate_cutoffs(mixing_matrix, unstained, &global)?;
            let _ = e;
            Ok((cutoffs, true))
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn unmix_unstained_all(
    mixing_matrix: MatRef<'_, f64>,
    unstained: MatRef<'_, f64>,
) -> Result<Vec<Vec<f64>>, TruOlsError> {
    let indices: Vec<usize> = (0..unstained.nrows()).collect();
    unmix_unstained_rows(mixing_matrix, unstained, &indices)
}

/// Summarize already-unmixed unstained rows into cutoffs, falling back to all rows
/// when a mask keeps fewer than [`MIN_FENCE_EVENTS`].
pub fn estimate_cutoffs_from_abundance_rows(
    rows: &[Vec<f64>],
    kind: FenceKind,
    mask: Option<&[bool]>,
) -> Result<(Col<f64>, bool), TruOlsError> {
    if rows.is_empty() {
        return Err(TruOlsError::InsufficientData(
            "Unstained control must contain at least one event".into(),
        ));
    }
    if let Some(mask) = mask {
        if mask.len() != rows.len() {
            return Err(TruOlsError::DimensionMismatch {
                expected: rows.len(),
                actual: mask.len(),
            });
        }
        let kept = mask.iter().filter(|&&flag| flag).count();
        if kept < MIN_FENCE_EVENTS {
            let cutoffs = cutoffs_from_row_subset(rows, None, kind)?;
            return Ok((cutoffs, true));
        }
        let cutoffs = cutoffs_from_row_subset(rows, Some(mask), kind)?;
        return Ok((cutoffs, false));
    }
    let cutoffs = cutoffs_from_row_subset(rows, None, kind)?;
    Ok((cutoffs, false))
}

fn cutoffs_from_row_subset(
    rows: &[Vec<f64>],
    mask: Option<&[bool]>,
    kind: FenceKind,
) -> Result<Col<f64>, TruOlsError> {
    let n_em = rows[0].len();
    let mut columns = vec![Vec::new(); n_em];
    for (index, row) in rows.iter().enumerate() {
        if mask.is_some_and(|flags| !flags[index]) {
            continue;
        }
        for (endmember, &value) in row.iter().enumerate() {
            if value.is_finite() {
                columns[endmember].push(value);
            }
        }
    }
    let cutoffs = cutoffs_from_abundance_columns(&columns, kind)?;
    Ok(Col::from_fn(cutoffs.len(), |i| cutoffs[i]))
}

fn unmix_unstained_rows(
    mixing_matrix: MatRef<'_, f64>,
    unstained: MatRef<'_, f64>,
    row_indices: &[usize],
) -> Result<Vec<Vec<f64>>, TruOlsError> {
    let n_detectors = mixing_matrix.nrows();
    let n = row_indices.len();

    if crate::use_parallel_independent_events(n) {
        use rayon::prelude::*;
        let mut rows: Vec<(usize, Vec<f64>)> = row_indices
            .par_iter()
            .enumerate()
            .map(|(ord, &event_idx)| {
                let observation = Col::from_fn(n_detectors, |i| unstained[(event_idx, i)]);
                let abundances = solve_linear_system(mixing_matrix, observation.as_ref())
                    .map_err(|e| TruOlsError::LinearAlgebra(format!("Failed to solve: {e}")))?;
                let v: Vec<f64> = (0..abundances.nrows()).map(|i| abundances[i]).collect();
                Ok((ord, v))
            })
            .collect::<Result<Vec<_>, _>>()?;
        rows.sort_by_key(|(i, _)| *i);
        Ok(rows.into_iter().map(|(_, v)| v).collect())
    } else {
        let mut out = Vec::with_capacity(n);
        for &event_idx in row_indices {
            let observation = Col::from_fn(n_detectors, |i| unstained[(event_idx, i)]);
            let abundances = solve_linear_system(mixing_matrix, observation.as_ref())
                .map_err(|e| TruOlsError::LinearAlgebra(format!("Failed to solve: {e}")))?;
            out.push((0..abundances.nrows()).map(|i| abundances[i]).collect());
        }
        Ok(out)
    }
}

fn percentile_nearest_rank(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn asinh_rsd_cutoff(values: &[f64], k: f64, cofactor: f64) -> Result<f64, TruOlsError> {
    let c = if cofactor.is_finite() && cofactor > 0.0 {
        cofactor
    } else {
        DEFAULT_ASINH_COFACTOR
    };
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    let rsd = robust_rsd_asinh(values, c).unwrap_or(0.0);
    let asinh_vals = asinh_transform(&[median], c);
    let med_asinh = asinh_vals[0];
    let fence_asinh = med_asinh + k * rsd;
    Ok(c * fence_asinh.sinh())
}

/// Build a boolean mask of unstained events whose pair-test assignment (1-based)
/// equals `signature_1based`.
pub fn mask_for_signature(assignment_1based: &[u32], signature_1based: u32) -> Vec<bool> {
    assignment_1based
        .iter()
        .map(|&a| a == signature_1based)
        .collect()
}

/// Build a simple asinh-space histogram for fence diagnostics.
pub fn asinh_histogram(values: &[f64], cofactor: f64, n_bins: usize) -> (Vec<f64>, Vec<u32>) {
    let c = if cofactor.is_finite() && cofactor > 0.0 {
        cofactor
    } else {
        DEFAULT_ASINH_COFACTOR
    };
    let n_bins = n_bins.max(1);
    let asinh_vals: Vec<f64> = asinh_transform(values, c)
        .into_iter()
        .filter(|v| v.is_finite())
        .collect();
    if asinh_vals.is_empty() {
        return (vec![0.0, 1.0], vec![0]);
    }
    let min_v = asinh_vals.iter().copied().fold(f64::INFINITY, f64::min);
    let max_v = asinh_vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max_v - min_v).max(1e-9);
    let mut edges = Vec::with_capacity(n_bins + 1);
    for i in 0..=n_bins {
        edges.push(min_v + span * (i as f64) / n_bins as f64);
    }
    let mut counts = vec![0_u32; n_bins];
    for v in asinh_vals {
        let mut idx = ((v - min_v) / span * n_bins as f64).floor() as usize;
        if idx >= n_bins {
            idx = n_bins - 1;
        }
        counts[idx] += 1;
    }
    (edges, counts)
}

/// OLS-unmix unstained (optional mask) and return per-endmember abundance vectors.
pub fn unstained_abundance_columns(
    mixing_matrix: MatRef<'_, f64>,
    unstained: MatRef<'_, f64>,
    mask: Option<&[bool]>,
) -> Result<Vec<Vec<f64>>, TruOlsError> {
    let n_events = unstained.nrows();
    let row_indices: Vec<usize> = match mask {
        Some(m) => {
            if m.len() != n_events {
                return Err(TruOlsError::DimensionMismatch {
                    expected: n_events,
                    actual: m.len(),
                });
            }
            (0..n_events).filter(|&i| m[i]).collect()
        }
        None => (0..n_events).collect(),
    };
    let rows = unmix_unstained_rows(mixing_matrix, unstained, &row_indices)?;
    let n_em = mixing_matrix.ncols();
    let mut cols = vec![Vec::with_capacity(rows.len()); n_em];
    for row in rows {
        for (em, v) in row.into_iter().enumerate() {
            if v.is_finite() {
                cols[em].push(v);
            }
        }
    }
    Ok(cols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    fn bulk_plus_tail() -> (faer::Mat<f64>, faer::Mat<f64>) {
        // Identity mixing: abundances ≈ detector values.
        let mixing = mat![[1.0, 0.0], [0.0, 1.0]];
        // 200 bulk events ~ N(0, 0.1) in linear space for fluor 0; 2 bright outliers.
        let n = 202;
        let unstained = faer::Mat::from_fn(n, 2, |ev, det| {
            if det == 1 {
                return 0.05;
            }
            if ev >= 200 {
                50.0 // bright tail
            } else {
                ((ev as f64) * 0.017).sin() * 0.1
            }
        });
        (mixing, unstained)
    }

    #[test]
    fn percentile_fence_follows_bright_tail() {
        let (mixing, unstained) = bulk_plus_tail();
        let pct = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::percentile(0.995),
        )
        .expect("percentile");
        // 99.5th of 202 ≈ near the bright outliers (~50).
        assert!(
            pct[0] > 10.0,
            "percentile should sit in the bright tail, got {}",
            pct[0]
        );
    }

    #[test]
    fn asinh_rsd_fence_stays_near_bulk() {
        let (mixing, unstained) = bulk_plus_tail();
        let rsd = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::asinh_rsd(4.0, 200.0),
        )
        .expect("asinh rsd");
        assert!(
            rsd[0] < 5.0,
            "asinh-rSD should ignore the two outliers, got {}",
            rsd[0]
        );
        assert!(rsd[0] > pct_bulk_floor(), "fence should still clear bulk noise");
    }

    fn pct_bulk_floor() -> f64 {
        -1.0
    }

    #[test]
    fn masked_subset_too_small_errors_without_fallback() {
        let mixing = mat![[1.0], [0.0]];
        let unstained = faer::Mat::from_fn(40, 2, |ev, det| {
            if det == 0 {
                ev as f64 * 0.01
            } else {
                0.1
            }
        });
        let mut mask = vec![false; 40];
        for m in mask.iter_mut().take(10) {
            *m = true;
        }
        let err = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::percentile(0.995).with_mask(Some(mask)),
        );
        assert!(err.is_err());
    }

    #[test]
    fn fallback_uses_global_when_mask_too_small() {
        let mixing = mat![[1.0], [0.0]];
        let unstained = faer::Mat::from_fn(40, 2, |ev, det| {
            if det == 0 {
                ev as f64 * 0.01
            } else {
                0.1
            }
        });
        let mut mask = vec![false; 40];
        for m in mask.iter_mut().take(5) {
            *m = true;
        }
        let (cutoffs, fell_back) = estimate_cutoffs_with_fallback(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::percentile(0.995).with_mask(Some(mask)),
        )
        .expect("fallback");
        assert!(fell_back);
        assert!(cutoffs[0].is_finite());
    }

    #[test]
    fn adaptive_subset_tracks_matching_width() {
        // Two populations on fluor 0: narrow (sig 1) vs wide (sig 2).
        let mixing = mat![[1.0, 0.0], [0.0, 1.0]];
        let n = 100;
        let unstained = faer::Mat::from_fn(n, 2, |ev, det| {
            if det == 1 {
                return 0.05;
            }
            if ev < 50 {
                ((ev as f64) * 0.1).sin() * 0.05 // narrow
            } else {
                ((ev as f64) * 0.1).sin() * 2.0 // wide
            }
        });
        let mut assignment = vec![1_u32; n];
        for a in assignment.iter_mut().skip(50) {
            *a = 2;
        }
        let mask_narrow = mask_for_signature(&assignment, 1);
        let mask_wide = mask_for_signature(&assignment, 2);
        let narrow = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::asinh_rsd(4.0, 200.0).with_mask(Some(mask_narrow)),
        )
        .expect("narrow");
        let wide = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::asinh_rsd(4.0, 200.0).with_mask(Some(mask_wide)),
        )
        .expect("wide");
        assert!(
            wide[0] > narrow[0] * 1.5,
            "wide subset fence ({}) should exceed narrow ({})",
            wide[0],
            narrow[0]
        );
    }

    #[test]
    fn cutoffs_from_abundance_columns_match_estimate_cutoffs() {
        let (mixing, unstained) = bulk_plus_tail();
        let columns = unstained_abundance_columns(mixing.as_ref(), unstained.as_ref(), None)
            .expect("unstained columns");
        let from_columns = cutoffs_from_abundance_columns(
            &columns,
            FenceKind::Percentile { q: 0.995 },
        )
        .expect("cutoffs from columns");
        let direct = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::percentile(0.995),
        )
        .expect("direct cutoffs");
        assert_eq!(from_columns.len(), direct.nrows());
        for i in 0..from_columns.len() {
            assert!(
                (from_columns[i] - direct[i]).abs() < 1e-12,
                "endmember {i}: column cutoff {} != estimate_cutoffs {}",
                from_columns[i],
                direct[i]
            );
        }
    }

    #[test]
    fn masked_cutoffs_subset_a_shared_unstained_ols() {
        let (mixing, unstained) = bulk_plus_tail();
        let rows = unmix_unstained_all(mixing.as_ref(), unstained.as_ref()).expect("shared ols");
        let mut mask = vec![false; unstained.nrows()];
        for (i, flag) in mask.iter_mut().enumerate() {
            *flag = i % 2 == 0;
        }
        let (from_shared, fell_back) = estimate_cutoffs_from_abundance_rows(
            &rows,
            FenceKind::AsinhRsd {
                k: 4.0,
                cofactor: 200.0,
            },
            Some(mask.as_slice()),
        )
        .expect("subset shared ols");
        let direct = estimate_cutoffs(
            mixing.as_ref(),
            unstained.as_ref(),
            &FenceRequest::asinh_rsd(4.0, 200.0).with_mask(Some(mask)),
        )
        .expect("direct masked ols");
        assert!(!fell_back);
        assert_eq!(from_shared.nrows(), direct.nrows());
        for i in 0..direct.nrows() {
            assert!(
                (from_shared[i] - direct[i]).abs() < 1e-12,
                "endmember {i}: shared {} != direct {}",
                from_shared[i],
                direct[i]
            );
        }
    }
}
