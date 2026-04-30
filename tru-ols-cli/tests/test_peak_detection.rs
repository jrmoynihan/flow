/// Test peak detection in single-stain control processing
#[cfg(test)]
mod tests {
    use tru_ols::SingleStainConfig;

    #[test]
    fn test_peak_detection_enabled_by_default() {
        let config = SingleStainConfig::default();
        assert!(
            config.peak_detection,
            "Peak detection should be enabled by default"
        );
    }

    #[test]
    fn test_peak_detection_parameters() {
        let config = SingleStainConfig::default();
        assert_eq!(config.peak_threshold, 0.3, "Peak threshold should be 0.3");
        assert_eq!(config.peak_bias, 0.5, "Peak bias should be 0.5");
    }

    #[test]
    fn test_peak_detection_params_sanity() {
        let config = SingleStainConfig::default();
        // Peak threshold should be reasonable (between 0 and 1)
        assert!(config.peak_threshold > 0.0 && config.peak_threshold < 1.0);
        // Peak bias should select from a reasonable fraction (0-1)
        assert!(config.peak_bias > 0.0 && config.peak_bias <= 1.0);
    }
}
