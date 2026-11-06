//! TRU-OLS unmixing implementation.
//!
//! This module contains the main TRU-OLS algorithm that performs per-event
//! unmixing with iterative endmember removal.

use crate::ensure_endmember_limit;
use crate::error::TruOlsError;
#[cfg(all(feature = "blas", not(feature = "unmix-cache")))]
use crate::preprocessing::solve_least_squares_blas_in_place;
#[cfg(all(not(feature = "blas"), not(feature = "unmix-cache")))]
use crate::preprocessing::solve_least_squares_faer_in_place;
use crate::preprocessing::{CutoffCalculator, NonspecificObservation};
use crate::unmix_buffer::{
    UnmixScratch, copy_mixing_into_scratch, swap_columns_cm, swap_index_entries,
};
use faer::{Col, ColRef, Mat, MatRef};
use rand::RngExt;

/// Summary of inner (column-removal) iteration counts over a dataset, for profiling.
#[derive(Debug, Clone)]
pub struct TruncationStats {
    pub n_events: usize,
    pub inner_iterations_min: usize,
    pub inner_iterations_max: usize,
    pub inner_iterations_mean: f64,
}

/// Event-level outputs from one TRU-OLS batch solve.
#[derive(Debug)]
pub struct TruOlsBatchDiagnostics {
    pub abundances: Mat<f64>,
    pub residual_norms: Vec<f64>,
    pub any_fluor_truncated: Vec<bool>,
    #[cfg(test)]
    pub(crate) final_active_indices: Vec<Vec<usize>>,
}

/// Shared mask-factorization cache for batch runs (requires **`unmix-cache`**).
#[cfg(feature = "unmix-cache")]
#[derive(Clone)]
pub struct SharedMaskFactorCache(
    std::sync::Arc<
        quick_cache::sync::Cache<u128, std::sync::Arc<crate::unmix_cache::MaskFactorization>>,
    >,
);

#[cfg(feature = "unmix-cache")]
impl SharedMaskFactorCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(std::sync::Arc::new(quick_cache::sync::Cache::new(capacity)))
    }

    pub(crate) fn cache(
        &self,
    ) -> &quick_cache::sync::Cache<u128, std::sync::Arc<crate::unmix_cache::MaskFactorization>>
    {
        self.0.as_ref()
    }
}

/// Create a shared cache for [`TruOls::from_preprocessed_with_factor_cache`].
#[cfg(feature = "unmix-cache")]
pub fn shared_mask_factor_cache_with_capacity(capacity: usize) -> SharedMaskFactorCache {
    SharedMaskFactorCache::with_capacity(capacity)
}

/// Shared shape checks for the `from_preprocessed*` constructors.
fn validate_preprocessed_shapes(
    mixing_matrix: MatRef<'_, f64>,
    unstained_control: MatRef<'_, f64>,
    cutoffs: ColRef<'_, f64>,
    nonspecific_observation: ColRef<'_, f64>,
    autofluorescence_idx: Option<usize>,
) -> Result<(), TruOlsError> {
    let n_detectors = mixing_matrix.nrows();
    let n_endmembers = mixing_matrix.ncols();
    if unstained_control.ncols() != n_detectors {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors,
            actual: unstained_control.ncols(),
        });
    }
    if cutoffs.nrows() != n_endmembers {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_endmembers,
            actual: cutoffs.nrows(),
        });
    }
    if nonspecific_observation.nrows() != n_detectors {
        return Err(TruOlsError::DimensionMismatch {
            expected: n_detectors,
            actual: nonspecific_observation.nrows(),
        });
    }
    if autofluorescence_idx.is_some_and(|idx| idx >= n_endmembers) {
        return Err(TruOlsError::NoAutofluorescenceEndmember);
    }
    Ok(())
}

/// Strategy for handling irrelevant endmember abundances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmixingStrategy {
    /// Set irrelevant abundances to zero.
    Zero,
    /// Map irrelevant abundances to match unstained control distribution (UCM).
    UnstainedControlMapping,
}

/// Main TRU-OLS unmixing algorithm.
pub struct TruOls {
    mixing_matrix: Mat<f64>,
    cutoffs: Col<f64>,
    nonspecific_observation: Col<f64>,
    unstained_control: Mat<f64>,
    /// `None` for a fluorophore-only panel (after AF subtract): there is no AF
    /// column to protect from truncation or to exempt from UCM. See [`TruOls::new_fluor_only`].
    /// Catalog Then TRU uses [`Self::from_preprocessed`] with a catalog-signature AF column.
    autofluorescence_idx: Option<usize>,
    strategy: UnmixingStrategy,
    /// When `Some(eps)`, only drop a fluor below its cutoff if removing it increases
    /// residual norm by a relative factor of at most `eps` (dual-gate).
    residual_epsilon: Option<f64>,
    /// Present when built with the **`unmix-cache`** feature; keyed by active global-endmember mask.
    #[cfg(feature = "unmix-cache")]
    factor_cache: SharedMaskFactorCache,
}

