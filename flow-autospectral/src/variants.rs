//! Spectral variants from single-stained positives (SOM + cosine QC).
//!
//! Algorithm: brightest non-AF events in the peak detector (capped at
//! `n_cells`), optional scatter-matched background subtraction, batch SOM,
//! drop codebook nodes whose cosine to the master spectrum is below
//! `sim_threshold`. Deltas (variant − master) feed the joint-unmix covariance
//! ridge. See Burton *et al.*, *bioRxiv* 2025.10.27.684855.

#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

use crate::config::{VariantDiscoverConfig, quantile_type7};
use crate::error::{AutospectralError, Result};
use crate::library::{cosine_similarity, normalize_unit_peak};
use crate::unmix_ols::unmix_events_ols;
use faer::{Mat, MatRef};
use flow_clustering::{Som, SomConfig};
use flow_knn::AnnIndex;
use ndarray::Array2;
use std::collections::HashMap;

/// Per-fluorophore variant library for [`crate::unmix_autospectral_joint`].
///
/// Matrices are detectors × variants (column-major), matching [`crate::AfLibrary`].
#[derive(Debug, Clone)]
pub struct SpectralVariants {
    /// Positivity thresholds in unmixed space (99.5th of unstained), one per fluor.
    pub thresholds: Vec<f64>,
    pub fluor_names: Vec<String>,
    /// Variant spectra keyed by fluorophore name. Empty map → AF-only joint path.
    pub variants: HashMap<String, Mat<f64>>,
    /// Variant-minus-master observations for the leakage covariance ridge.
    pub deltas: HashMap<String, Mat<f64>>,
}

