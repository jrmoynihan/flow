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

pub mod batched_ols;
pub mod benchmark;
pub mod error;
pub mod metrics;
pub mod preprocessing;
pub(crate) mod unmix_buffer;
#[cfg(feature = "unmix-cache")]
pub(crate) mod unmix_cache;
pub mod mixing_matrix;
pub mod pipeline;
pub mod unmixing;

#[cfg(feature = "cubecl")]
pub mod gpu;

#[cfg(feature = "flow-fcs")]
pub mod fcs_integration;

#[cfg(feature = "flow-fcs")]
pub mod provenance;

#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub mod plotting;

pub use batched_ols::run_ols_normal_equations;
pub use error::TruOlsError;
pub use preprocessing::{CutoffCalculator, NonspecificObservation};
#[cfg(feature = "unmix-cache")]
pub use unmixing::{SharedMaskFactorCache, shared_mask_factor_cache_with_capacity};
pub use mixing_matrix::{MixingMatrix, MixingMatrixBuilder};
pub use pipeline::{
    matrix_from_row_major_flat, resolve_or_append_af_endmember, unmixed_output_path,
};
pub use unmixing::{TruOls, TruncationStats, UnmixingStrategy};

#[cfg(feature = "cubecl")]
pub use gpu::{GpuWgpuContext, run_ols_normal_equations_gpu_rhs, try_shared_gpu_context};

pub use benchmark::{BenchmarkConfig, comparison_report_markdown, run_comparison, run_ols};
pub use metrics::{
    ComparisonReport, DimensionalityMetrics, FitMetrics, SpilloverSpreadingMatrix, SpreadMetrics,
    UnmixingSpreadingError, compute_fit_metrics, compute_ssm, compute_use, dimensionality_metrics,
    spread_metrics,
};

#[cfg(all(feature = "flow-fcs", feature = "unmix-cache"))]
pub use fcs_integration::apply_tru_ols_unmixing_from_preprocessed_with_shared_factor_cache;
#[cfg(feature = "flow-fcs")]
pub use fcs_integration::{
    DEFAULT_AF_CHANNEL_NAME, TruOlsUnmixing, UNMIXED_KEYWORD, UNMIXED_METHOD_FALSE,
    UNMIXED_METHOD_OLS, UNMIXED_METHOD_TRU_OLS, apply_tru_ols_unmixing_from_preprocessed,
    extract_detector_data,
};
#[cfg(feature = "flow-fcs")]
pub use pipeline::{
    FitMetricsSummary, UnmixExportRequest, UnmixExportResult, export_unmixed_fcs,
    set_raw_datasource_guid,
};

#[cfg(all(feature = "flow-fcs", feature = "plotting"))]
pub use plotting::{plot_abundance_distribution, plot_ucm_comparison, plot_unmixed_comparison};
