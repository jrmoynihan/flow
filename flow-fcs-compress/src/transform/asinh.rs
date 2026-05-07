//! Hyperbolic-arcsine transform — the modern flow-cytometry display transform.
//!
//! `y = asinh(x / cofactor)` is approximately linear near zero and approximately
//! `sign(x) * ln(2|x|/c)` for `|x| ≫ c`. It handles post-compensation negatives
//! natively, unlike pure log10. FlowJo, OMIQ, Cytobank, and most current
//! analysis tools default to arcsinh for fluorescence channels.
//!
//! The plan also calls out biexponential (Parks–Roederer, 2006) as a candidate
//! for `$PnE`-driven decode. We start with arcsinh because:
//!   1. It's the de-facto display transform in current practice.
//!   2. The inverse is a single `sinh` call — fast on the decode path.
//!   3. Biexp's parameters live across multiple FCS keywords and require
//!      keyword-table plumbing we haven't done yet.
//!
//! Biexp is a candidate enhancement once the `$PnE`/`$TR`/`$PnG` reading lands
//! end-to-end (M5+).

/// Cofactor convention: typical fluorescence values use 150–500 on Cytek
/// instruments, 5 on BD instruments. Flow ranges 5..1000 are reasonable
/// defaults; 150 matches Cytek SpectroFlo's default.
pub const DEFAULT_COFACTOR: f32 = 150.0;

#[inline]
pub fn forward(x: f32, cofactor: f32) -> f32 {
    (x / cofactor).asinh()
}

#[inline]
pub fn inverse(y: f32, cofactor: f32) -> f32 {
    cofactor * y.sinh()
}

/// Forward-then-inverse round-trip max error for a sample of values, useful
/// for diagnostic checks. Returns `(max_abs_err, max_rel_err)`.
pub fn forward_inverse_error(samples: &[f32], cofactor: f32) -> (f32, f32) {
    let mut max_abs = 0f32;
    let mut max_rel = 0f32;
    for &x in samples {
        let y = forward(x, cofactor);
        let r = inverse(y, cofactor);
        let abs = (r - x).abs();
        max_abs = max_abs.max(abs);
        if x.abs() > 1e-3 {
            max_rel = max_rel.max(abs / x.abs());
        }
    }
    (max_abs, max_rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_inverse_identity_within_eps() {
        // Range [-50_000, 50_000] in steps of 50.
        let samples: Vec<f32> = (-1000..1000).map(|i| (i as f32) * 50.0).collect();
        let (_abs, rel) = forward_inverse_error(&samples, DEFAULT_COFACTOR);
        // f32 transcendental round-trip ULPs accumulate; rel error is the
        // load-bearing guarantee. Absolute error grows with |x| as expected.
        assert!(rel < 1e-5, "max rel err = {rel}");
    }

    #[test]
    fn handles_negatives() {
        let y = forward(-100.0, 5.0);
        assert!(y < 0.0);
        let r = inverse(y, 5.0);
        assert!((r - (-100.0)).abs() < 1e-3);
    }

    #[test]
    fn near_zero_is_near_linear() {
        // For |x| << c, asinh(x/c) ≈ x/c, so x ≈ c * y.
        let c = 150.0;
        for &x in &[-1.0f32, -0.1, 0.0, 0.1, 1.0] {
            let y = forward(x, c);
            assert!((c * y - x).abs() < 1e-3);
        }
    }
}
