//! # flow-autospectral
//!
//! Multi-autofluorescence spectral library discovery and per-event matching for
//! spectral flow cytometry. Phase 1 focuses on AF signatures from unstained
//! controls; later milestones add fluorophore variants and IRLS.
//!
//! Compatible **with or without** TRU-OLS: use standalone OLS helpers here, or
//! enable the `tru-ols` feature to assemble a mixing matrix with a selected AF
//! column and construct `TruOls` from precomputed cutoffs (`from_preprocessed`).

mod clean;
mod config;
mod discover;
mod error;
mod library;
mod match_af;
mod unmix_ols;

#[cfg(feature = "tru-ols")]
mod tru_ols;

pub use clean::{CleanedEvents, ScatterInput, clean_unstained};
pub use config::{
    CleanConfig, DiscoverConfig, DiscoveryBackend, MatchConfig, MatchStrategy, PcaCleanConfig,
    ScatterCleanConfig, SomDiscoverConfig, force_sequential,
};
pub use discover::{discover_af_library, discover_af_library_cleaned};
pub use error::{AutospectralError, Result};
pub use library::{
    AfLibrary, AfLibraryBuilder, FlowSomAfLibraryBuilder, GmmAfLibraryBuilder,
    HnswMedoidAfLibraryBuilder, KMeansAfLibraryBuilder, cosine_similarity, merge_near_duplicates,
    normalize_unit_peak,
};
pub use match_af::{AfMatchResult, group_events_by_af, match_events, mixing_matrices_by_af};
pub use unmix_ols::{ols_residual, swap_af_column, unmix_event_ols, unmix_events_ols};

#[cfg(feature = "tru-ols")]
pub use tru_ols::{
    MixingMatrixAfOptions, SelectedAfTruOls, events_row_major_to_mat,
    mixing_matrix_with_selected_af, tru_ols_from_selected_af,
};
