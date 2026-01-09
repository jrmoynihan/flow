use crate::density_calc::RawPixelData;

/// Information about plot rendering progress
#[derive(Clone, Debug)]
pub struct ProgressInfo {
    /// Pixel data for the current progress chunk
    pub pixels: Vec<RawPixelData>,
    /// Progress percentage (0.0 to 100.0)
    pub percent: f32,
}

/// Callback function type for reporting plot rendering progress
///
/// The callback receives progress information and returns a result.
/// Errors from the callback are logged but do not stop rendering.
pub type ProgressCallback =
    Box<dyn FnMut(ProgressInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>>>;
