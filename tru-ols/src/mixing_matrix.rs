//! Mixing matrix assembly from single-stain endmember spectra.

use crate::TruOlsError;
use faer::{Mat, MatRef};
use flow_linalg::{condition_number_2, hotspot_from_similarity};

/// Assembled mixing matrix plus QC metadata.
#[derive(Debug, Clone)]
pub struct MixingMatrix {
    pub matrix: Mat<f64>,
    pub detector_names: Vec<String>,
    pub endmember_names: Vec<String>,
    pub primary_detectors: Vec<String>,
    pub condition_number: f64,
    pub cosine_similarity: Mat<f64>,
    /// Mage hotspot \(H = \sqrt{|S^{-1}|}\); `None` if similarity invert failed.
    pub hotspot: Option<Mat<f64>>,
    pub hotspot_error: Option<String>,
}

impl MixingMatrix {
    /// Row-major detectors × endmembers flat buffer.
    pub fn flat_matrix(&self) -> Vec<f64> {
        let n_det = self.matrix.nrows();
        let n_em = self.matrix.ncols();
        let mut flat = Vec::with_capacity(n_det * n_em);
        for i in 0..n_det {
            for j in 0..n_em {
                flat.push(self.matrix[(i, j)]);
            }
        }
        flat
    }

    /// Row-major endmembers × endmembers cosine similarity.
    pub fn flat_cosine_similarity(&self) -> Vec<f64> {
        let n = self.cosine_similarity.nrows();
        let mut flat = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                flat.push(self.cosine_similarity[(i, j)]);
            }
        }
        flat
    }

    /// log₁₀(max(κ, 1)) when κ is finite.
    pub fn complexity_index(&self) -> Option<f64> {
        if self.condition_number.is_finite() {
            Some(self.condition_number.max(1.0).log10())
        } else {
            None
        }
    }

    /// Row-major \(n \times n\) hotspot, if available.
    pub fn flat_hotspot(&self) -> Option<Vec<f64>> {
        let h = self.hotspot.as_ref()?;
        let n = h.nrows();
        let mut flat = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                flat.push(h[(i, j)]);
            }
        }
        Some(flat)
    }

    /// Diagonal SIFs, if hotspot is available.
    pub fn sifs(&self) -> Option<Vec<f64>> {
        let h = self.hotspot.as_ref()?;
        Some((0..h.nrows()).map(|i| h[(i, i)]).collect())
    }
}

/// Builder for [`MixingMatrix`] from per-endmember positive medians (± AF correction).
#[derive(Debug, Clone)]
pub struct MixingMatrixBuilder {
    detector_names: Vec<String>,
    endmembers: Vec<(String, Vec<f64>, bool)>,
    autofluorescence: Option<Vec<f64>>,
}

impl MixingMatrixBuilder {
    pub fn new(detector_names: Vec<String>) -> Self {
        Self {
            detector_names,
            endmembers: Vec::new(),
            autofluorescence: None,
        }
    }

    pub fn set_autofluorescence(&mut self, af_medians: Vec<f64>) -> &mut Self {
        self.autofluorescence = Some(af_medians);
        self
    }

    /// `af_correction`: when true, subtract stored AF medians from this endmember column.
    pub fn add_endmember(
        &mut self,
        name: impl Into<String>,
        positive_medians: Vec<f64>,
        af_correction: bool,
    ) -> &mut Self {
        self.endmembers
            .push((name.into(), positive_medians, af_correction));
        self
    }

