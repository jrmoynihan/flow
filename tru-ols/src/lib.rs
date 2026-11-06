//! TRU-OLS (Truncated ReUnmixing Ordinary Least Squares) algorithm for flow cytometry unmixing.
//!
//! This crate implements the TRU-OLS algorithm, which reduces the variance of unmixed
//! abundance distributions by removing irrelevant endmembers (dyes) from the mixing matrix
//! on a per-event basis.
//!
//! # Overview
//!
//! TRU-OLS is a variant of stepwise regression that uses unstained control data to determine
//! which endmembers are relevant for each event. By unmixing each event with only its relevant
//! endmembers, the algorithm reduces variance and improves separation between populations.
//!
//! # Basic Usage
//!
//! ```no_run
//! use flow_tru_ols::{TruOls, UnmixingStrategy};
//! use faer::mat;
//!
//! // Mixing matrix (detectors × endmembers), unstained control (events × detectors)
//! let mixing_matrix = mat![
//!     [0.9, 0.1],
//!     [0.1, 0.9],
//!     [0.05, 0.05],
//! ];
//! let unstained_control = mat![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
//! let dataset = mat![[100.0, 50.0, 10.0], [200.0, 150.0, 20.0]];
//!
//! // Create a TRU-OLS instance (autofluorescence is endmember index 1).
//! // Default strategy is UnstainedControlMapping (UCM); Zero is opt-in.
//! let mut tru_ols = TruOls::new(mixing_matrix, unstained_control.clone(), 1)?;
//!
//! // Configure the algorithm
//! tru_ols.set_cutoff_percentile(0.995, unstained_control.as_ref())?;
//!
//! // Unmix a dataset
//! let unmixed = tru_ols.unmix(dataset.as_ref())?;
//! # Ok::<(), flow_tru_ols::TruOlsError>(())
//! ```

/// Independent per-event work (OLS, preprocessing, metrics) uses Rayon when event count exceeds this.
pub const PARALLEL_INDEPENDENT_EVENTS_THRESHOLD: usize = 256;
/// `TruOls::unmix` enables Rayon only above this (avoids pool overhead on smaller datasets).
pub const PARALLEL_UNMIX_THRESHOLD: usize = 10_000;

/// Maximum endmember count in the default build (`u128` active-set masks). Panels with more endmembers
/// require the **`large-panels`** Cargo feature on `flow-tru-ols`.
pub const MAX_ENDMEMBERS_DEFAULT: usize = 128;

/// When set to `1`, disables Rayon for independent-event loops (A/B vs parallel builds).
pub(crate) fn force_sequential_independent_events() -> bool {
    std::env::var("FLOW_TRU_OLS_FORCE_SEQUENTIAL")
        .ok()
        .as_deref()
        == Some("1")
}

pub(crate) fn use_parallel_independent_events(n_events: usize) -> bool {
    !force_sequential_independent_events() && n_events > PARALLEL_INDEPENDENT_EVENTS_THRESHOLD
}

pub(crate) fn use_parallel_unmix(n_events: usize) -> bool {
    !force_sequential_independent_events() && n_events > PARALLEL_UNMIX_THRESHOLD
}

/// Enforces [`MAX_ENDMEMBERS_DEFAULT`] unless the **`large-panels`** feature is enabled.
pub(crate) fn ensure_endmember_limit(n_endmembers: usize) -> Result<(), TruOlsError> {
    #[cfg(not(feature = "large-panels"))]
    {
        if n_endmembers > MAX_ENDMEMBERS_DEFAULT {
            return Err(TruOlsError::EndmemberCountExceedsDefaultLimit {
                max: MAX_ENDMEMBERS_DEFAULT,
                actual: n_endmembers,
            });
        }
    }
    Ok(())
}

pub mod af_match;
pub mod af_validation;
pub mod batched_ols;
pub mod benchmark;
pub mod error;
pub mod fence;
pub mod fence_comparison;
pub mod metrics;
pub mod mixing_matrix;
pub mod pipeline;
pub mod preprocessing;
pub mod spreading;
pub(crate) mod unmix_buffer;
#[cfg(feature = "unmix-cache")]
pub(crate) mod unmix_cache;
pub mod unmixing;

#[cfg(feature = "cubecl")]
pub mod gpu;

#[cfg(feature = "flow-fcs")]
pub mod fcs_integration;

