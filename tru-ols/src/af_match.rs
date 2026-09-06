//! Autofluorescence matching: per-event pair-test against an AF catalog.
//!
//! Autofluorescence is kept out of the mixing matrix. A catalog of AF signatures is
//! matched to each event, the **winner** is subtracted, and fluorophore-only unmixing
//! then runs on the residual. Two pieces of evidence decide the match:
//!
//! * **Spectral.** Each catalog signature \(a_k\) is projected off the fluorophore span
//!   once, up front, leaving \(r_k = (I - P)a_k\) — the part of that AF shape no
//!   combination of fluorophores can imitate. Scaling \(r_k\) to the event gives the
//!   only AF amount the data can actually distinguish; the leftover
//!   \(\lVert (I-P)(y' - k_k a_k)\rVert\) scores the fit. The assignment is the
//!   residual argmin (lowest index on an exact tie).
//! * **Morphological.** FSC/SSC is a cheap proxy for cell type, so each catalog entry
//!   also carries a 2-D Gaussian over scatter. Scatter is consulted **only** when the
//!   two best leftover norms are within 5% relative gap
//!   \((r_2 - r_\star) \le 0.05\cdot\max(r_\star, 10^{-12})\). If both component
//!   densities fall below [`PreparedAfLibrary::morph_outlier_tau`], the residual
//!   winner is kept.
//!
//! Subtraction is winner-only (one-hot weights). Confidence is the residual gap
//! \(1 - r_\star / (r_\star + r_2)\), or `1` when the catalog has a single entry.
//! Occupancy cannot override a clearly better residual.
//!
//! # Numerics
//!
//! The fluorophore projector is never formed as \(S(S^\top S)^{-1}S^\top\). Building the
//! library orthonormalizes the fluorophore columns once and every projection is
//! \(v - Q(Q^\top v)\) against that basis, so a badly conditioned panel degrades into a
//! rank error instead of silently producing a garbage projector.

use crate::error::TruOlsError;
use crate::preprocessing::solve_linear_system;
use faer::{Col, Mat, MatRef};

/// Relative residual \(\lVert r_k\rVert^2 / \lVert a_k\rVert^2\) below which a catalog
/// entry is discarded: it lies inside the fluorophore span, so no amount of it can be
/// distinguished from a combination of dyes.
pub const DEFAULT_RESIDUAL_EPSILON: f64 = 1e-6;

/// Default morphological outlier floor, as a fraction of the summed peak density of the
/// mixture.
///
/// The prior is a density, so an absolute threshold would depend on the units FSC/SSC
/// happen to be recorded in (raw channel values put peak densities near `1e-9`).
/// Scaling by \(\sum_k \pi_k / (2\pi\sqrt{\lvert\Sigma_k\rvert})\) — the largest value
/// the summed prior could ever reach — makes the default scale-free.
pub const DEFAULT_MORPH_OUTLIER_FRACTION: f64 = 1e-6;

/// A column whose norm drops below this fraction of its original norm during
/// orthogonalization is linearly dependent on the columns before it.
const RANK_TOLERANCE: f64 = 1e-10;

const LN_TWO_PI: f64 = 1.837_877_066_409_345_5;

/// One autofluorescence signature, with the morphology of the cells it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct AfCatalogEntry {
    /// Stable identifier used in reports and in the exported assignment keyword.
    pub id: String,
    /// Signature over detectors, max-normalized the same way [`crate::MixingMatrix`]
    /// normalizes endmember columns. Length must equal the detector count.
    pub values: Vec<f64>,
    /// Mean of the `[FSC, SSC]` cluster this signature was derived from.
    pub fsc_ssc_mean: [f64; 2],
    /// Covariance of that cluster; must be positive definite.
    pub fsc_ssc_cov: [[f64; 2]; 2],
    /// Mixture weight \(\pi_k\). Renormalized across surviving entries.
    pub mixing_proportion: f64,
}

/// An AF catalog plus the sharpness of the spectral likelihood.
#[derive(Debug, Clone, PartialEq)]
pub struct AfMatchLibrary {
    pub entries: Vec<AfCatalogEntry>,
    /// Inverse scale of the spectral likelihood \(\exp(-\beta\lVert\cdot\rVert)\). Larger
    /// values make the assignment more nearly winner-take-all.
    pub beta: f64,
    /// Detector order the signatures are expressed in; empty to skip the check.
    pub detector_names: Vec<String>,
}

/// Tuning for [`build_af_match_library`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AfMatchOptions {
    /// See [`DEFAULT_RESIDUAL_EPSILON`].
    pub residual_epsilon: f64,
    /// Absolute floor on the summed morphological prior below which an event is treated
    /// as a morphological outlier. `None` derives one from the mixture itself; see
    /// [`DEFAULT_MORPH_OUTLIER_FRACTION`].
    pub morph_outlier_tau: Option<f64>,
}

impl Default for AfMatchOptions {
    fn default() -> Self {
        Self {
            residual_epsilon: DEFAULT_RESIDUAL_EPSILON,
            morph_outlier_tau: None,
        }
    }
}

/// Why a catalog entry did not make it into a [`PreparedAfLibrary`].
#[derive(Debug, Clone, PartialEq)]
pub struct DroppedAfEntry {
    pub id: String,
    /// \(\lVert r_k\rVert^2 / \lVert a_k\rVert^2\), or `0.0` for an all-zero signature.
    pub relative_residual: f64,
}

/// A catalog entry with its fluorophore-orthogonal residual and prior precomputed.
#[derive(Debug, Clone)]
struct PreparedAfEntry {
    id: String,
    /// \(a_k\).
    values: Vec<f64>,
    /// \(r_k = (I - P)a_k\).
    residual: Vec<f64>,
    residual_norm_sq: f64,
    mean: [f64; 2],
    inv_cov: [[f64; 2]; 2],
    /// \(\ln \pi_k - \ln 2\pi - \tfrac{1}{2}\ln\lvert\Sigma_k\rvert\): the log prior at
    /// the cluster mean, so the per-event log prior is this minus half the Mahalanobis
    /// distance.
    log_prior_peak: f64,
}

/// An [`AfMatchLibrary`] with everything that does not depend on the event precomputed.
///
/// Build once per (catalog, fluorophore matrix) pair and reuse across files: the
/// orthonormal basis, the residuals and the Gaussian inverses are all event-independent.
#[derive(Debug, Clone)]
pub struct PreparedAfLibrary {
    entries: Vec<PreparedAfEntry>,
    dropped: Vec<DroppedAfEntry>,
    /// Orthonormal basis for the fluorophore span, detectors × rank.
    fluor_basis: Mat<f64>,
    n_detectors: usize,
    beta: f64,
    detector_names: Vec<String>,
    morph_outlier_tau: f64,
}

