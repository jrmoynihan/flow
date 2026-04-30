//! Comparison utilities for doublet detection methods
//!
//! Provides head-to-head comparison of different doublet detection methods
//! to help users choose the best approach for their data.

use super::doublets::{DoubletGateConfig, DoubletGateResult, DoubletMethod, detect_doublets};
use flow_fcs::Fcs;

/// Comparison result for doublet detection methods
#[derive(Debug, Clone)]
pub struct DoubletComparisonResult {
    /// Results for each method
    pub results: Vec<MethodResult>,
    /// Agreement matrix (how often methods agree)
    pub agreement_matrix: Vec<Vec<f64>>,
    /// Recommended method based on comparison
    pub recommended_method: String,
}

/// Result for a single method
#[derive(Debug, Clone)]
pub struct MethodResult {
    /// Method name
    pub method_name: String,
    /// Detection result
    pub result: DoubletGateResult,
    /// Performance metrics (if available)
    pub performance_ms: Option<f64>,
}

/// Compare multiple doublet detection methods head-to-head
///
/// # Arguments
/// * `fcs` - FCS file data
/// * `methods` - Vector of methods to compare
/// * `base_config` - Base configuration (channels, etc.)
///
/// # Returns
/// ComparisonResult with results and agreement analysis
pub fn compare_doublet_methods(
    fcs: &Fcs,
    methods: Vec<DoubletMethod>,
    base_config: DoubletGateConfig,
) -> Result<DoubletComparisonResult, crate::GateError> {
    let mut results = Vec::new();

    // Run each method
    for method in methods {
        let mut config = base_config.clone();
        config.method = method.clone();

        let start = std::time::Instant::now();
        let result = detect_doublets(fcs, &config)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0; // Convert to ms

        let method_name = match &method {
            DoubletMethod::RatioMAD { nmad } => format!("RatioMAD(nmad={})", nmad),
            DoubletMethod::DensityBased { threshold } => {
                format!("DensityBased(threshold={})", threshold)
            }
            DoubletMethod::Clustering { .. } => "Clustering".to_string(),
            DoubletMethod::Hybrid => "Hybrid".to_string(),
            DoubletMethod::RatioInflectionOrFixed {
                min_peaks,
                min_ratio,
                fixed_threshold,
            } => format!(
                "RatioInflectionOrFixed(min_peaks={},min_ratio={},fixed={})",
                min_peaks, min_ratio, fixed_threshold
            ),
        };

        results.push(MethodResult {
            method_name,
            result,
            performance_ms: Some(elapsed),
        });
    }

    // Calculate agreement matrix
    let n_methods = results.len();
    if n_methods == 0 {
        return Err(crate::GateError::Other {
            message: "No methods to compare".to_string(),
            source: None,
        });
    }

    let n_events = results[0].result.singlet_mask.len();
    let mut agreement_matrix = vec![vec![0.0; n_methods]; n_methods];

    for i in 0..n_methods {
        for j in 0..n_methods {
            if i == j {
                agreement_matrix[i][j] = 1.0;
            } else {
                let mask_i = &results[i].result.singlet_mask;
                let mask_j = &results[j].result.singlet_mask;
                let agreement = mask_i
                    .iter()
                    .zip(mask_j.iter())
                    .filter(|(a, b)| a == b)
                    .count() as f64
                    / n_events as f64;
                agreement_matrix[i][j] = agreement;
            }
        }
    }

    // Recommend method based on:
    // 1. Performance (if performance is priority)
    // 2. Agreement with other methods (if accuracy is priority)
    // For now, recommend the fastest method that agrees well with others
    let recommended_idx = results
        .iter()
        .enumerate()
        .min_by(|(i, a), (j, b)| {
            let avg_agreement_i: f64 = agreement_matrix[*i].iter().sum::<f64>() / n_methods as f64;
            let avg_agreement_j: f64 = agreement_matrix[*j].iter().sum::<f64>() / n_methods as f64;

            // Prefer methods with good agreement, then performance
            avg_agreement_j
                .partial_cmp(&avg_agreement_i)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.performance_ms
                        .unwrap_or(f64::INFINITY)
                        .partial_cmp(&b.performance_ms.unwrap_or(f64::INFINITY))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    // Extract recommended method name before moving results
    let recommended_method = results[recommended_idx].method_name.clone();

    Ok(DoubletComparisonResult {
        results,
        agreement_matrix,
        recommended_method,
    })
}

/// Compare peacoqc-rs method with new methods
///
/// This is a convenience function that compares the peacoqc-rs compatible
/// RatioMAD method with density-based and hybrid methods.
pub fn compare_with_peacoqc(
    fcs: &Fcs,
    base_config: DoubletGateConfig,
) -> Result<DoubletComparisonResult, crate::GateError> {
    let methods = vec![
        DoubletMethod::RatioMAD { nmad: 4.0 }, // peacoqc-rs default
        DoubletMethod::DensityBased { threshold: 0.1 },
        DoubletMethod::Hybrid,
    ];

    compare_doublet_methods(fcs, methods, base_config)
}