#[cfg(feature = "flow-fcs")]
pub mod provenance;

#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub mod plotting;

pub use af_match::{
    assess_af_purity, build_af_match_library, inspect_match_event, match_and_subtract,
    AfCatalogEntry, AfEventInspect, AfMatchLibrary, AfMatchOptions, AfMatchResult, AfPurityCulprit,
    AfPurityOptions, AfPurityReport, AfPurityVerdict, DroppedAfEntry, PreparedAfLibrary,
};
pub use af_validation::{
    metrics_for_fluor_index, run_path_a, run_path_b,
    EventLabels, PathSolveResult, SolveMetrics, SyntheticPanel, ValidationPanelId,
};
pub use batched_ols::run_ols_normal_equations;
pub use error::TruOlsError;
pub use fence::{
    asinh_histogram, cutoffs_from_abundance_columns, estimate_cutoffs,
    estimate_cutoffs_with_fallback, mask_for_signature, unstained_abundance_columns, FenceKind,
    FenceRequest, MIN_FENCE_EVENTS,
};
pub use fence_comparison::{
    compare_fence_outputs, FenceComparisonInput, FenceComparisonMetrics,
    FenceComparisonThresholds, FenceFluorComparison, FenceRankingStatus,
};
pub use mixing_matrix::{MixingMatrix, MixingMatrixBuilder};
pub use pipeline::{
    matrix_from_row_major_flat, resolve_or_append_af_endmember, unmixed_output_path,
};
pub use preprocessing::{CutoffCalculator, NonspecificObservation};
#[cfg(feature = "unmix-cache")]
pub use unmixing::{shared_mask_factor_cache_with_capacity, SharedMaskFactorCache};
pub use unmixing::{TruOls, TruncationStats, UnmixingStrategy};

#[cfg(feature = "cubecl")]
pub use gpu::{run_ols_normal_equations_gpu_rhs, try_shared_gpu_context, GpuWgpuContext};

pub use benchmark::{comparison_report_markdown, run_comparison, run_ols, BenchmarkConfig};
pub use metrics::{
    compute_fit_metrics, compute_ssm, compute_use, dimensionality_metrics, spread_metrics,
    ComparisonReport, DimensionalityMetrics, FitMetrics, SpilloverSpreadingMatrix, SpreadMetrics,
    UnmixingSpreadingError,
};
pub use spreading::{
    asinh_transform, compute_empirical_use_asinh, empirical_use_ratio, per_fluor_rsd_masked,
    percentile_low, quantile, robust_rsd_asinh, EmpiricalUseComputed, DEFAULT_ASINH_COFACTOR,
    MIN_EMPIRICAL_USE_EVENTS, MIN_SPREAD_EVENTS,
};

#[cfg(all(feature = "flow-fcs", feature = "unmix-cache"))]
pub use fcs_integration::apply_tru_ols_unmixing_from_preprocessed_with_shared_factor_cache;
#[cfg(feature = "flow-fcs")]
pub use fcs_integration::{
    append_linear_parameter_column, apply_tru_ols_unmixing_from_preprocessed,
    extract_detector_data, TruOlsUnmixing, AF_ASSIGNMENT_KEYWORD, AF_CONFIDENCE_CHANNEL_NAME,
    AF_CONFIDENCE_KEYWORD, AF_SIGNATURE_CHANNEL_NAME, AF_SOURCE_GUID_KEYWORD, AF_SOURCE_KEYWORD,
    DEFAULT_AF_CHANNEL_NAME, UNMIXED_KEYWORD, UNMIXED_METHOD_FALSE, UNMIXED_METHOD_OLS,
    UNMIXED_METHOD_OLS_MULTI_AF, UNMIXED_METHOD_TRU_OLS, UNMIXED_METHOD_TRU_OLS_MULTI_AF,
};
#[cfg(feature = "flow-fcs")]
pub use pipeline::{
    export_unmixed_fcs, export_unmixed_fcs_catalog_af, set_raw_datasource_guid,
    tru_ols_grouped_by_catalog_af, CatalogAfExportRequest, CatalogAfTruOptions, CatalogAfTruResult,
    FitMetricsSummary, SignatureFenceDiag, UnmixExportRequest, UnmixExportResult,
};

#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub use plotting::{plot_abundance_distribution, plot_ucm_comparison, plot_unmixed_comparison};
