use colorgrad::Gradient;
use plotters::{
    prelude::*,
    style::colors::colormaps::{BlackWhite, Bone, MandelbrotHSL, ViridisRGB, VulcanoHSL},
};

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

    // Plotters built-in colormaps (kept for backward compatibility)
    /// Bone - grayscale colormap
    Bone(Bone),
    /// Mandelbrot - artistic HSL colormap
    Mandelbrot(MandelbrotHSL),
    /// BlackWhite - simple grayscale
    BlackWhite(BlackWhite),
    /// Volcano - HSL colormap
    Volcano(VulcanoHSL),
    /// ViridisRGB - Plotters' Viridis implementation (use Viridis instead)
    #[deprecated(note = "Use ColorMaps::Viridis instead")]
    ViridisRGB(ViridisRGB),
}

impl Clone for ColorMaps {
    fn clone(&self) -> Self {
        match self {
            // colorgrad colormaps are zero-sized, so we can just copy the variant
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
            // Plotters colormaps
            ColorMaps::Bone(_) => ColorMaps::Bone(Bone),
            ColorMaps::Mandelbrot(_) => ColorMaps::Mandelbrot(MandelbrotHSL),
            ColorMaps::BlackWhite(_) => ColorMaps::BlackWhite(BlackWhite),
            ColorMaps::Volcano(_) => ColorMaps::Volcano(VulcanoHSL),
            #[allow(deprecated)]
            ColorMaps::ViridisRGB(_) => ColorMaps::Viridis,
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
    /// An RGB color as `RGBColor(r, g, b)` where each component is 0-255
    pub fn map(&self, value: f32) -> RGBColor {
        // Clamp value to [0.0, 1.0]
        let clamped_value = value.max(0.0).min(1.0);

        match self {
            // colorgrad colormaps (from preset module)
            // Note: colorgrad Color has r, g, b, a as f32 in range [0.0, 1.0]
            ColorMaps::Viridis => {
                let grad = colorgrad::preset::viridis();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Plasma => {
                let grad = colorgrad::preset::plasma();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Inferno => {
                let grad = colorgrad::preset::inferno();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Magma => {
                let grad = colorgrad::preset::magma();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Turbo => {
                let grad = colorgrad::preset::turbo();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Cividis => {
                let grad = colorgrad::preset::cividis();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Warm => {
                let grad = colorgrad::preset::warm();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Cool => {
                let grad = colorgrad::preset::cool();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::CubehelixDefault => {
                let grad = colorgrad::preset::cubehelix_default();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Rainbow => {
                let grad = colorgrad::preset::rainbow();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Jet => {
                // colorgrad doesn't have Jet, use sinebow as a similar alternative
                let grad = colorgrad::preset::sinebow();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            ColorMaps::Spectral => {
                let grad = colorgrad::preset::spectral();
                let color = grad.at(clamped_value);
                RGBColor(
                    (color.r * 255.0) as u8,
                    (color.g * 255.0) as u8,
                    (color.b * 255.0) as u8,
                )
            }
            // Plotters built-in colormaps (backward compatibility)
            ColorMaps::Bone(c) => c.get_color(clamped_value),
            ColorMaps::Mandelbrot(c) => convert_hsl_to_rgb(c.get_color(clamped_value)),
            ColorMaps::BlackWhite(c) => c.get_color(clamped_value),
            ColorMaps::Volcano(c) => convert_hsl_to_rgb(c.get_color(clamped_value)),
            #[allow(deprecated)]
            ColorMaps::ViridisRGB(c) => c.get_color(clamped_value),
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
            ColorMaps::Bone(_) => write!(f, "Bone"),
            ColorMaps::Mandelbrot(_) => write!(f, "Mandelbrot"),
            ColorMaps::BlackWhite(_) => write!(f, "BlackWhite"),
            ColorMaps::Volcano(_) => write!(f, "Volcano"),
            #[allow(deprecated)]
            ColorMaps::ViridisRGB(_) => write!(f, "ViridisRGB"),
        }
    }
}
impl Default for ColorMaps {
    fn default() -> Self {
        ColorMaps::Viridis
    }
}

fn convert_hsl_to_rgb(hsl: HSLColor) -> RGBColor {
    let (r, g, b) = hsl.rgb();
    RGBColor(r, g, b)
}

// Define your custom color map
pub struct CustomColorMap;

macro_rules! def_linear_colormap{
    ($color_scale_name:ident, $color_type:ident, $doc:expr, $(($($color_value:expr),+)),*) => {
        #[doc = $doc]
        pub struct $color_scale_name;

        impl $color_scale_name {
            // const COLORS: [$color_type; $number_colors] = [$($color_type($($color_value),+)),+];
            // const COLORS: [$color_type; $crate::count!($(($($color_value:expr),+))*)] = [$($color_type($($color_value),+)),+];
            const COLORS: [$color_type; $crate::count!($(($($color_value:expr),+))*)] = $crate::define_colors_from_list_of_values_or_directly!{$color_type, $(($($color_value),+)),*};
        }

        $crate::implement_linear_interpolation_color_map!{$color_scale_name, $color_type}
    };
    ($color_scale_name:ident, $color_type:ident, $doc:expr, $($color_complete:tt),+) => {
        #[doc = $doc]
        pub struct $color_scale_name;

        impl $color_scale_name {
            const COLORS: [$color_type; $crate::count!($($color_complete)*)] = $crate::define_colors_from_list_of_values_or_directly!{$($color_complete),+};
        }

        $crate::implement_linear_interpolation_color_map!{$color_scale_name, $color_type}
    }
}

#[macro_export]
#[doc(hidden)]
/// Implements the [ColorMap] trait on a given color scale.
macro_rules! implement_linear_interpolation_color_map {
    ($color_scale_name:ident, $color_type:ident) => {
        impl<FloatType: std::fmt::Debug + num_traits::Float + num_traits::FromPrimitive + num_traits::ToPrimitive>
            ColorMap<$color_type, FloatType> for $color_scale_name
        {
            fn get_color_normalized(
                &self,
                h: FloatType,
                min: FloatType,
                max: FloatType,
            ) -> $color_type {
                let (
                    relative_difference,
                    index_lower,
                    index_upper
                ) = calculate_relative_difference_index_lower_upper(
                    h,
                    min,
                    max,
                    Self::COLORS.len()
                );
                // Interpolate the final color linearly
                $crate::calculate_new_color_value!(
                    relative_difference,
                    Self::COLORS,
                    index_upper,
                    index_lower,
                    $color_type
                )
            }
        }

        impl $color_scale_name {
            #[doc = "Get color value from `"]
            #[doc = stringify!($color_scale_name)]
            #[doc = "` by supplying a parameter 0.0 <= h <= 1.0"]
            pub fn get_color<FloatType: std::fmt::Debug + num_traits::Float + num_traits::FromPrimitive + num_traits::ToPrimitive>(
                h: FloatType,
            ) -> $color_type {
                let color_scale = $color_scale_name {};
                color_scale.get_color(h)
            }

            #[doc = "Get color value from `"]
            #[doc = stringify!($color_scale_name)]
            #[doc = "` by supplying lower and upper bounds min, max and a parameter h where min <= h <= max"]
            pub fn get_color_normalized<
                FloatType: std::fmt::Debug + num_traits::Float + num_traits::FromPrimitive + num_traits::ToPrimitive,
            >(
                h: FloatType,
                min: FloatType,
                max: FloatType,
            ) -> $color_type {
                let color_scale = $color_scale_name {};
                color_scale.get_color_normalized(h, min, max)
            }
        }
    };
}

pub fn calculate_relative_difference_index_lower_upper<
    FloatType: num_traits::Float + num_traits::FromPrimitive + num_traits::ToPrimitive,
>(
    h: FloatType,
    min: FloatType,
    max: FloatType,
    n_steps: usize,
) -> (FloatType, usize, usize) {
    // Ensure that we do have a value in bounds
    let h = num_traits::clamp(h, min, max);
    // Next calculate a normalized value between 0.0 and 1.0
    let t = (h - min) / (max - min);
    let approximate_index = t
        * (FloatType::from_usize(n_steps).expect("should be able to get a float type from usize")
            - FloatType::one())
        .max(FloatType::zero());
    // Calculate which index are the two most nearest of the supplied value
    let index_lower = approximate_index
        .floor()
        .to_usize()
        .expect("should be able to get the lower index");
    let index_upper = approximate_index
        .ceil()
        .to_usize()
        .expect("should be able to get the upper index");
    // Calculate the relative difference, ie. is the actual value more towards the color of index_upper or index_lower?
    let relative_difference = approximate_index.ceil() - approximate_index;
    (relative_difference, index_lower, index_upper)
}

/// Converts a given color identifier and a sequence of colors to an array of them.
macro_rules! define_colors_from_list_of_values_or_directly{
    ($color_type:ident, $(($($color_value:expr),+)),+) => {
        [$($color_type($($color_value),+)),+]
    };
    ($($color_complete:tt),+) => {
        [$($color_complete),+]
    };
}
