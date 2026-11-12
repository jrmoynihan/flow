//! Mini-batch k-means with exponential-moving-average centroids (**e-means**).
//!
//! Reimplements the default pipeline of Bessa, Mazaré, Szilvasy, Touvron, Picard,
//! Douze and Jégou, ["e-means: outperforming k-means (in one-tenth the epochs)"](
//! https://arxiv.org/abs/2611.09501), arXiv:2611.09501.
//!
//! # Design notes
//!
//! - Input is row-major `&[f32]` with shape `n × d` (same layout as
//!   `flow_knn::compute_knn`), avoiding the `Array2<f64>` copy used by the
//!   linfa-backed types.
//! - Assignment sits behind [`assign::Assign`] so Lloyd, the CPU loop, an
//!   optional faer product (`emeans-faer`), and the cubeCL kernel (`emeans-gpu`)
//!   share one interface.
//! - Revival perturbation δ defaults to BF16 machine epsilon (Appendix H.1).
//! - Slow-EMA finalisation uses a constant step equal to
//!   `multiplier · schedule_floor · α₀` (Appendix I); a half-life can be set via
//!   [`EMeansConfig::with_slow_ema_half_life`].

mod assign;
mod config;
pub mod lloyd;
mod revive;
mod schedule;

#[cfg(feature = "emeans-faer")]
mod faer_assign;

#[cfg(feature = "emeans-gpu")]
pub mod gpu;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::clustering::{ClusteringError, ClusteringResult};

pub use assign::{Assign, CpuLoopAssign, make_assigner};
pub use config::{
    AssignBackend, EMeansConfig, EMeansProgress, EMeansResult, EMeansStage, Finalization,
    InitMethod, REVIVAL_DELTA_BF16,
};
pub use lloyd::{Lloyd, LloydConfig, LloydResult};
pub use schedule::{
    SCHEDULE_FLOOR, alpha0_law, batch_size_law, cosine_step, nearest_power_of_two, prefer_emeans,
};

use assign::accumulate_batch;
use config::{EMeansControl, resolve_slow_ema_step};
use lloyd::sample_distinct_rows;
use revive::{revive_dead_clusters, write_centroids_from_ema};
use schedule::{alpha0_law as law_alpha0, batch_size_law as law_batch, cosine_step as step_at};

/// Mini-batch EMA k-means.
pub struct EMeans;

impl EMeans {
    /// Fit e-means on row-major `f32` data of shape `n × d`.
    pub fn fit(data: &[f32], n: usize, d: usize, config: &EMeansConfig) -> ClusteringResult<EMeansResult> {
        Self::fit_with_control(data, n, d, config, None, None)
    }