impl PreparedAfLibrary {
    /// Number of usable catalog entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn n_detectors(&self) -> usize {
        self.n_detectors
    }

    pub fn beta(&self) -> f64 {
        self.beta
    }

    pub fn detector_names(&self) -> &[String] {
        &self.detector_names
    }

    /// Ids of the usable entries, in the order the 1-based assignments index them.
    pub fn entry_ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    /// Entries discarded for lying inside the fluorophore span.
    pub fn dropped_entries(&self) -> &[DroppedAfEntry] {
        &self.dropped
    }

    /// The morphological outlier floor in effect.
    pub fn morph_outlier_tau(&self) -> f64 {
        self.morph_outlier_tau
    }

    /// Rank of the fluorophore span the residuals were taken against.
    pub fn fluor_rank(&self) -> usize {
        self.fluor_basis.ncols()
    }

    /// Catalog signature \(a_k\) for a 0-based prepared-entry index.
    ///
    /// [`AfMatchResult::assignment_1based`] indexes these entries (1-based).
    pub fn entry_signature(&self, index: usize) -> Option<&[f64]> {
        self.entries.get(index).map(|e| e.values.as_slice())
    }

    /// All prepared signatures in assignment order.
    pub fn entry_signatures(&self) -> impl Iterator<Item = &[f64]> {
        self.entries.iter().map(|e| e.values.as_slice())
    }
}

/// Per-event outcome of [`match_and_subtract`].
#[derive(Debug, Clone)]
pub struct AfMatchResult {
    /// Winner AF amount \(k_\star\), one per event.
    pub af_abundance: Vec<f64>,
    /// Residual (then scatter-tie) winner, 1-based so that `0` is never a valid
    /// assignment in exported data.
    pub assignment_1based: Vec<u32>,
    /// Residual-gap confidence \(1 - r_\star / (r_\star + r_2)\); `1` when the catalog
    /// has a single entry. Near `0.5` means the two best leftovers were equally good.
    pub confidence: Vec<f64>,
    /// Events × detectors, with \(k_\star a_\star\) removed. When `o_ns` was supplied
    /// this is on the nonspecific-subtracted baseline.
    pub residual_observations: Mat<f64>,
}

/// Single-event match inspect (Observation / Matched AF / Residual overlay).
#[derive(Debug, Clone)]
pub struct AfEventInspect {
    /// Observation after optional unstained baseline subtraction.
    pub observation: Vec<f64>,
    /// Winner-only fitted AF \(k_\star a_\star\).
    pub fitted_af: Vec<f64>,
    /// Winning signature scaled by its amount \(k_\star\).
    pub winner_af: Vec<f64>,
    pub residual: Vec<f64>,
    pub assignment_1based: u32,
    pub assignment_id: String,
    pub confidence: f64,
    pub af_abundance: f64,
}

/// Precompute the fluorophore-orthogonal residuals and morphological priors of a catalog.
///
/// `fluor_matrix` is detectors × fluorophore endmembers and must **not** contain an AF
/// column — the whole point is that AF is handled outside the mixing matrix.
///
/// Entries whose relative residual falls below [`AfMatchOptions::residual_epsilon`] are
/// dropped and reported by [`PreparedAfLibrary::dropped_entries`]: such a signature is
/// reproducible from the dyes themselves, so matching against it would just move
/// fluorophore signal into the AF channel.
pub fn build_af_match_library(
    library: AfMatchLibrary,
    fluor_matrix: MatRef<'_, f64>,
    options: AfMatchOptions,
) -> Result<PreparedAfLibrary, TruOlsError> {
    let n_detectors = fluor_matrix.nrows();
    let n_fluor = fluor_matrix.ncols();

    if n_detectors == 0 || n_fluor == 0 {
        return Err(TruOlsError::InvalidMixingMatrix(
            "fluor matrix must have at least one detector and one endmember".into(),
        ));
    }
    if n_fluor > n_detectors {
        return Err(TruOlsError::InvalidMixingMatrix(format!(
            "fluor matrix has more endmembers ({n_fluor}) than detectors ({n_detectors}); AF matching needs an overdetermined panel"
        )));
    }
    if library.entries.is_empty() {
        return Err(TruOlsError::InsufficientData(
            "AF catalog is empty; at least one autofluorescence signature is required".into(),
        ));
    }
    if !library.beta.is_finite() || library.beta <= 0.0 {
        return Err(TruOlsError::InsufficientData(format!(
            "AF match beta must be finite and positive, got {}",
            library.beta
        )));
    }
    if !library.detector_names.is_empty() && library.detector_names.len() != n_detectors {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors,
            actual: library.detector_names.len(),
        });
    }
    if !options.residual_epsilon.is_finite() || options.residual_epsilon < 0.0 {
        return Err(TruOlsError::InsufficientData(format!(
            "residual_epsilon must be finite and non-negative, got {}",
            options.residual_epsilon
        )));
    }
    if let Some(tau) = options
        .morph_outlier_tau
        .filter(|tau| !tau.is_finite() || *tau < 0.0)
    {
        return Err(TruOlsError::InsufficientData(format!(
            "morph_outlier_tau must be finite and non-negative, got {tau}"
        )));
    }

    let fluor_basis = orthonormal_basis(fluor_matrix);
    if fluor_basis.ncols() != n_fluor {
        return Err(TruOlsError::InvalidMixingMatrix(format!(
            "fluor matrix is rank deficient: {n_fluor} endmembers span only {} dimensions. \
             Remove duplicate or near-duplicate endmember signatures before AF matching.",
            fluor_basis.ncols()
        )));
    }

    let mut kept: Vec<PreparedAfEntry> = Vec::with_capacity(library.entries.len());
    let mut kept_proportions: Vec<f64> = Vec::with_capacity(library.entries.len());
    let mut dropped: Vec<DroppedAfEntry> = Vec::new();
    let mut proportion_total = 0.0_f64;

    for entry in &library.entries {
        if entry.values.len() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: entry.values.len(),
            });
        }
        if !entry.mixing_proportion.is_finite() || entry.mixing_proportion <= 0.0 {
            return Err(TruOlsError::InsufficientData(format!(
                "AF catalog entry '{}' has a non-positive mixing proportion ({})",
                entry.id, entry.mixing_proportion
            )));
        }
        if entry.values.iter().any(|v| !v.is_finite())
            || entry.fsc_ssc_mean.iter().any(|v| !v.is_finite())
        {
            return Err(TruOlsError::InsufficientData(format!(
                "AF catalog entry '{}' contains non-finite values",
                entry.id
            )));
        }

        let values_norm_sq: f64 = entry.values.iter().map(|v| v * v).sum();
        if values_norm_sq <= 0.0 {
            dropped.push(DroppedAfEntry {
                id: entry.id.clone(),
                relative_residual: 0.0,
            });
            continue;
        }

        // QR least squares against the fluorophore matrix, not a normal-equations
        // inverse: the Gram matrix of a spectral panel is routinely too ill conditioned
        // for its inverse to mean anything.
        let target = Col::from_fn(n_detectors, |i| entry.values[i]);
        let fit_coefficients = solve_linear_system(fluor_matrix, target.as_ref())?;
        let mut residual = vec![0.0_f64; n_detectors];
        let mut residual_norm_sq = 0.0_f64;
        for i in 0..n_detectors {
            let mut fitted = 0.0_f64;
            for j in 0..n_fluor {
                fitted += fluor_matrix[(i, j)] * fit_coefficients[j];
            }
            let r = entry.values[i] - fitted;
            residual[i] = r;
            residual_norm_sq += r * r;
        }

        let relative_residual = residual_norm_sq / values_norm_sq;
        if !relative_residual.is_finite() || relative_residual < options.residual_epsilon {
            dropped.push(DroppedAfEntry {
                id: entry.id.clone(),
                relative_residual,
            });
            continue;
        }

        let (inv_cov, log_det) = invert_2x2_covariance(&entry.fsc_ssc_cov, &entry.id)?;
        proportion_total += entry.mixing_proportion;
        kept_proportions.push(entry.mixing_proportion);
        kept.push(PreparedAfEntry {
            id: entry.id.clone(),
            values: entry.values.clone(),
            residual,
            residual_norm_sq,
            mean: entry.fsc_ssc_mean,
            inv_cov,
            // `mixing_proportion` is folded in below, once the total is known.
            log_prior_peak: -LN_TWO_PI - 0.5 * log_det,
        });
    }

    if kept.is_empty() {
        return Err(TruOlsError::InsufficientData(format!(
            "every AF catalog entry ({}) lies inside the fluorophore span at epsilon {}; \
             none of them can be distinguished from a combination of dyes",
            library.entries.len(),
            options.residual_epsilon
        )));
    }

    // Renormalize over survivors so dropping an entry rescales the mixture rather than
    // shrinking the prior that the outlier test is compared against.
    let mut peak_density_total = 0.0_f64;
    for (entry, proportion) in kept.iter_mut().zip(&kept_proportions) {
        entry.log_prior_peak += (proportion / proportion_total).ln();
        peak_density_total += entry.log_prior_peak.exp();
    }

    let morph_outlier_tau = options
        .morph_outlier_tau
        .unwrap_or(DEFAULT_MORPH_OUTLIER_FRACTION * peak_density_total);

    Ok(PreparedAfLibrary {
        entries: kept,
        dropped,
        fluor_basis,
        n_detectors,
        beta: library.beta,
        detector_names: library.detector_names,
        morph_outlier_tau,
    })
}

