/// Shared two-data-set `$NEXTDATA`-chain fixture used by both
/// `write::offset_convergence_tests::open_all_traverses_nextdata_chain_across_two_datasets`
/// (flow-crates-1mg) and `file::nextdata_escaping_tests` (flow-crates-1xb). Both
/// exercise the same file shape: data set 1 has a HEADER whose `$NEXTDATA` points at
/// data set 2's TEXT start, and data set 2 has NO HEADER of its own.
///
/// Note on what this fixture does *not* prove: because both data sets are produced
/// by `serialize_metadata`, which always emits `$BEGINDATA` ahead of every user
/// keyword, `Fcs::find_begindata_offset`'s early-stopping scan of a headerless data
/// set here only ever tokenizes synthesized digit-only values before it matches — it
/// never crosses an escaped keyword. `find_begindata_offset` scans the *headerless*
/// data set's own TEXT (called from `header_for_dataset_at` with that data set's own
/// `dataset_start`), never a preceding data set's TEXT. An escaped `$CYT` on data set
/// 1 here is read by `Metadata::from_text_segment` off a real HEADER and round-trips
/// across the chain, which is worth checking, but it does not exercise
/// `find_begindata_offset` at all. See `file::nextdata_escaping_tests` for the
/// hand-assembled TEXT segment that actually pins that scan's escaping policy.
///
/// Writes a fixed, deterministic file: data set 1 holds one `FSC-A` event triple
/// `[1.0, 2.0, 3.0]`, data set 2 holds `[10.0, 20.0, 30.0]`, both under the default
/// space delimiter. `cyt` becomes data set 1's `$CYT` value (it must be non-empty —
/// the writer rejects empty keyword values under `V3_1`+). Data set 2 carries no
/// `$CYT` at all, so it stays a "plain" data set for tests that only care about data
/// set 1's escaping.
///
/// # Panics
/// Panics if `cyt` does not contain the fixture's active delimiter — the
/// same delimiter `Metadata::new()` defaults to, since this fixture never
/// overrides it. Callers that assert an escaped value round-trips depend on
/// `cyt` actually needing escaping; a `cyt` without the delimiter would make
/// that assertion vacuously true even if escaping were broken. This is a
/// plain `assert!`, not `debug_assert!`, because it guards `#[cfg(test)]`
/// code only — there is no release-build cost to justify `debug_assert!`,
/// and `debug_assert!` silently compiles out under `cargo nextest run
/// --release`, which would defeat the guard entirely.
#[cfg(test)]
pub(crate) fn write_two_dataset_fixture(
    path: &std::path::Path,
    version: crate::version::Version,
    cyt: &str,
) {
    use crate::byteorder::ByteOrder;
    use crate::keyword::{ByteKeyword, IntegerKeyword, Keyword, MixedKeyword};
    use crate::metadata::Metadata;
    use crate::write::{FcsLayout, build_header, resolve_layout, serialize_f32_columns};

    // Derived from `Metadata::new()` rather than hardcoded, so this guard
    // can't silently go vacuous if that default ever changes.
    let active_delimiter = Metadata::new().delimiter;
    assert!(
        cyt.contains(active_delimiter),
        "write_two_dataset_fixture's `cyt` must contain the active delimiter \
         ({active_delimiter:?}, from Metadata::new()'s default), or callers \
         asserting escaped-value round-trips would pass vacuously: got {cyt:?}"
    );

    fn build_dataset_metadata(nextdata: usize, cyt: Option<&str>) -> Metadata {
        let mut metadata = Metadata::new();
        metadata.keywords.insert(
            "$BYTEORD".to_string(),
            Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)),
        );
        metadata.keywords.insert(
            "$DATATYPE".to_string(),
            Keyword::Byte(ByteKeyword::DATATYPE(crate::datatype::FcsDataType::F)),
        );
        metadata.insert_string_keyword("$MODE".into(), "L".into());
        metadata.insert_string_keyword("$NEXTDATA".into(), nextdata.to_string());
        if let Some(cyt) = cyt {
            metadata.insert_string_keyword("$CYT".into(), cyt.into());
        }
        metadata.insert_string_keyword("$P1N".into(), "FSC-A".into());
        metadata
            .keywords
            .insert("$P1B".to_string(), Keyword::Int(IntegerKeyword::PnB(32)));
        metadata.keywords.insert(
            "$P1R".to_string(),
            Keyword::Int(IntegerKeyword::PnR(262_144)),
        );
        metadata.keywords.insert(
            "$P1E".to_string(),
            Keyword::Mixed(MixedKeyword::PnE(0.0, 0.0)),
        );
        metadata
    }

    /// Converge TEXT/DATA offsets for a dataset whose TEXT starts at `text_start`.
    /// Chained datasets do not start at [`crate::header::HEADER_SIZE`] — the 58-byte
    /// primary HEADER exists only once, at file start — which is why `resolve_layout`
    /// takes `text_start` rather than assuming it.
    fn build_dataset_bytes(
        metadata: &Metadata,
        text_start: usize,
        n_events: usize,
        n_params: usize,
        data_bytes: &[u8],
        version: crate::version::Version,
    ) -> (Vec<u8>, usize, usize, usize) {
        let FcsLayout {
            text_segment,
            text_end,
            data_start,
            data_end,
            ..
        } = resolve_layout(
            metadata,
            text_start,
            n_events,
            n_params,
            data_bytes.len(),
            version,
        )
        .expect("layout");
        (text_segment, text_end, data_start, data_end)
    }

    let n_events = 3usize;
    let dataset1_values: [f32; 3] = [1.0, 2.0, 3.0];
    let dataset2_values: [f32; 3] = [10.0, 20.0, 30.0];
    let data_bytes1 = serialize_f32_columns(&[&dataset1_values], true).expect("data1");
    let data_bytes2 = serialize_f32_columns(&[&dataset2_values], true).expect("data2");

    let text_start1 = 58usize;

    // Converge dataset 1's own TEXT/DATA layout AND the file offset where dataset 2's
    // TEXT begins ($NEXTDATA) together: changing $NEXTDATA's digit count changes
    // dataset 1's TEXT length, which shifts where dataset 2 starts.
    let mut next_data_guess = text_start1 + data_bytes1.len() * 2; // arbitrary seed
    let (text_segment1, _text_end1, data_start1, data_end1) = loop {
        let metadata1 = build_dataset_metadata(next_data_guess, Some(cyt));
        let (text_segment1, text_end1, data_start1, data_end1) =
            build_dataset_bytes(&metadata1, text_start1, n_events, 1, &data_bytes1, version);
        let actual_next_data = data_end1 + 1;
        if actual_next_data == next_data_guess {
            break (text_segment1, text_end1, data_start1, data_end1);
        }
        next_data_guess = actual_next_data;
    };
    let text_start2 = data_end1 + 1;

    let metadata2 = build_dataset_metadata(0, None);
    let (text_segment2, _text_end2, _data_start2, data_end2) =
        build_dataset_bytes(&metadata2, text_start2, n_events, 1, &data_bytes2, version);

    let header = build_header(&version, text_start1, data_start1 - 1, data_start1, data_end1)
        .expect("header");

    let mut bytes = header;
    bytes.extend_from_slice(&text_segment1);
    bytes.extend_from_slice(&data_bytes1);
    bytes.extend_from_slice(&text_segment2);
    bytes.extend_from_slice(&data_bytes2);
    assert_eq!(bytes.len(), data_end2 + 1);
    std::fs::write(path, &bytes).expect("write fcs bytes");
}