    /// Fit with optional progress channel and cooperative cancel flag.
    pub fn fit_with_control(
        data: &[f32],
        n: usize,
        d: usize,
        config: &EMeansConfig,
        progress: Option<mpsc::Sender<EMeansProgress>>,
        cancel: Option<Arc<AtomicBool>>,
    ) -> ClusteringResult<EMeansResult> {
        validate_input(data, n, d, config.n_clusters)?;
        if config.epochs == 0 {
            return Err(ClusteringError::InvalidConfig(
                "epochs must be > 0".into(),
            ));
        }

        let k = config.n_clusters;
        let seed = config.seed.unwrap_or_else(rand::random);
        let mut rng = StdRng::seed_from_u64(seed);
        let assigner = make_assigner(config.backend)?;

        let batch = config
            .batch_size
            .unwrap_or_else(|| law_batch(n, k, config.epochs))
            .clamp(1, n);
        let alpha0 = config
            .alpha0
            .unwrap_or_else(|| law_alpha0(n, k, config.epochs, batch));
        let floor = config.schedule_floor.clamp(0.0, 1.0);
        let m_updates = config.epochs.saturating_mul(n / batch).max(1);
        let slow_step = resolve_slow_ema_step(config.finalization, floor, alpha0);

        let stage_count = config.epochs + 2; // init + epochs + finalize/done
        let control = EMeansControl::new(progress.as_ref(), cancel.as_deref(), stage_count);
        control.emit(EMeansStage::Init, 0, None)?;

        let mut centroids = init_centroids(data, n, d, k, config.init, &mut rng)?;
        // EMA state
        let mut ema_n = vec![1e-10_f64; k];
        let mut ema_s = vec![0.0_f64; k * d];
        for j in 0..k {
            for t in 0..d {
                ema_s[j * d + t] = centroids[j * d + t] as f64 * ema_n[j];
            }
        }

        // Slow-EMA state (optional)
        let mut slow_n = ema_n.clone();
        let mut slow_s = ema_s.clone();

        // Last-epoch accumulators
        let mut last_n = vec![0.0_f64; k];
        let mut last_s = vec![0.0_f64; k * d];

        let mut order: Vec<usize> = (0..n).collect();
        let mut batch_buf = vec![0.0f32; batch * d];
        let mut batch_assign = vec![0u32; batch];
        let mut batch_counts = vec![0.0f64; k];
        let mut batch_sums = vec![0.0f64; k * d];

        let mut t = 0usize;
        let mut n_revivals = 0usize;
        let revival_interval = config.revival_interval.max(1);

        for epoch in 0..config.epochs {
            control.emit(
                EMeansStage::Epoch,
                epoch + 1,
                Some(format!("epoch={}/{}", epoch + 1, config.epochs)),
            )?;
            order.shuffle(&mut rng);

            if matches!(config.finalization, Finalization::LastEpochMean)
                && epoch + 1 == config.epochs
            {
                last_n.fill(0.0);
                last_s.fill(0.0);
            }

            let mut offset = 0usize;
            let mut batches_this_epoch = 0usize;
            while offset + batch <= n {
                let end = offset + batch;
                for (bi, &row) in order[offset..end].iter().enumerate() {
                    let src = &data[row * d..(row + 1) * d];
                    batch_buf[bi * d..(bi + 1) * d].copy_from_slice(src);
                }

                t += 1;
                let alpha = step_at(alpha0, floor, t, m_updates);
                let gamma = 1.0 - alpha as f64;

                write_centroids_from_ema(&ema_n, &ema_s, k, d, &mut centroids);
                assigner.assign(&batch_buf, batch, d, &centroids, k, &mut batch_assign)?;
                accumulate_batch(
                    &batch_buf,
                    batch,
                    d,
                    &batch_assign,
                    k,
                    &mut batch_counts,
                    &mut batch_sums,
                );

                for j in 0..k {
                    ema_n[j] = gamma * ema_n[j] + (1.0 - gamma) * batch_counts[j];
                    for dim in 0..d {
                        let idx = j * d + dim;
                        ema_s[idx] = gamma * ema_s[idx] + (1.0 - gamma) * batch_sums[idx];
                    }
                }

                if let Some(alpha_slow) = slow_step {
                    let g_slow = 1.0 - alpha_slow as f64;
                    for j in 0..k {
                        slow_n[j] = g_slow * slow_n[j] + (1.0 - g_slow) * batch_counts[j];
                        for dim in 0..d {
                            let idx = j * d + dim;
                            slow_s[idx] =
                                g_slow * slow_s[idx] + (1.0 - g_slow) * batch_sums[idx];
                        }
                    }
                }

                if matches!(config.finalization, Finalization::LastEpochMean)
                    && epoch + 1 == config.epochs
                {
                    for j in 0..k {
                        last_n[j] += batch_counts[j];
                        for dim in 0..d {
                            let idx = j * d + dim;
                            last_s[idx] += batch_sums[idx];
                        }
                    }
                }

                batches_this_epoch += 1;
                if batches_this_epoch.is_multiple_of(revival_interval) {
                    write_centroids_from_ema(&ema_n, &ema_s, k, d, &mut centroids);
                    let revived = revive_dead_clusters(
                        &mut ema_n,
                        &mut ema_s,
                        k,
                        d,
                        config.revival_threshold,
                        config.revival_delta,
                        &mut rng,
                    );
                    n_revivals += revived;
                    if revived > 0 && slow_step.is_some() {
                        // Keep slow EMA consistent with splits: mirror from main EMA.
                        slow_n.copy_from_slice(&ema_n);
                        slow_s.copy_from_slice(&ema_s);
                    }
                }

                offset = end;
                control.check_cancel()?;
            }
        }

        control.emit(EMeansStage::Finalize, config.epochs + 1, None)?;

        match config.finalization {
            Finalization::LastEpochMean => {
                for j in 0..k {
                    if last_n[j] > 0.0 {
                        let inv = 1.0 / last_n[j];
                        for dim in 0..d {
                            centroids[j * d + dim] = (last_s[j * d + dim] * inv) as f32;
                        }
                    } else {
                        // Fall back to EMA centroid for empty last-epoch clusters.
                        let inv = if ema_n[j] > 0.0 { 1.0 / ema_n[j] } else { 0.0 };
                        for dim in 0..d {
                            centroids[j * d + dim] = (ema_s[j * d + dim] * inv) as f32;
                        }
                    }
                }
            }
            Finalization::SlowEma { .. } => {
                write_centroids_from_ema(&slow_n, &slow_s, k, d, &mut centroids);
            }
            Finalization::None => {
                write_centroids_from_ema(&ema_n, &ema_s, k, d, &mut centroids);
            }
        }

        let mut assignments = vec![0u32; n];
        assigner.assign(data, n, d, &centroids, k, &mut assignments)?;
        let inertia = assign::inertia(data, n, d, &centroids, &assignments);
        let mut counts_u = vec![0usize; k];
        for &a in &assignments {
            if (a as usize) < k {
                counts_u[a as usize] += 1;
            }
        }

        control.emit(EMeansStage::Done, stage_count - 1, None)?;

        Ok(EMeansResult {
            centroids,
            n_clusters: k,
            n_features: d,
            assignments,
            counts: counts_u,
            inertia,
            epochs_run: config.epochs,
            batch_size: batch,
            alpha0,
            n_revivals,
        })
    }
}

