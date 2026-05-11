use anyhow::Result;
use faer::{Mat, MatRef};
use std::collections::HashMap;

/// Invert a spillover matrix using partial-pivot LU decomposition.
/// Returns the inverse matrix; errors if the matrix is singular.
pub fn invert_spillover(spillover: MatRef<'_, f32>) -> Result<Mat<f32>> {
    use faer::linalg::solvers::{DenseSolveCore, PartialPivLu};
    let lu = PartialPivLu::new(spillover);
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
