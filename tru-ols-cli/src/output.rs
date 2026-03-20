//! Output formatting for TRU-OLS CLI

/// Output format options (reserved for export/plot output selection).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// PNG image format
    Png,
    /// SVG vector format
    Svg,
    /// PDF format
    Pdf,
}

#[allow(dead_code)]
impl OutputFormat {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "png" => Some(OutputFormat::Png),
            "svg" => Some(OutputFormat::Svg),
            "pdf" => Some(OutputFormat::Pdf),
            _ => None,
        }
    }

    /// Get file extension
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Svg => "svg",
            OutputFormat::Pdf => "pdf",
        }
    }
}
