//! Multi-autofluorescence spectral library discovery, per-event AF matching,
//! and joint per-cell fluorophore-variant unmixing for spectral flow cytometry.
//!
//! Compatible **with or without** TRU-OLS: use standalone OLS helpers here, or
//! enable the `tru-ols` feature to assemble a mixing matrix with a selected AF
//! column and construct `TruOls` from precomputed cutoffs (`from_preprocessed`).
//!
//! The joint unmix path ports AutoSpectral v1.6 `pipeline = "joint"`
//! (Burton *et al.*, *bioRxiv* 2025.10.27.684855). QC-core
//! timings vs AutoSpectralRcpp: crate README and `docs/comparison-with-r.md`.

mod clean;
mod config;
mod discover;
mod error;
mod joint;
mod library;
mod match_af;
mod unmix_ols;
mod variants;

#[cfg(feature = "tru-ols")]
mod tru_ols;

pub use clean::{CleanedEvents, ScatterInput, clean_unstained};
pub use config::{
    CleanConfig, DiscoverConfig, DiscoveryBackend, JointUnmixConfig, JointUnmixPrecision,
    MatchConfig, MatchStrategy,
    OlsUnmixConfig, PcaCleanConfig, ScatterCleanConfig, SomDiscoverConfig, VariantDiscoverConfig,
    force_sequential,
};
pub use discover::{discover_af_library, discover_af_library_cleaned};
pub use error::{AutospectralError, Result};
pub use joint::{JointUnmixResult, unmix_autospectral_joint};
pub use library::{
    AfLibrary, AfLibraryBuilder, FlowSomAfLibraryBuilder, GmmAfLibraryBuilder,
    HnswMedoidAfLibraryBuilder, KMeansAfLibraryBuilder, cosine_similarity, merge_near_duplicates,
    normalize_unit_peak,
};
pub use match_af::{AfMatchResult, group_events_by_af, match_events, mixing_matrices_by_af};
pub use unmix_ols::{
    ols_residual, ols_residual_with_matrix, swap_af_column, unmix_event_ols, unmix_events_ols,
    unmix_events_ols_with,
};
pub use variants::{FluorControl, SpectralVariants, discover_spectral_variants};

#[cfg(feature = "tru-ols")]
pub use tru_ols::{
    MixingMatrixAfOptions, SelectedAfTruOls, events_row_major_to_mat,
    mixing_matrix_with_selected_af, tru_ols_from_selected_af,
};
