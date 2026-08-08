use super::*;
use crate::datatype::FcsDataType;

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
    fn test_parse_p5r_float() {
        // Some cytometers output floats for $PnR (e.g., "1.1")
        let result = match_and_parse_keyword("$P5R", "1.1");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::PnR(1))
        ));
    }

    #[test]
    fn test_parse_p61r_float() {
        // Test with larger parameter number and float value
        let result = match_and_parse_keyword("$P61R", "1.1");
        assert!(matches!(
            result,
            KeywordCreationResult::Int(IntegerKeyword::PnR(1))
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

    #[test]
    fn test_parse_p23display_uppercase() {
        let result = match_and_parse_keyword("$P23DISPLAY", "4");
        assert!(matches!(
            result,
            KeywordCreationResult::String(StringKeyword::PnDISPLAY(s)) if s.as_ref() == "4"
        ));
    }

    #[test]
    fn test_parse_p1display_mixed_case() {
        let result = match_and_parse_keyword("$P1Display", "2");
        assert!(matches!(
            result,
            KeywordCreationResult::String(StringKeyword::PnDISPLAY(s)) if s.as_ref() == "2"
        ));
    }

    #[test]
    fn test_parse_p5display_lowercase() {
        let result = match_and_parse_keyword("$P5display", "1");
        assert!(matches!(
            result,
            KeywordCreationResult::String(StringKeyword::PnDISPLAY(s)) if s.as_ref() == "1"
        ));
    }

    #[test]
    fn test_parse_p10type_uppercase() {
        let result = match_and_parse_keyword("$P10TYPE", "FSC");
        if let KeywordCreationResult::String(StringKeyword::PnType(ty)) = result {
            assert_eq!(ty.as_ref(), "FSC");
        } else {
            panic!("Expected P10TYPE keyword");
        }
    }

    #[test]
    fn test_parse_p20type_mixed_case() {
        let result = match_and_parse_keyword("$P20Type", "SSC");
        if let KeywordCreationResult::String(StringKeyword::PnType(ty)) = result {
            assert_eq!(ty.as_ref(), "SSC");
        } else {
            panic!("Expected P20Type keyword");
        }
    }

    #[test]
    fn test_parse_p1datatype_character_f() {
        // FCS 3.2 spec: $PnDATATYPE uses character format like $DATATYPE
        let result = match_and_parse_keyword("$P1DATATYPE", "F");
        if let KeywordCreationResult::Byte(ByteKeyword::PnDATATYPE(data_type)) = result {
            assert_eq!(data_type, FcsDataType::F);
        } else {
            panic!("Expected P1DATATYPE as ByteKeyword");
        }
    }

    #[test]
    fn test_parse_p61datatype_character_f() {
        // Test with larger parameter number
        let result = match_and_parse_keyword("$P61DATATYPE", "F");
        if let KeywordCreationResult::Byte(ByteKeyword::PnDATATYPE(data_type)) = result {
            assert_eq!(data_type, FcsDataType::F);
        } else {
            panic!("Expected P61DATATYPE as ByteKeyword");
        }
    }

    #[test]
    fn test_parse_p2datatype_character_d() {
        let result = match_and_parse_keyword("$P2DATATYPE", "D");
        if let KeywordCreationResult::Byte(ByteKeyword::PnDATATYPE(data_type)) = result {
            assert_eq!(data_type, FcsDataType::D);
        } else {
            panic!("Expected P2DATATYPE as ByteKeyword");
        }
    }

    #[test]
    fn test_parse_p3datatype_character_i() {
        let result = match_and_parse_keyword("$P3DATATYPE", "I");
        if let KeywordCreationResult::Byte(ByteKeyword::PnDATATYPE(data_type)) = result {
            assert_eq!(data_type, FcsDataType::I);
        } else {
            panic!("Expected P3DATATYPE as ByteKeyword");
        }
    }

    #[test]
    fn test_parse_p4datatype_invalid() {
        // Invalid values should return UnableToParse
        let result = match_and_parse_keyword("$P4DATATYPE", "X");
        assert!(matches!(result, KeywordCreationResult::UnableToParse));
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
    use crate::keyword::helpers::validate_pnd_scale_type;

    #[test]
    fn test_validate_pnd_scale_type_linear() {
        assert!(validate_pnd_scale_type("Linear"));
    }

    #[test]
    fn test_validate_pnd_scale_type_logarithmic() {
        assert!(validate_pnd_scale_type("Logarithmic"));
    }

    #[test]
    fn test_validate_pnd_scale_type_invalid() {
        assert!(!validate_pnd_scale_type("Invalid"));
        assert!(!validate_pnd_scale_type("linear"));
        assert!(!validate_pnd_scale_type("LOGARITHMIC"));
    }
}

#[cfg(test)]
mod helpers {
    use crate::keyword::helpers::{
        extract_parameter_suffix, is_parameter_keyword, parse_float_tuple, parse_float_vector,
        parse_float_with_comma_decimal,
    };

    #[test]
    fn test_extract_parameter_suffix_p1n() {
        let suffix = extract_parameter_suffix("P1N").unwrap();
        assert_eq!(suffix, "N");
    }

    #[test]
    fn test_extract_parameter_suffix_p123n() {
        let suffix = extract_parameter_suffix("P123N").unwrap();
        assert_eq!(suffix, "N");
    }

    #[test]
    fn test_extract_parameter_suffix_g1e() {
        let suffix = extract_parameter_suffix("G1E").unwrap();
        assert_eq!(suffix, "E");
    }

    #[test]
    fn test_extract_parameter_suffix_r1w() {
        let suffix = extract_parameter_suffix("R1W").unwrap();
        assert_eq!(suffix, "W");
    }

    #[test]
    fn test_extract_parameter_suffix_invalid() {
        assert!(extract_parameter_suffix("INVALID").is_none());
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
        // Valid parameter keywords must have a suffix (e.g., P1N, P123S)
        // According to FCS spec, $PnX format requires suffix letter X
        // This function checks for P, G, and R prefixes (parameter, gate, region keywords)
        assert!(is_parameter_keyword("P1N")); // Valid: has suffix N
        assert!(is_parameter_keyword("P123S")); // Valid: has suffix S
        assert!(is_parameter_keyword("P2G")); // Valid: has suffix G
        assert!(is_parameter_keyword("G1E")); // Valid gate keyword (deprecated but has suffix)
        assert!(is_parameter_keyword("R1W")); // Valid region keyword (has suffix)
        assert!(!is_parameter_keyword("P1")); // Invalid: no suffix
        assert!(!is_parameter_keyword("P123")); // Invalid: no suffix
        assert!(!is_parameter_keyword("G1")); // Invalid: no suffix
        assert!(!is_parameter_keyword("R1")); // Invalid: no suffix
        assert!(!is_parameter_keyword("INVALID")); // Invalid: not a parameter keyword pattern
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

/// flow-crates-x17.4: the FCS 3.2 keyword set. `$UNSTAINEDINFO` and
/// `$UNSTAINEDCENTERS` are new here; the rest were already implemented but
/// untested, and they share the same failure mode worth guarding against.
///
/// A missing dispatch arm in `match_and_parse_keyword` does not fail loudly -
/// it falls through to `StringKeyword::Other`, which still parses, still
/// serializes, and still round-trips. The keyword simply stops being
/// recognizable by type. So these tests assert the *variant*, not just the
/// value, and separately assert that `get_str` (the accessor the writer uses
/// at write.rs:596) returns the value rather than an empty catch-all.
mod fcs_3_2_keywords {
    use super::*;

    /// Assert `key` parses to the variant selected by `expect`, and that the
    /// value survives the `get_str` accessor the serializer goes through.
    fn assert_round_trips(
        key: &str,
        value: &str,
        expect: fn(&StringKeyword) -> bool,
    ) {
        let result = match_and_parse_keyword(key, value);
        let KeywordCreationResult::String(sk) = result else {
            panic!("{key} did not parse as a string keyword");
        };
        assert!(
            expect(&sk),
            "{key} parsed to the wrong variant (likely fell through to Other)"
        );
        assert_eq!(
            sk.get_str(),
            value,
            "{key} lost its value passing through get_str"
        );
    }

    #[test]
    fn unstainedinfo_parses_and_round_trips() {
        assert_round_trips(
            "$UNSTAINEDINFO",
            "unstained control acquired 2026-08-06, 50000 events",
            |sk| matches!(sk, StringKeyword::UNSTAINEDINFO(_)),
        );
    }

    #[test]
    fn unstainedcenters_parses_and_round_trips() {
        // The spec's structured form. Kept opaque on purpose - this asserts
        // the delimiters and precision survive verbatim.
        assert_round_trips(
            "$UNSTAINEDCENTERS",
            "3,FSC-A,SSC-A,FL1-A,102.5,88.25,1043.0",
            |sk| matches!(sk, StringKeyword::UNSTAINEDCENTERS(_)),
        );
    }

    #[test]
    fn the_rest_of_the_3_2_string_keywords_reach_their_variants() {
        assert_round_trips("$BEGINDATETIME", "2026-08-06T09:15:00Z", |sk| {
            matches!(sk, StringKeyword::BEGINDATETIME(_))
        });
        assert_round_trips("$ENDDATETIME", "2026-08-06T09:22:31Z", |sk| {
            matches!(sk, StringKeyword::ENDDATETIME(_))
        });
        assert_round_trips("$CARRIERID", "PLATE-0042", |sk| {
            matches!(sk, StringKeyword::CARRIERID(_))
        });
        assert_round_trips("$CARRIERTYPE", "96 well plate", |sk| {
            matches!(sk, StringKeyword::CARRIERTYPE(_))
        });
        assert_round_trips("$LOCATIONID", "H12", |sk| {
            matches!(sk, StringKeyword::LOCATIONID(_))
        });
        assert_round_trips("$FLOWRATE", "30 uL/min", |sk| {
            matches!(sk, StringKeyword::FLOWRATE(_))
        });
    }
}

/// `$TRUOLS_MIXMAT` - the rectangular mixing matrix an unmixing step solved
/// against.
#[cfg(test)]
mod mixing_matrix {
    use super::*;

    /// 2 detectors x 3 endmembers. Deliberately non-square: the whole reason
    /// this keyword exists rather than reusing `$SPILLOVER` is that a real
    /// panel never has one endmember per detector.
    const RECTANGULAR: &str = "2,3,B1-A,B2-A,FITC,PE,AF,0.9,0.1,0.02,0.05,0.8,0.3";

    fn parse(value: &str) -> KeywordCreationResult {
        match_and_parse_keyword("$TRUOLS_MIXMAT", value)
    }

    #[test]
    fn a_rectangular_matrix_keeps_both_dimensions_and_both_name_lists() {
        let KeywordCreationResult::Mixed(MixedKeyword::MixingMatrix {
            n_detectors,
            n_endmembers,
            detector_names,
            endmember_names,
            matrix_values,
        }) = parse(RECTANGULAR)
        else {
            panic!("$TRUOLS_MIXMAT did not reach MixedKeyword::MixingMatrix");
        };

        assert_eq!((n_detectors, n_endmembers), (2, 3));
        assert_eq!(detector_names, ["B1-A", "B2-A"]);
        assert_eq!(endmember_names, ["FITC", "PE", "AF"]);
        // Row-major: detector B1-A's three coefficients come first.
        assert_eq!(matrix_values, [0.9, 0.1, 0.02, 0.05, 0.8, 0.3]);
    }

    /// The names are split by count, not by any marker, so a wrong `nDet`
    /// would silently steal an endmember name. Pin the boundary with
    /// dimensions that cannot be confused for each other.
    #[test]
    fn the_name_split_follows_the_declared_detector_count() {
        let KeywordCreationResult::Mixed(MixedKeyword::MixingMatrix {
            detector_names,
            endmember_names,
            ..
        }) = parse("3,1,D1,D2,D3,ONLY-EM,1.0,2.0,3.0")
        else {
            panic!("did not parse");
        };
        assert_eq!(detector_names, ["D1", "D2", "D3"]);
        assert_eq!(endmember_names, ["ONLY-EM"]);
    }

    /// A matrix of the wrong shape produces silently wrong abundances
    /// downstream, so anything that does not parse exactly must be rejected
    /// rather than reshaped. Falling through to `StringKeyword::Other` keeps
    /// the raw text recoverable without pretending it is a matrix.
    #[test]
    fn malformed_input_falls_through_instead_of_panicking() {
        for (case, value) in [
            ("one value short", "2,3,B1-A,B2-A,FITC,PE,AF,0.9,0.1,0.02,0.05,0.8"),
            (
                "one value long",
                "2,3,B1-A,B2-A,FITC,PE,AF,0.9,0.1,0.02,0.05,0.8,0.3,0.7",
            ),
            ("names truncated", "2,3,B1-A,FITC,PE,AF,0.9,0.1,0.02,0.05,0.8,0.3"),
            (
                "non-numeric value",
                "2,3,B1-A,B2-A,FITC,PE,AF,0.9,0.1,0.02,0.05,0.8,not-a-number",
            ),
            ("no endmember count", "2"),
            ("empty", ""),
        ] {
            assert!(
                matches!(
                    parse(value),
                    KeywordCreationResult::String(StringKeyword::Other(_))
                ),
                "{case}: expected a fall-through to Other, got a parsed matrix"
            );
        }
    }

    /// `n_detectors * n_endmembers` is computed from untrusted file input.
    #[test]
    fn an_absurd_declared_size_does_not_overflow() {
        let value = format!("{},{},x,y", usize::MAX, usize::MAX);
        assert!(matches!(
            parse(&value),
            KeywordCreationResult::String(StringKeyword::Other(_))
        ));
    }
}