    pub fn build(self) -> Result<MixingMatrix, TruOlsError> {
        let n_det = self.detector_names.len();
        if n_det == 0 {
            return Err(TruOlsError::InvalidMixingMatrix("no detectors".into()));
        }
        if self.endmembers.is_empty() {
            return Err(TruOlsError::InvalidMixingMatrix("no endmembers".into()));
        }
        let n_em = self.endmembers.len();
        let af = self.autofluorescence.as_ref();
        if let Some(afv) = af {
            if afv.len() != n_det {
                return Err(TruOlsError::InvalidMixingMatrix(
                    "AF vector length mismatch".into(),
                ));
            }
        }
        // Include AF as its own endmember column when present (Mage / OLS+AF QC).
        let n_cols = n_em + usize::from(af.is_some());
        let mut matrix = Mat::<f64>::zeros(n_det, n_cols);
        let mut endmember_names = Vec::with_capacity(n_cols);
        let mut primary_detectors = Vec::with_capacity(n_cols);

        for (j, (name, medians, af_corr)) in self.endmembers.iter().enumerate() {
            if medians.len() != n_det {
                return Err(TruOlsError::InvalidMixingMatrix(format!(
                    "endmember {name}: expected {n_det} medians, got {}",
                    medians.len()
                )));
            }
            let mut col: Vec<f64> = medians.clone();
            if *af_corr {
                if let Some(afv) = af {
                    for (c, a) in col.iter_mut().zip(afv.iter()) {
                        *c -= *a;
                    }
                }
            }
            let primary_idx = normalize_signature_column(&mut col);
            for i in 0..n_det {
                matrix[(i, j)] = col[i];
            }
            endmember_names.push(name.clone());
            primary_detectors.push(self.detector_names[primary_idx].clone());
        }

        if let Some(afv) = af {
            let j = n_em;
            let mut col = afv.clone();
            let primary_idx = normalize_signature_column(&mut col);
            for i in 0..n_det {
                matrix[(i, j)] = col[i];
            }
            endmember_names.push("Autofluorescence".into());
            primary_detectors.push(self.detector_names[primary_idx].clone());
        }

        let cosine_similarity = cosine_similarity_matrix(matrix.as_ref());
        let condition_number = condition_number_2(matrix.as_ref());
        let (hotspot, hotspot_error) = match hotspot_from_similarity(cosine_similarity.as_ref()) {
            Ok(h) => (Some(h.matrix), None),
            Err(e) => (None, Some(e)),
        };

        Ok(MixingMatrix {
            matrix,
            detector_names: self.detector_names,
            endmember_names,
            primary_detectors,
            condition_number,
            cosine_similarity,
            hotspot,
            hotspot_error,
        })
    }
}

/// Floor negatives and max-normalize; returns primary (peak) detector index.
fn normalize_signature_column(col: &mut [f64]) -> usize {
    for c in col.iter_mut() {
        if *c < 0.0 {
            *c = 0.0;
        }
    }
    let max = col.iter().copied().fold(0.0_f64, f64::max);
    if max > 0.0 {
        for c in col.iter_mut() {
            *c /= max;
        }
    }
    col.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn cosine_similarity_matrix(m: MatRef<'_, f64>) -> Mat<f64> {
    let n = m.ncols();
    let mut out = Mat::<f64>::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            let mut dot = 0.0;
            let mut ni = 0.0;
            let mut nj = 0.0;
            for r in 0..m.nrows() {
                let a = m[(r, i)];
                let b = m[(r, j)];
                dot += a * b;
                ni += a * a;
                nj += b * b;
            }
            let denom = (ni.sqrt() * nj.sqrt()).max(f64::EPSILON);
            out[(i, j)] = dot / denom;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_normalized_columns() {
        let mut b = MixingMatrixBuilder::new(vec!["D1".into(), "D2".into()]);
        b.add_endmember("A", vec![100.0, 20.0], false);
        b.add_endmember("B", vec![10.0, 80.0], false);
        let m = b.build().unwrap();
        assert_eq!(m.endmember_names.len(), 2);
        assert!((m.matrix[(0, 0)] - 1.0).abs() < 1e-9);
        assert!(m.condition_number.is_finite());
        assert!((m.cosine_similarity[(0, 0)] - 1.0).abs() < 1e-9);
        assert_eq!(m.flat_matrix().len(), 4);
    }

    #[test]
    fn assemble_emits_hotspot_sifs() {
        let mut b = MixingMatrixBuilder::new(vec!["D1".into(), "D2".into(), "D3".into()]);
        b.add_endmember("A", vec![100.0, 5.0, 1.0], false);
        b.add_endmember("B", vec![1.0, 5.0, 100.0], false);
        let m = b.build().unwrap();
        let sifs = m.sifs().expect("sifs");
        assert_eq!(sifs.len(), 2);
        assert!(sifs.iter().all(|s| s.is_finite() && *s >= 1.0));
        assert!(m.hotspot_error.is_none());
    }

    #[test]
    fn assemble_appends_af_endmember_column() {
        let mut b = MixingMatrixBuilder::new(vec!["D1".into(), "D2".into(), "D3".into()]);
        b.set_autofluorescence(vec![10.0, 20.0, 5.0]);
        b.add_endmember("A", vec![100.0, 25.0, 6.0], true);
        b.add_endmember("B", vec![12.0, 25.0, 100.0], true);
        let m = b.build().unwrap();
        assert_eq!(m.endmember_names.len(), 3);
        assert_eq!(m.endmember_names[2], "Autofluorescence");
        assert_eq!(m.matrix.ncols(), 3);
        assert!((m.matrix[(1, 2)] - 1.0).abs() < 1e-9); // AF peak on D2
        let sifs = m.sifs().expect("sifs");
        assert_eq!(sifs.len(), 3);
    }
}