impl TruOls {
    /// Create a new TRU-OLS instance.
    ///
    /// # Arguments
    /// * `mixing_matrix` - The full mixing matrix (detectors × endmembers)
    /// * `unstained_control` - Unstained control observations (events × detectors)
    /// * `autofluorescence_idx` - Index of the autofluorescence endmember
    ///
    /// # Returns
    /// Configured TRU-OLS instance with default settings (99.5th percentile cutoff,
    /// [`UnmixingStrategy::UnstainedControlMapping`]).
    ///
    /// This runs [`CutoffCalculator::calculate`] and [`NonspecificObservation::calculate`] on every
    /// call. If you already computed cutoffs and the nonspecific observation (for example in a CLI
    /// pipeline that prints them before unmixing), use [`Self::from_preprocessed`] to avoid doing
    /// that work twice.
    pub fn new(
        mixing_matrix: Mat<f64>,
        unstained_control: Mat<f64>,
        autofluorescence_idx: usize,
    ) -> Result<Self, TruOlsError> {
        ensure_endmember_limit(mixing_matrix.ncols())?;
        let cutoffs =
            CutoffCalculator::calculate(mixing_matrix.as_ref(), unstained_control.as_ref(), 0.995)?;
        let nonspecific = NonspecificObservation::calculate(
            mixing_matrix.as_ref(),
            unstained_control.as_ref(),
            autofluorescence_idx,
        )?;

        Ok(Self {
            mixing_matrix,
            cutoffs: cutoffs.cutoffs().clone(),
            nonspecific_observation: nonspecific.observation().clone(),
            unstained_control,
            autofluorescence_idx: Some(autofluorescence_idx),
            strategy: UnmixingStrategy::UnstainedControlMapping,
            residual_epsilon: None,
            #[cfg(feature = "unmix-cache")]
            factor_cache: shared_mask_factor_cache_with_capacity(512),
        })
    }

    /// Create a TRU-OLS instance for a **fluorophore-only** mixing matrix.
    ///
    /// Catalog match+OLS (see [`crate::af_match`]) removes autofluorescence from the observation
    /// before unmixing, so the matrix passed here has no AF column. Every endmember is
    /// then an ordinary fluorophore: none is protected from truncation, and none is
    /// exempt from [`UnmixingStrategy::UnstainedControlMapping`].
    ///
    /// **Do not use for catalog Then TRU.** AutoSpectral → TRU appends the argmax catalog
    /// signature as a protected AF column via [`Self::from_preprocessed`] /
    /// [`Self::new`]. Prefer this over synthesizing a placeholder AF column only when
    /// the pipeline is truly fluorophore-only after AF subtract (OLS).
    pub fn new_fluor_only(
        mixing_matrix: Mat<f64>,
        unstained_control: Mat<f64>,
    ) -> Result<Self, TruOlsError> {
        ensure_endmember_limit(mixing_matrix.ncols())?;
        let cutoffs =
            CutoffCalculator::calculate(mixing_matrix.as_ref(), unstained_control.as_ref(), 0.995)?;
        let nonspecific = NonspecificObservation::calculate_fluor_only(
            mixing_matrix.as_ref(),
            unstained_control.as_ref(),
        )?;

        Ok(Self {
            mixing_matrix,
            cutoffs: cutoffs.cutoffs().clone(),
            nonspecific_observation: nonspecific.observation().clone(),
            unstained_control,
            autofluorescence_idx: None,
            strategy: UnmixingStrategy::UnstainedControlMapping,
            residual_epsilon: None,
            #[cfg(feature = "unmix-cache")]
            factor_cache: shared_mask_factor_cache_with_capacity(512),
        })
    }

    /// [`Self::new_fluor_only`] with cutoffs and the nonspecific observation already computed.
    pub fn from_preprocessed_fluor_only(
        mixing_matrix: Mat<f64>,
        unstained_control: Mat<f64>,
        cutoffs: Col<f64>,
        nonspecific_observation: Col<f64>,
    ) -> Result<Self, TruOlsError> {
        Self::from_preprocessed_with_optional_af(
            mixing_matrix,
            unstained_control,
            cutoffs,
            nonspecific_observation,
            None,
        )
    }

    /// The AF column index, or `None` for a fluorophore-only panel.
    pub fn autofluorescence_index(&self) -> Option<usize> {
        self.autofluorescence_idx
    }

    /// Build a [`TruOls`] from a mixing matrix, unstained control, and **already computed** cutoffs
    /// and nonspecific observation.
    ///
    /// Use this when preprocessing was computed separately so [`Self::new`] does not repeat
    /// `O(events × least_squares)` cutoff work and the nonspecific pass.
    pub fn from_preprocessed(
        mixing_matrix: Mat<f64>,
        unstained_control: Mat<f64>,
        cutoffs: Col<f64>,
        nonspecific_observation: Col<f64>,
        autofluorescence_idx: usize,
    ) -> Result<Self, TruOlsError> {
        Self::from_preprocessed_with_optional_af(
            mixing_matrix,
            unstained_control,
            cutoffs,
            nonspecific_observation,
            Some(autofluorescence_idx),
        )
    }

    fn from_preprocessed_with_optional_af(
        mixing_matrix: Mat<f64>,
        unstained_control: Mat<f64>,
        cutoffs: Col<f64>,
        nonspecific_observation: Col<f64>,
        autofluorescence_idx: Option<usize>,
    ) -> Result<Self, TruOlsError> {
        let n_endmembers = mixing_matrix.ncols();
        validate_preprocessed_shapes(
            mixing_matrix.as_ref(),
            unstained_control.as_ref(),
            cutoffs.as_ref(),
            nonspecific_observation.as_ref(),
            autofluorescence_idx,
        )?;
        ensure_endmember_limit(n_endmembers)?;

        Ok(Self {
            mixing_matrix,
            cutoffs,
            nonspecific_observation,
            unstained_control,
            autofluorescence_idx,
            strategy: UnmixingStrategy::UnstainedControlMapping,
            residual_epsilon: None,
            #[cfg(feature = "unmix-cache")]
            factor_cache: shared_mask_factor_cache_with_capacity(512),
        })
    }