impl SpectralVariants {
    /// AF-only path: no fluorophore variants, thresholds still required.
    pub fn af_only(fluor_names: Vec<String>, thresholds: Vec<f64>) -> Result<Self> {
        if fluor_names.len() != thresholds.len() {
            return Err(AutospectralError::InvalidConfig(
                "fluor_names and thresholds must have the same length".into(),
            ));
        }
        Ok(Self {
            thresholds,
            fluor_names,
            variants: HashMap::new(),
            deltas: HashMap::new(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.variants.values().all(|m| m.ncols() == 0)
    }

    pub fn n_variants_mean(&self) -> f64 {
        if self.fluor_names.is_empty() {
            return 0.0;
        }
        let sum: usize = self
            .fluor_names
            .iter()
            .map(|n| self.variants.get(n).map(Mat::ncols).unwrap_or(0))
            .sum();
        sum as f64 / self.fluor_names.len() as f64
    }
}

/// One single-stained control for variant discovery.
#[derive(Debug, Clone, Copy)]
pub struct FluorControl<'a> {
    pub name: &'a str,
    /// Row-major `n_events × n_detectors` fluorescence.
    pub events_row_major: &'a [f64],
    pub n_events: usize,
    /// Optional row-major scatter (`n_events × scatter_dims`) for `k.neighbors`.
    pub scatter_row_major: Option<&'a [f64]>,
}

/// Discover SOM variants from single-stained positives.
///
/// `fluor_matrix` is detectors × fluorophores; column `j` is the master spectrum
/// for `fluor_names[j]`. Controls need not cover every fluor: missing controls
/// keep the master as a single variant.
pub fn discover_spectral_variants(
    controls: &[FluorControl<'_>],
    unstained_row_major: &[f64],
    n_unstained: usize,
    n_detectors: usize,
    fluor_matrix: MatRef<'_, f64>,
    fluor_names: &[String],
    unstained_scatter: Option<&[f64]>,
    scatter_dims: usize,
    config: &VariantDiscoverConfig,
) -> Result<SpectralVariants> {
    validate_discover_inputs(
        unstained_row_major,
        n_unstained,
        n_detectors,
        fluor_matrix,
        fluor_names,
        unstained_scatter,
        scatter_dims,
        config,
    )?;

    let n_fluor = fluor_names.len();
    let unmixed_u = unmix_events_ols(fluor_matrix, unstained_row_major, n_unstained)?;
    let mut thresholds = vec![0.0; n_fluor];
    for f in 0..n_fluor {
        let col: Vec<f64> = (0..n_unstained)
            .map(|e| unmixed_u[e * n_fluor + f])
            .collect();
        thresholds[f] = quantile_type7(&col, config.positivity_quantile);
    }

    let mut variants = HashMap::new();
    let mut deltas = HashMap::new();
    let control_by_name: HashMap<&str, &FluorControl<'_>> =
        controls.iter().map(|c| (c.name, c)).collect();

    for (f, name) in fluor_names.iter().enumerate() {
        let mut master = vec![0.0; n_detectors];
        for d in 0..n_detectors {
            master[d] = fluor_matrix[(d, f)];
        }
        normalize_unit_peak(&mut master);

        let (vmat, dmat) = if let Some(ctrl) = control_by_name.get(name.as_str()) {
            variants_for_fluor(
                ctrl,
                &master,
                unstained_row_major,
                n_unstained,
                n_detectors,
                unstained_scatter,
                scatter_dims,
                config,
            )?
        } else {
            master_only_mats(&master)
        };
        variants.insert(name.clone(), vmat);
        deltas.insert(name.clone(), dmat);
    }

    Ok(SpectralVariants {
        thresholds,
        fluor_names: fluor_names.to_vec(),
        variants,
        deltas,
    })
}

fn validate_discover_inputs(
    unstained_row_major: &[f64],
    n_unstained: usize,
    n_detectors: usize,
    fluor_matrix: MatRef<'_, f64>,
    fluor_names: &[String],
    unstained_scatter: Option<&[f64]>,
    scatter_dims: usize,
    config: &VariantDiscoverConfig,
) -> Result<()> {
    if n_unstained == 0 {
        return Err(AutospectralError::EmptyEvents);
    }
    if n_detectors == 0 || unstained_row_major.len() != n_unstained * n_detectors {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_detectors,
            got: unstained_row_major
                .len()
                .checked_div(n_unstained.max(1))
                .unwrap_or(0),
        });
    }
    if fluor_matrix.nrows() != n_detectors {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_detectors,
            got: fluor_matrix.nrows(),
        });
    }
    if fluor_names.len() != fluor_matrix.ncols() {
        return Err(AutospectralError::InvalidConfig(format!(
            "fluor_names length {} != mixing columns {}",
            fluor_names.len(),
            fluor_matrix.ncols()
        )));
    }
    if config.n_cells == 0 || config.som_width == 0 || config.som_height == 0 {
        return Err(AutospectralError::InvalidConfig(
            "n_cells, som_width, and som_height must be >= 1".into(),
        ));
    }
    if !(0.0..=1.0).contains(&config.sim_threshold) {
        return Err(AutospectralError::InvalidConfig(
            "sim_threshold must be in [0, 1]".into(),
        ));
    }
    if !(0.0..=1.0).contains(&config.positivity_quantile) {
        return Err(AutospectralError::InvalidConfig(
            "positivity_quantile must be in [0, 1]".into(),
        ));
    }
    if config.k_neighbors == 0 {
        return Err(AutospectralError::InvalidConfig(
            "k_neighbors must be >= 1".into(),
        ));
    }
    if let Some(sc) = unstained_scatter
        && (scatter_dims == 0 || sc.len() != n_unstained * scatter_dims)
    {
        return Err(AutospectralError::DetectorMismatch {
            expected: scatter_dims,
            got: sc.len().checked_div(n_unstained.max(1)).unwrap_or(0),
        });
    }
    Ok(())
}

fn master_only_mats(master: &[f64]) -> (Mat<f64>, Mat<f64>) {
    let d = master.len();
    let mut variants = Mat::<f64>::zeros(d, 1);
    let deltas = Mat::<f64>::zeros(d, 1);
    for i in 0..d {
        variants[(i, 0)] = master[i];
    }
    (variants, deltas)
}