pub(crate) fn validate_input(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
) -> ClusteringResult<()> {
    if n == 0 {
        return Err(ClusteringError::EmptyData);
    }
    if d == 0 {
        return Err(ClusteringError::InvalidConfig(
            "feature dimension must be > 0".into(),
        ));
    }
    if data.len() != n.saturating_mul(d) {
        return Err(ClusteringError::InvalidConfig(format!(
            "data length {} != n·d = {n}·{d}",
            data.len()
        )));
    }
    if k == 0 {
        return Err(ClusteringError::InvalidConfig(
            "n_clusters must be > 0".into(),
        ));
    }
    if n < k {
        return Err(ClusteringError::InsufficientData {
            min: k,
            actual: n,
        });
    }
    Ok(())
}

pub(crate) fn init_centroids(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    method: InitMethod,
    rng: &mut StdRng,
) -> ClusteringResult<Vec<f32>> {
    match method {
        InitMethod::Random => {
            let idxs = sample_distinct_rows(n, k, rng)?;
            let mut centroids = vec![0.0f32; k * d];
            for (j, &row) in idxs.iter().enumerate() {
                centroids[j * d..(j + 1) * d].copy_from_slice(&data[row * d..(row + 1) * d]);
            }
            Ok(centroids)
        }
        InitMethod::KMeansPlusPlus => kmeans_plus_plus(data, n, d, k, rng),
    }
}

fn kmeans_plus_plus(
    data: &[f32],
    n: usize,
    d: usize,
    k: usize,
    rng: &mut StdRng,
) -> ClusteringResult<Vec<f32>> {
    let mut centroids = vec![0.0f32; k * d];
    let first = rng.random_range(0..n);
    centroids[..d].copy_from_slice(&data[first * d..(first + 1) * d]);

    let mut closest = vec![f64::INFINITY; n];
    for j in 1..k {
        // Update closest-centroid squared distances.
        let prev = &centroids[(j - 1) * d..j * d];
        for i in 0..n {
            let row = &data[i * d..(i + 1) * d];
            let mut dist = 0.0f64;
            for t in 0..d {
                let diff = row[t] as f64 - prev[t] as f64;
                dist += diff * diff;
            }
            if dist < closest[i] {
                closest[i] = dist;
            }
        }
        let total: f64 = closest.iter().sum();
        let pick = if total > 0.0 {
            rng.random::<f64>() * total
        } else {
            0.0
        };
        let mut acc = 0.0;
        let mut chosen = n - 1;
        for (i, &dist) in closest.iter().enumerate() {
            acc += dist;
            if acc >= pick {
                chosen = i;
                break;
            }
        }
        centroids[j * d..(j + 1) * d].copy_from_slice(&data[chosen * d..(chosen + 1) * d]);
    }
    Ok(centroids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_blobs() -> (Vec<f32>, usize, usize) {
        // 20 points near (0,0) and 20 near (10,10)
        let mut data = Vec::with_capacity(40 * 2);
        for i in 0..20 {
            data.push((i as f32) * 0.01);
            data.push((i as f32) * 0.01);
        }
        for i in 0..20 {
            data.push(10.0 + (i as f32) * 0.01);
            data.push(10.0 + (i as f32) * 0.01);
        }
        (data, 40, 2)
    }

    #[test]
    fn emeans_separates_two_blobs() {
        let (data, n, d) = two_blobs();
        let cfg = EMeansConfig {
            n_clusters: 2,
            epochs: 5,
            batch_size: Some(16),
            alpha0: Some(0.5),
            seed: Some(7),
            ..Default::default()
        };
        let result = EMeans::fit(&data, n, d, &cfg).expect("fit");
        assert_eq!(result.assignments.len(), n);
        assert_eq!(result.n_clusters, 2);
        // Both clusters should be non-empty on this trivial problem.
        assert!(result.counts.iter().all(|&c| c > 0));
        assert!(result.inertia < 1.0);
    }

    #[test]
    fn lloyd_baseline_runs() {
        let (data, n, d) = two_blobs();
        let cfg = LloydConfig {
            n_clusters: 2,
            max_iterations: 10,
            seed: Some(3),
            ..Default::default()
        };
        let result = Lloyd::fit(&data, n, d, &cfg).expect("lloyd");
        assert_eq!(result.iterations, 10);
        assert!(result.inertia < 1.0);
    }

    #[test]
    fn cancel_before_start() {
        let (data, n, d) = two_blobs();
        let cancel = Arc::new(AtomicBool::new(true));
        let err = EMeans::fit_with_control(
            &data,
            n,
            d,
            &EMeansConfig {
                n_clusters: 2,
                epochs: 1,
                batch_size: Some(8),
                seed: Some(1),
                ..Default::default()
            },
            None,
            Some(cancel),
        )
        .unwrap_err();
        assert!(matches!(err, ClusteringError::Cancelled));
    }
}
