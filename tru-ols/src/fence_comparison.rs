use crate::error::TruOlsError;
use crate::spreading::{MIN_SPREAD_EVENTS, quantile, robust_rsd_asinh};
use faer::{ColRef, MatRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceRankingStatus {
    Eligible,
    TradeOff,
    Worse,
    NoClearDifference,
    CouldNotEvaluate,
}

#[derive(Debug, Clone)]
pub struct FenceComparisonThresholds {
    pub min_median_bright_tail_retention: f64,
    pub max_worst_bright_tail_loss: f64,
    pub max_residual_rmse_increase: f64,
    pub max_worst_negative_rsd_regression: f64,
    pub rsd_equivalence_band: f64,
}

impl Default for FenceComparisonThresholds {
    fn default() -> Self {
        Self {
            min_median_bright_tail_retention: 0.98,
            max_worst_bright_tail_loss: 0.05,
            max_residual_rmse_increase: 0.05,
            max_worst_negative_rsd_regression: 0.10,
            rsd_equivalence_band: 0.02,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FenceFluorComparison {
    pub endmember_name: String,
    pub baseline_rsd: Option<f64>,
    pub candidate_rsd: Option<f64>,
    pub rsd_improvement: Option<f64>,
    pub bright_tail_retention: Option<f64>,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FenceComparisonMetrics {
    pub status: FenceRankingStatus,
    pub eligible: bool,
    pub median_negative_rsd_improvement: Option<f64>,
    pub worst_negative_rsd_regression: Option<f64>,
    pub median_bright_tail_retention: Option<f64>,
    pub worst_bright_tail_loss: Option<f64>,
    pub residual_rmse_increase: Option<f64>,
    pub candidate_truncation_fraction: f64,
    pub skipped_fluors: usize,
    pub blocking_reason: Option<String>,
    pub fluor_rows: Vec<FenceFluorComparison>,
}

pub struct FenceComparisonInput<'a> {
    pub endmember_names: &'a [String],
    pub baseline_abundances: MatRef<'a, f64>,
    pub candidate_abundances: MatRef<'a, f64>,
    pub baseline_residual_norms: &'a [f64],
    pub candidate_residual_norms: &'a [f64],
    pub candidate_any_truncated: &'a [bool],
    pub negative_masks: &'a [Vec<bool>],
    pub baseline_cutoffs: ColRef<'a, f64>,
    pub cofactor: f64,
}

pub fn compare_fence_outputs(
    input: FenceComparisonInput<'_>,
    thresholds: &FenceComparisonThresholds,
) -> Result<FenceComparisonMetrics, TruOlsError> {
    validate_input(&input, thresholds)?;

    let n_events = input.baseline_abundances.nrows();
    let mut fluor_rows = Vec::with_capacity(input.endmember_names.len());
    let mut rsd_improvements = Vec::new();
    let mut bright_tail_retentions = Vec::new();
    let mut skipped_fluors = 0;

    for fluor_index in 0..input.endmember_names.len() {
        let mut skip_reasons = Vec::new();
        let negative_indices = input.negative_masks[fluor_index]
            .iter()
            .enumerate()
            .filter_map(|(event_index, selected)| selected.then_some(event_index))
            .collect::<Vec<_>>();

        let (baseline_rsd, candidate_rsd, rsd_improvement) = if negative_indices.len()
            < MIN_SPREAD_EVENTS
        {
            skip_reasons.push(format!(
                "need at least {MIN_SPREAD_EVENTS} negative events, got {}",
                negative_indices.len()
            ));
            (None, None, None)
        } else {
            let baseline_values = negative_indices
                .iter()
                .map(|&event_index| input.baseline_abundances[(event_index, fluor_index)])
                .collect::<Vec<_>>();
            let candidate_values = negative_indices
                .iter()
                .map(|&event_index| input.candidate_abundances[(event_index, fluor_index)])
                .collect::<Vec<_>>();
            let baseline_rsd = robust_rsd_asinh(&baseline_values, input.cofactor);
            let candidate_rsd = robust_rsd_asinh(&candidate_values, input.cofactor);
            let improvement = baseline_rsd
                .zip(candidate_rsd)
                .and_then(|(baseline, candidate)| {
                    safe_relative_change(baseline - candidate, baseline)
                });
            if improvement.is_none() {
                skip_reasons.push(
                    "negative rSD unavailable because spread denominator is zero or non-finite"
                        .into(),
                );
            }
            (baseline_rsd, candidate_rsd, improvement)
        };
        if let Some(improvement) = rsd_improvement {
            rsd_improvements.push(improvement);
        }

        let baseline_column = (0..n_events)
            .map(|event_index| input.baseline_abundances[(event_index, fluor_index)])
            .collect::<Vec<_>>();
        let baseline_p95 = quantile(&baseline_column, 0.95);
        let bright_tail_retention = match baseline_p95 {
            Some(p95) if p95 > input.baseline_cutoffs[fluor_index] => {
                let tail_indices = baseline_column
                    .iter()
                    .enumerate()
                    .filter_map(|(event_index, value)| (*value >= p95).then_some(event_index))
                    .collect::<Vec<_>>();
                let baseline_tail = tail_indices
                    .iter()
                    .map(|&event_index| input.baseline_abundances[(event_index, fluor_index)])
                    .collect::<Vec<_>>();
                let candidate_tail = tail_indices
                    .iter()
                    .map(|&event_index| input.candidate_abundances[(event_index, fluor_index)])
                    .collect::<Vec<_>>();
                let retention = quantile(&baseline_tail, 0.5)
                    .zip(quantile(&candidate_tail, 0.5))
                    .and_then(|(baseline_median, candidate_median)| {
                        safe_relative_change(candidate_median, baseline_median)
                    });
                if retention.is_none() {
                    skip_reasons.push(
                        "positive events above the shipped cutoff could not be compared because their baseline median is not positive"
                            .into(),
                    );
                }
                retention
            }
            _ => {
                skip_reasons.push(
                    "this dye has no positive events above the shipped cutoff".into(),
                );
                None
            }
        };
        if let Some(retention) = bright_tail_retention {
            bright_tail_retentions.push(retention);
        }

        if !skip_reasons.is_empty() {
            skipped_fluors += 1;
        }
        fluor_rows.push(FenceFluorComparison {
            endmember_name: input.endmember_names[fluor_index].clone(),
            baseline_rsd,
            candidate_rsd,
            rsd_improvement,
            bright_tail_retention,
            skip_reason: (!skip_reasons.is_empty()).then(|| skip_reasons.join("; ")),
        });
    }

    let median_negative_rsd_improvement = median(&rsd_improvements);
    let worst_negative_rsd_regression = rsd_improvements
        .iter()
        .map(|improvement| -improvement)
        .reduce(f64::max);
    let median_bright_tail_retention = median(&bright_tail_retentions);
    let worst_bright_tail_loss = bright_tail_retentions
        .iter()
        .map(|retention| 1.0 - retention)
        .reduce(f64::max);
    let residual_rmse_increase = rmse(&input.candidate_residual_norms)
        .zip(rmse(&input.baseline_residual_norms))
        .and_then(|(candidate, baseline)| safe_relative_change(candidate - baseline, baseline));
    let candidate_truncation_fraction = input
        .candidate_any_truncated
        .iter()
        .filter(|truncated| **truncated)
        .count() as f64
        / n_events as f64;

    let blocking_reason = blocking_reason(
        worst_bright_tail_loss,
        median_bright_tail_retention,
        residual_rmse_increase,
        worst_negative_rsd_regression,
        thresholds,
    );
    let (status, eligible, blocking_reason) = classify(
        median_negative_rsd_improvement,
        median_bright_tail_retention,
        residual_rmse_increase,
        blocking_reason,
        thresholds,
    );

    Ok(FenceComparisonMetrics {
        status,
        eligible,
        median_negative_rsd_improvement,
        worst_negative_rsd_regression,
        median_bright_tail_retention,
        worst_bright_tail_loss,
        residual_rmse_increase,
        candidate_truncation_fraction,
        skipped_fluors,
        blocking_reason,
        fluor_rows,
    })
}

fn validate_input(
    input: &FenceComparisonInput<'_>,
    thresholds: &FenceComparisonThresholds,
) -> Result<(), TruOlsError> {
    let n_events = input.baseline_abundances.nrows();
    let n_fluors = input.baseline_abundances.ncols();
    if input.candidate_abundances.nrows() != n_events {
        return Err(dimension_error(
            "candidate_abundances row count",
            n_events,
            input.candidate_abundances.nrows(),
        ));
    }
    if input.candidate_abundances.ncols() != n_fluors {
        return Err(dimension_error(
            "candidate_abundances column count",
            n_fluors,
            input.candidate_abundances.ncols(),
        ));
    }
    for (input_name, actual) in [
        ("endmember_names length", input.endmember_names.len()),
        ("negative_masks length", input.negative_masks.len()),
        ("baseline_cutoffs length", input.baseline_cutoffs.nrows()),
    ] {
        if actual != n_fluors {
            return Err(dimension_error(input_name, n_fluors, actual));
        }
    }
    for (input_name, actual) in [
        (
            "baseline_residual_norms length",
            input.baseline_residual_norms.len(),
        ),
        (
            "candidate_residual_norms length",
            input.candidate_residual_norms.len(),
        ),
        (
            "candidate_any_truncated length",
            input.candidate_any_truncated.len(),
        ),
    ] {
        if actual != n_events {
            return Err(dimension_error(input_name, n_events, actual));
        }
    }
    for (fluor_index, mask) in input.negative_masks.iter().enumerate() {
        if mask.len() != n_events {
            return Err(dimension_error(
                &format!("negative_masks[{fluor_index}] length"),
                n_events,
                mask.len(),
            ));
        }
    }
    if n_events == 0 || n_fluors == 0 {
        return Err(TruOlsError::InsufficientData(
            "fence comparison requires at least one event and fluor".into(),
        ));
    }
    if !input.cofactor.is_finite() || input.cofactor <= 0.0 {
        return Err(TruOlsError::InsufficientData(format!(
            "cofactor must be finite and positive, got {}",
            input.cofactor
        )));
    }
    for (input_name, value) in [
        (
            "thresholds.min_median_bright_tail_retention",
            thresholds.min_median_bright_tail_retention,
        ),
        (
            "thresholds.max_worst_bright_tail_loss",
            thresholds.max_worst_bright_tail_loss,
        ),
        (
            "thresholds.max_residual_rmse_increase",
            thresholds.max_residual_rmse_increase,
        ),
        (
            "thresholds.max_worst_negative_rsd_regression",
            thresholds.max_worst_negative_rsd_regression,
        ),
        (
            "thresholds.rsd_equivalence_band",
            thresholds.rsd_equivalence_band,
        ),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(TruOlsError::InsufficientData(format!(
                "{input_name} must be finite and non-negative, got {value}"
            )));
        }
    }
    for event_index in 0..n_events {
        for fluor_index in 0..n_fluors {
            if !input.baseline_abundances[(event_index, fluor_index)].is_finite() {
                return Err(non_finite_matrix_error(
                    "baseline_abundances",
                    event_index,
                    fluor_index,
                ));
            }
            if !input.candidate_abundances[(event_index, fluor_index)].is_finite() {
                return Err(non_finite_matrix_error(
                    "candidate_abundances",
                    event_index,
                    fluor_index,
                ));
            }
        }
    }
    for (input_name, values) in [
        ("baseline_residual_norms", input.baseline_residual_norms),
        ("candidate_residual_norms", input.candidate_residual_norms),
    ] {
        if let Some((index, _)) = values
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(TruOlsError::InsufficientData(format!(
                "{input_name} contains non-finite value at index {index}"
            )));
        }
    }
    for fluor_index in 0..n_fluors {
        if !input.baseline_cutoffs[fluor_index].is_finite() {
            return Err(TruOlsError::InsufficientData(format!(
                "baseline_cutoffs contains non-finite value at fluor {fluor_index}"
            )));
        }
    }
    Ok(())
}

fn dimension_error(input_name: &str, expected: usize, actual: usize) -> TruOlsError {
    TruOlsError::InsufficientData(format!(
        "{input_name} mismatch: expected {expected}, got {actual}"
    ))
}

fn non_finite_matrix_error(
    input_name: &str,
    event_index: usize,
    fluor_index: usize,
) -> TruOlsError {
    TruOlsError::InsufficientData(format!(
        "{input_name} contains non-finite value at event {event_index}, fluor {fluor_index}"
    ))
}

fn safe_relative_change(numerator: f64, denominator: f64) -> Option<f64> {
    (numerator.is_finite() && denominator.is_finite() && denominator > 0.0)
        .then_some(numerator / denominator)
        .filter(|ratio| ratio.is_finite())
}

fn median(values: &[f64]) -> Option<f64> {
    quantile(values, 0.5)
}

fn rmse(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mean_square = values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64;
    let result = mean_square.sqrt();
    result.is_finite().then_some(result)
}

fn blocking_reason(
    worst_bright_tail_loss: Option<f64>,
    median_bright_tail_retention: Option<f64>,
    residual_rmse_increase: Option<f64>,
    worst_negative_rsd_regression: Option<f64>,
    thresholds: &FenceComparisonThresholds,
) -> Option<String> {
    if worst_bright_tail_loss.is_some_and(|loss| loss > thresholds.max_worst_bright_tail_loss) {
        return Some(
            "too many positive events above the shipped cutoff were reduced on the worst dye"
                .into(),
        );
    }
    if median_bright_tail_retention
        .is_some_and(|retention| retention < thresholds.min_median_bright_tail_retention)
    {
        return Some(
            "too many positive events above the shipped cutoff were reduced across dyes".into(),
        );
    }
    if residual_rmse_increase
        .is_some_and(|increase| increase > thresholds.max_residual_rmse_increase)
    {
        return Some("detector-space reconstruction error increased too much".into());
    }
    if worst_negative_rsd_regression
        .is_some_and(|regression| regression > thresholds.max_worst_negative_rsd_regression)
    {
        return Some("unstained-like events got noisier on the worst dye".into());
    }
    None
}

fn classify(
    median_rsd_improvement: Option<f64>,
    median_bright_tail_retention: Option<f64>,
    residual_rmse_increase: Option<f64>,
    blocking_reason: Option<String>,
    thresholds: &FenceComparisonThresholds,
) -> (FenceRankingStatus, bool, Option<String>) {
    if median_bright_tail_retention.is_none() {
        return (
            FenceRankingStatus::CouldNotEvaluate,
            false,
            Some("no dye had enough positive events above the shipped cutoff to compare".into()),
        );
    }
    let Some(improvement) = median_rsd_improvement else {
        return (
            FenceRankingStatus::CouldNotEvaluate,
            false,
            Some("no dye had enough unstained-like events to compare spread".into()),
        );
    };
    if residual_rmse_increase.is_none() {
        return (
            FenceRankingStatus::CouldNotEvaluate,
            false,
            Some("residual RMSE comparison has a zero or non-finite baseline".into()),
        );
    }
    if let Some(reason) = blocking_reason {
        let status = if improvement > thresholds.rsd_equivalence_band {
            FenceRankingStatus::TradeOff
        } else {
            FenceRankingStatus::Worse
        };
        return (status, false, Some(reason));
    }

    let status = if improvement.abs() <= thresholds.rsd_equivalence_band {
        FenceRankingStatus::NoClearDifference
    } else if improvement > 0.0 {
        FenceRankingStatus::Eligible
    } else {
        FenceRankingStatus::Worse
    };
    (status, true, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use faer::{Col, Mat};

    const N_EVENTS: usize = 40;

    struct Fixture {
        names: Vec<String>,
        baseline: Mat<f64>,
        candidate: Mat<f64>,
        baseline_residuals: Vec<f64>,
        candidate_residuals: Vec<f64>,
        candidate_truncated: Vec<bool>,
        masks: Vec<Vec<bool>>,
        cutoffs: Col<f64>,
    }

    fn baseline_abundances() -> Mat<f64> {
        Mat::from_fn(N_EVENTS, 1, |event, _| match event {
            0..30 => (event % 5) as f64 - 2.0,
            30..37 => 12.0 + event as f64,
            _ => 100.0 + event as f64,
        })
    }

    fn candidate_with_tight_negatives_and_tail_scale(tail_scale: f64) -> Mat<f64> {
        Mat::from_fn(N_EVENTS, 1, |event, _| match event {
            0..30 => ((event % 5) as f64 - 2.0) * 0.5,
            30..37 => 12.0 + event as f64,
            _ => (100.0 + event as f64) * tail_scale,
        })
    }

    fn fixture(candidate: Mat<f64>) -> Fixture {
        Fixture {
            names: vec!["FITC".into()],
            baseline: baseline_abundances(),
            candidate,
            baseline_residuals: vec![1.0; N_EVENTS],
            candidate_residuals: vec![1.0; N_EVENTS],
            candidate_truncated: vec![false; N_EVENTS],
            masks: vec![(0..N_EVENTS).map(|event| event < 30).collect::<Vec<_>>()],
            cutoffs: Col::from_fn(1, |_| 10.0),
        }
    }

    fn compare_fixture(fixture: &Fixture) -> FenceComparisonMetrics {
        compare_result(fixture).expect("synthetic fixture should be valid")
    }

    fn compare_result(fixture: &Fixture) -> Result<FenceComparisonMetrics, TruOlsError> {
        compare_fence_outputs(
            FenceComparisonInput {
                endmember_names: &fixture.names,
                baseline_abundances: fixture.baseline.as_ref(),
                candidate_abundances: fixture.candidate.as_ref(),
                baseline_residual_norms: &fixture.baseline_residuals,
                candidate_residual_norms: &fixture.candidate_residuals,
                candidate_any_truncated: &fixture.candidate_truncated,
                negative_masks: &fixture.masks,
                baseline_cutoffs: fixture.cutoffs.as_ref(),
                cofactor: 200.0,
            },
            &FenceComparisonThresholds::default(),
        )
    }

    #[test]
    fn lower_rsd_cannot_win_after_dim_tail_loss() {
        let fixture = fixture(candidate_with_tight_negatives_and_tail_scale(0.8));
        let metrics = compare_fixture(&fixture);

        assert!(metrics.median_negative_rsd_improvement.unwrap() > 0.0);
        assert!(!metrics.eligible);
        assert_eq!(metrics.status, FenceRankingStatus::TradeOff);
        assert!(metrics.blocking_reason.unwrap().contains("positive events"));
    }

    #[test]
    fn preserved_signal_and_lower_rsd_is_eligible() {
        let fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        let metrics = compare_fixture(&fixture);

        assert!(metrics.eligible);
        assert_eq!(metrics.status, FenceRankingStatus::Eligible);
    }

    #[test]
    fn equivalent_candidate_prefers_conservative_result() {
        let fixture = fixture(Mat::from_fn(N_EVENTS, 1, |event, _| {
            baseline_abundances()[(event, 0)] * 0.99
        }));
        let metrics = compare_fixture(&fixture);

        assert!(metrics.eligible);
        assert_eq!(metrics.status, FenceRankingStatus::NoClearDifference);
    }

    #[test]
    fn rejects_shape_mismatches() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        fixture.candidate = Mat::zeros(N_EVENTS - 1, 1);

        let error = compare_result(&fixture).expect_err("shape mismatch should fail");

        assert_eq!(
            error.to_string(),
            "Insufficient data: candidate_abundances row count mismatch: expected 40, got 39"
        );
    }

    #[test]
    fn grouped_dimension_errors_name_input_and_mask_index() {
        let mut names_fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        names_fixture.names.clear();
        let error = compare_result(&names_fixture).expect_err("name count mismatch should fail");
        assert_eq!(
            error.to_string(),
            "Insufficient data: endmember_names length mismatch: expected 1, got 0"
        );

        let mut mask_fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        mask_fixture.masks[0].pop();
        let error = compare_result(&mask_fixture).expect_err("mask length mismatch should fail");
        assert_eq!(
            error.to_string(),
            "Insufficient data: negative_masks[0] length mismatch: expected 40, got 39"
        );
    }

    #[test]
    fn rejects_non_finite_input_values() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        fixture.candidate[(3, 0)] = f64::NAN;

        let error = compare_result(&fixture).expect_err("non-finite abundance should fail");

        assert_eq!(
            error.to_string(),
            "Insufficient data: candidate_abundances contains non-finite value at event 3, fluor 0"
        );
    }

    #[test]
    fn non_finite_vector_and_cutoff_errors_include_indices() {
        let mut residual_fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        residual_fixture.candidate_residuals[4] = f64::INFINITY;
        let error = compare_result(&residual_fixture).expect_err("non-finite residual should fail");
        assert_eq!(
            error.to_string(),
            "Insufficient data: candidate_residual_norms contains non-finite value at index 4"
        );

        let mut cutoff_fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        cutoff_fixture.cutoffs[0] = f64::NAN;
        let error = compare_result(&cutoff_fixture).expect_err("non-finite cutoff should fail");
        assert_eq!(
            error.to_string(),
            "Insufficient data: baseline_cutoffs contains non-finite value at fluor 0"
        );
    }

    #[test]
    fn insufficient_negative_events_are_skipped() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        fixture.masks[0] = (0..N_EVENTS).map(|event| event < 29).collect();
        let metrics = compare_fixture(&fixture);

        assert_eq!(metrics.skipped_fluors, 1);
        assert_eq!(metrics.fluor_rows[0].baseline_rsd, None);
        assert!(
            metrics.fluor_rows[0]
                .skip_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("negative events"))
        );
    }

    #[test]
    fn channel_without_tail_above_cutoff_skips_tail_metric() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        fixture.cutoffs[0] = 1_000.0;
        let metrics = compare_fixture(&fixture);

        assert_eq!(metrics.fluor_rows[0].bright_tail_retention, None);
        assert!(
            metrics.fluor_rows[0]
                .skip_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("positive events"))
        );
    }

    #[test]
    fn missing_bright_tail_comparison_cannot_be_eligible() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        fixture.cutoffs[0] = 1_000.0;
        let metrics = compare_fixture(&fixture);

        assert_eq!(metrics.median_bright_tail_retention, None);
        assert!(!metrics.eligible);
        assert_eq!(metrics.status, FenceRankingStatus::CouldNotEvaluate);
        assert_eq!(
            metrics.blocking_reason.as_deref(),
            Some("no dye had enough positive events above the shipped cutoff to compare")
        );
    }

    #[test]
    fn zero_ratio_denominators_are_skipped() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(1.0));
        for event in 0..N_EVENTS {
            fixture.baseline[(event, 0)] = 0.0;
        }
        fixture.cutoffs[0] = -1.0;
        let metrics = compare_fixture(&fixture);

        assert_eq!(metrics.fluor_rows[0].bright_tail_retention, None);
        assert!(metrics.fluor_rows[0].skip_reason.is_some());
    }

    #[test]
    fn guardrails_use_documented_blocking_order() {
        let mut fixture = fixture(candidate_with_tight_negatives_and_tail_scale(0.8));
        fixture.candidate_residuals.fill(2.0);
        let metrics = compare_fixture(&fixture);

        assert!(
            metrics
                .blocking_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("worst dye"))
        );
    }
}
