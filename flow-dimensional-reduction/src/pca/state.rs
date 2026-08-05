//! Typestate markers for [`super::Pca`].
//!
//! `Unfitted` and `Fitted` are declared here rather than at `pca` top level to
//! avoid stuttering inside `Pca<Unfitted>` / `Pca<Fitted>` and to keep them
//! out of the crate root, where nothing outside this crate needs to name
//! them — callers rely on type inference from `Pca::new(k).fit(..)`.

use super::PcaComponent;

/// Unfitted state: holds only the requested component count.
#[derive(Debug, Clone, Copy)]
pub struct Unfitted {
    pub(super) n_components: usize,
}

impl super::sealed::Sealed for Unfitted {}

impl PcaComponent for Unfitted {
    fn n_components(&self) -> usize {
        self.n_components
    }
}

/// Fitted state: holds the basis produced by [`super::Pca::fit`].
#[derive(Debug, Clone)]
pub struct Fitted {
    pub(super) n_components: usize,
    /// `k * d` row-major: axis `i` occupies `[i*d .. (i+1)*d]`.
    pub(super) components: Vec<f32>,
    pub(super) explained_variance_ratio: Vec<f32>,
    pub(super) mean: Vec<f32>,
}

impl super::sealed::Sealed for Fitted {}

impl PcaComponent for Fitted {
    fn n_components(&self) -> usize {
        self.n_components
    }
}
