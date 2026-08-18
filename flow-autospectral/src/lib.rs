//! # flow-autospectral
//!
//! Multi-autofluorescence spectral library discovery and per-event matching for
//! spectral flow cytometry. Phase 1 focuses on AF signatures from unstained
//! controls; later milestones add fluorophore variants and IRLS.
//!
//! Compatible **with or without** TRU-OLS: use standalone OLS helpers here, or
//! enable the `tru-ols` feature to build a [`flow_tru_ols::MixingMatrix`] with a
//! selected AF column.

mod config;
mod discover;
mod error;
mod library;
mod match_af;
mod unmix_ols;

#[cfg(feature = "tru-ols")]
mod tru_ols;

pub use config::{
    force_sequential, DiscoverConfig, DiscoveryBackend, MatchConfig, MatchStrategy,
};
pub use discover::discover_af_library;
pub use error::{AutospectralError, Result};
pub use library::{
    cosine_similarity, merge_near_duplicates, normalize_unit_peak, AfLibrary, AfLibraryBuilder,
    GmmAfLibraryBuilder, KMeansAfLibraryBuilder,
};
pub use match_af::{
    group_events_by_af, match_events, mixing_matrices_by_af, AfMatchResult,
};
pub use unmix_ols::{ols_residual, swap_af_column, unmix_event_ols, unmix_events_ols};

#[cfg(feature = "tru-ols")]
pub use tru_ols::mixing_matrix_with_selected_af;
