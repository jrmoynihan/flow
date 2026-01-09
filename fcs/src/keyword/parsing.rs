use super::helpers::{
    extract_parameter_suffix, is_parameter_keyword, parse_float_tuple, parse_float_vector,
    parse_float_with_comma_decimal, parse_pnd, parse_spillover,
};
use super::{
    ByteKeyword, FloatKeyword, IntegerKeyword, KeywordCreationResult, MixedKeyword, StringKeyword,
};
use crate::{byteorder::ByteOrder, datatype::FcsDataType};
use std::sync::Arc;

/// Parse fixed (non-parameterized) keywords
#[allow(deprecated)]
pub fn parse_fixed_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    let trimmed_value = value.trim();

    match key {
        "FIL" => Some(KeywordCreationResult::String(StringKeyword::FIL(
            Arc::from(trimmed_value),
        ))),
        "GUID" => Some(KeywordCreationResult::String(StringKeyword::GUID(
            Arc::from(trimmed_value),
        ))),
        "BYTEORD" => Some(
            ByteOrder::from_keyword_str(trimmed_value)
                .map(|byte_order| KeywordCreationResult::Byte(ByteKeyword::BYTEORD(byte_order)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "DATATYPE" => Some(
            FcsDataType::from_keyword_str(trimmed_value)
                .map(|data_type| KeywordCreationResult::Byte(ByteKeyword::DATATYPE(data_type)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "PAR" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::PAR(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "TOT" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::TOT(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "BEGINDATA" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::BeginData(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "ENDDATA" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::EndData(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "BEGINANALYSIS" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::BeginAnalysis(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "ENDANALYSIS" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::EndAnalysis(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "BEGINTEXT" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::BeginText(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        "ENDTEXT" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::EndText(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        // Modern FCS 3.2 keywords
        "BEGINDATETIME" => Some(KeywordCreationResult::String(StringKeyword::BEGINDATETIME(
            Arc::from(trimmed_value),
        ))),
        "ENDDATETIME" => Some(KeywordCreationResult::String(StringKeyword::ENDDATETIME(
            Arc::from(trimmed_value),
        ))),
        "CARRIERID" => Some(KeywordCreationResult::String(StringKeyword::CARRIERID(
            Arc::from(trimmed_value),
        ))),
        "CARRIERTYPE" => Some(KeywordCreationResult::String(StringKeyword::CARRIERTYPE(
            Arc::from(trimmed_value),
        ))),
        "LOCATIONID" => Some(KeywordCreationResult::String(StringKeyword::LOCATIONID(
            Arc::from(trimmed_value),
        ))),
        "FLOWRATE" => Some(KeywordCreationResult::String(StringKeyword::FLOWRATE(
            Arc::from(trimmed_value),
        ))),
        "SPILLOVER" => parse_spillover(trimmed_value).map(KeywordCreationResult::Mixed),
        "VOL" => Some(KeywordCreationResult::String(StringKeyword::VOL(
            Arc::from(trimmed_value),
        ))),
        "ORIGINALITY" => Some(KeywordCreationResult::String(StringKeyword::ORIGINALITY(
            Arc::from(trimmed_value),
        ))),
        "LAST_MODIFIER" => Some(KeywordCreationResult::String(StringKeyword::LastModifier(
            Arc::from(trimmed_value),
        ))),
        "LAST_MODIFIED" => Some(KeywordCreationResult::String(StringKeyword::LastModified(
            Arc::from(trimmed_value),
        ))),
        "CYT" => Some(KeywordCreationResult::String(StringKeyword::CYT(
            Arc::from(trimmed_value),
        ))),
        // Deprecated keywords
        #[allow(deprecated)]
        "DATE" => Some(KeywordCreationResult::String(StringKeyword::DATE(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "BTIM" => Some(KeywordCreationResult::String(StringKeyword::BTIM(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "ETIM" => Some(KeywordCreationResult::String(StringKeyword::ETIM(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "MODE" => Some(KeywordCreationResult::String(StringKeyword::MODE(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "PLATEID" => Some(KeywordCreationResult::String(StringKeyword::PLATEID(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "PLATENAME" => Some(KeywordCreationResult::String(StringKeyword::PLATENAME(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "WELLID" => Some(KeywordCreationResult::String(StringKeyword::WELLID(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "GATE" => Some(KeywordCreationResult::String(StringKeyword::GATE(
            Arc::from(trimmed_value),
        ))),
        _ => None,
    }
}

/// Parse parameter keywords (Pn*)
pub fn parse_parameter_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    if !is_parameter_keyword(key) {
        return None;
    }

    let suffix = extract_parameter_suffix(key)?;
    let trimmed_value = value.trim();

    match suffix.as_str() {
        // Gain for parameter n → [`FloatKeyword::PnG`]
        "G" => parse_float_with_comma_decimal(trimmed_value)
            .map(|gain| KeywordCreationResult::Float(FloatKeyword::PnG(gain)))
            .map_or(Some(KeywordCreationResult::UnableToParse), Some),
        // Amplification type for parameter n (f1=decades, f2=offset) → [`MixedKeyword::PnE`]
        "E" => {
            // Try parsing as tuple (f32, f32) first
            if let Some((f1, f2)) = parse_float_tuple(trimmed_value) {
                Some(KeywordCreationResult::Mixed(MixedKeyword::PnE(f1, f2)))
            } else if let Some(single_val) = parse_float_with_comma_decimal(trimmed_value) {
                // Fallback: try single float for backwards compatibility
                Some(KeywordCreationResult::Mixed(MixedKeyword::PnE(
                    single_val, 0.0,
                )))
            } else {
                Some(KeywordCreationResult::UnableToParse)
            }
        }
        // Range for parameter n → [`IntegerKeyword::PnR`]
        "R" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|range| KeywordCreationResult::Int(IntegerKeyword::PnR(range)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        // Number of bits reserved for parameter n → [`IntegerKeyword::PnB`]
        "B" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|bits| KeywordCreationResult::Int(IntegerKeyword::PnB(bits)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        // Voltage range for parameter n → [`IntegerKeyword::PnV`]
        "V" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|voltage_range| KeywordCreationResult::Int(IntegerKeyword::PnV(voltage_range)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        // Excitation wavelength(s) for parameter n → [`MixedKeyword::PnL`]
        "L" => {
            // Parse as comma-separated list of wavelengths
            // Handle FCS format with parentheses: (488) or (488,532,633)
            let cleaned_value = trimmed_value
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(trimmed_value);

            cleaned_value
                .split(',')
                .map(|s| s.trim().parse::<usize>().ok())
                .collect::<Option<Vec<usize>>>()
                .map(|wl| KeywordCreationResult::Mixed(MixedKeyword::PnL(wl)))
                .map_or(Some(KeywordCreationResult::UnableToParse), Some)
        }
        // Transformation to apply when displaying the data → [`IntegerKeyword::PnDisplay`]
        "Display" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|display_scale| {
                    KeywordCreationResult::Int(IntegerKeyword::PnDisplay(display_scale))
                })
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        // Short name for parameter n → [`StringKeyword::PnN`]
        "N" => Some(KeywordCreationResult::String(StringKeyword::PnN(
            Arc::from(trimmed_value),
        ))),
        // Label name for parameter n → [`StringKeyword::PnS`]
        "S" => Some(KeywordCreationResult::String(StringKeyword::PnS(
            Arc::from(trimmed_value),
        ))),
        // Name of the optical filter for parameter n → [`StringKeyword::PnF`]
        "F" => Some(KeywordCreationResult::String(StringKeyword::PnF(
            Arc::from(trimmed_value),
        ))),
        // FCS measurement signal types and evaluation features → [`StringKeyword::PnType`]
        "Type" | "TYPE" => Some(KeywordCreationResult::String(StringKeyword::PnType(
            Arc::from(trimmed_value),
        ))),
        // Detector name for parameter n (FCS 3.2+) → [`StringKeyword::PnDET`]
        "DET" => Some(KeywordCreationResult::String(StringKeyword::PnDET(
            Arc::from(trimmed_value),
        ))),
        // Dye specification for parameter n (FCS 3.2+) → [`StringKeyword::PnTAG`]
        "TAG" => Some(KeywordCreationResult::String(StringKeyword::PnTAG(
            Arc::from(trimmed_value),
        ))),
        // Target molecule or process for parameter n (FCS 3.2+) → [`StringKeyword::PnANALYTE`]
        "ANALYTE" => Some(KeywordCreationResult::String(StringKeyword::PnANALYTE(
            Arc::from(trimmed_value),
        ))),
        // Evaluation features for parameter n (FCS 3.2+) → [`StringKeyword::PnFEATURE`]
        "FEATURE" => Some(KeywordCreationResult::String(StringKeyword::PnFEATURE(
            Arc::from(trimmed_value),
        ))),
        // Visualization scale for parameter n (Linear/Logarithmic with bounds) → [`MixedKeyword::PnD`]
        "D" => parse_pnd(trimmed_value)
            .map(KeywordCreationResult::Mixed)
            .map_or(Some(KeywordCreationResult::UnableToParse), Some),
        // Data type for parameter n, overriding default $DATATYPE (FCS 3.2+) → [`IntegerKeyword::PnDATATYPE`]
        "DATATYPE" => Some(
            trimmed_value
                .parse::<usize>()
                .map(|n| KeywordCreationResult::Int(IntegerKeyword::PnDATATYPE(n)))
                .unwrap_or(KeywordCreationResult::UnableToParse),
        ),
        _ => {
            eprintln!(
                "Unknown parameter keyword suffix: '{}' for key: '{}' with value: '{}'",
                suffix, key, value
            );
            None
        }
    }
}

/// Parse deprecated gate keywords (Gn*)
pub fn parse_gate_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    let suffix = extract_parameter_suffix(key)?;

    // Check if it's a gate keyword (starts with G)
    if !key.starts_with("G") {
        return None;
    }

    let trimmed_value = value.trim();

    match suffix.as_str() {
        #[allow(deprecated)]
        "E" => {
            if let Some((f1, f2)) = parse_float_tuple(trimmed_value) {
                Some(KeywordCreationResult::Mixed(MixedKeyword::GnE(f1, f2)))
            } else {
                Some(KeywordCreationResult::Mixed(MixedKeyword::GnE(0.0, 0.0)))
            }
        }
        #[allow(deprecated)]
        "F" => Some(KeywordCreationResult::String(StringKeyword::GnF(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "N" => Some(KeywordCreationResult::String(StringKeyword::GnN(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "P" => Some(KeywordCreationResult::String(StringKeyword::GnP(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "R" => Some(KeywordCreationResult::String(StringKeyword::GnR(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "S" => Some(KeywordCreationResult::String(StringKeyword::GnS(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "T" => Some(KeywordCreationResult::String(StringKeyword::GnT(
            Arc::from(trimmed_value),
        ))),
        #[allow(deprecated)]
        "V" => Some(KeywordCreationResult::String(StringKeyword::GnV(
            Arc::from(trimmed_value),
        ))),
        _ => None,
    }
}

/// Parse region keywords (Rn*)
pub fn parse_region_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    let suffix = extract_parameter_suffix(key)?;

    // Check if it's a region keyword (starts with R)
    if !key.starts_with("R") {
        return None;
    }

    let trimmed_value = value.trim();

    match suffix.as_str() {
        // Region n width values - vector of width values for region boundaries → [`MixedKeyword::RnW`]
        "W" => parse_float_vector(trimmed_value)
            .map(|vec| KeywordCreationResult::Mixed(MixedKeyword::RnW(vec)))
            .map_or(Some(KeywordCreationResult::UnableToParse), Some),
        _ => None,
    }
}
