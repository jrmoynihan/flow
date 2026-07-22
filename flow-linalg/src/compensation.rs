use anyhow::Result;
use faer::{Mat, MatRef};
use std::collections::HashMap;

/// Median of a sample. Returns `NaN` for an empty slice. Does not mutate the
/// input (operates on a sorted copy). Even-length samples average the two
/// central order statistics.
pub fn median(values: &[f32]) -> f32 {
    median_opt(values).unwrap_or(f32::NAN)
}

fn median_opt(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<f32> = values.to_vec();
    sorted.sort_unstable_by(f32::total_cmp);
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some(0.5 * (sorted[mid - 1] + sorted[mid]))
    }
}

/// One single-stain control's per-detector event samples.
///
/// `pos_per_detector` / `neg_per_detector` are length `n_detectors`: each entry
/// holds the event values measured in that detector for the positive population
/// (inside the stain's positive gate) and the negative population (unstained or
/// internal negative), respectively. `primary_index` is the detector index that
/// is this stain's own detector (the matrix column this control fills).
pub struct SingleStainControl<'a> {
    pub primary_index: usize,
    pub pos_per_detector: &'a [Vec<f32>],
    pub neg_per_detector: &'a [Vec<f32>],
}

/// Estimate a diagonal-normalized `n_detectors × n_detectors` spillover matrix
/// from single-stain controls (traditional median-based compensation).
///
/// Convention matches [`invert_spillover`] / [`compensate_channels`]:
/// `S[i][j]` is the fraction of source `j`'s signal appearing in detector `i`,
/// so `measured[i] = Σ_j S[i][j] · true[j]`. Each control fills the column of
/// its `primary_index`: `col_i = median(pos_i) − median(neg_i)`, normalized by
/// the primary detector's entry so `S[j][j] = 1`. Detectors without a control
/// keep the identity column.
///
/// # Errors
/// - a control's `primary_index` is out of range, or its per-detector slices are
///   not length `n_detectors`;
/// - a control's primary-detector spillover (`pos − neg` median) is non-finite or
///   ~0, which would make the column unnormalizable (stain has no signal).
pub fn estimate_spillover(
    controls: &[SingleStainControl<'_>],
    n_detectors: usize,
) -> Result<Mat<f32>> {
    let mut s = Mat::<f32>::from_fn(n_detectors, n_detectors, |i, j| {
        if i == j { 1.0 } else { 0.0 }
    });

    for control in controls {
        let p = control.primary_index;
        anyhow::ensure!(
            p < n_detectors,
            "control primary_index {p} out of range for {n_detectors} detectors"
        );
        anyhow::ensure!(
            control.pos_per_detector.len() == n_detectors
                && control.neg_per_detector.len() == n_detectors,
            "control per-detector slices must be length {n_detectors} (got pos={}, neg={})",
            control.pos_per_detector.len(),
            control.neg_per_detector.len()
        );

        let mut column = vec![0.0f32; n_detectors];
        for i in 0..n_detectors {
            let pos = median_opt(&control.pos_per_detector[i]).unwrap_or(0.0);
            let neg = median_opt(&control.neg_per_detector[i]).unwrap_or(0.0);
            column[i] = pos - neg;
        }

        let primary = column[p];
        anyhow::ensure!(
            primary.is_finite() && primary.abs() > f32::EPSILON,
            "control at detector {p} has no usable positive signal (primary median delta = {primary})"
        );

        for (i, value) in column.iter().enumerate() {
            s[(i, p)] = value / primary;
        }
    }

    Ok(s)
}

/// Invert a spillover matrix using partial-pivot LU decomposition.
/// Returns the inverse matrix; errors if the matrix is singular.
pub fn invert_spillover(spillover: MatRef<'_, f32>) -> Result<Mat<f32>> {
    use faer::linalg::solvers::{DenseSolveCore, PartialPivLu};
    let lu = PartialPivLu::new(spillover);
    // Detect singular/ill-conditioned matrix via U diagonal
    let u = lu.U();
    for i in 0..u.nrows().min(u.ncols()) {
        if u[(i, i)].abs() < f32::EPSILON {
            anyhow::bail!(
                "spillover matrix is singular or ill-conditioned at diagonal index {i}"
            );
        }
    }
    Ok(lu.inverse())
}

/// Apply a pre-inverted compensation matrix to raw channel event data.
///
/// # Arguments
/// - `raw_channels`: slice of `(channel_name, events)` pairs involved in compensation.
/// - `comp_inv`: the already-inverted spillover matrix (n × n).
/// - `matrix_channel_names`: channel names corresponding to matrix rows/columns.
///
/// # Returns
/// Compensated events keyed by channel name, for every channel in `matrix_channel_names`
/// that was present in `raw_channels`.
pub fn apply_compensation_inv(
    raw_channels: &[(&str, &[f32])],
    comp_inv: MatRef<'_, f32>,
    matrix_channel_names: &[&str],
) -> Result<HashMap<String, Vec<f32>>> {
    use rayon::prelude::*;

    let n = matrix_channel_names.len();
    let raw_map: HashMap<&str, &[f32]> = raw_channels.iter().copied().collect();

    let channel_data: Vec<Option<&[f32]>> = matrix_channel_names
        .iter()
        .map(|&name| raw_map.get(name).copied())
        .collect();

    let n_events = channel_data
        .iter()
        .find_map(|c| c.map(|s| s.len()))
        .unwrap_or(0);

    // Verify all provided channel slices have the same event count
    for (i, opt) in channel_data.iter().enumerate() {
        if let Some(raw) = opt {
            anyhow::ensure!(
                raw.len() == n_events,
                "channel '{}' has {} events but expected {n_events}",
                matrix_channel_names[i],
                raw.len()
            );
        }
    }

    if n_events == 0 {
        return Ok(HashMap::new());
    }

    // result[i][event] = sum_j(comp_inv[i,j] * raw[j][event])
    let compensated: Vec<Vec<f32>> = (0..n)
        .into_par_iter()
        .map(|i| {
            let mut result = vec![0.0f32; n_events];
            for (event_idx, val) in result.iter_mut().enumerate() {
                let mut sum = 0.0f32;
                for j in 0..n {
                    if let Some(raw) = channel_data[j] {
                        sum += comp_inv[(i, j)] * raw[event_idx];
                    }
                }
                *val = sum;
            }
            result
        })
        .collect();

    let mut result = HashMap::new();
    for (i, name) in matrix_channel_names.iter().enumerate() {
        if raw_map.contains_key(name) {
            result.insert(name.to_string(), compensated[i].clone());
        }
    }
    Ok(result)
}

/// Convenience: invert spillover matrix then apply to raw channel data.
///
/// `channels_needed` filters the returned map — only these channels are in the result.
pub fn compensate_channels(
    raw_channels: &[(&str, &[f32])],
    spillover: MatRef<'_, f32>,
    matrix_channel_names: &[&str],
    channels_needed: &[&str],
) -> Result<HashMap<String, Vec<f32>>> {
    let comp_inv = invert_spillover(spillover)?;
    let all_compensated =
        apply_compensation_inv(raw_channels, comp_inv.as_ref(), matrix_channel_names)?;
    let needed_set: std::collections::HashSet<&str> = channels_needed.iter().copied().collect();
    Ok(all_compensated
        .into_iter()
        .filter(|(k, _)| needed_set.contains(k.as_str()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::mat;

    fn identity_2x2() -> Mat<f32> {
        mat![[1.0f32, 0.0], [0.0, 1.0]]
    }

    fn known_spillover_2x2() -> Mat<f32> {
        // Channel A spills 20% into B; B spills 0% into A
        // S[i,j] = fraction of channel j detected in channel i
        // So S[1,0] = 0.2 means 20% of A appears in B's detector
        mat![[1.0f32, 0.0], [0.2, 1.0]]
    }

    #[test]
    fn test_invert_identity_is_identity() {
        let m = identity_2x2();
        let inv = invert_spillover(m.as_ref()).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (inv[(i, j)] - expected).abs() < 1e-5,
                    "inv[{i},{j}] = {} expected {expected}",
                    inv[(i, j)]
                );
            }
        }
    }

    #[test]
    fn test_compensate_identity_returns_raw() {
        let m = identity_2x2();
        let ch_a: Vec<f32> = vec![1.0, 2.0, 3.0];
        let ch_b: Vec<f32> = vec![4.0, 5.0, 6.0];
        let raw = [("A", ch_a.as_slice()), ("B", ch_b.as_slice())];
        let names = ["A", "B"];
        let result = compensate_channels(&raw, m.as_ref(), &names, &names).unwrap();
        for (i, &v) in result["A"].iter().enumerate() {
            assert!((v - ch_a[i]).abs() < 1e-5);
        }
        for (i, &v) in result["B"].iter().enumerate() {
            assert!((v - ch_b[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn test_compensate_known_spillover_removes_spillover() {
        let spillover = known_spillover_2x2();
        let true_a: Vec<f32> = vec![100.0, 200.0];
        let true_b: Vec<f32> = vec![50.0, 80.0];
        let measured_a = true_a.clone();
        let measured_b: Vec<f32> = true_b
            .iter()
            .zip(true_a.iter())
            .map(|(b, a)| b + 0.2 * a)
            .collect();
        let raw = [("A", measured_a.as_slice()), ("B", measured_b.as_slice())];
        let names = ["A", "B"];
        let result = compensate_channels(&raw, spillover.as_ref(), &names, &names).unwrap();
        for (i, &v) in result["B"].iter().enumerate() {
            assert!(
                (v - true_b[i]).abs() < 1e-3,
                "compensated_b[{i}] = {v}, expected {}",
                true_b[i]
            );
        }
    }

    #[test]
    fn test_median_odd_even_empty() {
        assert!((median(&[3.0, 1.0, 2.0]) - 2.0).abs() < 1e-6);
        assert!((median(&[4.0, 1.0, 3.0, 2.0]) - 2.5).abs() < 1e-6);
        assert!(median(&[]).is_nan());
    }

    #[test]
    fn test_estimate_spillover_recovers_known_column() {
        // Source A (detector 0) spills 20% into detector B (index 1).
        // Single-stain A: positive events have A-detector median ~1000, B-detector
        // median ~200 (20%); negatives ~0. Single-stain B: only its own detector.
        let a_pos_det0 = vec![1000.0f32; 5];
        let a_pos_det1 = vec![200.0f32; 5];
        let b_pos_det0 = vec![0.0f32; 5];
        let b_pos_det1 = vec![800.0f32; 5];
        let neg = vec![0.0f32; 5];

        let ctrl_a = SingleStainControl {
            primary_index: 0,
            pos_per_detector: &[a_pos_det0, a_pos_det1],
            neg_per_detector: &[neg.clone(), neg.clone()],
        };
        let ctrl_b = SingleStainControl {
            primary_index: 1,
            pos_per_detector: &[b_pos_det0, b_pos_det1],
            neg_per_detector: &[neg.clone(), neg.clone()],
        };

        let s = estimate_spillover(&[ctrl_a, ctrl_b], 2).unwrap();
        // S[i][j] = fraction of source j in detector i; diagonal normalized to 1.
        assert!((s[(0, 0)] - 1.0).abs() < 1e-5, "S[0,0]={}", s[(0, 0)]);
        assert!((s[(1, 0)] - 0.2).abs() < 1e-5, "S[1,0]={}", s[(1, 0)]);
        assert!((s[(0, 1)] - 0.0).abs() < 1e-5, "S[0,1]={}", s[(0, 1)]);
        assert!((s[(1, 1)] - 1.0).abs() < 1e-5, "S[1,1]={}", s[(1, 1)]);
    }

    #[test]
    fn test_estimate_then_compensate_round_trip() {
        // Estimate S from single stains, then use it to compensate measured data.
        let neg = vec![0.0f32; 4];
        let ctrl_a = SingleStainControl {
            primary_index: 0,
            pos_per_detector: &[vec![1000.0; 4], vec![200.0; 4]],
            neg_per_detector: &[neg.clone(), neg.clone()],
        };
        let ctrl_b = SingleStainControl {
            primary_index: 1,
            pos_per_detector: &[vec![0.0; 4], vec![500.0; 4]],
            neg_per_detector: &[neg.clone(), neg.clone()],
        };
        let s = estimate_spillover(&[ctrl_a, ctrl_b], 2).unwrap();

        let true_a = vec![100.0f32, 200.0];
        let true_b = vec![50.0f32, 80.0];
        let measured_a = true_a.clone();
        let measured_b: Vec<f32> = true_b
            .iter()
            .zip(true_a.iter())
            .map(|(b, a)| b + 0.2 * a)
            .collect();
        let raw = [("A", measured_a.as_slice()), ("B", measured_b.as_slice())];
        let names = ["A", "B"];
        let result = compensate_channels(&raw, s.as_ref(), &names, &names).unwrap();
        for (i, &v) in result["B"].iter().enumerate() {
            assert!((v - true_b[i]).abs() < 1e-2, "comp_b[{i}]={v} want {}", true_b[i]);
        }
    }

    #[test]
    fn test_estimate_spillover_errors_on_dead_stain() {
        let neg = vec![0.0f32; 3];
        let ctrl = SingleStainControl {
            primary_index: 0,
            pos_per_detector: &[vec![0.0; 3], vec![0.0; 3]],
            neg_per_detector: &[neg.clone(), neg.clone()],
        };
        assert!(estimate_spillover(&[ctrl], 2).is_err());
    }

    #[test]
    fn test_channels_needed_filters_result() {
        let m = identity_2x2();
        let ch_a: Vec<f32> = vec![1.0];
        let ch_b: Vec<f32> = vec![2.0];
        let raw = [("A", ch_a.as_slice()), ("B", ch_b.as_slice())];
        let names = ["A", "B"];
        let result = compensate_channels(&raw, m.as_ref(), &names, &["A"]).unwrap();
        assert!(result.contains_key("A"));
        assert!(!result.contains_key("B"));
    }
}