    /// Same as [`Self::from_preprocessed`], but reuses a shared mask-factor cache across batch files.
    #[cfg(feature = "unmix-cache")]
    pub fn from_preprocessed_with_factor_cache(
        mixing_matrix: Mat<f64>,
        unstained_control: Mat<f64>,
        cutoffs: Col<f64>,
        nonspecific_observation: Col<f64>,
        autofluorescence_idx: usize,
        factor_cache: SharedMaskFactorCache,
    ) -> Result<Self, TruOlsError> {
        let n_endmembers = mixing_matrix.ncols();
        validate_preprocessed_shapes(
            mixing_matrix.as_ref(),
            unstained_control.as_ref(),
            cutoffs.as_ref(),
            nonspecific_observation.as_ref(),
            Some(autofluorescence_idx),
        )?;
        ensure_endmember_limit(n_endmembers)?;

        Ok(Self {
            mixing_matrix,
            cutoffs,
            nonspecific_observation,
            unstained_control,
            autofluorescence_idx: Some(autofluorescence_idx),
            strategy: UnmixingStrategy::UnstainedControlMapping,
            residual_epsilon: None,
            factor_cache,
        })
    }

    /// Set the cutoff percentile (default: 0.995).
    ///
    /// This will recalculate cutoffs from the unstained control.
    pub fn set_cutoff_percentile(
        &mut self,
        percentile: f64,
        unstained_control: MatRef<'_, f64>,
    ) -> Result<(), TruOlsError> {
        let cutoffs = CutoffCalculator::calculate(
            self.mixing_matrix.as_ref(),
            unstained_control,
            percentile,
        )?;
        self.cutoffs = cutoffs.cutoffs().clone();
        Ok(())
    }

    /// Set the unmixing strategy.
    pub fn set_strategy(&mut self, strategy: UnmixingStrategy) {
        self.strategy = strategy;
    }

    /// Optional residual dual-gate: only drop dyes below cutoff when removing them
    /// increases residual norm by a relative factor ≤ `eps`.
    pub fn set_residual_epsilon(&mut self, eps: Option<f64>) {
        self.residual_epsilon = eps.filter(|e| e.is_finite() && *e >= 0.0);
    }

    /// The strategy actually in effect, including the constructor default.
    ///
    /// Provenance records this rather than the caller's `Option<UnmixingStrategy>`
    /// so a `None` is written as the strategy that ran, not omitted.
    pub fn strategy(&self) -> UnmixingStrategy {
        self.strategy
    }

    /// Returns **`(hits, misses)`** for the optional mask factorization cache (**`unmix-cache`** feature).
    #[cfg(feature = "unmix-cache")]
    pub fn unmix_factor_cache_hits_misses(&self) -> (u64, u64) {
        (
            self.factor_cache.cache().hits(),
            self.factor_cache.cache().misses(),
        )
    }

    /// Unmix a single event.
    ///
    /// # Arguments
    /// * `observation` - Detector outputs for a single event (length = n_detectors)
    ///
    /// # Returns
    /// * `relevant_abundances` - Abundances for endmembers that survived TRU-OLS
    /// * `relevant_indices` - Indices of relevant endmembers in the original mixing matrix
    /// * `irrelevant_abundances` - Abundances for removed endmembers (before removal)
    /// * `irrelevant_indices` - Indices of irrelevant endmembers
    #[allow(clippy::type_complexity)]
    pub fn unmix_event(
        &self,
        observation: ColRef<'_, f64>,
    ) -> Result<(Col<f64>, Vec<usize>, Vec<(usize, f64)>), TruOlsError> {
        let n_det = self.mixing_matrix.nrows();
        let n_em = self.mixing_matrix.ncols();
        let mut scratch = UnmixScratch::new(n_det, n_em);
        for i in 0..n_det {
            scratch.row_obs[i] = observation[i];
        }
        self.unmix_event_inner_prepped(&mut scratch)
            .map(|(ab, idx, irr, _inner_iters)| (ab, idx, irr))
    }

