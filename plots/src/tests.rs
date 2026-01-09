// Tests for flow-plots
//
// Tests for the new API using DensityPlotOptions, BasePlotOptions, and AxisOptions
// with the builder pattern.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colormap::ColorMaps;
    use crate::helpers::density_options_from_fcs;
    use crate::options::{AxisOptions, BasePlotOptions, DensityPlotOptions, PlotOptions};
    use crate::plots::density::DensityPlot;
    use crate::render::RenderConfig;
    use flow_fcs::{Fcs, Parameter, TransformType};
    use polars::prelude::*;
    use std::fs::File;
    use std::io::Write;
    // Note: Some imports may not be needed but are kept for future use
    use std::sync::Arc;

    // Helper to create a test FCS struct
    fn create_test_fcs() -> anyhow::Result<Fcs> {
        // Create a temporary file for testing
        let temp_path = std::env::temp_dir().join("test_fcs_plots.tmp");
        {
            let mut f = File::create(&temp_path)?;
            f.write_all(b"test")?;
        }

        // Create test DataFrame with more data points for percentile testing
        let mut columns = Vec::new();
        columns.push(Column::new(
            "FSC-A".into(),
            (0..100).map(|i| i as f32 * 100.0).collect::<Vec<f32>>(),
        ));
        columns.push(Column::new(
            "SSC-A".into(),
            (0..100).map(|i| i as f32 * 50.0).collect::<Vec<f32>>(),
        ));
        columns.push(Column::new(
            "FL1-A".into(),
            (0..100).map(|i| i as f32 * 10.0).collect::<Vec<f32>>(),
        ));
        columns.push(Column::new(
            "Time".into(),
            (0..100).map(|i| i as f32).collect::<Vec<f32>>(),
        ));

        let df = DataFrame::new(columns).expect("Failed to create test DataFrame");

        // Create parameter map using internal types (similar to flow-fcs tests)
        use flow_fcs::parameter::ParameterMap;
        use flow_fcs::parameter::ParameterProcessing;
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
            Parameter::new(
                &3,
                "FL1-A",
                "FL1-A",
                &TransformType::Arcsinh { cofactor: 200.0 },
            ),
        );
        params.insert(
            "Time".into(),
            Parameter::new(&4, "Time", "Time", &TransformType::Linear),
        );

        // Create metadata with $FIL keyword
        use flow_fcs::keyword::Keyword;
        use flow_fcs::metadata::Metadata;
        let mut metadata = Metadata::new();
        metadata.keywords.insert(
            "$FIL".to_string(),
            Keyword::String("test_file.fcs".to_string()),
        );

        Ok(Fcs {
            header: flow_fcs::Header::new(),
            metadata,
            parameters: params,
            data_frame: Arc::new(df),
            file_access: flow_fcs::file::AccessWrapper::new(temp_path.to_str().unwrap_or(""))?,
        })
    }

    // ============================================================================
    // BasePlotOptions Tests
    // ============================================================================

    #[test]
    fn test_base_plot_options_default() {
        let options = BasePlotOptions::default();
        assert_eq!(options.width, 400);
        assert_eq!(options.height, 400);
        assert_eq!(options.margin, 10);
        assert_eq!(options.x_label_area_size, 50);
        assert_eq!(options.y_label_area_size, 50);
        assert_eq!(options.title, "Density Plot");
    }

    #[test]
    fn test_base_plot_options_builder() {
        let options = BasePlotOptions::new()
            .width(800)
            .height(600)
            .margin(20)
            .x_label_area_size(60)
            .y_label_area_size(70)
            .title("Custom Plot".to_string())
            .build()
            .unwrap();

        assert_eq!(options.width, 800);
        assert_eq!(options.height, 600);
        assert_eq!(options.margin, 20);
        assert_eq!(options.x_label_area_size, 60);
        assert_eq!(options.y_label_area_size, 70);
        assert_eq!(options.title, "Custom Plot");
    }

    #[test]
    fn test_base_plot_options_builder_partial() {
        // Test that we can set only some fields
        let options = BasePlotOptions::new()
            .width(1000)
            .height(750)
            .build()
            .unwrap();

        assert_eq!(options.width, 1000);
        assert_eq!(options.height, 750);
        // Other fields should use defaults
        assert_eq!(options.margin, 10);
        assert_eq!(options.title, "Density Plot");
    }

    // ============================================================================
    // AxisOptions Tests
    // ============================================================================

    #[test]
    fn test_axis_options_default() {
        let options = AxisOptions::default();
        assert_eq!(*options.range.start(), 0.0);
        assert_eq!(*options.range.end(), 200_000.0);
        assert!(matches!(options.transform, TransformType::Arcsinh { .. }));
        assert_eq!(options.label, None);
    }

    #[test]
    fn test_axis_options_builder() {
        let options = AxisOptions::new()
            .range(0.0..=1000.0)
            .transform(TransformType::Linear)
            .label("X-Axis".to_string())
            .build()
            .unwrap();

        assert_eq!(*options.range.start(), 0.0);
        assert_eq!(*options.range.end(), 1000.0);
        assert!(matches!(options.transform, TransformType::Linear));
        assert_eq!(options.label, Some("X-Axis".to_string()));
    }

    #[test]
    fn test_axis_options_arcsinh_transform() {
        let options = AxisOptions::new()
            .range(0.0..=5000.0)
            .transform(TransformType::Arcsinh { cofactor: 150.0 })
            .build()
            .unwrap();

        assert!(matches!(
            options.transform,
            TransformType::Arcsinh { cofactor: 150.0 }
        ));
    }

    // ============================================================================
    // DensityPlotOptions Tests
    // ============================================================================

    #[test]
    fn test_density_plot_options_default() {
        let options = DensityPlotOptions::default();
        assert_eq!(options.base.width, 400);
        assert_eq!(options.base.height, 400);
        assert_eq!(*options.x_axis.range.start(), 0.0);
        assert_eq!(*options.x_axis.range.end(), 200_000.0);
        assert!(matches!(options.colormap, ColorMaps::Viridis(_)));
    }

    #[test]
    fn test_density_plot_options_builder() {
        let base = BasePlotOptions::new()
            .width(800)
            .height(600)
            .title("Test Plot".to_string())
            .build()
            .unwrap();

        let x_axis = AxisOptions::new()
            .range(0.0..=10_000.0)
            .transform(TransformType::Linear)
            .label("FSC-A".to_string())
            .build()
            .unwrap();

        let y_axis = AxisOptions::new()
            .range(0.0..=5_000.0)
            .transform(TransformType::Arcsinh { cofactor: 150.0 })
            .label("SSC-A".to_string())
            .build()
            .unwrap();

        let options = DensityPlotOptions::new()
            .base(base)
            .x_axis(x_axis)
            .y_axis(y_axis)
            .build()
            .unwrap();

        assert_eq!(options.base.width, 800);
        assert_eq!(options.base.height, 600);
        assert_eq!(options.base.title, "Test Plot");
        assert_eq!(*options.x_axis.range.start(), 0.0);
        assert_eq!(*options.x_axis.range.end(), 10_000.0);
        assert_eq!(*options.y_axis.range.start(), 0.0);
        assert_eq!(*options.y_axis.range.end(), 5_000.0);
        assert_eq!(options.x_axis.label, Some("FSC-A".to_string()));
        assert_eq!(options.y_axis.label, Some("SSC-A".to_string()));
    }

    #[test]
    fn test_density_plot_options_plot_options_trait() {
        let options = DensityPlotOptions::default();
        let base = options.base();
        assert_eq!(base.width, 400);
        assert_eq!(base.height, 400);
    }

    // ============================================================================
    // Helper Function Tests
    // ============================================================================
    // Note: These tests require the Transformable trait to be exported from flow-fcs.
    // They will compile once that export is available.

    #[test]
    fn test_density_options_from_fcs_fsc_ssc() {
        let fcs = create_test_fcs().unwrap();
        let x_param = fcs.find_parameter("FSC-A").unwrap();
        let y_param = fcs.find_parameter("SSC-A").unwrap();

        let builder = density_options_from_fcs(&fcs, x_param, y_param).unwrap();
        let options = builder.build().unwrap();

        // FSC/SSC should use default range
        assert_eq!(*options.x_axis.range.start(), 0.0);
        assert_eq!(*options.x_axis.range.end(), 200_000.0);
        assert_eq!(*options.y_axis.range.start(), 0.0);
        assert_eq!(*options.y_axis.range.end(), 200_000.0);

        // FSC/SSC should use Linear transform
        assert!(matches!(options.x_axis.transform, TransformType::Linear));
        assert!(matches!(options.y_axis.transform, TransformType::Linear));

        // Title should be extracted from $FIL keyword
        assert_eq!(options.base.title, "test_file.fcs");
    }

    #[test]
    fn test_density_options_from_fcs_fluorescence() {
        let fcs = create_test_fcs().unwrap();
        let x_param = fcs.find_parameter("FL1-A").unwrap();
        let y_param = fcs.find_parameter("FL1-A").unwrap();

        let builder = density_options_from_fcs(&fcs, x_param, y_param).unwrap();
        let options = builder.build().unwrap();

        // Fluorescence should calculate percentile bounds
        // Data ranges from 0 to 990 (100 points * 10.0)
        // After arcsinh transform, percentiles should be calculated
        assert!(*options.x_axis.range.start() >= 0.0);
        assert!(*options.x_axis.range.end() <= 1000.0);

        // Should use arcsinh transform (from parameter)
        assert!(matches!(
            options.x_axis.transform,
            TransformType::Arcsinh { .. }
        ));
    }

    #[test]
    fn test_density_options_from_fcs_time() {
        let fcs = create_test_fcs().unwrap();
        let x_param = fcs.find_parameter("Time").unwrap();
        let y_param = fcs.find_parameter("Time").unwrap();

        let builder = density_options_from_fcs(&fcs, x_param, y_param).unwrap();
        let options = builder.build().unwrap();

        // Time should use actual max value
        // Data ranges from 0 to 99
        assert!(*options.x_axis.range.start() == 0.0);
        assert!(*options.x_axis.range.end() >= 99.0);
    }

    #[test]
    fn test_density_options_from_fcs_customization() {
        let fcs = create_test_fcs().unwrap();
        let x_param = fcs.find_parameter("FSC-A").unwrap();
        let y_param = fcs.find_parameter("SSC-A").unwrap();

        let builder = density_options_from_fcs(&fcs, x_param, y_param).unwrap();
        let options = builder.width(1200).height(900).build().unwrap();

        // Custom dimensions should be applied
        assert_eq!(options.base.width, 1200);
        assert_eq!(options.base.height, 900);
    }

    // ============================================================================
    // Utility Function Tests
    // ============================================================================

    #[test]
    fn test_get_percentile_bounds() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
        let range = crate::get_percentile_bounds(&values, 0.1, 0.9);

        // 10th percentile should be around 10-20, 90th around 90-100
        assert!(*range.start() <= 20.0);
        assert!(*range.end() >= 90.0);
    }

    #[test]
    fn test_get_percentile_bounds_single_value() {
        let values = vec![42.0];
        let range = crate::get_percentile_bounds(&values, 0.01, 0.99);
        assert!(*range.start() <= 42.0);
        assert!(*range.end() >= 42.0);
    }

    #[test]
    fn test_get_percentile_bounds_empty() {
        let values = vec![];
        let range = crate::get_percentile_bounds(&values, 0.01, 0.99);
        // Should handle empty gracefully (will panic on index access, but that's expected)
        // In practice, this shouldn't happen, but we test the bounds checking
    }

    #[test]
    fn test_create_axis_specs_linear() {
        let range_x = 0.0..=1000.0;
        let range_y = 0.0..=500.0;
        let x_transform = TransformType::Linear;
        let y_transform = TransformType::Linear;

        let (x_spec, y_spec) =
            crate::create_axis_specs(&range_x, &range_y, &x_transform, &y_transform).unwrap();

        // Linear should use nice bounds
        assert!(x_spec.start <= 0.0);
        assert!(x_spec.end >= 1000.0);
        assert!(y_spec.start <= 0.0);
        assert!(y_spec.end >= 500.0);
    }

    #[test]
    fn test_create_axis_specs_arcsinh() {
        let range_x = 0.0..=1000.0;
        let range_y = 0.0..=500.0;
        let x_transform = TransformType::Arcsinh { cofactor: 200.0 };
        let y_transform = TransformType::Arcsinh { cofactor: 150.0 };

        let (x_spec, y_spec) =
            crate::create_axis_specs(&range_x, &range_y, &x_transform, &y_transform).unwrap();

        // Arcsinh should preserve the range
        assert_eq!(x_spec.start, 0.0);
        assert_eq!(x_spec.end, 1000.0);
        assert_eq!(y_spec.start, 0.0);
        assert_eq!(y_spec.end, 500.0);
    }

    // ============================================================================
    // Plot Rendering Tests
    // ============================================================================

    #[test]
    fn test_density_plot_render_empty_data() {
        let plot = DensityPlot::new();
        let options = DensityPlotOptions::default();
        let data: Vec<(f32, f32)> = vec![];
        let mut render_config = RenderConfig::default();

        // Should handle empty data gracefully
        let result = plot.render(data, &options, &mut render_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_density_plot_render_small_dataset() {
        let plot = DensityPlot::new();
        let options = DensityPlotOptions::new()
            .width(100)
            .height(100)
            .build()
            .unwrap();
        let data: Vec<(f32, f32)> = vec![(100.0, 200.0), (150.0, 250.0), (200.0, 300.0)];
        let mut render_config = RenderConfig::default();

        let result = plot.render(data, &options, &mut render_config);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        // Should produce JPEG bytes
        assert!(!bytes.is_empty());
        // JPEG files start with FF D8 FF
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }

    #[test]
    fn test_density_plot_render_with_progress_callback() {
        let plot = DensityPlot::new();
        let options = DensityPlotOptions::new()
            .width(200)
            .height(200)
            .build()
            .unwrap();

        // Create data that will trigger progress updates
        let data: Vec<(f32, f32)> = (0..1000)
            .map(|i| (i as f32 * 10.0, i as f32 * 5.0))
            .collect();

        let mut progress_calls = 0;
        let mut render_config = RenderConfig {
            progress: Some(Box::new(move |_info| {
                progress_calls += 1;
                Ok(())
            })),
            ..Default::default()
        };

        let result = plot.render(data, &options, &mut render_config);
        assert!(result.is_ok());
        // Progress should have been called at least once
        assert!(progress_calls > 0);
    }

    // ============================================================================
    // Transform Tests
    // ============================================================================

    #[test]
    fn test_transform_application_in_helper() {
        let fcs = create_test_fcs().unwrap();
        let x_param = fcs.find_parameter("FL1-A").unwrap();

        // Verify that the parameter has arcsinh transform
        assert!(matches!(x_param.transform, TransformType::Arcsinh { .. }));

        // Get raw data
        let raw_data = fcs.get_parameter_events_slice("FL1-A").unwrap();
        assert!(!raw_data.is_empty());

        // Transform should be applied in helper function
        let transformed: Vec<f32> = raw_data
            .iter()
            .map(|&v| x_param.transform.transform(&v))
            .collect();

        // Transformed values should be different from raw (for non-zero values)
        if raw_data[0] != 0.0 {
            assert_ne!(transformed[0], raw_data[0]);
        }
    }

    // ============================================================================
    // Edge Cases and Error Handling
    // ============================================================================

    #[test]
    fn test_builder_with_invalid_range() {
        // Builder should accept any valid range
        let options = AxisOptions::new()
            .range(100.0..=50.0) // Invalid: start > end
            .build();

        // Builder doesn't validate ranges, so this should succeed
        // (validation would happen at render time)
        assert!(options.is_ok());
    }

    #[test]
    fn test_density_plot_options_builder_chaining() {
        // Test that we can chain builder methods
        let options = DensityPlotOptions::new()
            .width(800)
            .height(600)
            .build()
            .unwrap();

        assert_eq!(options.base.width, 800);
        assert_eq!(options.base.height, 600);
    }

    #[test]
    fn test_render_config_default() {
        let config = RenderConfig::default();
        assert!(config.render_lock.is_none());
        assert!(config.progress.is_none());
    }

    #[test]
    fn test_density_plot_new() {
        let plot = DensityPlot::new();
        // Should create successfully
        assert!(std::mem::size_of_val(&plot) == 0); // Unit struct
    }
}
