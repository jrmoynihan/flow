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

        let df = DataFrame::new(columns).expect("Failed to create test DataFrame");

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

        Ok(Fcs {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        })
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

        // Verify arcsinh formula: arcsinh(x / cofactor) / ln(10)
        let expected = ((original_data[0] / 200.0).asinh()) / 10_f32.ln();
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

        // Verify it used cofactor = 200
        let expected = ((orig_fl1[0] / 200.0).asinh()) / 10_f32.ln();
        assert!(
            (fl1_data[0] - expected).abs() < 0.001,
            "Should use default cofactor 200"
        );
    }

    #[test]
    fn test_compensation_matrix() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        // Create a simple 2x2 compensation matrix for FSC-A and SSC-A
        use ndarray::Array2;
        let comp_matrix = Array2::from_shape_vec(
            (2, 2),
            vec![
                1.0, 0.1, // FSC-A compensation
                0.05, 1.0, // SSC-A compensation
            ],
        )
        .expect("Should create compensation matrix");

        let channels = vec!["FSC-A", "SSC-A"];
        let compensated = fcs
            .apply_compensation(&comp_matrix, &channels)
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
        use ndarray::Array2;
        let comp_matrix = Array2::from_shape_vec((2, 2), vec![1.0, 0.1, 0.05, 1.0]).unwrap();

        let channels = vec!["FSC-A", "SSC-A", "FL1-A"];
        let result = fcs.apply_compensation(&comp_matrix, &channels);

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
        use ndarray::Array2;
        let unmix_matrix = Array2::from_shape_vec((2, 2), vec![1.0, 0.15, 0.1, 1.0]).unwrap();

        let channels = vec!["FSC-A", "SSC-A"];
        let unmixed = fcs
            .apply_spectral_unmixing(&unmix_matrix, &channels, None)
            .expect("Should apply spectral unmixing");

        let fcs_unmixed = Fcs {
            data_frame: unmixed,
            ..fcs.clone()
        };

        // Verify data was unmixed
        let unmixed_fsc = fcs_unmixed
            .get_parameter_events_slice("FSC-A")
            .expect("Should get unmixed FSC-A");
        let orig_fsc = fcs.get_parameter_events_slice("FSC-A").unwrap();

        assert_ne!(unmixed_fsc[0], orig_fsc[0], "Data should be unmixed");
    }

    #[test]
    fn test_spectral_unmixing_custom_cofactor() {
        let fcs = create_test_fcs().expect("Failed to create test FCS");

        use ndarray::Array2;
        let unmix_matrix = Array2::from_shape_vec((2, 2), vec![1.0, 0.0, 0.0, 1.0]).unwrap();

        let channels = vec!["FSC-A", "SSC-A"];

        // Test with custom cofactor
        let unmixed_150 = fcs
            .apply_spectral_unmixing(&unmix_matrix, &channels, Some(150.0))
            .expect("Should unmix with cofactor 150");
        let unmixed_200 = fcs
            .apply_spectral_unmixing(&unmix_matrix, &channels, Some(200.0))
            .expect("Should unmix with cofactor 200");

        let fcs_150 = Fcs {
            data_frame: unmixed_150,
            ..fcs.clone()
        };
        let fcs_200 = Fcs {
            data_frame: unmixed_200,
            ..fcs.clone()
        };

        let data_150 = fcs_150.get_parameter_events_slice("FSC-A").unwrap();
        let data_200 = fcs_200.get_parameter_events_slice("FSC-A").unwrap();

        // Different cofactors should produce different results
        assert_ne!(
            data_150[0], data_200[0],
            "Different cofactors should give different results"
        );
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

        // Unmixed state
        let unmix = fl1.with_state(ParameterProcessing::Unmixed);
        assert_eq!(
            unmix.get_display_label(),
            "Unmix::FL1-A",
            "Should have Unmix:: prefix"
        );

        // Combined compensated+unmixed state
        let comp_unmix = fl1.with_state(ParameterProcessing::UnmixedCompensated);
        assert_eq!(
            comp_unmix.get_display_label(),
            "Comp+Unmix::FL1-A",
            "Should have Comp+Unmix:: prefix"
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
            4,
            "Should have transformed + comp_trans + unmix_trans + comp_unmix_trans"
        );
        assert_eq!(options[1].id, "comp_trans::FL1-A");
        assert_eq!(options[1].display_label, "Comp::FL1-A::CD3");
        assert_eq!(options[2].id, "unmix_trans::FL1-A");
        assert_eq!(options[2].display_label, "Unmix::FL1-A::CD3");
        assert_eq!(options[3].id, "comp_unmix_trans::FL1-A");
        assert_eq!(options[3].display_label, "Comp+Unmix::FL1-A::CD3");
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
        assert_eq!(matrix.shape(), &[2, 2], "Should be 2x2 matrix");
        assert_eq!(names.len(), 2, "Should have 2 channel names");
        assert_eq!(names[0], "FL1-A");
        assert_eq!(names[1], "FL2-A");
        assert_eq!(matrix[[0, 0]], 1.0);
        assert_eq!(matrix[[0, 1]], 0.1);
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