    /// Counts inner truncation iterations per event (same work as [`Self::unmix_event`]).
    pub fn summarize_truncation_iterations(
        &self,
        dataset: MatRef<'_, f64>,
    ) -> Result<TruncationStats, TruOlsError> {
        let n_events = dataset.nrows();
        let n_detectors = self.mixing_matrix.nrows();

        if dataset.ncols() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: dataset.ncols(),
            });
        }

        let counts: Vec<usize> = if crate::use_parallel_independent_events(n_events) {
            use rayon::prelude::*;
            let n_em = self.mixing_matrix.ncols();
            let rows: Result<Vec<_>, _> = (0..n_events)
                .into_par_iter()
                .map_init(
                    || UnmixScratch::new(n_detectors, n_em),
                    |scratch, ev| {
                        for i in 0..n_detectors {
                            scratch.row_obs[i] = dataset[(ev, i)];
                        }
                        self.unmix_event_inner_prepped(scratch)
                            .map(|(_, _, _, inner)| inner)
                    },
                )
                .collect();
            rows?
        } else {
            let n_em = self.mixing_matrix.ncols();
            let mut scratch = UnmixScratch::new(n_detectors, n_em);
            let mut out = Vec::with_capacity(n_events);
            for ev in 0..n_events {
                for i in 0..n_detectors {
                    scratch.row_obs[i] = dataset[(ev, i)];
                }
                let (_, _, _, inner) = self.unmix_event_inner_prepped(&mut scratch)?;
                out.push(inner);
            }
            out
        };

        let inner_iterations_min = counts.iter().copied().min().unwrap_or(0);
        let inner_iterations_max = counts.iter().copied().max().unwrap_or(0);
        let inner_iterations_mean =
            counts.iter().sum::<usize>() as f64 / counts.len().max(1) as f64;

        Ok(TruncationStats {
            n_events,
            inner_iterations_min,
            inner_iterations_max,
            inner_iterations_mean,
        })
    }

    #[allow(clippy::type_complexity)]
    /// [`UnmixScratch::row_obs`] must hold the raw detector row for this event.
    fn unmix_event_inner_prepped(
        &self,
        scratch: &mut UnmixScratch,
    ) -> Result<(Col<f64>, Vec<usize>, Vec<(usize, f64)>, usize), TruOlsError> {
        let n_detectors = self.mixing_matrix.nrows();
        let n_endmembers = self.mixing_matrix.ncols();

        for i in 0..n_detectors {
            scratch.adjusted_observation[i] = scratch.row_obs[i] - self.nonspecific_observation[i];
        }

        copy_mixing_into_scratch(
            scratch,
            self.mixing_matrix.as_ref(),
            n_detectors,
            n_endmembers,
        );

        let mut irrelevant_abundances: Vec<(usize, f64)> = Vec::new();
        let mut inner_iterations = 0usize;
        let mut active = n_endmembers;

        loop {
            inner_iterations += 1;
            #[cfg(feature = "unmix-cache")]
            {
                crate::unmix_cache::solve_with_mask_cache(
                    self.factor_cache.cache(),
                    self.mixing_matrix.as_ref(),
                    active,
                    scratch,
                )
                .map_err(|e| {
                    let endmember_indices_str = scratch.current_indices[..active]
                        .iter()
                        .map(|&idx| idx.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    TruOlsError::LinearAlgebra(format!(
                        "Failed to solve linear system: {e}\n  Matrix shape: {n_detectors}×{active} (detectors × endmembers)\n  Current endmember indices: [{endmember_indices_str}]\n  This usually indicates the mixing matrix is singular or numerically singular (linearly dependent columns).\n  Check for duplicate or highly similar spectral signatures in the mixing matrix.",
                    ))
                })?;
            }
            #[cfg(all(not(feature = "unmix-cache"), feature = "blas"))]
            {
                let m_view = MatRef::from_column_major_slice(
                    &scratch.working_m[..n_detectors * active],
                    n_detectors,
                    active,
                );
                let b_view = ColRef::from_slice(&scratch.adjusted_observation[..n_detectors]);
                solve_least_squares_blas_in_place(m_view, b_view, &mut scratch.x_out[..active]).map_err(
                    |e| {
                        let endmember_indices_str = scratch.current_indices[..active]
                            .iter()
                            .map(|&idx| idx.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        TruOlsError::LinearAlgebra(format!(
                            "Failed to solve linear system: {e}\n  Matrix shape: {n_detectors}×{active} (detectors × endmembers)\n  Current endmember indices: [{endmember_indices_str}]\n  This usually indicates the mixing matrix is singular or numerically singular (linearly dependent columns).\n  Check for duplicate or highly similar spectral signatures in the mixing matrix.",
                        ))
                    },
                )?;
            }
            #[cfg(all(not(feature = "unmix-cache"), not(feature = "blas")))]
            {
                let m_view = MatRef::from_column_major_slice(
                    &scratch.working_m[..n_detectors * active],
                    n_detectors,
                    active,
                );
                let b_view = ColRef::from_slice(&scratch.adjusted_observation[..n_detectors]);
                // Use QR directly here: the Gram/Cholesky fast path often fails on ill-conditioned
                // active submatrices, and attempting it first costs an extra Gram build before falling
                // back to QR (see `solve_least_squares_faer_in_place`).
                solve_least_squares_faer_in_place(
                    m_view,
                    b_view,
                    &mut scratch.b_rhs,
                    &mut scratch.x_out[..active],
                    &mut scratch.gram,
                    &mut scratch.rhs_gram,
                    false,
                )
                .map_err(|e| {
                    let endmember_indices_str = scratch.current_indices[..active]
                        .iter()
                        .map(|&idx| idx.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    TruOlsError::LinearAlgebra(format!(
                        "Failed to solve linear system: {}\n  Matrix shape: {}×{} (detectors × endmembers)\n  Current endmember indices: [{}]\n  This usually indicates the mixing matrix is singular or numerically singular (linearly dependent columns).\n  Check for duplicate or highly similar spectral signatures in the mixing matrix.",
                        e,
                        n_detectors,
                        active,
                        endmember_indices_str
                    ))
                })?;
            }

            scratch.to_remove.clear();
            let residual_eps = self.residual_epsilon;
            let full_residual_norm = residual_eps.map(|_| {
                residual_norm_sq(
                    &scratch.adjusted_observation[..n_detectors],
                    &scratch.working_m[..n_detectors * active],
                    n_detectors,
                    active,
                    &scratch.x_out[..active],
                )
            });
            for local_idx in 0..active {
                let global_idx = scratch.current_indices[local_idx];
                if Some(global_idx) == self.autofluorescence_idx {
                    continue;
                }
                if scratch.x_out[local_idx] < self.cutoffs[global_idx] {
                    if let (Some(eps), Some(full_norm)) = (residual_eps, full_residual_norm) {
                        let without_norm = residual_norm_sq_without_column(
                            &scratch.adjusted_observation[..n_detectors],
                            &scratch.working_m[..n_detectors * active],
                            n_detectors,
                            active,
                            &scratch.x_out[..active],
                            local_idx,
                        );
                        // Keep the dye if dropping it blows the residual past eps.
                        // Perfect fits have full_norm ≈ 0; any positive without_norm means
                        // the column carries signal and must not be truncated.
                        let full_r = full_norm.sqrt();
                        let without_r = without_norm.sqrt();
                        let keep = if full_r > 1e-12 {
                            (without_r / full_r) - 1.0 > eps
                        } else {
                            without_r > 1e-12
                        };
                        if keep {
                            continue;
                        }
                    }
                    scratch
                        .to_remove
                        .push((local_idx, global_idx, scratch.x_out[local_idx]));
                }
            }

            if scratch.to_remove.is_empty() {
                let abundances = Col::from_fn(active, |i| scratch.x_out[i]);
                return Ok((
                    abundances,
                    scratch.current_indices[..active].to_vec(),
                    irrelevant_abundances,
                    inner_iterations,
                ));
            }

            for &(_, global_idx, abundance) in &scratch.to_remove {
                irrelevant_abundances.push((global_idx, abundance));
            }

            scratch.to_remove.sort_by(|a, b| b.0.cmp(&a.0));
            for &(local_idx, _, _) in &scratch.to_remove {
                let last = active - 1;
                if local_idx < last {
                    swap_columns_cm(
                        &mut scratch.working_m,
                        n_detectors,
                        n_endmembers,
                        local_idx,
                        last,
                    );
                    swap_index_entries(&mut scratch.current_indices, local_idx, last);
                }
                active -= 1;
                if active == 0 {
                    return Err(TruOlsError::AllEndmembersRemoved { event_index: 0 });
                }
            }
        }
    }

    /// Unmix an entire dataset.
    ///
    /// # Arguments
    /// * `dataset` - Observations for all events (events × detectors)
    ///
    /// # Returns
    /// Full unmixed abundances matrix (events × endmembers) with irrelevant abundances
    /// set according to the configured strategy
    pub fn unmix(&self, dataset: MatRef<'_, f64>) -> Result<Mat<f64>, TruOlsError> {
        let n_events = dataset.nrows();
        let n_endmembers = self.mixing_matrix.ncols();
        let n_detectors = self.mixing_matrix.nrows();

        if dataset.ncols() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: dataset.ncols(),
            });
        }

        // Initialize result matrix with zeros
        let mut result = Mat::zeros(n_events, n_endmembers);

        if crate::use_parallel_unmix(n_events) {
            use rayon::prelude::*;

            // Note: SyncPtr direct scatter into `result` was A/B'd (see PROFILING.md)
            // and did not meet the ≥5% wall-time keep rule at 100k events; gather path kept.
            let results: Result<Vec<_>, _> = (0..n_events)
                .into_par_iter()
                .map_init(
                    || UnmixScratch::new(n_detectors, n_endmembers),
                    |scratch, event_idx| {
                        for i in 0..n_detectors {
                            scratch.row_obs[i] = dataset[(event_idx, i)];
                        }
                        self.unmix_event_inner_prepped(scratch).map(
                            |(relevant_abundances, relevant_indices, _, _)| {
                                (event_idx, relevant_abundances, relevant_indices)
                            },
                        )
                    },
                )
                .collect();

            for res in results? {
                let (event_idx, relevant_abundances, relevant_indices) = res;
                for (local_idx, &global_idx) in relevant_indices.iter().enumerate() {
                    result[(event_idx, global_idx)] = relevant_abundances[local_idx];
                }
            }
        } else {
            let mut scratch = UnmixScratch::new(n_detectors, n_endmembers);
            for event_idx in 0..n_events {
                for i in 0..n_detectors {
                    scratch.row_obs[i] = dataset[(event_idx, i)];
                }
                let (relevant_abundances, relevant_indices, _, _) =
                    self.unmix_event_inner_prepped(&mut scratch)?;

                for (local_idx, &global_idx) in relevant_indices.iter().enumerate() {
                    result[(event_idx, global_idx)] = relevant_abundances[local_idx];
                }
            }
        }

        // Handle irrelevant abundances according to strategy
        match self.strategy {
            UnmixingStrategy::Zero => {}
            UnmixingStrategy::UnstainedControlMapping => {
                self.apply_ucm_mapping(&mut result)?;
            }
        }

        Ok(result)
    }

    /// Unmix a dataset and report final-fit residuals and fluorophore truncation per event.
    ///
    /// The abundances and diagnostics are produced from the same event solves.
    pub fn unmix_with_diagnostics(
        &self,
        dataset: MatRef<'_, f64>,
    ) -> Result<TruOlsBatchDiagnostics, TruOlsError> {
        let n_events = dataset.nrows();
        let n_endmembers = self.mixing_matrix.ncols();
        let n_detectors = self.mixing_matrix.nrows();

        if dataset.ncols() != n_detectors {
            return Err(TruOlsError::DimensionMismatch {
                expected: n_detectors,
                actual: dataset.ncols(),
            });
        }

        let mut abundances = Mat::zeros(n_events, n_endmembers);
        let mut residual_norms = vec![0.0; n_events];
        let mut any_fluor_truncated = vec![false; n_events];
        #[cfg(test)]
        let mut final_active_indices = vec![Vec::new(); n_events];

        if crate::use_parallel_unmix(n_events) {
            use rayon::prelude::*;

            let results: Result<Vec<_>, _> = (0..n_events)
                .into_par_iter()
                .map_init(
                    || UnmixScratch::new(n_detectors, n_endmembers),
                    |scratch, event_idx| {
                        for detector_idx in 0..n_detectors {
                            scratch.row_obs[detector_idx] = dataset[(event_idx, detector_idx)];
                        }
                        self.unmix_event_inner_prepped(scratch).map(
                            |(relevant_abundances, relevant_indices, irrelevant, _)| {
                                let residual_norm = final_residual_norm(
                                    &scratch.adjusted_observation[..n_detectors],
                                    self.mixing_matrix.as_ref(),
                                    &relevant_indices,
                                    relevant_abundances.as_ref(),
                                );
                                (
                                    event_idx,
                                    relevant_abundances,
                                    relevant_indices,
                                    residual_norm,
                                    !irrelevant.is_empty(),
                                )
                            },
                        )
                    },
                )
                .collect();

            for (event_idx, relevant_abundances, relevant_indices, residual_norm, truncated) in
                results?
            {
                for (local_idx, &global_idx) in relevant_indices.iter().enumerate() {
                    abundances[(event_idx, global_idx)] = relevant_abundances[local_idx];
                }
                residual_norms[event_idx] = residual_norm;
                any_fluor_truncated[event_idx] = truncated;
                #[cfg(test)]
                let () = {
                    final_active_indices[event_idx] = relevant_indices;
                };
            }
        } else {
            let mut scratch = UnmixScratch::new(n_detectors, n_endmembers);
            for event_idx in 0..n_events {
                for detector_idx in 0..n_detectors {
                    scratch.row_obs[detector_idx] = dataset[(event_idx, detector_idx)];
                }
                let (relevant_abundances, relevant_indices, irrelevant, _) =
                    self.unmix_event_inner_prepped(&mut scratch)?;

                residual_norms[event_idx] = final_residual_norm(
                    &scratch.adjusted_observation[..n_detectors],
                    self.mixing_matrix.as_ref(),
                    &relevant_indices,
                    relevant_abundances.as_ref(),
                );
                any_fluor_truncated[event_idx] = !irrelevant.is_empty();
                for (local_idx, &global_idx) in relevant_indices.iter().enumerate() {
                    abundances[(event_idx, global_idx)] = relevant_abundances[local_idx];
                }
                #[cfg(test)]
                let () = {
                    final_active_indices[event_idx] = relevant_indices;
                };
            }
        }

        match self.strategy {
            UnmixingStrategy::Zero => {}
            UnmixingStrategy::UnstainedControlMapping => {
                self.apply_ucm_mapping(&mut abundances)?;
            }
        }

        Ok(TruOlsBatchDiagnostics {
            abundances,
            residual_norms,
            any_fluor_truncated,
            #[cfg(test)]
            final_active_indices,
        })
    }

    /// Apply Unstained Control Mapping (UCM) to irrelevant/zero abundances.
    fn apply_ucm_mapping(&self, result: &mut Mat<f64>) -> Result<(), TruOlsError> {
        let n_events = result.nrows();
        let n_endmembers = result.ncols();
        let n_unstained_events = self.unstained_control.nrows();

        if n_unstained_events == 0 {
            return Err(TruOlsError::InsufficientData(
                "No unstained control events available for UCM mapping".to_string(),
            ));
        }

        let mut rng = rand::rng();

        for event_idx in 0..n_events {
            for endmember_idx in 0..n_endmembers {
                if Some(endmember_idx) == self.autofluorescence_idx {
                    continue;
                }

                if result[(event_idx, endmember_idx)].abs() < 1e-10 {
                    let random_unstained_idx = rng.random_range(0..n_unstained_events);
                    let unstained_observation = Col::from_fn(self.mixing_matrix.nrows(), |i| {
                        self.unstained_control[(random_unstained_idx, i)]
                    });

                    let adjusted_observation = Col::from_fn(self.mixing_matrix.nrows(), |i| {
                        unstained_observation[i] - self.nonspecific_observation[i]
                    });

                    let norm_squared: f64 = (0..self.mixing_matrix.nrows())
                        .map(|i| {
                            let v = self.mixing_matrix[(i, endmember_idx)];
                            v * v
                        })
                        .sum();

                    if norm_squared > 0.0 {
                        let projection: f64 = (0..self.mixing_matrix.nrows())
                            .map(|i| {
                                self.mixing_matrix[(i, endmember_idx)] * adjusted_observation[i]
                            })
                            .sum();
                        let abundance = projection / norm_squared;
                        result[(event_idx, endmember_idx)] = abundance;
                    }
                }
            }
        }

        Ok(())
    }
}

