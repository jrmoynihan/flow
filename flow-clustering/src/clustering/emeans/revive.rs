//! Dead-cluster revival by donor split (Bessa et al., Appendix H.1).
//!
//! Every `revival_interval` batches, clusters with `Nⱼ < τ · mean(N)` are
//! rebuilt by splitting a count-weighted donor. The donor centroid is
//! element-wise perturbed with alternating ±δ (δ = BF16 ε by default). Each
//! child receives half the donor's count; sums are rewritten as `S = c · N`.

use rand::Rng;
use rand::RngExt;
use rand::seq::SliceRandom;

/// Revive dead clusters in-place. Returns the number of splits performed.
pub fn revive_dead_clusters(
    counts: &mut [f64],
    sums: &mut [f64],
    k: usize,
    d: usize,
    threshold: f32,
    delta: f32,
    rng: &mut impl Rng,
) -> usize {
    if k == 0 || d == 0 {
        return 0;
    }
    let mean = counts.iter().sum::<f64>() / k as f64;
    if mean <= 0.0 {
        return 0;
    }
    let cutoff = threshold as f64 * mean;

    let mut dead: Vec<usize> = (0..k).filter(|&j| counts[j] < cutoff).collect();
    if dead.is_empty() {
        return 0;
    }
    dead.shuffle(rng);

    // Candidate donors: not currently dead, positive count.
    let mut donors: Vec<usize> = (0..k)
        .filter(|&j| counts[j] >= cutoff && counts[j] > 0.0)
        .collect();
    if donors.is_empty() {
        return 0;
    }

    let mut n_revivals = 0usize;
    for dead_j in dead {
        // Recompute weights each time so a previously-split donor is less likely
        // to be drawn again immediately.
        donors.retain(|&j| j != dead_j && counts[j] > 0.0);
        if donors.is_empty() {
            break;
        }
        let total: f64 = donors.iter().map(|&j| counts[j]).sum();
        if total <= 0.0 {
            break;
        }
        let mut pick = rng.random::<f64>() * total;
        let mut donor = donors[0];
        for &j in &donors {
            pick -= counts[j];
            if pick <= 0.0 {
                donor = j;
                break;
            }
        }

        split_donor_into(counts, sums, donor, dead_j, d, delta);
        n_revivals += 1;
    }
    n_revivals
}

/// Apply Equation (8): child centroids get alternating ±δ, counts halved.
fn split_donor_into(
    counts: &mut [f64],
    sums: &mut [f64],
    donor: usize,
    dead: usize,
    d: usize,
    delta: f32,
) {
    let half = counts[donor] * 0.5;
    let donor_row = sums[donor * d..(donor + 1) * d].to_vec();
    // Current centroid of the donor.
    let inv_n = if counts[donor] > 0.0 {
        1.0 / counts[donor]
    } else {
        0.0
    };

    let mut c_dead = vec![0.0f64; d];
    let mut c_donor = vec![0.0f64; d];
    for t in 0..d {
        let c = donor_row[t] * inv_n;
        // Alternating +δ / −δ for the dead child; opposite for the donor child.
        let sign = if t % 2 == 0 { 1.0 } else { -1.0 };
        c_dead[t] = c * (1.0 + sign * delta as f64);
        c_donor[t] = c * (1.0 - sign * delta as f64);
    }

    counts[donor] = half;
    counts[dead] = half;
    for t in 0..d {
        sums[donor * d + t] = c_donor[t] * half;
        sums[dead * d + t] = c_dead[t] * half;
    }
}

/// Write centroids from EMA state: cⱼ = Sⱼ / Nⱼ.
pub fn write_centroids_from_ema(counts: &[f64], sums: &[f64], k: usize, d: usize, out: &mut [f32]) {
    debug_assert_eq!(out.len(), k * d);
    for j in 0..k {
        let n = counts[j];
        let inv = if n > 0.0 { 1.0 / n } else { 0.0 };
        for t in 0..d {
            out[j * d + t] = (sums[j * d + t] * inv) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn split_halves_count_and_perturbs() {
        let d = 2;
        let mut counts = vec![10.0, 0.0];
        let mut sums = vec![10.0, 20.0, 0.0, 0.0]; // centroid (1, 2) for donor 0
        split_donor_into(&mut counts, &mut sums, 0, 1, d, 0.1);
        assert!((counts[0] - 5.0).abs() < 1e-9);
        assert!((counts[1] - 5.0).abs() < 1e-9);
        // dead child: (1*1.1, 2*0.9) = (1.1, 1.8); sum = c * 5
        assert!((sums[2] - 1.1 * 5.0).abs() < 1e-6);
        assert!((sums[3] - 1.8 * 5.0).abs() < 1e-6);
    }

    #[test]
    fn revive_triggers_on_low_count() {
        let k = 3;
        let d = 1;
        let mut counts = vec![100.0, 100.0, 0.01];
        let mut sums = vec![0.0, 1.0, 0.0];
        let mut rng = StdRng::seed_from_u64(1);
        let n = revive_dead_clusters(&mut counts, &mut sums, k, d, 0.1, 0.0078125, &mut rng);
        assert!(n >= 1);
        assert!(counts[2] > 0.01);
    }
}
