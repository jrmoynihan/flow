use colorgrad::Gradient;

/// Simple RGB color tuple struct, replacing the plotters `RGBColor` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    /// Convert to an `(u8, u8, u8)` tuple.
    pub fn to_tuple(self) -> (u8, u8, u8) {
        (self.0, self.1, self.2)
    }

    /// Create from an `(u8, u8, u8)` tuple.
    pub fn from_tuple(t: (u8, u8, u8)) -> Self {
        Self(t.0, t.1, t.2)
    }
}

/// Color map options for density plots
///
/// This enum provides access to a wide variety of colormaps suitable for
/// scientific data visualization. Colormaps are categorized into:
///
/// - **Perceptually uniform sequential**: Viridis, Plasma, Inferno, Magma, Turbo
///   (excellent for continuous data, colorblind-friendly)
/// - **Traditional**: Rainbow, Jet (colorful but less perceptually uniform)
/// - **Grayscale**: Bone, BlackWhite (useful for printing)
/// - **Specialized**: Mandelbrot, Volcano (artistic/experimental)
///
/// # Recommendations
///
/// - **Default choice**: `Viridis` - perceptually uniform, colorblind-friendly
/// - **High contrast**: `Plasma`, `Inferno`, `Magma` - good for presentations
/// - **Traditional**: `Rainbow`, `Jet` - colorful but use with caution
/// - **Print-friendly**: `Bone`, `BlackWhite` - grayscale options
pub enum ColorMaps {
    // Perceptually uniform colormaps (from colorgrad)
    /// Viridis - perceptually uniform, colorblind-friendly (default)
    Viridis,
    /// Plasma - perceptually uniform, high contrast
    Plasma,
    /// Inferno - perceptually uniform, dark background friendly
    Inferno,
    /// Magma - perceptually uniform, dark to bright
    Magma,
    /// Turbo - perceptually uniform, vibrant colors
    Turbo,
    /// Cividis - colorblind-friendly, optimized for printing
    Cividis,
    /// Warm - warm color palette
    Warm,
    /// Cool - cool color palette
    Cool,
    /// Cubehelix - perceptually uniform, customizable
    CubehelixDefault,

    // Traditional colormaps (from colorgrad)
    /// Rainbow - traditional rainbow colors (use with caution)
    Rainbow,
    /// Jet - traditional jet colormap (use with caution)
    Jet,
    /// Spectral - diverging colormap
    Spectral,

    /// Bone - grayscale colormap (reimplemented without plotters)
    Bone,
    /// BlackWhite - simple grayscale
    BlackWhite,
}

impl Clone for ColorMaps {
    fn clone(&self) -> Self {
        match self {
            ColorMaps::Viridis => ColorMaps::Viridis,
            ColorMaps::Plasma => ColorMaps::Plasma,
            ColorMaps::Inferno => ColorMaps::Inferno,
            ColorMaps::Magma => ColorMaps::Magma,
            ColorMaps::Turbo => ColorMaps::Turbo,
            ColorMaps::Cividis => ColorMaps::Cividis,
            ColorMaps::Warm => ColorMaps::Warm,
            ColorMaps::Cool => ColorMaps::Cool,
            ColorMaps::CubehelixDefault => ColorMaps::CubehelixDefault,
            ColorMaps::Rainbow => ColorMaps::Rainbow,
            ColorMaps::Jet => ColorMaps::Jet,
            ColorMaps::Spectral => ColorMaps::Spectral,
            ColorMaps::Bone => ColorMaps::Bone,
            ColorMaps::BlackWhite => ColorMaps::BlackWhite,
        }
    }
}

/// Helper: sample a colorgrad gradient and return an `RgbColor`.
fn sample_gradient(grad: &impl Gradient, value: f32) -> RgbColor {
    let color = grad.at(value);
    RgbColor(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
    )
}

impl ColorMaps {
    /// Map a normalized value (0.0 to 1.0) to an RGB color
    ///
    /// # Arguments
    /// * `value` - Normalized density value between 0.0 and 1.0
    ///
    /// # Returns
    /// An `RgbColor(r, g, b)` where each component is 0-255
    pub fn map(&self, value: f32) -> RgbColor {
        let clamped_value = value.clamp(0.0, 1.0);

        match self {
            ColorMaps::Viridis => sample_gradient(&colorgrad::preset::viridis(), clamped_value),
            ColorMaps::Plasma => sample_gradient(&colorgrad::preset::plasma(), clamped_value),
            ColorMaps::Inferno => sample_gradient(&colorgrad::preset::inferno(), clamped_value),
            ColorMaps::Magma => sample_gradient(&colorgrad::preset::magma(), clamped_value),
            ColorMaps::Turbo => sample_gradient(&colorgrad::preset::turbo(), clamped_value),
            ColorMaps::Cividis => sample_gradient(&colorgrad::preset::cividis(), clamped_value),
            ColorMaps::Warm => sample_gradient(&colorgrad::preset::warm(), clamped_value),
            ColorMaps::Cool => sample_gradient(&colorgrad::preset::cool(), clamped_value),
            ColorMaps::CubehelixDefault => {
                sample_gradient(&colorgrad::preset::cubehelix_default(), clamped_value)
            }
            ColorMaps::Rainbow => sample_gradient(&colorgrad::preset::rainbow(), clamped_value),
            ColorMaps::Jet => {
                // colorgrad doesn't have Jet; sinebow is a similar alternative
                sample_gradient(&colorgrad::preset::sinebow(), clamped_value)
            }
            ColorMaps::Spectral => sample_gradient(&colorgrad::preset::spectral(), clamped_value),
            ColorMaps::Bone => {
                // Bone: blue-tinted grayscale ramp
                let v = clamped_value;
                let r = (v * 255.0 * 0.9) as u8;
                let g = (v * 255.0 * 0.9) as u8;
                let b = ((v * 0.9 + 0.1) * 255.0).min(255.0) as u8;
                RgbColor(r, g, b)
            }
            ColorMaps::BlackWhite => {
                let gray = (clamped_value * 255.0) as u8;
                RgbColor(gray, gray, gray)
            }
        }
    }
}

impl std::fmt::Debug for ColorMaps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorMaps::Viridis => write!(f, "Viridis"),
            ColorMaps::Plasma => write!(f, "Plasma"),
            ColorMaps::Inferno => write!(f, "Inferno"),
            ColorMaps::Magma => write!(f, "Magma"),
            ColorMaps::Turbo => write!(f, "Turbo"),
            ColorMaps::Cividis => write!(f, "Cividis"),
            ColorMaps::Warm => write!(f, "Warm"),
            ColorMaps::Cool => write!(f, "Cool"),
            ColorMaps::CubehelixDefault => write!(f, "CubehelixDefault"),
            ColorMaps::Rainbow => write!(f, "Rainbow"),
            ColorMaps::Jet => write!(f, "Jet"),
            ColorMaps::Spectral => write!(f, "Spectral"),
            ColorMaps::Bone => write!(f, "Bone"),
            ColorMaps::BlackWhite => write!(f, "BlackWhite"),
        }
    }
}

impl Default for ColorMaps {
    fn default() -> Self {
        ColorMaps::Viridis
    }
}
