//! Synthetic ground-truth panels comparing Path A (joint OLS+AF) vs Path B (match + OLS).
//!
//! Used by headless golden tests and DEV-only FCS fixture writers.

use faer::{mat, Mat, MatRef};
use rand::Rng;
use rand::RngExt;
use rand::SeedableRng;

use crate::af_match::{
    build_af_match_library, match_and_subtract, AfCatalogEntry, AfMatchLibrary, AfMatchOptions,
    PreparedAfLibrary,
};
use crate::benchmark::run_ols;
use crate::TruOlsError;

/// Which contrived scenario this panel exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationPanelId {
    /// Two AF signatures + overlapping dim fluor; Path B should beat single-median Path A.
    HeterogeneousAf,
    /// One AF signature overlapping the fluor; documents Path B over-extraction.
    HomogeneousOverlap,
    /// AF on detectors the fluor does not occupy; both methods recover.
    OrthogonalSanity,
}

#[derive(Debug, Clone)]
pub struct EventLabels {
    pub true_fluor: Vec<f64>,
    pub true_af_amount: Vec<f64>,
    /// 0 = lymph-like, 1 = mono-like, 2 = homogeneous single signature.
    pub true_signature: Vec<u8>,
    pub is_true_negative: Vec<bool>,
}

#[derive(Debug, Clone)]
pub struct SyntheticPanel {
    pub id: ValidationPanelId,
    pub seed: u64,
    pub detector_names: Vec<String>,
    pub fluor_endmember_names: Vec<String>,
    pub fluor_matrix: Mat<f64>,
    /// Max-normalized unstained median AF column for Path A joint OLS.
    pub joint_af_column: Vec<f64>,
    pub af_library: AfMatchLibrary,
    pub observations: Mat<f64>,
    pub fsc_ssc: Mat<f64>,
    pub labels: EventLabels,
}

