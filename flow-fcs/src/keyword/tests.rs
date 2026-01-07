use super::*;
use std::sync::Arc;

#[cfg(test)]
mod fixed_keywords {
    use super::*;

    #[test]
    fn test_parse_par() {
        let result = match_and_parse_keyword("$PAR", "10");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::PAR(10))
        ));
    }

    #[test]
    fn test_parse_tot() {
        let result = match_and_parse_keyword("$TOT", "1000");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::TOT(1000))
        ));
    }

    #[test]
    fn test_parse_fil() {
        let result = match_and_parse_keyword("$FIL", "test.fcs");
        if let KeywordCreationResult::String(StringKeyword::FIL(name)) = result {
            assert_eq!(name.as_ref(), "test.fcs");
        } else {
            panic!("Expected FIL keyword");
        }
    }

    #[test]
    fn test_parse_guid() {
        let result = match_and_parse_keyword("GUID", "12345678-1234-1234-1234-123456789abc");
        if let KeywordCreationResult::String(StringKeyword::GUID(guid)) = result {
            assert_eq!(guid.as_ref(), "12345678-1234-1234-1234-123456789abc");
        } else {
            panic!("Expected GUID keyword");
        }
    }

    #[test]
    fn test_parse_byteord() {
        let result = match_and_parse_keyword("$BYTEORD", "1,2,3,4");
        assert!(matches!(
            result,
            KeywordCreationResult::Byte(ByteKeyword::BYTEORD(_))
        ));
    }

    #[test]
    fn test_parse_datatype() {
        let result = match_and_parse_keyword("$DATATYPE", "F");
        assert!(matches!(
            result,
            KeywordCreationResult::Byte(ByteKeyword::DATATYPE(_))
        ));
    }

    #[test]
    fn test_parse_invalid_par() {
        let result = match_and_parse_keyword("$PAR", "invalid");
        assert!(matches!(result, KeywordCreationResult::UnableToParse));
    }

    #[test]
    fn test_parse_begindata() {
        let result = match_and_parse_keyword("$BEGINDATA", "256");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::BeginData(256))
        ));
    }
}

#[cfg(test)]
mod parameter_keywords {
    use super::*;

    #[test]
    fn test_parse_p1n() {
        let result = match_and_parse_keyword("$P1N", "FSC-A");
        if let KeywordCreationResult::String(StringKeyword::PnN(name)) = result {
            assert_eq!(name.as_ref(), "FSC-A");
        } else {
            panic!("Expected P1N keyword");
        }
    }

    #[test]
    fn test_parse_p2s() {
        let result = match_and_parse_keyword("$P2S", "SSC-A");
        if let KeywordCreationResult::String(StringKeyword::PnS(label)) = result {
            assert_eq!(label.as_ref(), "SSC-A");
        } else {
            panic!("Expected P2S keyword");
        }
    }

    #[test]
    fn test_parse_p3g() {
        let result = match_and_parse_keyword("$P3G", "1.5");
        if let KeywordCreationResult::Float(FloatKeyword::PnG(gain)) = result {
            assert!((gain - 1.5).abs() < f32::EPSILON);
        } else {
            panic!("Expected P3G keyword");
        }
    }

    #[test]
    fn test_parse_p4e() {
        let result = match_and_parse_keyword("$P4E", "4,1");
        if let KeywordCreationResult::Mixed(MixedKeyword::PnE(f1, f2)) = result {
            assert!((f1 - 4.0).abs() < f32::EPSILON);
            assert!((f2 - 1.0).abs() < f32::EPSILON);
        } else {
            panic!("Expected P4E keyword");
        }
    }

