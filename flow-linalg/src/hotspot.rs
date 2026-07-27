//! Mage Hotspot Matrix: \(H = \sqrt{|S^{-1}|}\) where \(S\) is cosine similarity.
//!
//! Diagonal entries are Spreading Inflation Factors (SIFs). Off-diagonal entries
//! indicate fluorochrome combinations that drive or suffer unmixing-dependent spread.

use faer::linalg::solvers::{DenseSolveCore, PartialPivLu};
use faer::{Mat, MatRef};

/// Hotspot matrix \(H_{ij} = \sqrt{|(S^{-1})_{ij}|}\).
#[derive(Debug, Clone)]
pub struct HotspotMatrix {
    pub matrix: Mat<f64>,
}

impl HotspotMatrix {
    /// Diagonal SIFs (one per endmember).
    pub fn sifs(&self) -> Vec<f64> {
        let n = self.matrix.nrows();
        (0..n).map(|i| self.matrix[(i, i)]).collect()
    }

    /// Row-major \(n \times n\) flat buffer.
    pub fn flat_row_major(&self) -> Vec<f64> {
        let n = self.matrix.nrows();
        let mut out = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                out.push(self.matrix[(i, j)]);
            }
        }
        out
    }
}

/// Compute hotspot from a square cosine-similarity (Gram) matrix \(S\).
pub fn hotspot_from_similarity(similarity: MatRef<'_, f64>) -> Result<HotspotMatrix, String> {
    let n = similarity.nrows();
    if n == 0 || similarity.ncols() != n {
        return Err("hotspot requires a non-empty square similarity matrix".into());
    }
    let lu = PartialPivLu::new(similarity);
    let u = lu.U();
    for i in 0..n {
        if !u[(i, i)].is_finite() || u[(i, i)].abs() < 1e-12 {
            return Err(format!(
                "similarity matrix is singular or ill-conditioned at diagonal index {i}"
            ));
        }
    }
    let inv = lu.inverse();
    let matrix = Mat::<f64>::from_fn(n, n, |i, j| inv[(i, j)].abs().sqrt());
    Ok(HotspotMatrix { matrix })
}

/// Unit-normalize mixing-matrix columns, form \(S = A_u^\top A_u\), then hotspot.
pub fn hotspot_from_mixing_matrix(mixing: MatRef<'_, f64>) -> Result<HotspotMatrix, String> {
    let m = mixing.nrows();
    let n = mixing.ncols();
    if m == 0 || n == 0 {
        return Err("hotspot requires a non-empty mixing matrix".into());
    }
    let mut au = Mat::<f64>::zeros(m, n);
    for j in 0..n {
        let mut norm_sq = 0.0;
        for i in 0..m {
            let v = mixing[(i, j)];
            norm_sq += v * v;
        }
        let denom = norm_sq.sqrt().max(f64::EPSILON);
        for i in 0..m {
            au[(i, j)] = mixing[(i, j)] / denom;
        }
    }
    let mut s = Mat::<f64>::zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            let mut dot = 0.0;
            for r in 0..m {
                dot += au[(r, i)] * au[(r, j)];
            }
            s[(i, j)] = dot;
        }
    }
    hotspot_from_similarity(s.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::Mat;

    #[test]
    fn identity_similarity_has_unit_sifs() {
        let s = Mat::<f64>::from_fn(3, 3, |i, j| if i == j { 1.0 } else { 0.0 });
        let h = hotspot_from_similarity(s.as_ref()).expect("hotspot");
        for sif in h.sifs() {
            assert!((sif - 1.0).abs() < 1e-9, "sif={sif}");
        }
    }

    #[test]
    fn collinear_columns_inflate_sifs() {
        // Two nearly identical unit columns → high CS → large SIFs.
        let a = Mat::<f64>::from_fn(4, 2, |i, j| {
            if j == 0 {
                if i == 0 {
                    1.0
                } else {
                    0.05
                }
            } else if i == 0 {
                0.98
            } else if i == 1 {
                0.2
            } else {
                0.05
            }
        });
        let h = hotspot_from_mixing_matrix(a.as_ref()).expect("hotspot");
        let sifs = h.sifs();
        assert!(sifs[0] > 1.5, "sif0={}", sifs[0]);
        assert!(sifs[1] > 1.5, "sif1={}", sifs[1]);
        assert!(h.matrix[(0, 1)] > 0.5, "offdiag={}", h.matrix[(0, 1)]);
    }

    #[test]
    fn singular_similarity_errors() {
        let s = Mat::<f64>::from_fn(2, 2, |_i, _j| 1.0);
        assert!(hotspot_from_similarity(s.as_ref()).is_err());
    }
}