/// Assign each event a winner AF signature and subtract it.
///
/// `observations` is events × detectors of raw detector signal, `fsc_ssc` is events × 2
/// in the same units the catalog's morphological clusters were fitted in, and `o_ns` is
/// the fluorophore-only nonspecific observation to remove before matching (see
/// [`crate::NonspecificObservation::calculate_fluor_only`]).
pub fn match_and_subtract(
    observations: MatRef<'_, f64>,
    fsc_ssc: MatRef<'_, f64>,
    o_ns: Option<&[f64]>,
    library: &PreparedAfLibrary,
) -> Result<AfMatchResult, TruOlsError> {
    let n_events = observations.nrows();
    let n_detectors = library.n_detectors;

    if observations.ncols() != n_detectors {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors,
            actual: observations.ncols(),
        });
    }
    if fsc_ssc.ncols() != 2 {
        return Err(TruOlsError::DimensionMismatch {
            expected: 2,
            actual: fsc_ssc.ncols(),
        });
    }
    if fsc_ssc.nrows() != n_events {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_events,
            actual: fsc_ssc.nrows(),
        });
    }
    if let Some(baseline) = o_ns.filter(|b| b.len() != n_detectors) {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors,
            actual: baseline.len(),
        });
    }
    if library.is_empty() {
        return Err(TruOlsError::InsufficientData(
            "prepared AF library has no usable entries".into(),
        ));
    }

    let outcomes: Vec<EventOutcome> = if crate::use_parallel_independent_events(n_events) {
        use rayon::prelude::*;
        // `collect` on an indexed parallel iterator preserves event order.
        (0..n_events)
            .into_par_iter()
            .map_init(
                || EventScratch::new(n_detectors, library.fluor_rank(), library.len()),
                |scratch, event| match_event(library, observations, fsc_ssc, o_ns, event, scratch),
            )
            .collect()
    } else {
        let mut scratch = EventScratch::new(n_detectors, library.fluor_rank(), library.len());
        (0..n_events)
            .map(|event| match_event(library, observations, fsc_ssc, o_ns, event, &mut scratch))
            .collect()
    };

    let mut af_abundance = Vec::with_capacity(n_events);
    let mut assignment_1based = Vec::with_capacity(n_events);
    let mut confidence = Vec::with_capacity(n_events);
    let mut residual_observations = Mat::<f64>::zeros(n_events, n_detectors);
    for (event, outcome) in outcomes.into_iter().enumerate() {
        af_abundance.push(outcome.af_abundance);
        assignment_1based.push(outcome.assignment_1based);
        confidence.push(outcome.confidence);
        for (detector, value) in outcome.residual.into_iter().enumerate() {
            residual_observations[(event, detector)] = value;
        }
    }

    Ok(AfMatchResult {
        af_abundance,
        assignment_1based,
        confidence,
        residual_observations,
    })
}

/// Match one event and return Observation / fitted AF / winner / residual for UI inspect.
pub fn inspect_match_event(
    observations: MatRef<'_, f64>,
    fsc_ssc: MatRef<'_, f64>,
    o_ns: Option<&[f64]>,
    library: &PreparedAfLibrary,
    event: usize,
) -> Result<AfEventInspect, TruOlsError> {
    if observations.nrows() == 0 {
        return Err(TruOlsError::InsufficientData(
            "inspect_match_event needs at least one event".into(),
        ));
    }
    if event >= observations.nrows() {
        return Err(TruOlsError::InsufficientData(format!(
            "event index {event} out of range (n={})",
            observations.nrows()
        )));
    }
    if library.is_empty() {
        return Err(TruOlsError::InsufficientData(
            "prepared AF library has no usable entries".into(),
        ));
    }
    let mut scratch = EventScratch::new(library.n_detectors, library.fluor_rank(), library.len());
    let outcome = match_event(library, observations, fsc_ssc, o_ns, event, &mut scratch);
    let best = outcome.assignment_1based.saturating_sub(1) as usize;
    let entry = library
        .entries
        .get(best)
        .ok_or_else(|| TruOlsError::InsufficientData("empty AF library".into()))?;
    let winner_af: Vec<f64> = entry
        .values
        .iter()
        .map(|v| v * outcome.winner_amount)
        .collect();
    Ok(AfEventInspect {
        observation: outcome.observation,
        fitted_af: outcome.fitted_af,
        winner_af,
        residual: outcome.residual,
        assignment_1based: outcome.assignment_1based,
        assignment_id: entry.id.clone(),
        confidence: outcome.confidence,
        af_abundance: outcome.af_abundance,
    })
}

