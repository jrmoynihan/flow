use super::MixedKeyword;

/// Helper function to parse comma-separated decimal numbers
/// Supports both standard (1.5) and European (1,5) decimal formats
pub fn parse_float_with_comma_decimal(value: &str) -> Option<f32> {
    // First try standard decimal format
    if let Ok(val) = value.trim().parse::<f32>() {
        return Some(val);
    }

    // If that fails, try comma as decimal separator
    let normalized = value.trim().replace(',', ".");
    normalized.parse::<f32>().ok()
}

/// Helper function to parse comma-separated tuple of 2 floats
pub fn parse_float_tuple(value: &str) -> Option<(f32, f32)> {
    let parts: Vec<&str> = value.trim().split(',').collect();
    if parts.len() == 2 {
        let f1 = parse_float_with_comma_decimal(parts[0])?;
        let f2 = parse_float_with_comma_decimal(parts[1])?;
        Some((f1, f2))
    } else {
        None
    }
}

/// Helper function to parse comma-separated vector of floats
pub fn parse_float_vector(value: &str) -> Option<Vec<f32>> {
    value
        .trim()
        .split(',')
        .map(parse_float_with_comma_decimal)
        .collect()
}

/// Validates that a scale type is either "Linear" or "Logarithmic"
pub fn validate_pnd_scale_type(scale_type: &str) -> bool {
    matches!(scale_type.trim(), "Linear" | "Logarithmic")
}

/// Helper function to parse $PnD format: (Linear|Logarithmic,f1,f2)
pub fn parse_pnd(value: &str) -> Option<MixedKeyword> {
    let parts: Vec<&str> = value.trim().split(',').collect();
    if parts.len() == 3 {
        let scale_type = parts[0].trim().to_string();

        // Validate scale type
        if !validate_pnd_scale_type(&scale_type) {
            return None;
        }

        let f1 = parse_float_with_comma_decimal(parts[1])?;
        let f2 = parse_float_with_comma_decimal(parts[2])?;
        Some(MixedKeyword::PnD(scale_type, f1, f2))
    } else {
        None
    }
}

/// Helper function to parse $SPILLOVER format
pub fn parse_spillover(value: &str) -> Option<MixedKeyword> {
    let parts: Vec<&str> = value.trim().split(',').collect();
    if parts.is_empty() {
        return None;
    }

    let n_parameters = parts[0].trim().parse::<usize>().ok()?;

    if parts.len() < 1 + n_parameters {
        return None; // Not enough parts for parameter names
    }

    let parameter_names: Vec<String> = parts[1..=n_parameters]
        .iter()
        .map(|s| s.trim().to_string())
        .collect();

    let expected_matrix_size = n_parameters * n_parameters;
    let matrix_start = 1 + n_parameters;

    if parts.len() < matrix_start + expected_matrix_size {
        return None; // Not enough parts for full matrix
    }

    let matrix_values: Option<Vec<f32>> = parts[matrix_start..matrix_start + expected_matrix_size]
        .iter()
        .map(|s| parse_float_with_comma_decimal(s))
        .collect();

    matrix_values.map(|matrix_values| MixedKeyword::SPILLOVER {
        n_parameters,
        parameter_names,
        matrix_values,
    })
}

/// Unified parameter extraction for Pn*, Gn*, Rn* patterns
pub struct ParameterParts {
    pub param_number: usize,
    pub suffix: String,
}

/// Extracts parameter number and suffix from parameter keywords
/// Handles patterns like: P123N -> (123, "N"), G456E -> (456, "E"), R789W -> (789, "W")
pub fn extract_parameter_parts(key: &str) -> Option<ParameterParts> {
    // Try Pn* pattern first
    if let Some(rest) = key.strip_prefix("P") {
        let mut chars = rest.chars();
        if let Some(first_char) = chars.next() {
            if first_char.is_numeric() {
                let mut param_str = first_char.to_string();
                param_str.extend(chars.by_ref().take_while(|c| c.is_numeric()));

                if let Ok(param_number) = param_str.parse::<usize>() {
                    let suffix: String = chars.collect();
                    return Some(ParameterParts {
                        param_number,
                        suffix,
                    });
                }
            }
        }
    }

    // Try Gn* pattern (deprecated)
    if let Some(rest) = key.strip_prefix("G") {
        let mut chars = rest.chars();
        if let Some(first_char) = chars.next() {
            if first_char.is_numeric() {
                let mut param_str = first_char.to_string();
                param_str.extend(chars.by_ref().take_while(|c| c.is_numeric()));

                if let Ok(param_number) = param_str.parse::<usize>() {
                    let suffix: String = chars.collect();
                    return Some(ParameterParts {
                        param_number,
                        suffix,
                    });
                }
            }
        }
    }

    // Try Rn* pattern
    if let Some(rest) = key.strip_prefix("R") {
        let mut chars = rest.chars();
        if let Some(first_char) = chars.next() {
            if first_char.is_numeric() {
                let mut param_str = first_char.to_string();
                param_str.extend(chars.by_ref().take_while(|c| c.is_numeric()));

                if let Ok(param_number) = param_str.parse::<usize>() {
                    let suffix: String = chars.collect();
                    return Some(ParameterParts {
                        param_number,
                        suffix,
                    });
                }
            }
        }
    }

    None
}

/// Checks if a keyword is a parameter keyword (P followed by digits)
pub fn is_parameter_keyword(key: &str) -> bool {
    key.strip_prefix("P")
        .map(|rest| rest.chars().next().map_or(false, |c| c.is_numeric()))
        .unwrap_or(false)
}
