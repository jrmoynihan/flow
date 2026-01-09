pub mod plotters_backend;
pub mod progress;

pub use progress::{ProgressCallback, ProgressInfo};

/// Configuration for plot rendering
///
/// This struct allows applications to inject their own execution and progress
/// reporting logic without the library depending on specific frameworks.
#[derive(Default)]
pub struct RenderConfig {
    /// Optional progress callback for reporting rendering progress
    ///
    /// Applications can provide a callback to receive progress updates during
    /// pixel rendering. This is useful for streaming/progressive rendering.
    pub progress: Option<ProgressCallback>,
}

impl RenderConfig {
    /// Create a new RenderConfig with no callbacks
    pub fn new() -> Self {
        Self::default()
    }

    /// Call the progress callback if present
    pub fn report_progress(&mut self, info: ProgressInfo) {
        if let Some(ref mut callback) = self.progress {
            if let Err(e) = callback(info) {
                eprintln!("⚠️ Failed to report progress: {}", e);
            }
        }
    }
}
