//! AF spectral library types and builders.

use crate::config::{DiscoverConfig, DiscoveryBackend};
use crate::discover;
use crate::error::{AutospectralError, Result};
use faer::Mat;

/// Detectors × K autofluorescence reference spectra plus provenance.
#[derive(Debug, Clone)]
pub struct AfLibrary {
    /// Column-major faer matrix: detectors × signatures.
    pub signatures: Mat<f64>,
    pub names: Vec<String>,
    pub detector_names: Vec<String>,
    pub provenance: String,
}

impl AfLibrary {
    pub fn n_detectors(&self) -> usize {
        self.signatures.nrows()
    }

    pub fn n_signatures(&self) -> usize {
        self.signatures.ncols()
    }

    pub fn column_slice(&self, index: usize) -> Result<Vec<f64>> {
        if index >= self.n_signatures() {
            return Err(AutospectralError::AfIndexOutOfRange {
                index,
                n: self.n_signatures(),
            });
        }
        let mut col = Vec::with_capacity(self.n_detectors());
        for i in 0..self.n_detectors() {
            col.push(self.signatures[(i, index)]);
        }
        Ok(col)
    }

    /// Row-major `n_signatures × n_detectors` f32 buffer for ANN indexing.
    pub fn signatures_row_major_f32(&self) -> Vec<f32> {
        let n = self.n_signatures();
        let d = self.n_detectors();
        let mut out = Vec::with_capacity(n * d);
        for j in 0..n {
            for i in 0..d {
                out.push(self.signatures[(i, j)] as f32);
            }
        }
        out
    }
}

/// Pluggable AF library construction (GMM today; FlowSOM later).
pub trait AfLibraryBuilder {
    fn build(
        &self,
        events_row_major: &[f64],
        n_events: usize,
        n_detectors: usize,
        detector_names: &[String],
        config: &DiscoverConfig,
    ) -> Result<AfLibrary>;
}

#[derive(Debug, Default)]
pub struct GmmAfLibraryBuilder;

impl AfLibraryBuilder for GmmAfLibraryBuilder {
    fn build(
        &self,
        events_row_major: &[f64],
        n_events: usize,
        n_detectors: usize,
        detector_names: &[String],
        config: &DiscoverConfig,
    ) -> Result<AfLibrary> {
        let mut cfg = config.clone();
        cfg.backend = DiscoveryBackend::Gmm;
        discover::discover_af_library(
            events_row_major,
            n_events,
            n_detectors,
            detector_names,
            &cfg,
        )
    }
}

#[derive(Debug, Default)]
pub struct KMeansAfLibraryBuilder;

impl AfLibraryBuilder for KMeansAfLibraryBuilder {
    fn build(
        &self,
        events_row_major: &[f64],
        n_events: usize,
        n_detectors: usize,
        detector_names: &[String],
        config: &DiscoverConfig,
    ) -> Result<AfLibrary> {
        let mut cfg = config.clone();
        cfg.backend = DiscoveryBackend::KMeans;
        discover::discover_af_library(
            events_row_major,
            n_events,
            n_detectors,
            detector_names,
            &cfg,
        )
    }
}

#[derive(Debug, Default)]
pub struct HnswMedoidAfLibraryBuilder;

impl AfLibraryBuilder for HnswMedoidAfLibraryBuilder {
    fn build(
        &self,
        events_row_major: &[f64],
        n_events: usize,
        n_detectors: usize,
        detector_names: &[String],
        config: &DiscoverConfig,
    ) -> Result<AfLibrary> {
        let mut cfg = config.clone();
        cfg.backend = DiscoveryBackend::HnswMedoid;
        discover::discover_af_library(
            events_row_major,
            n_events,
            n_detectors,
            detector_names,
            &cfg,
        )
    }
}

#[derive(Debug, Default)]
pub struct FlowSomAfLibraryBuilder;

impl AfLibraryBuilder for FlowSomAfLibraryBuilder {
    fn build(
        &self,
        events_row_major: &[f64],
        n_events: usize,
        n_detectors: usize,
        detector_names: &[String],
        config: &DiscoverConfig,
    ) -> Result<AfLibrary> {
        let mut cfg = config.clone();
        cfg.backend = DiscoveryBackend::FlowSom;
        discover::discover_af_library(
            events_row_major,
            n_events,
            n_detectors,
            detector_names,
            &cfg,
        )
    }
}

/// Normalize a spectrum in-place by its max absolute entry (unit-peak).
pub fn normalize_unit_peak(spectrum: &mut [f64]) {
    let max_abs = spectrum.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    if max_abs > 0.0 {
        for v in spectrum.iter_mut() {
            *v /= max_abs;
        }
    }
}

/// Cosine similarity of two equal-length vectors.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom <= f64::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

/// Drop near-duplicate columns (keep first), re-packing the matrix.
pub fn merge_near_duplicates(library: AfLibrary, threshold: f64) -> AfLibrary {
    let k = library.n_signatures();
    let d = library.n_detectors();
    if k <= 1 {
        return library;
    }
    let mut keep: Vec<usize> = Vec::new();
    'outer: for j in 0..k {
        let mut col_j = Vec::with_capacity(d);
        for i in 0..d {
            col_j.push(library.signatures[(i, j)]);
        }
        for &kept in &keep {
            let mut col_k = Vec::with_capacity(d);
            for i in 0..d {
                col_k.push(library.signatures[(i, kept)]);
            }
            if cosine_similarity(&col_j, &col_k) >= threshold {
                continue 'outer;
            }
        }
        keep.push(j);
    }
    if keep.len() == k {
        return library;
    }
    let mut signatures = Mat::<f64>::zeros(d, keep.len());
    let mut names = Vec::with_capacity(keep.len());
    for (new_j, &old_j) in keep.iter().enumerate() {
        for i in 0..d {
            signatures[(i, new_j)] = library.signatures[(i, old_j)];
        }
        names.push(library.names[old_j].clone());
    }
    AfLibrary {
        signatures,
        names,
        detector_names: library.detector_names,
        provenance: format!(
            "{}; merged_duplicates threshold={threshold} -> {}",
            library.provenance,
            keep.len()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::Mat;

    #[test]
    fn merge_drops_cosine_duplicates() {
        let mut signatures = Mat::<f64>::zeros(2, 3);
        signatures[(0, 0)] = 1.0;
        signatures[(1, 0)] = 0.0;
        signatures[(0, 1)] = 0.999;
        signatures[(1, 1)] = 0.001;
        signatures[(0, 2)] = 0.0;
        signatures[(1, 2)] = 1.0;
        let lib = AfLibrary {
            signatures,
            names: vec!["a".into(), "b".into(), "c".into()],
            detector_names: vec!["D1".into(), "D2".into()],
            provenance: "t".into(),
        };
        let merged = merge_near_duplicates(lib, 0.99);
        assert_eq!(merged.n_signatures(), 2);
    }
}
