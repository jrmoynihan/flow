use super::{ByteKeyword, FloatKeyword, IntegerKeyword, KeywordCreationResult, MixedKeyword, StringKeyword};
use crate::{byteorder::ByteOrder, datatype::FcsDataType};
use std::sync::Arc;
use super::helpers::{extract_parameter_parts, is_parameter_keyword, parse_float_tuple, parse_float_vector, parse_float_with_comma_decimal, parse_pnd, parse_spillover};

/// Parse fixed (non-parameterized) keywords
pub fn parse_fixed_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    let trimmed_value = value.trim();
    
    match key {
        "FIL" => Some(KeywordCreationResult::String(StringKeyword::FIL(Arc::from(trimmed_value)))),
        "GUID" => Some(KeywordCreationResult::String(StringKeyword::GUID(Arc::from(trimmed_value)))),
        "BYTEORD" => ByteOrder::from_keyword_str(trimmed_value).ok()
            .map(|byte_order| KeywordCreationResult::Byte(ByteKeyword::BYTEORD(byte_order))),
        "DATATYPE" => FcsDataType::from_keyword_str(trimmed_value).ok()
            .map(|data_type| KeywordCreationResult::Byte(ByteKeyword::DATATYPE(data_type))),
        "PAR" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::PAR(n))),
        "TOT" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::TOT(n))),
        "BEGINDATA" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::BeginData(n))),
        "ENDDATA" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::EndData(n))),
        "BEGINANALYSIS" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::BeginAnalysis(n))),
        "ENDANALYSIS" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::EndAnalysis(n))),
        "BEGINTEXT" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::BeginText(n))),
        "ENDTEXT" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::EndText(n))),
        // Modern FCS 3.2 keywords
        "BEGINDATETIME" => Some(KeywordCreationResult::String(StringKeyword::BEGINDATETIME(Arc::from(trimmed_value)))),
        "ENDDATETIME" => Some(KeywordCreationResult::String(StringKeyword::ENDDATETIME(Arc::from(trimmed_value)))),
        "CARRIERID" => Some(KeywordCreationResult::String(StringKeyword::CARRIERID(Arc::from(trimmed_value)))),
        "CARRIERTYPE" => Some(KeywordCreationResult::String(StringKeyword::CARRIERTYPE(Arc::from(trimmed_value)))),
        "LOCATIONID" => Some(KeywordCreationResult::String(StringKeyword::LOCATIONID(Arc::from(trimmed_value)))),
        "FLOWRATE" => Some(KeywordCreationResult::String(StringKeyword::FLOWRATE(Arc::from(trimmed_value)))),
        "SPILLOVER" => parse_spillover(trimmed_value)
            .map(KeywordCreationResult::Mixed),
        "VOL" => Some(KeywordCreationResult::String(StringKeyword::VOL(Arc::from(trimmed_value)))),
        "ORIGINALITY" => Some(KeywordCreationResult::String(StringKeyword::ORIGINALITY(Arc::from(trimmed_value)))),
        "LAST_MODIFIER" => Some(KeywordCreationResult::String(StringKeyword::LastModifier(Arc::from(trimmed_value)))),
        "LAST_MODIFIED" => Some(KeywordCreationResult::String(StringKeyword::LastModified(Arc::from(trimmed_value)))),
        // Deprecated keywords
        "DATE" => Some(KeywordCreationResult::String(StringKeyword::DATE(Arc::from(trimmed_value)))),
        "BTIM" => Some(KeywordCreationResult::String(StringKeyword::BTIM(Arc::from(trimmed_value)))),
        "ETIM" => Some(KeywordCreationResult::String(StringKeyword::ETIM(Arc::from(trimmed_value)))),
        "MODE" => Some(KeywordCreationResult::String(StringKeyword::MODE(Arc::from(trimmed_value)))),
        "PLATEID" => Some(KeywordCreationResult::String(StringKeyword::PLATEID(Arc::from(trimmed_value)))),
        "PLATENAME" => Some(KeywordCreationResult::String(StringKeyword::PLATENAME(Arc::from(trimmed_value)))),
        "WELLID" => Some(KeywordCreationResult::String(StringKeyword::WELLID(Arc::from(trimmed_value)))),
        "GATE" => Some(KeywordCreationResult::String(StringKeyword::GATE(Arc::from(trimmed_value)))),
        "CYT" => Some(KeywordCreationResult::String(StringKeyword::CYT(Arc::from(trimmed_value)))),
        _ => None,
    }
}

