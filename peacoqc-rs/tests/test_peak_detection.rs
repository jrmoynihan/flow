// Test peak detection against R's FindThemPeaks
// Run with: cargo test --package peacoqc-rs test_peak_detection -- --nocapture

#[cfg(test)]
mod tests {
    use peacoqc_rs::stats::density::KernelDensity;

    #[test]
    fn test_peak_detection_vs_r() {
        // Test data: first 1000 events from B530-A channel after preprocessing
        // This should match R's FindThemPeaks output
        let test_data = vec![
            // Add some test data here - we'll populate from actual file
        ];

        if test_data.is_empty() {
            eprintln!("Skipping test - no test data");
            return;
        }

        let kde = KernelDensity::estimate(&test_data, 1.0, 512).unwrap();
        let peaks = kde.find_peaks(1.0 / 3.0);

        eprintln!("Rust peaks: {:?}", peaks);
        eprintln!("Number of peaks: {}", peaks.len());
    }
}
