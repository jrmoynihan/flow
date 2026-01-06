use serde::{Deserialize, Serialize};
use std::hash::Hash;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransformType {
    Linear,
    Arcsinh { cofactor: f32 },
}

impl TransformType {
    /// Create a TransformType from a string. If no string is provided or the string is not matched, the default `arcsinh` transform is used.
    pub fn create_from_str(s: Option<&str>) -> Self {
        match s {
            Some("linear") => TransformType::Linear,
            Some("arcsinh") => TransformType::default(),
            _ => TransformType::default(),
        }
    }
}

pub trait Transformable {
    fn transform(&self, value: &f32) -> f32;
    fn inverse_transform(&self, value: &f32) -> f32;
}
pub trait Formattable {
    fn format(&self, value: &f32) -> String;
}

impl Transformable for TransformType {
    fn transform(&self, value: &f32) -> f32 {
        match self {
            TransformType::Linear => *value,
            TransformType::Arcsinh { cofactor } => (value / cofactor).asinh(),
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
                let sinh_result = value.sinh();
                eprintln!("🔧 [INVERSE_TRANSFORM] sinh({}) = {}", value, sinh_result);
                let final_result = sinh_result * cofactor;
                eprintln!(
                    "🔧 [INVERSE_TRANSFORM] final result: {} * {} = {}",
                    sinh_result, cofactor, final_result
                );
                final_result
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
        }
    }
}

#[test]
fn test_transform() {
    let t = TransformType::Linear;
    assert_eq!(t.transform(&1.0), 1.0);
    assert_eq!(t.inverse_transform(&1.0), 1.0);

    let t = TransformType::Arcsinh { cofactor: 200.0 };
    assert_eq!(t.transform(&1.0), 0.005);
    assert_eq!(t.inverse_transform(&0.005), 1.0);
    // Assert that the transform results in a number
    assert!(!t.transform(&-1.0).is_nan());
    assert!(!t.transform(&0.0).is_nan());
    assert!(!t.transform(&-200.0).is_nan());
}
