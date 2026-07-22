//! Three-phase weight schedule for near-neighbour, mid-near, and further pairs.

/// Weights for the three pair types at a given iteration.
#[derive(Debug, Clone, Copy)]
pub struct Weights {
    pub w_nb: f32,
    pub w_mn: f32,
    pub w_fp: f32,
}

/// Compute weights for iteration `t` (1-indexed) given phase boundaries.
///
/// Phases are defined by `phase_iters = [p1, p2, p3]`:
/// - Phase 1: t in [1, p1]          — wNB=2, wMN decreases 1000→3, wFP=1
/// - Phase 2: t in [p1+1, p1+p2]   — wNB=3, wMN=3, wFP=1
/// - Phase 3: t in [p1+p2+1, total] — wNB=1, wMN=0, wFP=1
pub fn weights_at(t: usize, phase_iters: &[usize; 3]) -> Weights {
    let p1 = phase_iters[0];
    let p2 = phase_iters[1];

    if t <= p1 {
        // Phase 1: wMN decreases linearly from 1000 to 3 over p1 iterations
        let progress = if p1 > 1 { (t - 1) as f32 / (p1 - 1) as f32 } else { 1.0 };
        let w_mn = 1000.0 * (1.0 - progress) + 3.0 * progress;
        Weights { w_nb: 2.0, w_mn, w_fp: 1.0 }
    } else if t <= p1 + p2 {
        // Phase 2: fixed weights
        Weights { w_nb: 3.0, w_mn: 3.0, w_fp: 1.0 }
    } else {
        // Phase 3: wMN = 0, focus on local structure
        Weights { w_nb: 1.0, w_mn: 0.0, w_fp: 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn phase_1_start() {
        let w = weights_at(1, &[100, 100, 250]);
        assert_abs_diff_eq!(w.w_nb, 2.0);
        assert_abs_diff_eq!(w.w_mn, 1000.0, epsilon = 0.01);
        assert_abs_diff_eq!(w.w_fp, 1.0);
    }

    #[test]
    fn phase_1_end() {
        let w = weights_at(100, &[100, 100, 250]);
        assert_abs_diff_eq!(w.w_mn, 3.0, epsilon = 0.01);
    }

    #[test]
    fn phase_2() {
        let w = weights_at(150, &[100, 100, 250]);
        assert_abs_diff_eq!(w.w_nb, 3.0);
        assert_abs_diff_eq!(w.w_mn, 3.0);
    }

    #[test]
    fn phase_3() {
        let w = weights_at(300, &[100, 100, 250]);
        assert_abs_diff_eq!(w.w_nb, 1.0);
        assert_abs_diff_eq!(w.w_mn, 0.0);
        assert_abs_diff_eq!(w.w_fp, 1.0);
    }
}
