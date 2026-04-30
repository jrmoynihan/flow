//! Automated gating algorithms
//!
//! This module provides automated gate generation for common flow cytometry
//! preprocessing steps, including scatter gating and doublet detection.

pub mod comparison;
pub mod debris_fsc_consensus;
pub mod doublets;
pub mod interactive;
pub mod scatter;

pub use comparison::{
    DoubletComparisonResult, MethodResult, compare_doublet_methods, compare_with_peacoqc,
};
pub use debris_fsc_consensus::{ConsensusFscConfig, FscConsensusResult, consensus_fsc_threshold};
pub use doublets::{DoubletGateConfig, DoubletGateResult, DoubletMethod, detect_doublets};
pub use interactive::{PipelineBreakpoint, UserReview};
pub use scatter::{
    ClusterAlgorithm, ScatterGateConfig, ScatterGateMethod, ScatterGateResult,
    ScatterQualityPolicy, create_scatter_gate,
};

use crate::Gate;
use crate::hierarchy::GateHierarchy;
use flow_fcs::Fcs;

/// Configuration for preprocessing pipeline
#[derive(Debug, Clone)]
pub struct PreprocessingConfig {
    /// Scatter gate configuration
    pub scatter_config: ScatterGateConfig,
    /// Doublet detection configuration
    pub doublet_config: DoubletGateConfig,
}

/// Result of preprocessing pipeline
#[derive(Debug)]
pub struct PreprocessingGates {
    /// Scatter gate geometry (if generated)
    pub scatter_gate: Option<Gate>,
    /// Full scatter gate outcome (mask, stats); use [`DoubletGateResult::singlet_mask`] for doublets.
    pub scatter_result: ScatterGateResult,
    /// Doublet exclusion gate (if generated; often `None` until polygon gates exist)
    pub doublet_gate: Option<Gate>,
    /// Doublet detection outcome including `singlet_mask` for filtering
    pub doublet_result: DoubletGateResult,
    /// Gate hierarchy
    pub hierarchy: GateHierarchy,
}

/// Fully automated preprocessing pipeline
///
/// Creates scatter gate and doublet exclusion gate automatically.
pub fn create_preprocessing_gates(
    fcs: &Fcs,
    config: PreprocessingConfig,
) -> Result<PreprocessingGates, crate::GateError> {
    let hierarchy = GateHierarchy::new();

    // 1. Scatter gate (multi-population)
    let scatter_result = create_scatter_gate(fcs, &config.scatter_config)?;
    // Note: Gates are stored separately, hierarchy tracks relationships
    // If scatter gate has a parent, we'd add it here: hierarchy.add_child(parent_id, gate.id())

    // 2. Doublet exclusion
    let doublet_result = detect_doublets(fcs, &config.doublet_config)?;
    // If doublet gate should be a child of scatter gate, add relationship:
    // if let (Some(scatter_gate), Some(doublet_gate)) = (&scatter_result.gate, &doublet_result.exclusion_gate) {
    //     hierarchy.add_child(scatter_gate.id(), doublet_gate.id());
    // }

    Ok(PreprocessingGates {
        scatter_gate: scatter_result.gate.clone(),
        scatter_result,
        doublet_gate: doublet_result.exclusion_gate.clone(),
        doublet_result,
        hierarchy,
    })
}

/// Semi-automated preprocessing pipeline with user review breakpoints
///
/// Allows user to review and tweak gates at each step before proceeding.
pub fn create_preprocessing_gates_interactive(
    fcs: &Fcs,
    config: PreprocessingConfig,
    review_callback: impl Fn(PipelineBreakpoint) -> UserReview,
) -> Result<PreprocessingGates, crate::GateError> {
    let hierarchy = GateHierarchy::new();

    // 1. Scatter gate (with user review)
    let scatter_result = create_scatter_gate(fcs, &config.scatter_config)?;
    let scatter_review = review_callback(PipelineBreakpoint::ScatterGate(scatter_result.clone()));

    if let UserReview::Accept = scatter_review {
        // Gate stored in result, hierarchy tracks relationships if needed
    }

    // 2. Doublet exclusion (with user review)
    let doublet_result = detect_doublets(fcs, &config.doublet_config)?;
    let doublet_review = review_callback(PipelineBreakpoint::DoubletGate(doublet_result.clone()));

    if let UserReview::Accept = doublet_review {
        // Gate stored in result, hierarchy tracks relationships if needed
        // If doublet should be child of scatter:
        // if let (Some(scatter_gate), Some(doublet_gate)) = (&scatter_result.gate, &doublet_result.exclusion_gate) {
        //     hierarchy.add_child(scatter_gate.id(), doublet_gate.id());
        // }
    }

    Ok(PreprocessingGates {
        scatter_gate: scatter_result.gate.clone(),
        scatter_result,
        doublet_gate: doublet_result.exclusion_gate.clone(),
        doublet_result,
        hierarchy,
    })
}
