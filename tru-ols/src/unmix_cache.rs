//! Optional concurrent cache of least-squares factors keyed by active-endmember [`u128`] mask.

use crate::TruOlsError;
use faer::Mat;
use faer::MatRef;
use faer::Side;
use faer::linalg::solvers::{Llt, Qr};
use faer::prelude::{Solve, SolveLstsq};
use std::sync::Arc;

enum MaskFactorInner {
    /// Normal equations via \(M^\top M\) Cholesky (fast when the Gram matrix is SPD).
    Cholesky(Llt<f64>),
    /// Dense QR on \(M_{\text{sub}}\) when the Gram route is not numerically SPD (matches non-cache QR path).
    Qr(Qr<f64>),
}

/// Cached factorization for \(\min_x \|M_{\text{sub}} x - b\|\) for a fixed active column set.
pub(crate) struct MaskFactorization {
    pub(crate) k: usize,
    pub(crate) n_det: usize,
    pub(crate) m_sub: Mat<f64>,
    inner: MaskFactorInner,
}

impl MaskFactorization {
    pub(crate) fn build(mixing: MatRef<'_, f64>, mask: u128) -> Result<Arc<Self>, TruOlsError> {
        let n_det = mixing.nrows();
        let globals: Vec<usize> = (0..crate::MAX_ENDMEMBERS_DEFAULT)
            .filter(|g| (mask & (1u128 << g)) != 0)
            .collect();
        let k = globals.len();
        if k == 0 {
            return Err(TruOlsError::LinearAlgebra(
                "Empty endmember mask for cached factorization".to_string(),
            ));
        }
        let mut m_sub = Mat::zeros(n_det, k);
        for (j, &g) in globals.iter().enumerate() {
            for i in 0..n_det {
                m_sub[(i, j)] = mixing[(i, g)];
            }
        }
        let mt = m_sub.transpose().to_owned();
        let gram: Mat<f64> = &mt * &m_sub;
        let inner = if let Ok(llt) = Llt::new(gram.as_ref(), Side::Lower) {
            MaskFactorInner::Cholesky(llt)
        } else {
            MaskFactorInner::Qr(Qr::new(m_sub.as_ref()))
        };

        Ok(Arc::new(Self {
            k,
            n_det,
            m_sub,
            inner,
        }))
    }

    /// Solve \(\min_x \|M_{\text{sub}} x - b\|\) using the cached factor; reads RHS from
    /// [`crate::unmix_buffer::UnmixScratch::adjusted_observation`] (length `n_det`) and writes `k`
    /// coefficients into [`crate::unmix_buffer::UnmixScratch::x_out`].
    pub(crate) fn solve_into(
        &self,
        scratch: &mut crate::unmix_buffer::UnmixScratch,
    ) -> Result<(), TruOlsError> {
        match &self.inner {
            MaskFactorInner::Cholesky(llt) => {
                for c in 0..self.k {
                    let mut s = 0.0_f64;
                    for i in 0..self.n_det {
                        s += self.m_sub[(i, c)] * scratch.adjusted_observation[i];
                    }
                    scratch.rhs_gram[c] = s;
                }
                let rhs =
                    faer::MatRef::from_column_major_slice(&scratch.rhs_gram[..self.k], self.k, 1);
                let x = llt.solve(rhs);
                for i in 0..self.k {
                    scratch.x_out[i] = x[(i, 0)];
                }
            }
            MaskFactorInner::Qr(qr) => {
                let b_mat = MatRef::from_column_major_slice(
                    &scratch.adjusted_observation[..self.n_det],
                    self.n_det,
                    1,
                );
                let x_faer = qr.solve_lstsq(b_mat);
                for i in 0..self.k {
                    scratch.x_out[i] = x_faer[(i, 0)];
                }
            }
        }
        Ok(())
    }
}

/// Look up or build cached factorization for `active` leading columns described by `scratch.current_indices`.
pub(crate) fn solve_with_mask_cache(
    cache: &quick_cache::sync::Cache<u128, Arc<MaskFactorization>>,
    mixing: MatRef<'_, f64>,
    active: usize,
    scratch: &mut crate::unmix_buffer::UnmixScratch,
) -> Result<(), TruOlsError> {
    let mask = crate::unmix_buffer::active_global_mask(&scratch.current_indices, active);
    let fac: Arc<MaskFactorization> =
        cache.get_or_insert_with(&mask, || MaskFactorization::build(mixing, mask))?;
    fac.solve_into(scratch)
}