fn final_residual_norm(
    observation: &[f64],
    mixing: MatRef<'_, f64>,
    relevant_indices: &[usize],
    relevant_abundances: ColRef<'_, f64>,
) -> f64 {
    let residual_sum_squares = observation
        .iter()
        .enumerate()
        .map(|(detector_idx, &observed)| {
            let reconstructed = relevant_indices
                .iter()
                .enumerate()
                .map(|(local_idx, &global_idx)| {
                    mixing[(detector_idx, global_idx)] * relevant_abundances[local_idx]
                })
                .sum::<f64>();
            let residual = observed - reconstructed;
            residual * residual
        })
        .sum::<f64>();
    residual_sum_squares.sqrt()
}

fn residual_norm_sq(
    observation: &[f64],
    working_m: &[f64],
    n_detectors: usize,
    active: usize,
    abundances: &[f64],
) -> f64 {
    let mut sum = 0.0_f64;
    for r in 0..n_detectors {
        let mut pred = 0.0_f64;
        for c in 0..active {
            pred += working_m[c * n_detectors + r] * abundances[c];
        }
        let d = observation[r] - pred;
        sum += d * d;
    }
    sum
}

fn residual_norm_sq_without_column(
    observation: &[f64],
    working_m: &[f64],
    n_detectors: usize,
    active: usize,
    abundances: &[f64],
    drop_local: usize,
) -> f64 {
    let mut sum = 0.0_f64;
    for r in 0..n_detectors {
        let mut pred = 0.0_f64;
        for c in 0..active {
            if c == drop_local {
                continue;
            }
            pred += working_m[c * n_detectors + r] * abundances[c];
        }
        let d = observation[r] - pred;
        sum += d * d;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocessing::{CutoffCalculator, NonspecificObservation};
    use faer::mat;

    #[cfg(feature = "unmix-cache")]
    #[test]
    fn from_preprocessed_with_factor_cache_matches_from_preprocessed_unmix() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let unstained = mat![[0.0, 0.0], [0.1, 0.1]];
        let c =
            CutoffCalculator::calculate(mixing_matrix.as_ref(), unstained.as_ref(), 0.995).unwrap();
        let n = NonspecificObservation::calculate(mixing_matrix.as_ref(), unstained.as_ref(), 0)
            .unwrap();
        let cache = super::shared_mask_factor_cache_with_capacity(64);
        let t1 = TruOls::from_preprocessed_with_factor_cache(
            mixing_matrix.clone(),
            unstained.clone(),
            c.cutoffs().clone(),
            n.observation().clone(),
            0,
            cache,
        )
        .unwrap();
        let t2 = TruOls::from_preprocessed(
            mixing_matrix,
            unstained,
            c.cutoffs().clone(),
            n.observation().clone(),
            0,
        )
        .unwrap();
        let dataset = mat![[1.0, 1.0]];
        let u1 = t1.unmix(dataset.as_ref()).unwrap();
        let u2 = t2.unmix(dataset.as_ref()).unwrap();
        assert!((u1[(0, 0)] - u2[(0, 0)]).abs() < 1e-12);
        assert!((u1[(0, 1)] - u2[(0, 1)]).abs() < 1e-12);
    }

    #[test]
    fn from_preprocessed_matches_new_unmix() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let unstained = mat![[0.0, 0.0], [0.1, 0.1]];
        let c =
            CutoffCalculator::calculate(mixing_matrix.as_ref(), unstained.as_ref(), 0.995).unwrap();
        let n = NonspecificObservation::calculate(mixing_matrix.as_ref(), unstained.as_ref(), 0)
            .unwrap();
        let t1 = TruOls::from_preprocessed(
            mixing_matrix.clone(),
            unstained.clone(),
            c.cutoffs().clone(),
            n.observation().clone(),
            0,
        )
        .unwrap();
        let t2 = TruOls::new(mixing_matrix, unstained, 0).unwrap();
        let dataset = mat![[1.0, 1.0]];
        let u1 = t1.unmix(dataset.as_ref()).unwrap();
        let u2 = t2.unmix(dataset.as_ref()).unwrap();
        assert!((u1[(0, 0)] - u2[(0, 0)]).abs() < 1e-12);
        assert!((u1[(0, 1)] - u2[(0, 1)]).abs() < 1e-12);
    }

    /// The AF column is exempt from truncation and from UCM. A fluorophore-only panel has
    /// no such column, so an endmember that falls under its cutoff must actually be
    /// removed — which is exactly what a synthesized placeholder AF column would prevent.
    #[test]
    fn a_fluor_only_panel_truncates_every_endmember() {
        let mixing_matrix = mat![[1.0, 0.0], [0.0, 1.0]];
        // Endmember 0 sits at 10 in the control, endmember 1 at 0, so the cutoffs are
        // (10, 0) and the event below is under the first one.
        let unstained = mat![[10.0, 0.0], [10.0, 0.0], [10.0, 0.0]];
        let dataset = mat![[5.0, 100.0]];

        let mut with_af = TruOls::new(mixing_matrix.clone(), unstained.clone(), 0).unwrap();
        with_af.set_strategy(UnmixingStrategy::Zero);
        assert_eq!(with_af.autofluorescence_index(), Some(0));
        let protected = with_af.unmix(dataset.as_ref()).unwrap();
        assert!(
            (protected[(0, 0)] - 5.0).abs() < 1e-9,
            "the AF endmember is never truncated: {}",
            protected[(0, 0)]
        );

        let mut fluor_only = TruOls::new_fluor_only(mixing_matrix, unstained).unwrap();
        fluor_only.set_strategy(UnmixingStrategy::Zero);
        assert_eq!(fluor_only.autofluorescence_index(), None);
        let truncated = fluor_only.unmix(dataset.as_ref()).unwrap();
        assert!(
            truncated[(0, 0)].abs() < 1e-9,
            "endmember 0 is under its cutoff and has no AF exemption: {}",
            truncated[(0, 0)]
        );
        assert!((truncated[(0, 1)] - 100.0).abs() < 1e-9);
    }

    #[test]
    fn unmix_with_diagnostics_matches_unmix_and_reports_truncation() {
        let mixing = mat![[1.0, 0.0], [0.0, 1.0]];
        let unstained = mat![[10.0, 0.0], [10.0, 0.0], [10.0, 0.0]];
        let mut tru = TruOls::new_fluor_only(mixing, unstained).unwrap();
        tru.set_strategy(UnmixingStrategy::Zero);
        let events = mat![[5.0, 100.0], [20.0, 100.0]];

        let ordinary = tru.unmix(events.as_ref()).unwrap();
        let diagnosed = tru.unmix_with_diagnostics(events.as_ref()).unwrap();

        assert_eq!(diagnosed.abundances, ordinary);
        assert_eq!(diagnosed.any_fluor_truncated, vec![true, false]);
        assert_eq!(diagnosed.residual_norms.len(), 2);
        assert!(diagnosed.residual_norms.iter().all(|v| v.is_finite()));
        assert_eq!(diagnosed.final_active_indices, vec![vec![1], vec![0, 1]]);
    }

    /// The AF column is exempt from truncation. Path B TRU appends a catalog signature at
    /// `autofluorescence_idx = n_fluor` for the same reason — that index must survive even
    /// when its abundance sits under the AF cutoff.
    #[test]
    fn protected_af_endmember_stays_in_the_active_set() {
        let mixing_matrix = mat![[1.0, 0.0, 0.1], [0.0, 1.0, 0.1], [0.0, 0.0, 1.0]];
        // Fluors bright in control → high cutoffs; AF column near zero → low AF cutoff.
        let unstained = mat![[20.0, 20.0, 0.0], [20.0, 20.0, 0.0], [20.0, 20.0, 0.0]];
        let mut tru = TruOls::new(mixing_matrix, unstained, 2).unwrap();
        tru.set_strategy(UnmixingStrategy::Zero);
        assert_eq!(tru.autofluorescence_index(), Some(2));

        let observation = faer::col![5.0, 5.0, 50.0];
        let (relevant, relevant_indices, _) = tru.unmix_event(observation.as_ref()).unwrap();
        assert!(
            relevant_indices.contains(&2),
            "AF index must remain active: {relevant_indices:?}"
        );
        let af_local = relevant_indices.iter().position(|&i| i == 2).unwrap();
        assert!(
            relevant[af_local] > 1.0,
            "protected AF abundance should not be zeroed: {}",
            relevant[af_local]
        );
    }

    #[test]
    fn test_unmix_event() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let unstained = mat![[0.0, 0.0], [0.1, 0.1]];

        let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();
        let observation = faer::col![1.0, 1.0];

        let (relevant, relevant_indices, _irrelevant) =
            tru_ols.unmix_event(observation.as_ref()).unwrap();
        assert!(!relevant_indices.is_empty());
        assert_eq!(relevant.nrows(), 2);
    }

    #[test]
    fn summarize_truncation_iterations_matches_event_count() {
        let mixing_matrix = mat![[1.0, 0.1], [0.1, 1.0]];
        let unstained = mat![[0.0, 0.0], [0.1, 0.1]];
        let tru_ols = TruOls::new(mixing_matrix, unstained, 0).unwrap();
        let dataset = mat![[1.0, 1.0], [0.5, 0.5], [2.0, 2.0]];
        let stats = tru_ols
            .summarize_truncation_iterations(dataset.as_ref())
            .unwrap();
        assert_eq!(stats.n_events, 3);
        assert!(stats.inner_iterations_max >= stats.inner_iterations_min);
        assert!(stats.inner_iterations_min >= 1);
        assert!(
            (stats.inner_iterations_mean - stats.inner_iterations_min as f64).abs()
                <= (stats.inner_iterations_max - stats.inner_iterations_min) as f64
        );
    }

    /// Dim true dye under a high fence is kept when dropping it blows residual;
    /// a noise-only dye under the same fence still drops.
    #[test]
    fn residual_dual_gate_keeps_dim_true_dye_and_drops_noise() {
        let mixing = mat![[1.0, 0.0], [0.0, 1.0]];
        let ust = mat![
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
            [0.0, 0.0],
        ];
        // Protect column 1 so only dye0 is truncatable.
        let cut = faer::Col::from_fn(2, |_| 100.0);
        let ns = NonspecificObservation::calculate(mixing.as_ref(), ust.as_ref(), 1).unwrap();
        let mut tru = TruOls::from_preprocessed(
            mixing.clone(),
            ust.clone(),
            cut,
            ns.observation().clone(),
            1,
        )
        .unwrap();
        tru.set_strategy(UnmixingStrategy::Zero);

        let bright = mat![[40.0, 0.0]];
        let dropped = tru.unmix(bright.as_ref()).unwrap();
        assert!(
            dropped[(0, 0)].abs() < 1e-6,
            "without residual gate dim true dye0 truncates, got {}",
            dropped[(0, 0)]
        );

        tru.set_residual_epsilon(Some(0.05));
        let kept = tru.unmix(bright.as_ref()).unwrap();
        assert!(
            kept[(0, 0)] > 20.0,
            "residual gate should keep dim true dye0, got {}",
            kept[(0, 0)]
        );

        // Near-zero dye0 under the fence: dropping it does not change residual → still drop.
        let noise = mat![[0.0, 50.0]];
        let noise_out = tru.unmix(noise.as_ref()).unwrap();
        assert!(
            noise_out[(0, 0)].abs() < 1e-6,
            "near-zero dye0 should still truncate under residual gate, got {}",
            noise_out[(0, 0)]
        );
    }
}