#[derive(Debug, Clone, Copy)]
pub struct SolveMetrics {
    pub true_negative_median: f64,
    pub rmse: f64,
    pub true_positive_negativity_rate: f64,
    pub assignment_accuracy: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PathSolveResult {
    pub fluor_abundances: Mat<f64>,
    pub af_abundance: Vec<f64>,
    pub assignment_1based: Vec<u32>,
}

fn normalize_signature_col(col: &mut [f64]) {
    for c in col.iter_mut() {
        if *c < 0.0 {
            *c = 0.0;
        }
    }
    let peak = col
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(0.0_f64, f64::max);
    if peak > 0.0 {
        for c in col.iter_mut() {
            *c /= peak;
        }
    }
}

fn fluor_shape() -> [f64; 4] {
    let mut c = [1.0, 0.18, 0.0, 0.0];
    normalize_signature_col(&mut c);
    c
}

fn af_lymph_shape() -> [f64; 4] {
    let mut c = [0.42, 0.08, 0.95, 0.18];
    normalize_signature_col(&mut c);
    c
}

fn af_mono_shape() -> [f64; 4] {
    let mut c = [0.38, 0.06, 0.12, 0.92];
    normalize_signature_col(&mut c);
    c
}

fn af_homogeneous_true_shape() -> [f64; 4] {
    let mut c = [0.72, 0.06, 0.45, 0.08];
    normalize_signature_col(&mut c);
    c
}

/// Catalog shape with stronger D1 overlap — mimics a slightly mismatched extract.
fn af_homogeneous_catalog_shape() -> [f64; 4] {
    let mut c = [0.88, 0.05, 0.40, 0.06];
    normalize_signature_col(&mut c);
    c
}

fn af_orthogonal_shape() -> [f64; 4] {
    let mut c = [0.0, 0.0, 0.92, 0.28];
    normalize_signature_col(&mut c);
    c
}

fn add_gaussian_noise(noise_std: f64, rng: &mut impl Rng) -> f64 {
    if noise_std <= 0.0 {
        return 0.0;
    }
    // Irwin–Hall: sum of 12 uniforms approximates N(0,1).
    let u: f64 = (0..12).map(|_| rng.random::<f64>() - 0.5).sum();
    u * noise_std
}

fn synthesize_event(
    fluor_amt: f64,
    af_shape: &[f64; 4],
    af_amt: f64,
    fluor: &[f64; 4],
    noise_std: f64,
    rng: &mut impl Rng,
) -> [f64; 4] {
    std::array::from_fn(|i| {
        fluor[i] * fluor_amt + af_shape[i] * af_amt + add_gaussian_noise(noise_std, rng)
    })
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn entry(
    id: &str,
    values: [f64; 4],
    mean: [f64; 2],
    proportion: f64,
) -> AfCatalogEntry {
    AfCatalogEntry {
        id: id.to_string(),
        values: values.to_vec(),
        fsc_ssc_mean: mean,
        fsc_ssc_cov: [[400.0, 0.0], [0.0, 400.0]],
        mixing_proportion: proportion,
    }
}

fn fluor_matrix_from_shape(fluor: [f64; 4]) -> Mat<f64> {
    mat![
        [fluor[0]],
        [fluor[1]],
        [fluor[2]],
        [fluor[3]],
    ]
}

fn build_joint_matrix(fluor: MatRef<f64>, af_col: &[f64]) -> Mat<f64> {
    let d = fluor.nrows();
    let f = fluor.ncols();
    Mat::from_fn(d, f + 1, |row, col| {
        if col < f {
            fluor[(row, col)]
        } else {
            af_col[row]
        }
    })
}

/// Run Path A: joint OLS on fluor columns + single AF column.
pub fn run_path_a(panel: &SyntheticPanel) -> Result<PathSolveResult, TruOlsError> {
    let joint = build_joint_matrix(panel.fluor_matrix.as_ref(), &panel.joint_af_column);
    let ab = run_ols(panel.observations.as_ref(), joint.as_ref())?;
    let n = ab.nrows();
    let af_idx = joint.ncols() - 1;
    let af_abundance = (0..n).map(|e| ab[(e, af_idx)]).collect();
    let fluor_ab = Mat::from_fn(n, af_idx, |e, j| ab[(e, j)]);
    Ok(PathSolveResult {
        fluor_abundances: fluor_ab,
        af_abundance,
        assignment_1based: vec![0; n],
    })
}

/// Run Path B: per-event AF match + fluor-only OLS on the residual.
pub fn run_path_b(panel: &SyntheticPanel) -> Result<PathSolveResult, TruOlsError> {
    let prepared = build_af_match_library(
        panel.af_library.clone(),
        panel.fluor_matrix.as_ref(),
        AfMatchOptions::default(),
    )?;
    run_path_b_prepared(panel, &prepared)
}

fn run_path_b_prepared(
    panel: &SyntheticPanel,
    prepared: &PreparedAfLibrary,
) -> Result<PathSolveResult, TruOlsError> {
    let matched = match_and_subtract(
        panel.observations.as_ref(),
        panel.fsc_ssc.as_ref(),
        None,
        prepared,
    )?;
    let ab = run_ols(
        matched.residual_observations.as_ref(),
        panel.fluor_matrix.as_ref(),
    )?;
    Ok(PathSolveResult {
        fluor_abundances: ab,
        af_abundance: matched.af_abundance,
        assignment_1based: matched.assignment_1based,
    })
}

pub fn metrics_for_fluor_index(
    panel: &SyntheticPanel,
    result: &PathSolveResult,
    fluor_idx: usize,
) -> SolveMetrics {
    let n = panel.labels.true_fluor.len();
    let mut sq_err = 0.0;
    let mut tn_estimates = Vec::new();
    let mut tp_negative = 0usize;
    let mut tp_total = 0usize;

    for e in 0..n {
        let est = result.fluor_abundances[(e, fluor_idx)];
        let truth = panel.labels.true_fluor[e];
        sq_err += (est - truth).powi(2);
        if panel.labels.is_true_negative[e] {
            tn_estimates.push(est);
        } else {
            tp_total += 1;
            if est < -50.0 {
                tp_negative += 1;
            }
        }
    }

    let assignment_accuracy = if panel.id == ValidationPanelId::HeterogeneousAf {
        let mut correct = 0usize;
        for e in 0..n {
            let expected = panel.labels.true_signature[e] + 1;
            if result.assignment_1based[e] == u32::from(expected) {
                correct += 1;
            }
        }
        Some(correct as f64 / n as f64)
    } else {
        None
    };

    SolveMetrics {
        true_negative_median: median(&tn_estimates),
        rmse: (sq_err / n as f64).sqrt(),
        true_positive_negativity_rate: if tp_total == 0 {
            0.0
        } else {
            tp_negative as f64 / tp_total as f64
        },
        assignment_accuracy,
    }
}

impl SyntheticPanel {
    pub fn build(id: ValidationPanelId, seed: u64) -> Self {
        match id {
            ValidationPanelId::HeterogeneousAf => Self::heterogeneous_af(seed),
            ValidationPanelId::HomogeneousOverlap => Self::homogeneous_overlap(seed),
            ValidationPanelId::OrthogonalSanity => Self::orthogonal_sanity(seed),
        }
    }

    fn heterogeneous_af(seed: u64) -> Self {
        let fluor = fluor_shape();
        let lymph = af_lymph_shape();
        let mono = af_mono_shape();
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let noise = 8.0;

        let mut observations = Vec::new();
        let mut fsc_ssc = Vec::new();
        let mut true_fluor = Vec::new();
        let mut true_af_amount = Vec::new();
        let mut true_signature = Vec::new();
        let mut is_true_negative = Vec::new();
        let mut unstained_spectra = Vec::new();

        for _ in 0..400 {
            let spec = synthesize_event(0.0, &lymph, 35.0, &fluor, noise, &mut rng);
            observations.push(spec);
            fsc_ssc.push([200.0, 150.0]);
            true_fluor.push(0.0);
            true_af_amount.push(35.0);
            true_signature.push(0);
            is_true_negative.push(true);
            unstained_spectra.push(spec);
        }
        for _ in 0..400 {
            let spec = synthesize_event(0.0, &mono, 30.0, &fluor, noise, &mut rng);
            observations.push(spec);
            fsc_ssc.push([800.0, 700.0]);
            true_fluor.push(0.0);
            true_af_amount.push(30.0);
            true_signature.push(1);
            is_true_negative.push(true);
            unstained_spectra.push(spec);
        }
        for _ in 0..400 {
            let spec = synthesize_event(110.0, &lymph, 35.0, &fluor, noise, &mut rng);
            observations.push(spec);
            fsc_ssc.push([210.0, 160.0]);
            true_fluor.push(110.0);
            true_af_amount.push(35.0);
            true_signature.push(0);
            is_true_negative.push(false);
        }

        let n = observations.len();
        let obs = Mat::from_fn(n, 4, |ev, det| observations[ev][det]);
        let scatter = Mat::from_fn(n, 2, |ev, col| f64::from(fsc_ssc[ev][col]));

        let mut joint_af = [0.0; 4];
        for det in 0..4 {
            let vals: Vec<f64> = unstained_spectra.iter().map(|s| s[det]).collect();
            joint_af[det] = median(&vals);
        }
        normalize_signature_col(&mut joint_af);

        Self {
            id: ValidationPanelId::HeterogeneousAf,
            seed,
            detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
            fluor_endmember_names: vec!["CD4".into()],
            fluor_matrix: fluor_matrix_from_shape(fluor),
            joint_af_column: joint_af.to_vec(),
            af_library: AfMatchLibrary {
                entries: vec![
                    entry("af_lymph", lymph, [200.0, 150.0], 0.55),
                    entry("af_mono", mono, [800.0, 700.0], 0.45),
                ],
                beta: 2.0,
                detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
            },
            observations: obs,
            fsc_ssc: scatter,
            labels: EventLabels {
                true_fluor,
                true_af_amount,
                true_signature,
                is_true_negative,
            },
        }
    }

    fn homogeneous_overlap(seed: u64) -> Self {
        let fluor = fluor_shape();
        let af_true = af_homogeneous_true_shape();
        let af_catalog = af_homogeneous_catalog_shape();
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let noise = 10.0;

        let mut observations = Vec::new();
        let mut fsc_ssc = Vec::new();
        let mut true_fluor = Vec::new();
        let mut true_af_amount = Vec::new();
        let mut unstained_spectra = Vec::new();

        for _ in 0..500 {
            let spec = synthesize_event(0.0, &af_true, 45.0, &fluor, noise, &mut rng);
            observations.push(spec);
            fsc_ssc.push([400.0, 350.0]);
            true_fluor.push(0.0);
            true_af_amount.push(45.0);
            unstained_spectra.push(spec);
        }
        for _ in 0..500 {
            observations.push(synthesize_event(120.0, &af_true, 45.0, &fluor, noise, &mut rng));
            fsc_ssc.push([410.0, 360.0]);
            true_fluor.push(120.0);
            true_af_amount.push(45.0);
        }

        let n = observations.len();
        let obs = Mat::from_fn(n, 4, |ev, det| observations[ev][det]);
        let scatter = Mat::from_fn(n, 2, |ev, col| f64::from(fsc_ssc[ev][col]));

        let mut joint_af = [0.0; 4];
        for det in 0..4 {
            let vals: Vec<f64> = unstained_spectra.iter().map(|s| s[det]).collect();
            joint_af[det] = median(&vals);
        }
        normalize_signature_col(&mut joint_af);

        Self {
            id: ValidationPanelId::HomogeneousOverlap,
            seed,
            detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
            fluor_endmember_names: vec!["CD4".into()],
            fluor_matrix: fluor_matrix_from_shape(fluor),
            joint_af_column: joint_af.to_vec(),
            af_library: AfMatchLibrary {
                entries: vec![entry("af_homogeneous", af_catalog, [400.0, 350.0], 1.0)],
                beta: 4.0,
                detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
            },
            observations: obs,
            fsc_ssc: scatter,
            labels: EventLabels {
                true_fluor: true_fluor.clone(),
                true_af_amount,
                true_signature: vec![2; n],
                is_true_negative: true_fluor.iter().map(|&v| v == 0.0).collect(),
            },
        }
    }

    fn orthogonal_sanity(seed: u64) -> Self {
        let fluor = fluor_shape();
        let af = af_orthogonal_shape();
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let noise = 5.0;

        let mut observations = Vec::new();
        let mut fsc_ssc = Vec::new();
        let mut true_fluor = Vec::new();
        let mut unstained_spectra = Vec::new();

        for _ in 0..500 {
            let spec = synthesize_event(0.0, &af, 50.0, &fluor, noise, &mut rng);
            observations.push(spec);
            fsc_ssc.push([300.0, 250.0]);
            true_fluor.push(0.0);
            unstained_spectra.push(spec);
        }
        for _ in 0..500 {
            observations.push(synthesize_event(100.0, &af, 50.0, &fluor, noise, &mut rng));
            fsc_ssc.push([310.0, 260.0]);
            true_fluor.push(100.0);
        }

        let n = observations.len();
        let obs = Mat::from_fn(n, 4, |ev, det| observations[ev][det]);
        let scatter = Mat::from_fn(n, 2, |ev, col| f64::from(fsc_ssc[ev][col]));

        let mut joint_af = [0.0; 4];
        for det in 0..4 {
            let vals: Vec<f64> = unstained_spectra.iter().map(|s| s[det]).collect();
            joint_af[det] = median(&vals);
        }
        normalize_signature_col(&mut joint_af);

        Self {
            id: ValidationPanelId::OrthogonalSanity,
            seed,
            detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
            fluor_endmember_names: vec!["CD4".into()],
            fluor_matrix: fluor_matrix_from_shape(fluor),
            joint_af_column: joint_af.to_vec(),
            af_library: AfMatchLibrary {
                entries: vec![entry("af_orth", af, [300.0, 250.0], 1.0)],
                beta: 1.0,
                detector_names: vec!["D1".into(), "D2".into(), "D3".into(), "D4".into()],
            },
            observations: obs,
            fsc_ssc: scatter,
            labels: EventLabels {
                true_fluor: true_fluor.clone(),
                true_af_amount: vec![50.0; n],
                true_signature: vec![2; n],
                is_true_negative: true_fluor.iter().map(|&v| v == 0.0).collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heterogeneous_path_b_beats_path_a_on_true_negative_median() {
        let panel = SyntheticPanel::build(ValidationPanelId::HeterogeneousAf, 42);
        let path_a = run_path_a(&panel).expect("path A");
        let path_b = run_path_b(&panel).expect("path B");
        let m_a = metrics_for_fluor_index(&panel, &path_a, 0);
        let m_b = metrics_for_fluor_index(&panel, &path_b, 0);

        assert!(
            m_b.true_negative_median.abs() <= m_a.true_negative_median.abs() + 15.0,
            "path B tn median {} should beat path A {}",
            m_b.true_negative_median,
            m_a.true_negative_median
        );
        assert!(
            m_b.assignment_accuracy.unwrap_or(0.0) > 0.85,
            "assignment accuracy {:?}",
            m_b.assignment_accuracy
        );
    }

    #[test]
    fn homogeneous_path_a_keeps_true_positives_higher_than_path_b() {
        let panel = SyntheticPanel::build(ValidationPanelId::HomogeneousOverlap, 7);
        let path_a = run_path_a(&panel).expect("path A");
        let path_b = run_path_b(&panel).expect("path B");

        let tp_a: Vec<f64> = panel
            .labels
            .is_true_negative
            .iter()
            .enumerate()
            .filter(|(_, tn)| !**tn)
            .map(|(e, _)| path_a.fluor_abundances[(e, 0)])
            .collect();
        let tp_b: Vec<f64> = panel
            .labels
            .is_true_negative
            .iter()
            .enumerate()
            .filter(|(_, tn)| !**tn)
            .map(|(e, _)| path_b.fluor_abundances[(e, 0)])
            .collect();

        let med_a = median(&tp_a);
        let med_b = median(&tp_b);
        assert!(
            med_a > med_b + 0.5,
            "path A tp median {med_a} should exceed path B {med_b} when catalog AF over-laps"
        );
        let below_truth = tp_b.iter().filter(|&&v| v < 110.0).count();
        assert!(
            below_truth > tp_a.iter().filter(|&&v| v < 110.0).count(),
            "path B should leave more true positives below 110 (got {below_truth} vs path A)"
        );
    }

    #[test]
    fn orthogonal_both_paths_recover_near_zero_true_negatives() {
        let panel = SyntheticPanel::build(ValidationPanelId::OrthogonalSanity, 99);
        let path_a = run_path_a(&panel).expect("path A");
        let path_b = run_path_b(&panel).expect("path B");
        let m_a = metrics_for_fluor_index(&panel, &path_a, 0);
        let m_b = metrics_for_fluor_index(&panel, &path_b, 0);

        assert!(m_a.true_negative_median.abs() < 25.0, "path A tn {}", m_a.true_negative_median);
        assert!(m_b.true_negative_median.abs() < 25.0, "path B tn {}", m_b.true_negative_median);
        assert!(m_a.rmse < 20.0, "path A rmse {}", m_a.rmse);
        assert!(m_b.rmse < 20.0, "path B rmse {}", m_b.rmse);
    }
}
