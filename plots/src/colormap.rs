use colorgrad::Gradient;

/// An RGB color as (r, g, b) where each component is 0-255
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    /// Format as a CSS hex color string like "#FF0000"
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
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
    /// Rainbow - traditional rainbow colors (use with caution)
    Rainbow,
    /// Jet - traditional jet colormap (use with caution)
    Jet,
    /// Spectral - diverging colormap
    Spectral,
    /// Bone - grayscale with slight blue tint
    Bone,
    /// Mandelbrot - artistic HSL-based colormap
    Mandelbrot,
    /// BlackWhite - simple linear grayscale
    BlackWhite,
    /// Volcano - warm orange-red colormap
    Volcano,
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
            ColorMaps::Mandelbrot => ColorMaps::Mandelbrot,
            ColorMaps::BlackWhite => ColorMaps::BlackWhite,
            ColorMaps::Volcano => ColorMaps::Volcano,
        }
    }
}

impl ColorMaps {
    /// Map a normalized value (0.0 to 1.0) to an RGB color
    ///
    /// # Arguments
    /// * `value` - Normalized density value between 0.0 and 1.0
    ///
    /// # Returns
    /// An RGB color as `RgbColor(r, g, b)` where each component is 0-255
    pub fn map(&self, value: f32) -> RgbColor {
        let clamped_value = value.max(0.0).min(1.0);

        match self {
            ColorMaps::Viridis => colorgrad_map(&colorgrad::preset::viridis(), clamped_value),
            ColorMaps::Plasma => colorgrad_map(&colorgrad::preset::plasma(), clamped_value),
            ColorMaps::Inferno => colorgrad_map(&colorgrad::preset::inferno(), clamped_value),
            ColorMaps::Magma => colorgrad_map(&colorgrad::preset::magma(), clamped_value),
            ColorMaps::Turbo => colorgrad_map(&colorgrad::preset::turbo(), clamped_value),
            ColorMaps::Cividis => colorgrad_map(&colorgrad::preset::cividis(), clamped_value),
            ColorMaps::Warm => colorgrad_map(&colorgrad::preset::warm(), clamped_value),
            ColorMaps::Cool => colorgrad_map(&colorgrad::preset::cool(), clamped_value),
            ColorMaps::CubehelixDefault => {
                colorgrad_map(&colorgrad::preset::cubehelix_default(), clamped_value)
            }
            ColorMaps::Rainbow => colorgrad_map(&colorgrad::preset::rainbow(), clamped_value),
            ColorMaps::Jet => {
                colorgrad_map(&colorgrad::preset::sinebow(), clamped_value)
            }
            ColorMaps::Spectral => colorgrad_map(&colorgrad::preset::spectral(), clamped_value),
            ColorMaps::Bone => {
                let v = clamped_value;
                let r = (v * 0.875 * 255.0) as u8;
                let g = (v * 0.958 * 255.0) as u8;
                let b = (v * 255.0) as u8;
                RgbColor(r, g, b)
            }
            ColorMaps::Mandelbrot => {
                let hue = clamped_value * 360.0;
                hsl_to_rgb(hue, 1.0, 0.5)
            }
            ColorMaps::BlackWhite => {
                let v = (clamped_value * 255.0) as u8;
                RgbColor(v, v, v)
            }
            ColorMaps::Volcano => {
                let (r, g, b) = if clamped_value < 0.33 {
                    let t = clamped_value / 0.33;
                    ((t * 200.0) as u8, 0u8, 0u8)
                } else if clamped_value < 0.66 {
                    let t = (clamped_value - 0.33) / 0.33;
                    (200 + (t * 55.0) as u8, (t * 200.0) as u8, 0u8)
                } else {
                    let t = (clamped_value - 0.66) / 0.34;
                    (255u8, 200 + (t * 55.0) as u8, (t * 255.0) as u8)
                };
                RgbColor(r, g, b)
            }
        }
    }
}

fn colorgrad_map(grad: &impl Gradient, value: f32) -> RgbColor {
    let color = grad.at(value);
    RgbColor(
        (color.r * 255.0) as u8,
        (color.g * 255.0) as u8,
        (color.b * 255.0) as u8,
    )
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> RgbColor {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    RgbColor(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
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
            ColorMaps::Mandelbrot => write!(f, "Mandelbrot"),
            ColorMaps::BlackWhite => write!(f, "BlackWhite"),
            ColorMaps::Volcano => write!(f, "Volcano"),
        }
    }
}

impl Default for ColorMaps {
    fn default() -> Self {
        ColorMaps::Viridis
    }
}