struct EventOutcome {
    af_abundance: f64,
    assignment_1based: u32,
    confidence: f64,
    residual: Vec<f64>,
    observation: Vec<f64>,
    fitted_af: Vec<f64>,
    winner_amount: f64,
}

/// Per-event buffers, reused across events (and across Rayon tasks via `map_init`).
struct EventScratch {
    adjusted: Vec<f64>,
    /// \((I-P)y'\).
    orthogonal: Vec<f64>,
    basis_coefficients: Vec<f64>,
    /// \(k_k\) per entry.
    amounts: Vec<f64>,
    log_spectral: Vec<f64>,
    log_morph: Vec<f64>,
    weights: Vec<f64>,
}

impl EventScratch {
    fn new(n_detectors: usize, fluor_rank: usize, n_entries: usize) -> Self {
        Self {
            adjusted: vec![0.0; n_detectors],
            orthogonal: vec![0.0; n_detectors],
            basis_coefficients: vec![0.0; fluor_rank],
            amounts: vec![0.0; n_entries],
            log_spectral: vec![0.0; n_entries],
            log_morph: vec![0.0; n_entries],
            weights: vec![0.0; n_entries],
        }
    }
}

fn match_event(
    library: &PreparedAfLibrary,
    observations: MatRef<'_, f64>,
    fsc_ssc: MatRef<'_, f64>,
    o_ns: Option<&[f64]>,
    event: usize,
    scratch: &mut EventScratch,
) -> EventOutcome {
    let n_detectors = library.n_detectors;

    for i in 0..n_detectors {
        let baseline = o_ns.map_or(0.0, |b| b[i]);
        scratch.adjusted[i] = observations[(event, i)] - baseline;
    }
    project_out(
        library.fluor_basis.as_ref(),
        &scratch.adjusted,
        &mut scratch.basis_coefficients,
        &mut scratch.orthogonal,
    );

    let fsc = fsc_ssc[(event, 0)];
    let ssc = fsc_ssc[(event, 1)];
    let n_entries = library.entries.len();

    for (k, entry) in library.entries.iter().enumerate() {
        let mut projection = 0.0_f64;
        for i in 0..n_detectors {
            projection += scratch.adjusted[i] * entry.residual[i];
        }
        // Negative AF is not physical, and letting it go negative lets the AF channel
        // absorb fluorophore signal with the wrong sign.
        let amount = (projection / entry.residual_norm_sq).max(0.0);
        scratch.amounts[k] = amount;

        // (I-P)(y' - k a_k) = (I-P)y' - k r_k, so the projection above is reused.
        let mut leftover_norm_sq = 0.0_f64;
        for i in 0..n_detectors {
            let d = scratch.orthogonal[i] - amount * entry.residual[i];
            leftover_norm_sq += d * d;
        }
        scratch.log_spectral[k] = leftover_norm_sq.sqrt();

        let d0 = fsc - entry.mean[0];
        let d1 = ssc - entry.mean[1];
        let mahalanobis = d0 * (entry.inv_cov[0][0] * d0 + entry.inv_cov[0][1] * d1)
            + d1 * (entry.inv_cov[1][0] * d0 + entry.inv_cov[1][1] * d1);
        scratch.log_morph[k] = entry.log_prior_peak - 0.5 * mahalanobis;
    }

    let mut k_star = 0usize;
    for k in 1..n_entries {
        if scratch.log_spectral[k] < scratch.log_spectral[k_star] {
            k_star = k;
        }
    }

    let mut k_2 = None;
    for k in 0..n_entries {
        if k == k_star {
            continue;
        }
        match k_2 {
            None => k_2 = Some(k),
            Some(j) if scratch.log_spectral[k] < scratch.log_spectral[j] => k_2 = Some(k),
            _ => {}
        }
    }

    let mut winner = k_star;
    if let Some(k2) = k_2 {
        let r_star = scratch.log_spectral[k_star];
        let r2 = scratch.log_spectral[k2];
        let residual_tie = (r2 - r_star) <= 0.05 * r_star.max(1e-12);
        if residual_tie {
            let density_star = scratch.log_morph[k_star].exp();
            let density_2 = scratch.log_morph[k2].exp();
            let both_outliers =
                density_star < library.morph_outlier_tau && density_2 < library.morph_outlier_tau;
            if !both_outliers && density_2 > density_star {
                winner = k2;
            }
        }
    }

    scratch.weights.fill(0.0);
    scratch.weights[winner] = 1.0;

    let af_abundance = scratch.amounts[winner];
    let observation = scratch.adjusted[..n_detectors].to_vec();
    let mut fitted_af = vec![0.0_f64; n_detectors];
    let scale = scratch.amounts[winner];
    for (fitted, value) in fitted_af.iter_mut().zip(&library.entries[winner].values) {
        *fitted = scale * value;
    }
    let residual: Vec<f64> = observation
        .iter()
        .zip(&fitted_af)
        .map(|(o, f)| o - f)
        .collect();

    let confidence = if n_entries == 1 {
        1.0
    } else if let Some(k2) = k_2 {
        let r_star = scratch.log_spectral[k_star];
        let r2 = scratch.log_spectral[k2];
        (1.0 - r_star / (r_star + r2)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    EventOutcome {
        af_abundance,
        assignment_1based: (winner + 1) as u32,
        confidence,
        residual,
        observation,
        fitted_af,
        winner_amount: scratch.amounts[winner],
    }
}

/// Softmax in place, via log-sum-exp so a very peaked likelihood cannot underflow to a
/// zero denominator. Degenerate input (all `-inf`, any `NaN`) becomes a uniform blend,
/// which subtracts the average AF rather than an arbitrary one.
#[cfg(test)]
fn normalize_log_weights(weights: &mut [f64]) {
    let uniform = 1.0 / weights.len() as f64;
    let max = weights
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, |acc, w| if w > acc { w } else { acc });
    if !max.is_finite() {
        weights.fill(uniform);
        return;
    }
    let mut total = 0.0_f64;
    for w in weights.iter_mut() {
        *w = (*w - max).exp();
        total += *w;
    }
    if !total.is_finite() || total <= 0.0 {
        weights.fill(uniform);
        return;
    }
    for w in weights.iter_mut() {
        *w /= total;
    }
}

/// Orthonormal basis for the column space of `m`, as a detectors × rank matrix.
///
/// Modified Gram-Schmidt with a second orthogonalization pass; the reorthogonalization
/// is what keeps the basis orthogonal to working precision for the near-collinear
/// columns a real spectral panel produces.
fn orthonormal_basis(m: MatRef<'_, f64>) -> Mat<f64> {
    let n_rows = m.nrows();
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(m.ncols());

    for j in 0..m.ncols() {
        let mut v: Vec<f64> = (0..n_rows).map(|i| m[(i, j)]).collect();
        let original_norm = euclidean_norm(&v);
        // A norm is non-negative by construction, so `== 0.0` catches the empty column
        // and `is_finite` catches a column carrying NaN or an overflow.
        if original_norm == 0.0 || !original_norm.is_finite() {
            continue;
        }
        for _ in 0..2 {
            for b in &basis {
                let overlap: f64 = v.iter().zip(b).map(|(vi, bi)| vi * bi).sum();
                for (vi, bi) in v.iter_mut().zip(b) {
                    *vi -= overlap * bi;
                }
            }
        }
        let norm = euclidean_norm(&v);
        if norm <= RANK_TOLERANCE * original_norm {
            continue;
        }
        for vi in v.iter_mut() {
            *vi /= norm;
        }
        basis.push(v);
    }

    Mat::from_fn(n_rows, basis.len(), |i, j| basis[j][i])
}

/// `out = v - Q(Qᵗ v)`, with `coefficients` used as the `Qᵗ v` scratch.
fn project_out(basis: MatRef<'_, f64>, v: &[f64], coefficients: &mut [f64], out: &mut [f64]) {
    let n_rows = basis.nrows();
    let rank = basis.ncols();
    for j in 0..rank {
        let mut dot = 0.0_f64;
        for i in 0..n_rows {
            dot += basis[(i, j)] * v[i];
        }
        coefficients[j] = dot;
    }
    for i in 0..n_rows {
        let mut fitted = 0.0_f64;
        for j in 0..rank {
            fitted += basis[(i, j)] * coefficients[j];
        }
        out[i] = v[i] - fitted;
    }
}

fn euclidean_norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Closed-form 2×2 inverse plus `ln|Σ|`, rejecting anything that is not positive definite.
fn invert_2x2_covariance(
    cov: &[[f64; 2]; 2],
    entry_id: &str,
) -> Result<([[f64; 2]; 2], f64), TruOlsError> {
    let reject = |reason: &str| {
        TruOlsError::InsufficientData(format!(
            "AF catalog entry '{entry_id}' has an unusable FSC/SSC covariance: {reason}"
        ))
    };

    if cov.iter().flatten().any(|v| !v.is_finite()) {
        return Err(reject("contains non-finite values"));
    }
    if cov[0][0] <= 0.0 || cov[1][1] <= 0.0 {
        return Err(reject("a variance is not positive"));
    }
    let determinant = cov[0][0] * cov[1][1] - cov[0][1] * cov[1][0];
    if determinant <= 0.0 {
        return Err(reject(
            "the determinant is not positive (the covariance is singular or indefinite)",
        ));
    }

    let inv = [
        [cov[1][1] / determinant, -cov[0][1] / determinant],
        [-cov[1][0] / determinant, cov[0][0] / determinant],
    ];
    Ok((inv, determinant.ln()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    /// Four detectors, two fluorophores confined to the first two. AF signatures then
    /// have detectors 3/4 to live in, so the fluorophore-orthogonal residual is exact
    /// and the expected answers are known in closed form.
    fn fluor_matrix() -> Mat<f64> {
        mat![[1.00, 0.20], [0.15, 1.00], [0.00, 0.00], [0.00, 0.00],]
    }

    fn entry(
        id: &str,
        values: [f64; 4],
        mean: [f64; 2],
        variance: f64,
        proportion: f64,
    ) -> AfCatalogEntry {
        AfCatalogEntry {
            id: id.to_string(),
            values: values.to_vec(),
            fsc_ssc_mean: mean,
            fsc_ssc_cov: [[variance, 0.0], [0.0, variance]],
            mixing_proportion: proportion,
        }
    }

    /// Two AF populations: a "small cell" one peaking on detector 3 and a "large cell"
    /// one peaking on detector 4, with well-separated scatter clusters.
    fn two_cluster_library(beta: f64) -> AfMatchLibrary {
        AfMatchLibrary {
            entries: vec![
                entry(
                    "af_small",
                    [0.10, 0.05, 1.00, 0.20],
                    [200.0, 150.0],
                    400.0,
                    0.6,
                ),
                entry(
                    "af_large",
                    [0.05, 0.10, 0.20, 1.00],
                    [800.0, 700.0],
                    400.0,
                    0.4,
                ),
            ],
            beta,
            detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
        }
    }

    fn prepared(beta: f64) -> PreparedAfLibrary {
        build_af_match_library(
            two_cluster_library(beta),
            fluor_matrix().as_ref(),
            AfMatchOptions::default(),
        )
        .expect("library builds")
    }

    /// `fluor_amounts · fluor_matrix + af_amount · signature`, i.e. the forward model.
    fn synthesize(fluor_amounts: [f64; 2], af_signature: &[f64], af_amount: f64) -> [f64; 4] {
        let m = fluor_matrix();
        let mut out = [0.0_f64; 4];
        for i in 0..4 {
            out[i] = m[(i, 0)] * fluor_amounts[0]
                + m[(i, 1)] * fluor_amounts[1]
                + af_signature[i] * af_amount;
        }
        out
    }

    #[test]
    fn residuals_are_orthogonal_to_the_fluor_span() {
        let library = prepared(1.0);
        let m = fluor_matrix();
        for entry in &library.entries {
            for j in 0..m.ncols() {
                let dot: f64 = (0..4).map(|i| m[(i, j)] * entry.residual[i]).sum();
                assert!(
                    dot.abs() < 1e-12,
                    "entry {} residual not orthogonal to fluor column {j}: {dot}",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn entries_inside_the_fluor_span_are_dropped() {
        let m = fluor_matrix();
        let inside: Vec<f64> = (0..4).map(|i| m[(i, 0)] + 0.5 * m[(i, 1)]).collect();
        let mut library = two_cluster_library(1.0);
        library.entries.push(AfCatalogEntry {
            id: "af_collinear".into(),
            values: inside,
            fsc_ssc_mean: [500.0, 500.0],
            fsc_ssc_cov: [[400.0, 0.0], [0.0, 400.0]],
            mixing_proportion: 0.3,
        });

        let built =
            build_af_match_library(library, fluor_matrix().as_ref(), AfMatchOptions::default())
                .expect("library builds");

        assert_eq!(
            built.len(),
            2,
            "only the two distinguishable entries survive"
        );
        assert_eq!(built.entry_ids(), vec!["af_small", "af_large"]);
        assert_eq!(built.dropped_entries().len(), 1);
        assert_eq!(built.dropped_entries()[0].id, "af_collinear");
    }

    #[test]
    fn an_all_collinear_catalog_is_an_error() {
        let m = fluor_matrix();
        let inside: Vec<f64> = (0..4).map(|i| m[(i, 1)]).collect();
        let library = AfMatchLibrary {
            entries: vec![AfCatalogEntry {
                id: "af_collinear".into(),
                values: inside,
                fsc_ssc_mean: [500.0, 500.0],
                fsc_ssc_cov: [[400.0, 0.0], [0.0, 400.0]],
                mixing_proportion: 1.0,
            }],
            beta: 1.0,
            detector_names: Vec::new(),
        };
        let err = build_af_match_library(library, m.as_ref(), AfMatchOptions::default())
            .expect_err("no usable entries");
        assert!(
            matches!(err, TruOlsError::InsufficientData(ref msg) if msg.contains("fluorophore span")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn a_rank_deficient_fluor_matrix_is_rejected() {
        let duplicated = mat![[1.0, 1.0], [0.2, 0.2], [0.0, 0.0], [0.0, 0.0]];
        let err = build_af_match_library(
            two_cluster_library(1.0),
            duplicated.as_ref(),
            AfMatchOptions::default(),
        )
        .expect_err("rank deficiency is fatal");
        assert!(
            matches!(err, TruOlsError::InvalidMixingMatrix(ref msg) if msg.contains("rank deficient")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn recovers_the_generating_af_population_and_amount() {
        let library = prepared(2.0);
        let small = &library.entries[0].values.clone();
        let large = &library.entries[1].values.clone();

        // One event from each cluster, generated by the forward model.
        let small_event = synthesize([120.0, 45.0], small, 30.0);
        let large_event = synthesize([80.0, 200.0], large, 55.0);
        let observations = Mat::from_fn(2, 4, |ev, det| {
            if ev == 0 {
                small_event[det]
            } else {
                large_event[det]
            }
        });
        let scatter = mat![[200.0, 150.0], [800.0, 700.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");

        assert_eq!(result.assignment_1based, vec![1, 2]);
        assert!(
            result.confidence.iter().all(|&c| c > 0.99),
            "unambiguous events should be confident: {:?}",
            result.confidence
        );
        assert!(
            (result.af_abundance[0] - 30.0).abs() < 1e-6,
            "af_abundance {:?}",
            result.af_abundance
        );
        assert!((result.af_abundance[1] - 55.0).abs() < 1e-6);

        // With the AF removed, what is left is exactly the fluorophore contribution.
        let expected_small = synthesize([120.0, 45.0], &[0.0; 4], 0.0);
        for (det, expected) in expected_small.iter().enumerate() {
            assert!(
                (result.residual_observations[(0, det)] - expected).abs() < 1e-6,
                "residual detector {det}: {} vs {expected}",
                result.residual_observations[(0, det)],
            );
        }
    }

    #[test]
    fn morphology_breaks_a_spectral_tie() {
        // A 50/50 spectral blend of the two AF signatures: spectrally ambiguous by
        // construction, so only FSC/SSC can decide.
        let library = prepared(0.05);
        let blended: Vec<f64> = (0..4)
            .map(|i| 0.5 * (library.entries[0].values[i] + library.entries[1].values[i]))
            .collect();
        let event = synthesize([50.0, 50.0], &blended, 40.0);
        let observations = Mat::from_fn(2, 4, |_, det| event[det]);
        // Row 0 sits on the small-cell cluster, row 1 on the large-cell cluster.
        let scatter = mat![[200.0, 150.0], [800.0, 700.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");
        assert_eq!(
            result.assignment_1based,
            vec![1, 2],
            "identical spectra must follow their scatter cluster"
        );
    }

    #[test]
    fn occupancy_cannot_steal_a_clearer_residual() {
        // Entry 0 = rare AF (small occupancy / prior). Entry 1 = common AF.
        // Event is a scaled copy of entry 0 sitting on entry 1's FSC/SSC.
        // Pair-test must assign 1 (entry 0), not the occupancy-heavy Gaussian.
        let library = AfMatchLibrary {
            entries: vec![
                entry(
                    "af_rare",
                    [0.10, 0.05, 1.00, 0.20],
                    [200.0, 150.0],
                    400.0,
                    0.01,
                ),
                entry(
                    "af_common",
                    [0.05, 0.10, 0.20, 1.00],
                    [800.0, 700.0],
                    400.0,
                    0.99,
                ),
            ],
            beta: 2.0,
            detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
        };
        let prepared =
            build_af_match_library(library, fluor_matrix().as_ref(), AfMatchOptions::default())
                .expect("library builds");
        let rare = prepared.entries[0].values.clone();
        let event = synthesize([50.0, 50.0], &rare, 40.0);
        let observations = Mat::from_fn(1, 4, |_, det| event[det]);
        let scatter = mat![[800.0, 700.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &prepared)
            .expect("match succeeds");
        assert_eq!(
            result.assignment_1based,
            vec![1],
            "occupancy must not override a clearer residual"
        );
    }

    #[test]
    fn scatter_ignored_when_residual_gap_exceeds_five_percent() {
        // Distinct spectra; event matches entry 0; FSC/SSC sits on entry 1.
        // assignment_1based == 1.
        let library = prepared(2.0);
        let small = library.entries[0].values.clone();
        let event = synthesize([50.0, 50.0], &small, 40.0);
        let observations = Mat::from_fn(1, 4, |_, det| event[det]);
        let scatter = mat![[800.0, 700.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");
        assert_eq!(
            result.assignment_1based,
            vec![1],
            "scatter must be ignored when the residual gap exceeds 5%"
        );
    }

    #[test]
    fn winner_only_subtract_does_not_blend() {
        // Two similar-but-not-tied signatures; residual of fitted AF must equal
        // y - k_star * a_star, not a weighted sum.
        let library = AfMatchLibrary {
            entries: vec![
                entry("af_a", [0.10, 0.05, 1.00, 0.20], [200.0, 150.0], 400.0, 0.5),
                entry("af_b", [0.12, 0.07, 0.90, 0.28], [220.0, 160.0], 400.0, 0.5),
            ],
            beta: 2.0,
            detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
        };
        let prepared =
            build_af_match_library(library, fluor_matrix().as_ref(), AfMatchOptions::default())
                .expect("library builds");
        let signature = prepared.entries[0].values.clone();
        let event = synthesize([80.0, 40.0], &signature, 35.0);
        let observations = Mat::from_fn(1, 4, |_, det| event[det]);
        let scatter = mat![[200.0, 150.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &prepared)
            .expect("match succeeds");
        assert_eq!(result.assignment_1based, vec![1]);
        assert!(
            (result.af_abundance[0] - 35.0).abs() < 1e-6,
            "winner amount {:?}",
            result.af_abundance
        );
        for det in 0..4 {
            let expected = event[det] - 35.0 * signature[det];
            assert!(
                (result.residual_observations[(0, det)] - expected).abs() < 1e-6,
                "detector {det}: {} vs {expected} (must be winner subtract, not a blend)",
                result.residual_observations[(0, det)],
            );
        }
    }

    #[test]
    fn confidence_is_residual_gap_not_softmax_weight() {
        // Two equally good residuals → confidence ≈ 0.5.
        // One library entry → confidence == 1.0.
        let library = prepared(2.0);
        let blended: Vec<f64> = (0..4)
            .map(|i| 0.5 * (library.entries[0].values[i] + library.entries[1].values[i]))
            .collect();
        let event = synthesize([50.0, 50.0], &blended, 40.0);
        let observations = Mat::from_fn(1, 4, |_, det| event[det]);
        let scatter = mat![[200.0, 150.0]];

        let two_entry = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");
        assert!(
            (two_entry.confidence[0] - 0.5).abs() < 1e-6,
            "equal residuals must score confidence 0.5, got {:?}",
            two_entry.confidence
        );

        let mean_values: Vec<f64> = blended;
        let single = AfMatchLibrary {
            entries: vec![entry(
                "af_only",
                [
                    mean_values[0],
                    mean_values[1],
                    mean_values[2],
                    mean_values[3],
                ],
                [200.0, 150.0],
                400.0,
                1.0,
            )],
            beta: 2.0,
            detector_names: library.detector_names.clone(),
        };
        let single_prepared =
            build_af_match_library(single, fluor_matrix().as_ref(), AfMatchOptions::default())
                .expect("single-entry library builds");
        let one_entry = match_and_subtract(
            observations.as_ref(),
            scatter.as_ref(),
            None,
            &single_prepared,
        )
        .expect("match succeeds");
        assert_eq!(one_entry.confidence[0], 1.0);
    }

    #[test]
    fn spectral_tie_uses_scatter() {
        // A 50/50 spectral blend is a 0% residual gap, so FSC/SSC still decides.
        let library = prepared(1e-6);
        let blended: Vec<f64> = (0..4)
            .map(|i| 0.5 * (library.entries[0].values[i] + library.entries[1].values[i]))
            .collect();
        let event = synthesize([50.0, 50.0], &blended, 40.0);
        let observations = Mat::from_fn(2, 4, |_, det| event[det]);
        let scatter = mat![[200.0, 150.0], [800.0, 700.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");
        assert_eq!(
            result.assignment_1based,
            vec![1, 2],
            "a spectral tie must let scatter decide"
        );
    }

    #[test]
    fn huge_beta_is_spectral_argmax() {
        // Distinct spectra: leftover gap exceeds 5%, so scatter must not flip the
        // assignment even when the event sits on the other cluster.
        let library = prepared(1e6);
        let small = library.entries[0].values.clone();
        let event = synthesize([50.0, 50.0], &small, 40.0);
        let observations = Mat::from_fn(1, 4, |_, det| event[det]);
        let scatter = mat![[800.0, 700.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");
        assert_eq!(
            result.assignment_1based,
            vec![1],
            "huge β must keep spectral argmax despite conflicting scatter"
        );
    }

    #[test]
    fn a_morphological_outlier_falls_back_to_spectral_evidence() {
        // `af_small` carries the larger mixing proportion, so a morphology-driven
        // assignment would prefer it. This event is spectrally pure `af_large` but its
        // scatter is implausible under both clusters.
        let library = prepared(2.0);
        let large = library.entries[1].values.clone();
        let event = synthesize([10.0, 10.0], &large, 60.0);
        let observations = Mat::from_fn(1, 4, |_, det| event[det]);
        let scatter = mat![[50_000.0, 50_000.0]];

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");
        assert_eq!(result.assignment_1based, vec![2]);
        assert!((result.af_abundance[0] - 60.0).abs() < 1e-6);
    }

    #[test]
    fn the_nonspecific_baseline_is_removed_before_matching() {
        let library = prepared(2.0);
        let small = library.entries[0].values.clone();
        let baseline = [3.0_f64, 2.0, 0.5, 0.25];
        let clean = synthesize([100.0, 40.0], &small, 25.0);
        let observations = Mat::from_fn(1, 4, |_, det| clean[det] + baseline[det]);
        let scatter = mat![[200.0, 150.0]];

        let with_baseline = match_and_subtract(
            observations.as_ref(),
            scatter.as_ref(),
            Some(&baseline),
            &library,
        )
        .expect("match succeeds");
        let without_baseline = match_and_subtract(
            Mat::from_fn(1, 4, |_, det| clean[det]).as_ref(),
            scatter.as_ref(),
            None,
            &library,
        )
        .expect("match succeeds");

        assert!((with_baseline.af_abundance[0] - 25.0).abs() < 1e-6);
        for det in 0..4 {
            assert!(
                (with_baseline.residual_observations[(0, det)]
                    - without_baseline.residual_observations[(0, det)])
                    .abs()
                    < 1e-9,
                "supplying o_ns must be equivalent to pre-subtracting it"
            );
        }
    }

    #[test]
    fn every_event_of_a_rayon_sized_batch_is_matched_correctly() {
        let library = prepared(2.0);
        let signatures = [
            library.entries[0].values.clone(),
            library.entries[1].values.clone(),
        ];
        let n_events = 4 * crate::PARALLEL_INDEPENDENT_EVENTS_THRESHOLD;
        assert!(crate::use_parallel_independent_events(n_events));

        let amounts: Vec<f64> = (0..n_events).map(|e| 10.0 + (e % 17) as f64).collect();
        let sources: Vec<usize> = (0..n_events).map(|e| e % 2).collect();
        let rows: Vec<[f64; 4]> = (0..n_events)
            .map(|e| {
                synthesize(
                    [20.0 + (e % 5) as f64, 30.0 + (e % 7) as f64],
                    &signatures[sources[e]],
                    amounts[e],
                )
            })
            .collect();

        let observations = Mat::from_fn(n_events, 4, |ev, det| rows[ev][det]);
        let scatter = Mat::from_fn(n_events, 2, |ev, axis| {
            let center = if sources[ev] == 0 {
                [200.0, 150.0]
            } else {
                [800.0, 700.0]
            };
            center[axis]
        });

        let result = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &library)
            .expect("match succeeds");

        assert_eq!(result.af_abundance.len(), n_events);
        for e in 0..n_events {
            assert_eq!(result.assignment_1based[e], (sources[e] + 1) as u32);
            assert!(
                (result.af_abundance[e] - amounts[e]).abs() < 1e-6,
                "event {e}: {} vs {}",
                result.af_abundance[e],
                amounts[e]
            );
        }
    }

    #[test]
    fn dimension_mismatches_are_reported_rather_than_indexed_into() {
        let library = prepared(1.0);
        let scatter = mat![[200.0, 150.0]];

        let wrong_detectors = Mat::<f64>::zeros(1, 3);
        assert!(matches!(
            match_and_subtract(wrong_detectors.as_ref(), scatter.as_ref(), None, &library),
            Err(TruOlsError::DimensionMismatch {
                expected: 4,
                actual: 3
            })
        ));

        let observations = Mat::<f64>::zeros(1, 4);
        let wrong_scatter = Mat::<f64>::zeros(1, 3);
        assert!(matches!(
            match_and_subtract(
                observations.as_ref(),
                wrong_scatter.as_ref(),
                None,
                &library
            ),
            Err(TruOlsError::DimensionMismatch {
                expected: 2,
                actual: 3
            })
        ));

        let short_scatter = Mat::<f64>::zeros(2, 2);
        assert!(matches!(
            match_and_subtract(
                observations.as_ref(),
                short_scatter.as_ref(),
                None,
                &library
            ),
            Err(TruOlsError::DimensionMismatch {
                expected: 1,
                actual: 2
            })
        ));

        assert!(matches!(
            match_and_subtract(
                observations.as_ref(),
                scatter.as_ref(),
                Some(&[0.0, 0.0]),
                &library
            ),
            Err(TruOlsError::DimensionMismatch {
                expected: 4,
                actual: 2
            })
        ));
    }

    #[test]
    fn a_singular_morphological_covariance_is_rejected() {
        let mut library = two_cluster_library(1.0);
        library.entries[0].fsc_ssc_cov = [[100.0, 100.0], [100.0, 100.0]];
        let err =
            build_af_match_library(library, fluor_matrix().as_ref(), AfMatchOptions::default())
                .expect_err("singular covariance is fatal");
        assert!(
            matches!(err, TruOlsError::InsufficientData(ref msg) if msg.contains("determinant")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn weights_sum_to_one_and_survive_degenerate_input() {
        let mut weights = vec![f64::NEG_INFINITY; 3];
        normalize_log_weights(&mut weights);
        assert!(weights.iter().all(|w| (w - 1.0 / 3.0).abs() < 1e-12));

        let mut weights = vec![-1000.0, -1001.0, -1002.0];
        normalize_log_weights(&mut weights);
        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(weights[0] > weights[1] && weights[1] > weights[2]);
    }

    /// Clean-room accuracy smoke: soft multi-AF assignment recovers known AF
    /// abundances better than a single mean-AF template (AutoSpectral-style idea,
    /// our own synthetic harness — not a port of AutoSpectral R).
    #[test]
    fn soft_assign_beats_mean_af_baseline_on_abundance_error() {
        let soft_library = prepared(2.0);
        let small = soft_library.entries[0].values.clone();
        let large = soft_library.entries[1].values.clone();

        let truth_amounts = [30.0_f64, 55.0, 42.0, 18.0];
        let small_events = [
            synthesize([120.0, 45.0], &small, truth_amounts[0]),
            synthesize([90.0, 30.0], &small, truth_amounts[2]),
        ];
        let large_events = [
            synthesize([80.0, 200.0], &large, truth_amounts[1]),
            synthesize([60.0, 150.0], &large, truth_amounts[3]),
        ];
        let observations = Mat::from_fn(4, 4, |ev, det| match ev {
            0 => small_events[0][det],
            1 => large_events[0][det],
            2 => small_events[1][det],
            _ => large_events[1][det],
        });
        let scatter = mat![
            [200.0, 150.0],
            [800.0, 700.0],
            [210.0, 140.0],
            [790.0, 710.0],
        ];

        let soft = match_and_subtract(observations.as_ref(), scatter.as_ref(), None, &soft_library)
            .expect("soft match succeeds");
        assert!(
            soft.af_abundance.iter().all(|a| a.is_finite()),
            "soft AF abundances must be finite: {:?}",
            soft.af_abundance
        );
        assert!(
            (0..4)
                .all(|ev| { (0..4).all(|det| soft.residual_observations[(ev, det)].is_finite()) }),
            "soft residuals must be finite"
        );

        let mean_values: Vec<f64> = (0..4).map(|i| 0.5 * (small[i] + large[i])).collect();
        let mean_catalog = AfMatchLibrary {
            entries: vec![entry(
                "af_mean",
                [
                    mean_values[0],
                    mean_values[1],
                    mean_values[2],
                    mean_values[3],
                ],
                [500.0, 425.0],
                400.0,
                1.0,
            )],
            beta: 2.0,
            detector_names: soft_library.detector_names.clone(),
        };
        let mean_prepared = build_af_match_library(
            mean_catalog,
            fluor_matrix().as_ref(),
            AfMatchOptions::default(),
        )
        .expect("mean library builds");
        let mean = match_and_subtract(
            observations.as_ref(),
            scatter.as_ref(),
            None,
            &mean_prepared,
        )
        .expect("mean match succeeds");

        let soft_rmse = (0..4)
            .map(|i| {
                let e = soft.af_abundance[i] - truth_amounts[i];
                e * e
            })
            .sum::<f64>()
            .sqrt()
            / 2.0;
        let mean_rmse = (0..4)
            .map(|i| {
                let e = mean.af_abundance[i] - truth_amounts[i];
                e * e
            })
            .sum::<f64>()
            .sqrt()
            / 2.0;

        assert!(
            soft_rmse < 1e-3,
            "soft-assign should recover known AF amounts (rmse={soft_rmse})"
        );
        assert!(
            mean_rmse > soft_rmse * 10.0,
            "mean-AF baseline should be worse than soft-assign (soft={soft_rmse}, mean={mean_rmse})"
        );

        // AF path must change the observation (not a no-op that looks like OLS).
        let mut residual_diff = 0.0;
        for ev in 0..4 {
            for det in 0..4 {
                residual_diff +=
                    (soft.residual_observations[(ev, det)] - observations[(ev, det)]).abs();
            }
        }
        assert!(
            residual_diff > 1.0,
            "AF subtraction must alter observations when AF is present (diff={residual_diff})"
        );
    }
}
