//! Integration tests for TRU-OLS FCS integration

#[cfg(feature = "flow-fcs")]
mod tests {
    use flow_fcs::Fcs;
    use flow_tru_ols::fcs_integration::extract_detector_data;

    #[test]
    fn test_extract_detector_data_basic() {
        // This test requires a real FCS file
        // For now, we'll create a placeholder test that can be expanded
        // when we have sample FCS files available
    }

    #[test]
    fn test_f32_to_f64_conversion() {
        // Test that f32 to f64 conversion works correctly
        let f32_data = vec![1.0f32, 2.5, 3.14159, -1.0, 0.0];
        let f64_data: Vec<f64> = f32_data.iter().map(|&x| x as f64).collect();

        assert_eq!(f64_data.len(), f32_data.len());
        for (f32_val, f64_val) in f32_data.iter().zip(f64_data.iter()) {
            assert!((*f32_val as f64 - f64_val).abs() < 1e-6);
        }
    }
}