fn variants_for_fluor(
    ctrl: &FluorControl<'_>,
    master: &[f64],
    unstained: &[f64],
    n_unstained: usize,
    n_detectors: usize,
    unstained_scatter: Option<&[f64]>,
    scatter_dims: usize,
    config: &VariantDiscoverConfig,
) -> Result<(Mat<f64>, Mat<f64>)> {
    if ctrl.n_events == 0 || ctrl.events_row_major.len() != ctrl.n_events * n_detectors {
        return Err(AutospectralError::DetectorMismatch {
            expected: n_detectors,
            got: ctrl
                .events_row_major
                .len()
                .checked_div(ctrl.n_events.max(1))
                .unwrap_or(0),
        });
    }

    let peak = peak_detector(master);
    let unstained_peak: Vec<f64> = (0..n_unstained)
        .map(|e| unstained[e * n_detectors + peak])
        .collect();
    let raw_thresh = quantile_type7(&unstained_peak, config.positivity_quantile);

    let mut pos: Vec<(usize, f64)> = (0..ctrl.n_events)
        .map(|e| {
            let v = ctrl.events_row_major[e * n_detectors + peak];
            (e, v)
        })
        .filter(|(_, v)| *v > raw_thresh)
        .collect();
    pos.sort_by(|a, b| b.1.total_cmp(&a.1));
    pos.truncate(config.n_cells);

    if pos.len() < 20 {
        return Ok(master_only_mats(master));
    }

    let mut selected = Vec::with_capacity(pos.len() * n_detectors);
    for &(e, _) in &pos {
        selected.extend_from_slice(&ctrl.events_row_major[e * n_detectors..(e + 1) * n_detectors]);
    }

    if let (Some(u_sc), Some(c_sc)) = (unstained_scatter, ctrl.scatter_row_major) {
        subtract_scatter_background(
            &mut selected,
            pos.len(),
            n_detectors,
            &pos,
            c_sc,
            ctrl.n_events,
            u_sc,
            n_unstained,
            scatter_dims,
            unstained,
            config,
        )?;
    }

    let n_sel = pos.len();
    let mut som_rows = Vec::with_capacity(n_sel * n_detectors);
    let mut kept = 0usize;
    for e in 0..n_sel {
        let mut row = selected[e * n_detectors..(e + 1) * n_detectors].to_vec();
        normalize_unit_peak(&mut row);
        if cosine_similarity(&row, master) < config.sim_threshold {
            continue;
        }
        som_rows.extend_from_slice(&row);
        kept += 1;
    }
    if kept < 20 {
        return Ok(master_only_mats(master));
    }

    let mut width = config.som_width;
    let mut height = config.som_height;
    if kept < 500 {
        let side = ((kept as f64 / 3.0).sqrt().floor() as usize).max(2);
        width = side.min(width);
        height = side.min(height);
    }

    let arr = Array2::from_shape_vec((kept, n_detectors), som_rows).map_err(|e| {
        AutospectralError::InvalidConfig(format!("SOM input shape: {e}"))
    })?;
    let som = Som::fit(
        &arr,
        &SomConfig {
            width,
            height,
            n_epochs: config.som_n_epochs.max(1),
            radius: config.som_radius,
            seed: config.seed,
        },
    )
    .map_err(|e| AutospectralError::Clustering(e.to_string()))?;

    let mut kept_spectra: Vec<Vec<f64>> = Vec::new();
    for r in 0..som.weights.nrows() {
        let mut spec: Vec<f64> = (0..n_detectors).map(|c| som.weights[(r, c)]).collect();
        normalize_unit_peak(&mut spec);
        if cosine_similarity(&spec, master) < config.sim_threshold {
            continue;
        }
        if let Some(blend) = config.off_peak_blend {
            for (d, v) in spec.iter_mut().enumerate() {
                if master[d] <= config.off_peak_master_min {
                    *v = blend * *v + (1.0 - blend) * master[d];
                }
            }
        }
        kept_spectra.push(spec);
    }

    if kept_spectra.is_empty() {
        return Ok(master_only_mats(master));
    }

    let n_v = kept_spectra.len();
    let mut vmat = Mat::<f64>::zeros(n_detectors, n_v);
    let mut dmat = Mat::<f64>::zeros(n_detectors, n_v);
    for (j, spec) in kept_spectra.iter().enumerate() {
        for d in 0..n_detectors {
            vmat[(d, j)] = spec[d];
            dmat[(d, j)] = spec[d] - master[d];
        }
    }
    Ok((vmat, dmat))
}