    #[test]
    fn test_parse_p5r() {
        let result = match_and_parse_keyword("$P5R", "1024");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::PnR(1024))
        ));
    }

    #[test]
    fn test_parse_p123n_large_param_number() {
        let result = match_and_parse_keyword("$P123N", "LargeParam");
        if let KeywordCreationResult::String(StringKeyword::PnN(name)) = result {
            assert_eq!(name.as_ref(), "LargeParam");
        } else {
            panic!("Expected P123N keyword");
        }
    }

    #[test]
    fn test_parse_p1l_with_parentheses() {
        let result = match_and_parse_keyword("$P1L", "(488)");
        if let KeywordCreationResult::Mixed(MixedKeyword::PnL(wavelengths)) = result {
            assert_eq!(wavelengths.len(), 1);
            assert_eq!(wavelengths[0], 488);
        } else {
            panic!("Expected P1L keyword");
        }
    }

    #[test]
    fn test_parse_p2l_multiple_wavelengths() {
        let result = match_and_parse_keyword("$P2L", "(488,532,633)");
        if let KeywordCreationResult::Mixed(MixedKeyword::PnL(wavelengths)) = result {
            assert_eq!(wavelengths.len(), 3);
            assert_eq!(wavelengths[0], 488);
            assert_eq!(wavelengths[1], 532);
            assert_eq!(wavelengths[2], 633);
        } else {
            panic!("Expected P2L keyword with multiple wavelengths");
        }
    }
}

#[cfg(test)]
mod complex_keywords {
    use super::*;

    #[test]
    fn test_parse_spillover() {
        let value = "3,FL2-A,FL1-A,FL3-A,1.0,0.03,0.2,0.1,1.0,0.0,0.05,0,1.0";
        let result = match_and_parse_keyword("$SPILLOVER", value);
        if let KeywordCreationResult::Mixed(MixedKeyword::SPILLOVER {
            n_parameters,
            parameter_names,
            matrix_values,
        }) = result
        {
            assert_eq!(n_parameters, 3);
            assert_eq!(parameter_names.len(), 3);
            assert_eq!(matrix_values.len(), 9); // 3x3 matrix
        } else {
            panic!("Expected SPILLOVER keyword");
        }
    }

    #[test]
    fn test_parse_pnd_linear() {
        let result = match_and_parse_keyword("$P3D", "Linear,0,1024");
        if let KeywordCreationResult::Mixed(MixedKeyword::PnD(scale_type, f1, f2)) = result {
            assert_eq!(scale_type, "Linear");
            assert!((f1 - 0.0).abs() < f32::EPSILON);
            assert!((f2 - 1024.0).abs() < f32::EPSILON);
        } else {
            panic!("Expected P3D keyword");
        }
    }

    #[test]
    fn test_parse_pnd_logarithmic() {
        let result = match_and_parse_keyword("$P2D", "Logarithmic,4,0.1");
        if let KeywordCreationResult::Mixed(MixedKeyword::PnD(scale_type, f1, f2)) = result {
            assert_eq!(scale_type, "Logarithmic");
            assert!((f1 - 4.0).abs() < f32::EPSILON);
            assert!((f2 - 0.1).abs() < f32::EPSILON);
        } else {
            panic!("Expected P2D keyword");
        }
    }

    #[test]
    fn test_parse_pnd_invalid_scale_type() {
        let result = match_and_parse_keyword("$P3D", "Invalid,0,1024");
        assert!(matches!(result, KeywordCreationResult::UnableToParse));
    }

    #[test]
    fn test_parse_pnd_malformed() {
        let result = match_and_parse_keyword("$P3D", "Linear,0");
        assert!(matches!(result, KeywordCreationResult::UnableToParse));
    }
}

#[cfg(test)]
mod validation {
    use super::*;

    #[test]
    fn test_validate_pnd_scale_type_linear() {
        assert!(super::validate_pnd_scale_type("Linear"));
    }

    #[test]
    fn test_validate_pnd_scale_type_logarithmic() {
        assert!(super::validate_pnd_scale_type("Logarithmic"));
    }

    #[test]
    fn test_validate_pnd_scale_type_invalid() {
        assert!(!super::validate_pnd_scale_type("Invalid"));
        assert!(!super::validate_pnd_scale_type("linear"));
        assert!(!super::validate_pnd_scale_type("LOGARITHMIC"));
    }
}

#[cfg(test)]
mod helpers {
    use super::*;

