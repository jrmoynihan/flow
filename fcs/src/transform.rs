use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// Transformation type to apply to flow cytometry parameter data
///
/// Transformations are used to convert raw instrument values into display-friendly scales.
/// The most common transformation for fluorescence data is arcsinh (inverse hyperbolic sine),
/// which provides a log-like scale that handles both positive and negative values.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum TransformType {
    /// Linear transformation (no scaling, identity function)
    /// Used for scatter parameters (FSC, SSC) and time
    Linear,
    /// Arcsinh (inverse hyperbolic sine) transformation with configurable cofactor
    /// Formula: `arcsinh(x / cofactor)`
    /// Common cofactors: 150-200 for modern instruments
    Arcsinh { cofactor: f32 },
    /// Biexponential (logicle) transformation matching FlowJo's default behavior
    /// Formula: `asinh(x * sinh(M * ln(10)) / T) + A * ln(10)`
    /// where T = top of scale, M = positive decades, A = negative decades
    /// Defaults match FlowJo: T=262144 (18-bit), M=4.5, A=0, W=0.5
    Biexponential {
        /// Top of scale value (typically 262144 for 18-bit or 1048576 for 20-bit data)
        top_of_scale: f32,
        /// Number of positive decades (typically 4.5)
        positive_decades: f32,
        /// Number of additional negative decades (typically 0)
        negative_decades: f32,
        /// Width basis parameter (typically 0.5)
        width: f32,
    },
}

impl TransformType {
    /// Create a TransformType from a string. If no string is provided or the string is not matched, the default `arcsinh` transform is used.
    pub fn create_from_str(s: Option<&str>) -> Self {
        match s {
            Some("linear") => TransformType::Linear,
            Some("arcsinh") => TransformType::Arcsinh { cofactor: 200.0 },
            Some("biexponential") | Some("logicle") => TransformType::Biexponential {
                top_of_scale: 262144.0,
                positive_decades: 4.5,
                negative_decades: 0.0,
                width: 0.5,
            },
            _ => TransformType::default(),
        }
    }
}

/// Trait for types that can transform values from raw to display scale
///
/// Transformations are typically applied when displaying data, not when storing it.
/// This allows the raw data to remain unchanged while providing flexible visualization options.
pub trait Transformable {
    fn transform(&self, value: &f32) -> f32;
    fn inverse_transform(&self, value: &f32) -> f32;
}
/// Trait for types that can format transformed values for display
///
/// Formatting converts numeric values into human-readable strings,
/// typically using scientific notation for large numbers.
#[allow(unused)]
pub trait Formattable {
    fn format(&self, value: &f32) -> String;
}

impl Transformable for TransformType {
    fn transform(&self, value: &f32) -> f32 {
        match self {
            TransformType::Linear => *value,
            TransformType::Arcsinh { cofactor } => (value / cofactor).asinh(),
            TransformType::Biexponential {
                top_of_scale,
                positive_decades,
                negative_decades,
                width: _,
            } => {
                // Logicle/biexponential transformation formula
                // f(x) = asinh(x * sinh(M * ln(10)) / T) + A * ln(10)
                // where T = top_of_scale, M = positive_decades, A = negative_decades
                let ln_10 = 10.0_f32.ln();
                let m_ln10 = positive_decades * ln_10;
                let sinh_m_ln10 = m_ln10.sinh();
                let a_ln10 = negative_decades * ln_10;

                // Handle division by zero and very small values
                if *top_of_scale == 0.0 {
                    return *value;
                }

                let scaled_x = value * sinh_m_ln10 / top_of_scale;
                scaled_x.asinh() + a_ln10
            }
        }
    }
    fn inverse_transform(&self, value: &f32) -> f32 {
        match self {
            TransformType::Linear => *value,
            TransformType::Arcsinh { cofactor } => {
                eprintln!(
                    "🔧 [INVERSE_TRANSFORM] Arcsinh inverse: value={}, cofactor={}",
                    value, cofactor
                );
                let final_result = (*value).sinh() * *cofactor;
                eprintln!(
                    "🔧 [INVERSE_TRANSFORM] final result: {} * {} = {}",
                    value.sinh(),
                    cofactor,
                    final_result
                );
                final_result
            }
            TransformType::Biexponential {
                top_of_scale,
                positive_decades,
                negative_decades,
                width: _,
            } => {
                // Inverse logicle/biexponential transformation
                // x = T * sinh((y - A * ln(10))) / sinh(M * ln(10))
                let ln_10 = 10.0_f32.ln();
                let m_ln10 = positive_decades * ln_10;
                let sinh_m_ln10 = m_ln10.sinh();
                let a_ln10 = negative_decades * ln_10;

                let y_minus_a = value - a_ln10;
                let sinh_y_minus_a = y_minus_a.sinh();

                top_of_scale * sinh_y_minus_a / sinh_m_ln10
            }
        }
    }
}
impl Formattable for TransformType {
    fn format(&self, value: &f32) -> String {
        match self {
            TransformType::Linear => format!("{:.1e}", value),
            TransformType::Arcsinh { cofactor: _ } => {
                // Convert from transformed space back to original space
                let original_value = self.inverse_transform(value);

                // Make nice rounded labels in original space
                format!("{:.1e}", original_value)
            }
            TransformType::Biexponential { .. } => {
                // Convert from transformed space back to original space
                let original_value = self.inverse_transform(value);

                // Make nice rounded labels in original space
                format!("{:.1e}", original_value)
            }
        }
    }
}
impl Default for TransformType {
    fn default() -> Self {
        TransformType::Arcsinh { cofactor: 200.0 }
    }
}
impl Hash for TransformType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            TransformType::Linear => "linear".hash(state),
            TransformType::Arcsinh { cofactor: _ } => "arcsinh".hash(state),
            TransformType::Biexponential { .. } => "biexponential".hash(state),
        }
    }
}

#[test]
fn test_transform() {
    let t = TransformType::Linear;
    assert_eq!(t.transform(&1.0), 1.0);
    assert_eq!(t.inverse_transform(&1.0), 1.0);

    let t = TransformType::Arcsinh { cofactor: 200.0 };
    // Use approximate equality for floating point comparisons
    let transformed = t.transform(&1.0);
    assert!(
        (transformed - 0.005).abs() < 1e-6,
        "Expected ~0.005, got {}",
        transformed
    );
    let inverse = t.inverse_transform(&0.005);
    // Use a slightly larger tolerance for inverse transform due to floating point precision
    assert!(
        (inverse - 1.0).abs() < 1e-5,
        "Expected ~1.0, got {}",
        inverse
    );
    // Assert that the transform results in a number
    assert!(!t.transform(&-1.0).is_nan());
    assert!(!t.transform(&0.0).is_nan());
    assert!(!t.transform(&-200.0).is_nan());
}