/// Parse parameter keywords (Pn*)
pub fn parse_parameter_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    if !is_parameter_keyword(key) {
        return None;
    }

    let parts = extract_parameter_parts(key)?;
    let trimmed_value = value.trim();

    match parts.suffix.as_str() {
        "G" => parse_float_with_comma_decimal(trimmed_value)
            .map(|gain| KeywordCreationResult::Float(FloatKeyword::PnG(gain))),
        "E" => {
            // Try parsing as tuple (f32, f32) first
            if let Some((f1, f2)) = parse_float_tuple(trimmed_value) {
                Some(KeywordCreationResult::Mixed(MixedKeyword::PnE(f1, f2)))
            } else {
                // Fallback: try single float for backwards compatibility
                parse_float_with_comma_decimal(trimmed_value)
                    .map(|single_val| KeywordCreationResult::Mixed(MixedKeyword::PnE(single_val, 0.0)))
            }
        }
        "R" => trimmed_value.parse::<usize>().ok()
            .map(|range| KeywordCreationResult::Int(IntegerKeyword::PnR(range))),
        "B" => trimmed_value.parse::<usize>().ok()
            .map(|bits| KeywordCreationResult::Int(IntegerKeyword::PnB(bits))),
        "V" => trimmed_value.parse::<usize>().ok()
            .map(|voltage_range| KeywordCreationResult::Int(IntegerKeyword::PnV(voltage_range))),
        "L" => {
            // Parse as comma-separated list of wavelengths
            // Handle FCS format with parentheses: (488) or (488,532,633)
            let cleaned_value = trimmed_value
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(trimmed_value);

            let wavelengths: Option<Vec<usize>> = cleaned_value
                .split(',')
                .map(|s| s.trim().parse::<usize>().ok())
                .collect();

            wavelengths.map(|wl| KeywordCreationResult::Mixed(MixedKeyword::PnL(wl)))
        }
        "Display" => trimmed_value.parse::<usize>().ok()
            .map(|display_scale| KeywordCreationResult::Int(IntegerKeyword::PnDisplay(display_scale))),
        "N" => Some(KeywordCreationResult::String(StringKeyword::PnN(Arc::from(trimmed_value)))),
        "S" => Some(KeywordCreationResult::String(StringKeyword::PnS(Arc::from(trimmed_value)))),
        "F" => Some(KeywordCreationResult::String(StringKeyword::PnF(Arc::from(trimmed_value)))),
        "Type" | "TYPE" => Some(KeywordCreationResult::String(StringKeyword::PnType(Arc::from(trimmed_value)))),
        "DET" => Some(KeywordCreationResult::String(StringKeyword::PnDET(Arc::from(trimmed_value)))),
        "TAG" => Some(KeywordCreationResult::String(StringKeyword::PnTAG(Arc::from(trimmed_value)))),
        "ANALYTE" => Some(KeywordCreationResult::String(StringKeyword::PnANALYTE(Arc::from(trimmed_value)))),
        "FEATURE" => Some(KeywordCreationResult::String(StringKeyword::PnFEATURE(Arc::from(trimmed_value)))),
        "D" => parse_pnd(trimmed_value)
            .map(KeywordCreationResult::Mixed),
        "DATATYPE" => trimmed_value.parse::<usize>().ok()
            .map(|n| KeywordCreationResult::Int(IntegerKeyword::PnDATATYPE(n))),
        _ => {
            eprintln!(
                "Unknown parameter keyword suffix: '{}' for key: '{}' with value: '{}'",
                parts.suffix, key, value
            );
            None
        }
    }
}

/// Parse deprecated gate keywords (Gn*)
pub fn parse_gate_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    let parts = extract_parameter_parts(key)?;
    
    // Check if it's a gate keyword (starts with G)
    if !key.starts_with("G") {
        return None;
    }
    
    let trimmed_value = value.trim();

    match parts.suffix.as_str() {
        "E" => {
            if let Some((f1, f2)) = parse_float_tuple(trimmed_value) {
                Some(KeywordCreationResult::Mixed(MixedKeyword::GnE(f1, f2)))
            } else {
                Some(KeywordCreationResult::Mixed(MixedKeyword::GnE(0.0, 0.0)))
            }
        }
        "F" => Some(KeywordCreationResult::String(StringKeyword::GnF(Arc::from(trimmed_value)))),
        "N" => Some(KeywordCreationResult::String(StringKeyword::GnN(Arc::from(trimmed_value)))),
        "P" => Some(KeywordCreationResult::String(StringKeyword::GnP(Arc::from(trimmed_value)))),
        "R" => Some(KeywordCreationResult::String(StringKeyword::GnR(Arc::from(trimmed_value)))),
        "S" => Some(KeywordCreationResult::String(StringKeyword::GnS(Arc::from(trimmed_value)))),
        "T" => Some(KeywordCreationResult::String(StringKeyword::GnT(Arc::from(trimmed_value)))),
        "V" => Some(KeywordCreationResult::String(StringKeyword::GnV(Arc::from(trimmed_value)))),
        _ => None,
    }
}

/// Parse region keywords (Rn*)
pub fn parse_region_keywords(key: &str, value: &str) -> Option<KeywordCreationResult> {
    let parts = extract_parameter_parts(key)?;
    
    // Check if it's a region keyword (starts with R)
    if !key.starts_with("R") {
        return None;
    }
    
    let trimmed_value = value.trim();

    match parts.suffix.as_str() {
        "W" => parse_float_vector(trimmed_value)
            .map(|vec| KeywordCreationResult::Mixed(MixedKeyword::RnW(vec))),
        _ => None,
    }
}

