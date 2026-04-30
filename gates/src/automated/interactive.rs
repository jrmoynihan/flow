//! Interactive pipeline support for user review and breakpoints

use super::{DoubletGateResult, ScatterGateResult};

/// User review decision for a pipeline breakpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserReview {
    /// Accept the gate and continue
    Accept,
    /// Reject and skip this step
    Reject,
    /// Modify parameters and retry (not yet implemented)
    Modify,
}

/// Pipeline breakpoint for user review
#[derive(Debug, Clone)]
pub enum PipelineBreakpoint {
    /// Scatter gate breakpoint
    ScatterGate(ScatterGateResult),
    /// Doublet gate breakpoint
    DoubletGate(DoubletGateResult),
}