fn peak_detector(master: &[f64]) -> usize {
    master
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn subtract_scatter_background(
    selected: &mut [f64],
    n_sel: usize,
    n_detectors: usize,
    pos: &[(usize, f64)],
    ctrl_scatter: &[f64],
    n_ctrl: usize,
    unstained_scatter: &[f64],
    n_unstained: usize,
    scatter_dims: usize,
    unstained_fluor: &[f64],
    config: &VariantDiscoverConfig,
) -> Result<()> {
    if scatter_dims == 0 {
        return Ok(());
    }
    if ctrl_scatter.len() != n_ctrl * scatter_dims {
        return Err(AutospectralError::DetectorMismatch {
            expected: scatter_dims,
            got: ctrl_scatter.len().checked_div(n_ctrl.max(1)).unwrap_or(0),
        });
    }
    let db: Vec<f32> = unstained_scatter.iter().map(|&x| x as f32).collect();
    let index = AnnIndex::build(
        &db,
        n_unstained,
        scatter_dims,
        &config.knn_method,
        config.metric,
    )
    .map_err(|e| AutospectralError::Knn(e.to_string()))?;
    let mut queries = Vec::with_capacity(n_sel * scatter_dims);
    for &(e, _) in pos {
        let start = e * scatter_dims;
        for &v in &ctrl_scatter[start..start + scatter_dims] {
            queries.push(v as f32);
        }
    }
    let k = config.k_neighbors.min(n_unstained);
    let nbrs = index
        .search_batch(&queries, n_sel, k)
        .map_err(|e| AutospectralError::Knn(e.to_string()))?;
    for (i, list) in nbrs.iter().enumerate() {
        if list.indices.is_empty() {
            continue;
        }
        let inv = 1.0 / list.indices.len() as f64;
        for d in 0..n_detectors {
            let mut bg = 0.0;
            for &idx in &list.indices {
                bg += unstained_fluor[idx as usize * n_detectors + d];
            }
            selected[i * n_detectors + d] -= bg * inv;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::normalize_unit_peak;
    use faer::Mat;

    fn unit_peak_col(d: usize, peak: usize) -> Vec<f64> {
        let mut v = vec![0.05; d];
        if peak < d {
            v[peak] = 1.0;
        }
        normalize_unit_peak(&mut v);
        v
    }

    #[test]
    fn af_only_constructor_rejects_length_mismatch() {
        let err = SpectralVariants::af_only(vec!["A".into()], vec![0.0, 1.0]);
        assert!(err.is_err());
    }

    #[test]
    fn missing_control_keeps_master_as_single_variant() {
        let d = 4;
        let mut fluor = Mat::<f64>::zeros(d, 1);
        let col = unit_peak_col(d, 1);
        for i in 0..d {
            fluor[(i, 0)] = col[i];
        }
        let n_u = 40;
        let mut unstained = Vec::with_capacity(n_u * d);
        for e in 0..n_u {
            for i in 0..d {
                unstained.push(10.0 + (e % 3) as f64 + i as f64 * 0.1);
            }
        }
        let names = vec!["FITC".to_string()];
        let out = discover_spectral_variants(
            &[],
            &unstained,
            n_u,
            d,
            fluor.as_ref(),
            &names,
            None,
            0,
            &VariantDiscoverConfig::default(),
        )
        .expect("discover");
        assert_eq!(out.variants["FITC"].ncols(), 1);
        for i in 0..d {
            assert!((out.variants["FITC"][(i, 0)] - col[i]).abs() < 1e-12);
            assert!(out.deltas["FITC"][(i, 0)].abs() < 1e-12);
        }
    }

    #[test]
    fn cosine_qc_keeps_near_master_nodes() {
        let d = 6;
        let peak = 2;
        let mut fluor = Mat::<f64>::zeros(d, 1);
        let master = unit_peak_col(d, peak);
        for i in 0..d {
            fluor[(i, 0)] = master[i];
        }
        let n_u = 80;
        let mut unstained = vec![0.0; n_u * d];
        for e in 0..n_u {
            for i in 0..d {
                unstained[e * d + i] = 20.0 + (e as f64) * 0.01;
            }
        }
        // Bright positives around the master, plus a few junk events.
        let n_s = 120;
        let mut stained = Vec::with_capacity(n_s * d);
        for e in 0..n_s {
            for i in 0..d {
                let mut v = master[i] * (800.0 + (e % 7) as f64);
                if e > 100 {
                    v = if i == 0 { 900.0 } else { 5.0 };
                }
                stained.push(v);
            }
        }
        let name = "PE".to_string();
        let ctrl = FluorControl {
            name: &name,
            events_row_major: &stained,
            n_events: n_s,
            scatter_row_major: None,
        };
        let cfg = VariantDiscoverConfig {
            n_cells: 80,
            som_width: 4,
            som_height: 4,
            som_n_epochs: 6,
            sim_threshold: 0.985,
            ..VariantDiscoverConfig::default()
        };
        let out = discover_spectral_variants(
            &[ctrl],
            &unstained,
            n_u,
            d,
            fluor.as_ref(),
            std::slice::from_ref(&name),
            None,
            0,
            &cfg,
        )
        .expect("discover");
        let v = &out.variants[&name];
        assert!(v.ncols() >= 1);
        for j in 0..v.ncols() {
            let col: Vec<f64> = (0..d).map(|i| v[(i, j)]).collect();
            assert!(
                cosine_similarity(&col, &master) >= 0.985 - 1e-9,
                "variant {j} cosine too low"
            );
        }
        assert_eq!(out.thresholds.len(), 1);
    }
}
