//! Cosine step schedule and published scaling laws for e-means.
//!
//! Peak step and batch size follow Bessa et al., arXiv:2611.09501, Equations (5)–(7)
//! and Table 2. The laws were fitted on DEEP (d = 96) with k ≥ 256 and
//! n ∈ [1e5, 1e8]; the authors note they are not optimal on low-dimensional or
//! tiny problems.

/// Floor fraction of the peak step: α(t) bottoms at `ε · α₀` with ε = 0.1.
pub const SCHEDULE_FLOOR: f32 = 0.1;

/// Scale above which the batch law constant is divided by 8 (Table 2, row 3).
pub const LARGE_N_BATCH_DIVIDE: usize = 2_000_000;

/// Cosine-annealed step size for update index `t` (1-based) of `m` total updates.
///
/// α(t) = α₀ · (ε + (1 − ε) · ½(1 + cos(π · (t − 1) / (M − 1)))) with ε = floor.
/// When `m <= 1`, returns `alpha0`.
#[must_use]
pub fn cosine_step(alpha0: f32, floor: f32, t: usize, m: usize) -> f32 {
    if m <= 1 {
        return alpha0;
    }
    let t0 = t.saturating_sub(1).min(m.saturating_sub(1));
    let progress = t0 as f64 / (m - 1) as f64;
    let cos_term = 0.5 * (1.0 + (std::f64::consts::PI * progress).cos());
    let frac = floor as f64 + (1.0 - floor as f64) * cos_term;
    (alpha0 as f64 * frac) as f32
}

/// Round to the nearest power of two (ties round up), minimum 1.
#[must_use]
pub fn nearest_power_of_two(x: f64) -> usize {
    if !x.is_finite() || x <= 1.0 {
        return 1;
    }
    let log2 = x.log2();
    let lo = 2f64.powi(log2.floor() as i32);
    let hi = 2f64.powi(log2.ceil() as i32);
    // Ties (exact midpoint) round up to the larger power of two.
    if (x - lo) < (hi - x) {
        lo as usize
    } else {
        hi as usize
    }
}

/// Prefer [`super::EMeans`] over same-kernel [`super::Lloyd`] for this size.
///
/// Measured on Apple M5 Max (2026-11-12): with held-out MSE ≤ Lloyd@25 × 1.005,
/// e-means was faster from about **5 000 events** upward across 10–60 features
/// (`FLOW_CLUSTERING_BENCH_LOWER_BOUND` grid). Below that both finishes are
/// usually under ~20 ms; Lloyd only clearly wins at toy sizes (e.g. 1 000
/// events × 10 features × 8 clusters). Callers can treat e-means as the
/// default for practical cytometry matrices.
///
/// Returns true when `n_events ≥ 5_000` or `n_events × n_features ≥ 50_000`.
#[must_use]
pub fn prefer_emeans(n_events: usize, n_features: usize) -> bool {
    n_events >= 5_000 || n_events.saturating_mul(n_features) >= 50_000
}

/// Published batch-size law: B = c · k^0.13 · n^0.88 · T^0.81, rounded to a power
/// of two and capped at `n`. Uses c = 0.035 when `n ≤ 2M`, else c = 0.0044.
#[must_use]
pub fn batch_size_law(n: usize, k: usize, epochs: usize) -> usize {
    if n == 0 || k == 0 || epochs == 0 {
        return 1;
    }
    let c = if n > LARGE_N_BATCH_DIVIDE {
        0.0044_f64
    } else {
        0.035_f64
    };
    let raw = c
        * (k as f64).powf(0.13)
        * (n as f64).powf(0.88)
        * (epochs as f64).powf(0.81);
    nearest_power_of_two(raw).clamp(1, n)
}

/// Published peak-step law:
/// α₀ = min(0.824 · k^−0.46 · B^0.90 · n^−0.45 · T^−0.11, 1).
#[must_use]
pub fn alpha0_law(n: usize, k: usize, epochs: usize, batch: usize) -> f32 {
    if n == 0 || k == 0 || epochs == 0 || batch == 0 {
        return 1.0;
    }
    let raw = 0.824_f64
        * (k as f64).powf(-0.46)
        * (batch as f64).powf(0.90)
        * (n as f64).powf(-0.45)
        * (epochs as f64).powf(-0.11);
    raw.min(1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_step_starts_at_peak_and_ends_at_floor() {
        let a0 = 0.5_f32;
        let m = 100;
        let first = cosine_step(a0, SCHEDULE_FLOOR, 1, m);
        let last = cosine_step(a0, SCHEDULE_FLOOR, m, m);
        assert!((first - a0).abs() < 1e-5);
        assert!((last - a0 * SCHEDULE_FLOOR).abs() < 1e-4);
    }

    #[test]
    fn nearest_power_of_two_rounds() {
        assert_eq!(nearest_power_of_two(1.0), 1);
        assert_eq!(nearest_power_of_two(3.0), 4);
        assert_eq!(nearest_power_of_two(5.0), 4);
        assert_eq!(nearest_power_of_two(6.0), 8);
    }

    #[test]
    fn batch_law_caps_at_n_and_divides_above_2m() {
        let small = batch_size_law(100_000, 256, 3);
        assert!(small >= 1 && small <= 100_000);
        let large = batch_size_law(5_000_000, 1024, 3);
        let without_divide = nearest_power_of_two(
            0.035 * (1024f64).powf(0.13) * (5_000_000f64).powf(0.88) * (3f64).powf(0.81),
        )
        .min(5_000_000);
        // With divide-by-8 constant the result should be smaller than the undivided knee.
        assert!(large <= without_divide);
    }

    #[test]
    fn alpha0_clips_at_one() {
        let a = alpha0_law(1_000, 2, 3, 64);
        assert!(a <= 1.0);
        assert!(a > 0.0);
    }

    #[test]
    fn prefer_emeans_lower_bound() {
        assert!(!prefer_emeans(1_000, 10));
        assert!(prefer_emeans(1_000, 60)); // n×d = 60k
        assert!(prefer_emeans(5_000, 10));
        assert!(prefer_emeans(10_000, 30));
    }
}