#[cfg(test)]
mod polars_tests {
    use std::sync::Arc;

    use crate::{
        Fcs, Header, Metadata, Parameter, TransformType,
        file::AccessWrapper,
        parameter::{ParameterMap, ParameterProcessing},
    };
    use polars::{frame::DataFrame, prelude::Column};

    fn create_test_fcs() -> Result<Fcs, Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;

        // Create a temporary file for testing
        let temp_path = std::env::temp_dir().join("test_fcs_temp.tmp");
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(b"test")?;
        }

        // Create test DataFrame
        let mut columns = Vec::new();
        columns.push(Column::new(
            "FSC-A".into(),
            vec![100.0f32, 200.0, 300.0, 400.0, 500.0],
        ));
        columns.push(Column::new(
            "SSC-A".into(),
            vec![50.0f32, 150.0, 250.0, 350.0, 450.0],
        ));
        columns.push(Column::new(
            "FL1-A".into(),
            vec![10.0f32, 20.0, 30.0, 40.0, 50.0],
        ));

        let df = DataFrame::new(5, columns).expect("Failed to create test DataFrame");

        // Create parameter map
        let mut params = ParameterMap::default();
        params.insert(
            "FSC-A".into(),
            Parameter::new(&1, "FSC-A", "FSC-A", &TransformType::Linear),
        );
        params.insert(
            "SSC-A".into(),
            Parameter::new(&2, "SSC-A", "SSC-A", &TransformType::Linear),
        );
        params.insert(
            "FL1-A".into(),
            Parameter::new(&3, "FL1-A", "FL1-A", &TransformType::Linear),
        );

        Ok(Fcs::for_testing(
            Header::new(),
            Metadata::new(),
            params,
            Arc::new(df),
            AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        ))
    }

    #[test]
    fn test_get_parameter_column() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Test successful column retrieval
        let events = fcs.get_parameter_events("FSC-A");
        assert!(
            events.is_ok(),
            "Should retrieve FSC-A column events successfully"
        );

        // Test missing column
        let result = fcs.get_parameter_events("NonExistent");
        assert!(result.is_err(), "Should error on non-existent parameter");
    }

    #[test]
    fn test_get_parameter_events_slice() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let slice = fcs
            .get_parameter_events_slice("FSC-A")
            .expect("Should retrieve FSC-A events");

        assert_eq!(slice.len(), 5, "Should have 5 events");
        assert_eq!(slice[0], 100.0, "First event should be 100.0");
        assert_eq!(slice[4], 500.0, "Last event should be 500.0");
    }

    #[test]
    fn test_get_xy_pairs() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let pairs = fcs
            .get_xy_pairs("FSC-A", "SSC-A")
            .expect("Should get XY pairs");

        assert_eq!(pairs.len(), 5, "Should have 5 pairs");
        assert_eq!(pairs[0], (100.0, 50.0), "First pair should match");
        assert_eq!(pairs[4], (500.0, 450.0), "Last pair should match");
    }

    #[test]
    fn test_get_dataframe_dimensions() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        assert_eq!(
            fcs.get_event_count_from_dataframe(),
            5,
            "Should have 5 events"
        );
        assert_eq!(
            fcs.get_parameter_count_from_dataframe(),
            3,
            "Should have 3 parameters"
        );
    }

    #[test]
    fn test_get_parameter_names() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let names = fcs.get_parameter_names_from_dataframe();
        assert_eq!(names.len(), 3, "Should have 3 parameter names");
        assert!(names.contains(&"FSC-A".to_string()), "Should contain FSC-A");
        assert!(names.contains(&"SSC-A".to_string()), "Should contain SSC-A");
        assert!(names.contains(&"FL1-A".to_string()), "Should contain FL1-A");
    }

    #[test]
    fn test_get_parameter_statistics() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let (min, max, mean, std) = fcs
            .get_parameter_statistics("FSC-A")
            .expect("Should get statistics");

        assert_eq!(min, 100.0, "Min should be 100");
        assert_eq!(max, 500.0, "Max should be 500");
        assert_eq!(mean, 300.0, "Mean should be 300");
        assert!(std > 0.0, "Std dev should be positive");
    }

    #[test]
    fn test_arcsinh_transformation() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Apply arcsinh transformation to FSC-A with cofactor 200
        let transformed = fcs
            .apply_arcsinh_transform("FSC-A", 200.0)
            .expect("Should apply arcsinh transform");

        // Verify the transformation was applied
        let fcs_transformed = Fcs {
            data_frame: transformed,
            ..fcs.clone()
        };

        let transformed_data = fcs_transformed
            .get_parameter_events_slice("FSC-A")
            .expect("Should get transformed data");

        // Verify values are different from original
        let original_data = fcs
            .get_parameter_events_slice("FSC-A")
            .expect("Should get original data");

        assert_ne!(
            transformed_data[0], original_data[0],
            "Data should be transformed"
        );

        // Verify arcsinh formula: arcsinh(x / cofactor) (no ln(10) scaling)
        let expected = (original_data[0] / 200.0).asinh();
        assert!(
            (transformed_data[0] - expected).abs() < 0.001,
            "Transform should match arcsinh formula"
        );
    }

    #[test]
    fn test_arcsinh_multiple_transforms() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Transform multiple parameters
        let params = vec![("FSC-A", 150.0), ("SSC-A", 200.0)];
        let transformed = fcs
            .apply_arcsinh_transforms(&params)
            .expect("Should apply multiple transforms");

        let fcs_transformed = Fcs {
            data_frame: transformed,
            ..fcs.clone()
        };

        // Verify both parameters were transformed
        let fsc_data = fcs_transformed
            .get_parameter_events_slice("FSC-A")
            .expect("Should get FSC-A");
        let ssc_data = fcs_transformed
            .get_parameter_events_slice("SSC-A")
            .expect("Should get SSC-A");

        let orig_fsc = fcs.get_parameter_events_slice("FSC-A").unwrap();
        let orig_ssc = fcs.get_parameter_events_slice("SSC-A").unwrap();

        assert_ne!(fsc_data[0], orig_fsc[0], "FSC-A should be transformed");
        assert_ne!(ssc_data[0], orig_ssc[0], "SSC-A should be transformed");
    }

    #[test]
    fn test_default_arcsinh_transform() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // This should transform FL1-A (fluorescence) but not FSC-A or SSC-A
        let transformed = fcs
            .apply_default_arcsinh_transform()
            .expect("Should apply default transform");

        let fcs_transformed = Fcs {
            data_frame: transformed,
            ..fcs.clone()
        };

        // FL1-A should be transformed (it's fluorescence)
        let fl1_data = fcs_transformed
            .get_parameter_events_slice("FL1-A")
            .expect("Should get FL1-A");
        let orig_fl1 = fcs.get_parameter_events_slice("FL1-A").unwrap();

        assert_ne!(fl1_data[0], orig_fl1[0], "FL1-A should be transformed");

        // Verify it used default cofactor = 2000 for fluorescence
        let expected = (orig_fl1[0] / 2000.0).asinh();
        assert!(
            (fl1_data[0] - expected).abs() < 0.001,
            "Should use default cofactor 2000"
        );
    }

    #[test]
    fn test_compensation_matrix() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Create a simple 2x2 compensation matrix for FSC-A and SSC-A
        let comp_matrix = faer::mat![[1.0, 0.1], [0.05, 1.0]];

        let channels = vec!["FSC-A", "SSC-A"];
        let compensated = fcs
            .apply_compensation(comp_matrix.as_ref(), &channels)
            .expect("Should apply compensation");

        let fcs_compensated = Fcs {
            data_frame: compensated,
            ..fcs.clone()
        };

        // Verify data was compensated (will be different from original)
        let comp_fsc = fcs_compensated
            .get_parameter_events_slice("FSC-A")
            .expect("Should get compensated FSC-A");
        let orig_fsc = fcs.get_parameter_events_slice("FSC-A").unwrap();

        assert_ne!(comp_fsc[0], orig_fsc[0], "Data should be compensated");

        // Verify dimensions unchanged
        assert_eq!(
            comp_fsc.len(),
            orig_fsc.len(),
            "Event count should be unchanged"
        );
    }

    #[test]
    fn test_compensation_wrong_dimensions() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Create a 2x2 matrix but provide 3 channels (should error)
        let comp_matrix = faer::mat![[1.0, 0.1], [0.05, 1.0]];

        let channels = vec!["FSC-A", "SSC-A", "FL1-A"];
        let result = fcs.apply_compensation(comp_matrix.as_ref(), &channels);

        assert!(result.is_err(), "Should error on dimension mismatch");
        assert!(
            result.unwrap_err().to_string().contains("dimensions"),
            "Error should mention dimensions"
        );
    }

    #[test]
    fn test_spectral_unmixing() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Create a simple unmixing matrix
        let unmix_matrix = faer::mat![[1.0, 0.15], [0.1, 1.0]];

        let channels = vec!["FSC-A", "SSC-A"];
        let unmixed = fcs
            .apply_spectral_unmixing(unmix_matrix.as_ref(), &channels, None)
            .expect("Should apply spectral unmixing");

        let fcs_unmixed = Fcs {
            data_frame: unmixed,
            ..fcs.clone()
        };

        // Verify data was unmixed (unmixed columns are Endmember1, Endmember2 when endmember_names=None)
        let unmixed_col = fcs_unmixed
            .get_parameter_events_slice("Endmember1")
            .expect("Should get unmixed Endmember1");
        let orig_fsc = fcs.get_parameter_events_slice("FSC-A").unwrap();

        assert_ne!(unmixed_col[0], orig_fsc[0], "Data should be unmixed");
    }

    #[test]
    fn test_parameter_is_fluorescence() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        let fsc = fcs.find_parameter("FSC-A").unwrap();
        let ssc = fcs.find_parameter("SSC-A").unwrap();
        let fl1 = fcs.find_parameter("FL1-A").unwrap();

        assert!(!fsc.is_fluorescence(), "FSC-A should not be fluorescence");
        assert!(!ssc.is_fluorescence(), "SSC-A should not be fluorescence");
        assert!(fl1.is_fluorescence(), "FL1-A should be fluorescence");
    }

    #[test]
    fn test_parameter_display_labels() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");
        let fl1 = fcs.find_parameter("FL1-A").unwrap();

        // Raw state
        assert_eq!(
            fl1.get_display_label(),
            "FL1-A",
            "Raw should be just channel name"
        );

        // Compensated state
        let comp = fl1.with_state(ParameterProcessing::Compensated);
        assert_eq!(
            comp.get_display_label(),
            "Comp::FL1-A",
            "Should have Comp:: prefix"
        );
    }

    #[test]
    fn test_parameter_with_label() {
        use crate::parameter::ParameterBuilder;

        let param = ParameterBuilder::default()
            .parameter_number(1_usize)
            .channel_name("UV379-A".to_string())
            .label_name("CD8".to_string())
            .transform(TransformType::Linear)
            .build()
            .unwrap();

        // Raw should show channel::label
        assert_eq!(param.get_short_label(), "UV379-A::CD8");
        assert_eq!(param.get_display_label(), "UV379-A::CD8");

        // Compensated should show Comp::channel::label
        let comp = param.with_state(ParameterProcessing::Compensated);
        assert_eq!(comp.get_display_label(), "Comp::UV379-A::CD8");
    }

    #[test]
    fn test_generate_plot_options_fluorescence() {
        use crate::parameter::ParameterBuilder;

        let param = ParameterBuilder::default()
            .parameter_number(1_usize)
            .channel_name("FL1-A".to_string())
            .label_name("CD3".to_string())
            .transform(TransformType::Linear)
            .build()
            .unwrap();

        // Without compensation
        let options = param.generate_plot_options(false);
        assert_eq!(
            options.len(),
            1,
            "Fluorescence returns transformed-only by default"
        );
        assert_eq!(options[0].id, "transformed::FL1-A");
        assert_eq!(options[0].display_label, "FL1-A::CD3");

        // With compensation
        let options = param.generate_plot_options(true);
        assert_eq!(
            options.len(),
            2,
            "Should have transformed + comp_trans"
        );
        assert_eq!(options[1].id, "comp_trans::FL1-A");
        assert_eq!(options[1].display_label, "Comp::FL1-A::CD3");
    }

    #[test]
    fn test_generate_plot_options_scatter() {
        use crate::parameter::{ParameterBuilder, ParameterCategory};

        let param = ParameterBuilder::default()
            .parameter_number(1_usize)
            .channel_name("FSC-A".to_string())
            .label_name("FSC-A".to_string())
            .transform(TransformType::Linear)
            .build()
            .unwrap();

        // Scatter parameters should only have raw option
        let options = param.generate_plot_options(false);
        assert_eq!(options.len(), 1, "Scatter should only have raw option");
        assert_eq!(options[0].id, "raw::FSC-A");
        assert_eq!(options[0].category, ParameterCategory::Raw);

        // Even with compensation enabled, scatter stays at 1
        let options = param.generate_plot_options(true);
        assert_eq!(
            options.len(),
            1,
            "Scatter should only have raw option even with comp"
        );
    }

    #[test]
    fn test_spillover_extraction() {
        use crate::keyword::{Keyword, MixedKeyword};

        // Create a minimal FCS with spillover
        let mut fcs = create_test_fcs().expect("Failed to create test FCS");

        // Add a spillover keyword to metadata
        let spillover = MixedKeyword::SPILLOVER {
            n_parameters: 2,
            parameter_names: vec!["FL1-A".to_string(), "FL2-A".to_string()],
            matrix_values: vec![1.0, 0.1, 0.15, 1.0],
        };

        fcs.metadata
            .keywords
            .insert("$SPILLOVER".to_string(), Keyword::Mixed(spillover));

        // Test extraction
        let result = fcs
            .get_spillover_matrix()
            .expect("Should extract spillover");
        assert!(result.is_some(), "Should have spillover matrix");

        let (matrix, names) = result.unwrap();
        assert_eq!(matrix.nrows(), 2, "Should be 2x2 matrix");
        assert_eq!(matrix.ncols(), 2, "Should be 2x2 matrix");
        assert_eq!(names.len(), 2, "Should have 2 channel names");
        assert_eq!(names[0], "FL1-A");
        assert_eq!(names[1], "FL2-A");
        assert_eq!(matrix[(0, 0)], 1.0);
        assert_eq!(matrix[(0, 1)], 0.1);
    }

    #[test]
    fn test_has_compensation() {
        use crate::keyword::{Keyword, MixedKeyword};

        let mut fcs = create_test_fcs().expect("Failed to create test FCS");

        // Initially should have no compensation
        assert!(
            !fcs.has_compensation(),
            "Should not have compensation initially"
        );

        // Add spillover
        let spillover = MixedKeyword::SPILLOVER {
            n_parameters: 2,
            parameter_names: vec!["FL1-A".to_string(), "FL2-A".to_string()],
            matrix_values: vec![1.0, 0.1, 0.15, 1.0],
        };
        fcs.metadata
            .keywords
            .insert("$SPILLOVER".to_string(), Keyword::Mixed(spillover));

        // Now should have compensation
        assert!(
            fcs.has_compensation(),
            "Should have compensation after adding spillover"
        );
    }

    #[test]
    fn test_apply_file_compensation() {
        use crate::keyword::{Keyword, MixedKeyword};

        let mut fcs = create_test_fcs().expect("Failed to create test FCS");

        // Add spillover for FSC-A and SSC-A
        let spillover = MixedKeyword::SPILLOVER {
            n_parameters: 2,
            parameter_names: vec!["FSC-A".to_string(), "SSC-A".to_string()],
            matrix_values: vec![1.0, 0.1, 0.05, 1.0],
        };
        fcs.metadata
            .keywords
            .insert("$SPILLOVER".to_string(), Keyword::Mixed(spillover));

        // Apply file compensation
        let compensated_df = fcs
            .apply_file_compensation()
            .expect("Should apply file compensation");

        let fcs_comp = Fcs {
            data_frame: compensated_df,
            ..fcs.clone()
        };

        // Verify data was compensated
        let comp_data = fcs_comp.get_parameter_events_slice("FSC-A").unwrap();
        let orig_data = fcs.get_parameter_events_slice("FSC-A").unwrap();

        assert_ne!(comp_data[0], orig_data[0], "Data should be compensated");
    }
}