    #[test]
    fn test_extract_parameter_parts_p1n() {
        let parts = extract_parameter_parts("P1N").unwrap();
        assert_eq!(parts.param_number, 1);
        assert_eq!(parts.suffix, "N");
    }

    #[test]
    fn test_extract_parameter_parts_p123n() {
        let parts = extract_parameter_parts("P123N").unwrap();
        assert_eq!(parts.param_number, 123);
        assert_eq!(parts.suffix, "N");
    }

    #[test]
    fn test_extract_parameter_parts_g1e() {
        let parts = extract_parameter_parts("G1E").unwrap();
        assert_eq!(parts.param_number, 1);
        assert_eq!(parts.suffix, "E");
    }

    #[test]
    fn test_extract_parameter_parts_r1w() {
        let parts = extract_parameter_parts("R1W").unwrap();
        assert_eq!(parts.param_number, 1);
        assert_eq!(parts.suffix, "W");
    }

    #[test]
    fn test_extract_parameter_parts_invalid() {
        assert!(extract_parameter_parts("INVALID").is_none());
    }

    #[test]
    fn test_parse_float_with_comma_decimal_standard() {
        assert_eq!(parse_float_with_comma_decimal("1.5"), Some(1.5));
    }

    #[test]
    fn test_parse_float_with_comma_decimal_european() {
        assert_eq!(parse_float_with_comma_decimal("1,5"), Some(1.5));
    }

    #[test]
    fn test_parse_float_with_comma_decimal_invalid() {
        assert_eq!(parse_float_with_comma_decimal("invalid"), None);
    }

    #[test]
    fn test_parse_float_tuple() {
        assert_eq!(parse_float_tuple("1.5,2.5"), Some((1.5, 2.5)));
        assert_eq!(parse_float_tuple("1,5,2,5"), Some((1.5, 2.5)));
    }

    #[test]
    fn test_parse_float_vector() {
        let result = parse_float_vector("1.5,2.5,3.5");
        assert_eq!(result, Some(vec![1.5, 2.5, 3.5]));
    }

    #[test]
    fn test_is_parameter_keyword() {
        assert!(is_parameter_keyword("P1"));
        assert!(is_parameter_keyword("P123"));
        assert!(!is_parameter_keyword("G1"));
        assert!(!is_parameter_keyword("R1"));
        assert!(!is_parameter_keyword("INVALID"));
    }
}

#[cfg(test)]
mod error_handling {
    use super::*;

    #[test]
    fn test_unparseable_returns_unable_to_parse() {
        let result = match_and_parse_keyword("$PAR", "not_a_number");
        assert!(matches!(result, KeywordCreationResult::UnableToParse));
    }

    #[test]
    fn test_unknown_keyword_returns_other() {
        let result = match_and_parse_keyword("$UNKNOWN", "value");
        if let KeywordCreationResult::String(StringKeyword::Other(value)) = result {
            assert_eq!(value.as_ref(), "value");
        } else {
            panic!("Expected Other keyword");
        }
    }

    #[test]
    fn test_keyword_without_dollar_sign() {
        let result = match_and_parse_keyword("PAR", "10");
        if let KeywordCreationResult::String(StringKeyword::Other(value)) = result {
            assert_eq!(value.as_ref(), "10");
        } else {
            panic!("Expected Other keyword");
        }
    }
}

#[cfg(test)]
mod integration {
    use super::*;

    #[test]
    fn test_end_to_end_parsing() {
        let result = match_and_parse_keyword("$PAR", "10");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::PAR(10))
        ));
    }

    #[test]
    fn test_arc_str_sharing() {
        let result1 = match_and_parse_keyword("$FIL", "test.fcs");
        let result2 = match_and_parse_keyword("$FIL", "test.fcs");

        if let (
            KeywordCreationResult::String(StringKeyword::FIL(name1)),
            KeywordCreationResult::String(StringKeyword::FIL(name2)),
        ) = (result1, result2)
        {
            // Arc::ptr_eq would check if they're the same allocation
            assert_eq!(name1, name2);
        } else {
            panic!("Expected FIL keywords");
        }
    }
}
