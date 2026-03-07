pub mod kuva_backend;

/// Configuration for plot rendering.
///
/// Reserved for future use (e.g. verbosity, output format).
#[derive(Default)]
pub struct RenderConfig {}

impl RenderConfig {
    /// Create a new RenderConfig with default settings.
    pub fn new() -> Self {
        Self::default()
    }
}
