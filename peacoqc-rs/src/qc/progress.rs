//! Optional progress callbacks for long-running PeacoQC analysis.

/// One progress event emitted during [`super::peacoqc::peacoqc_with_progress`].
#[derive(Debug, Clone)]
pub struct PeacoQCProgressEvent {
    /// Algorithm stage identifier (e.g. `peak_detection`, `isolation_tree`).
    pub stage: String,
    /// Overall progress for this file, 0–100.
    pub progress: u8,
    pub message: String,
}
