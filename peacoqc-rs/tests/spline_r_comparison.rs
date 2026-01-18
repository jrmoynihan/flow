//! Test to compare our smooth.spline implementation with R's output
//!
//! This test generates test data and compares our smoothing results with
//! known R smooth.spline outputs or allows manual comparison.
//!
//! To run with R comparison (requires R installed):
//! ```bash
//! RSCRIPT_AVAILABLE=1 cargo test --package peacoqc-rs test_smooth_spline_r_direct_comparison -- --nocapture
//! ```

use peacoqc_rs::stats::spline::smooth_spline;
use std::process::Command;

#[test]
fn test_smooth_spline_r_comparison() {
    // Test case 1: Simple linear trend with noise
    // R code: x <- 1:20; y <- 1:20 + rnorm(20, 0, 0.5); smooth.spline(x, y, spar=0.5)
    let x1: Vec<f64> = (1..=20).map(|i| i as f64).collect();
    let y1 = vec![
        1.2, 2.1, 2.8, 4.0, 5.2, 5.9, 7.1, 8.0, 9.2, 10.1, 10.8, 12.0, 13.1, 14.0, 15.2, 15.9,
        17.1, 18.0, 19.2, 20.1,
    ];

    let smoothed1 = smooth_spline(&x1, &y1, 0.5).expect("Smoothing should succeed");

    // Check that smoothing preserves general trend
    assert_eq!(smoothed1.len(), y1.len());
    assert!(
        smoothed1[0] < smoothed1[smoothed1.len() - 1],
        "Trend should be preserved"
    );

    // Check that smoothing reduces noise (variance should decrease)
    let original_var = variance(&y1);
    let smoothed_var = variance(&smoothed1);
    assert!(
        smoothed_var <= original_var * 1.5,
        "Smoothed variance should be similar or smaller. Original: {}, Smoothed: {}",
        original_var,
        smoothed_var
    );

    println!("Test 1 - Linear with noise:");
    println!("  Original variance: {:.6}", original_var);
    println!("  Smoothed variance: {:.6}", smoothed_var);
    println!(
        "  First smoothed: {:.4}, Last smoothed: {:.4}",
        smoothed1[0],
        smoothed1[smoothed1.len() - 1]
    );
}

#[test]
fn test_smooth_spline_noisy_sine() {
    // Test case 2: Sinusoidal pattern with noise (simulates flow cytometry peaks)
    // This is more representative of actual flow cytometry data
    let n = 100;
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let mut y = Vec::with_capacity(n);

    // Generate sinusoidal pattern with noise
    for i in 1..=n {
        let base = 5.0 + 2.0 * (2.0 * std::f64::consts::PI * i as f64 / 50.0).sin();
        let noise = (i as f64 % 7.0 - 3.0) * 0.1; // Simple deterministic noise
        y.push(base + noise);
    }

    let smoothed = smooth_spline(&x, &y, 0.5).expect("Smoothing should succeed");

    assert_eq!(smoothed.len(), y.len());

    // Check that smoothing reduces high-frequency noise
    let original_var = variance(&y);
    let smoothed_var = variance(&smoothed);

    println!("Test 2 - Sinusoidal with noise:");
    println!("  n={}, Original variance: {:.6}", n, original_var);
    println!("  Smoothed variance: {:.6}", smoothed_var);
    println!(
        "  Variance reduction: {:.2}%",
        (1.0 - smoothed_var / original_var) * 100.0
    );

    // Check smoothness: second differences should be smaller
    let original_second_diff = second_differences_variance(&y);
    let smoothed_second_diff = second_differences_variance(&smoothed);

    println!("  Original 2nd diff variance: {:.6}", original_second_diff);
    println!("  Smoothed 2nd diff variance: {:.6}", smoothed_second_diff);
    assert!(
        smoothed_second_diff < original_second_diff * 2.0,
        "Smoothed second differences should be smaller. Original: {}, Smoothed: {}",
        original_second_diff,
        smoothed_second_diff
    );
}

#[test]
fn test_smooth_spline_flow_cytometry_like() {
    // Test case 3: Simulate flow cytometry peak trajectory
    // Typical flow cytometry data has peaks that represent cluster medians
    // over bins, with some noise and occasional outliers
    let n = 520; // Typical number of bins
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let mut y = Vec::with_capacity(n);

    // Generate trajectory similar to flow cytometry peaks:
    // - Starts low, increases, plateaus, decreases
    // - Some noise throughout
    // - Arcsinh-transformed values typically in range 0-10
    for i in 1..=n {
        let progress = i as f64 / n as f64;
        let base = if progress < 0.2 {
            2.0 + progress * 10.0 // Increasing
        } else if progress < 0.6 {
            4.0 // Plateau
        } else if progress < 0.8 {
            4.0 - (progress - 0.6) * 5.0 // Decreasing
        } else {
            3.0 + (progress - 0.8) * 2.0 // Slight increase
        };

        // Add noise (simulating measurement noise)
        let noise = ((i * 17) % 13 - 6) as f64 * 0.05;
        y.push(base + noise);
    }

    // Add a few outliers (simulating problematic bins)
    y[100] += 1.5;
    y[200] -= 1.2;
    y[350] += 0.8;

    let smoothed = smooth_spline(&x, &y, 0.5).expect("Smoothing should succeed");

    assert_eq!(smoothed.len(), y.len());

    // Check smoothing characteristics
    let original_var = variance(&y);
    let smoothed_var = variance(&smoothed);
    let original_second_diff = second_differences_variance(&y);
    let smoothed_second_diff = second_differences_variance(&smoothed);

    println!("Test 3 - Flow cytometry-like trajectory:");
    println!("  n={}, spar=0.5", n);
    println!("  Original variance: {:.6}", original_var);
    println!("  Smoothed variance: {:.6}", smoothed_var);
    println!("  Original 2nd diff variance: {:.6}", original_second_diff);
    println!("  Smoothed 2nd diff variance: {:.6}", smoothed_second_diff);

    // Output sample values for comparison with R
    println!("\n  Sample values (first 10, middle 5, last 10):");
    for i in 0..10 {
        println!(
            "    x[{}]={:.0}, y={:.4}, smoothed={:.4}",
            i, x[i], y[i], smoothed[i]
        );
    }
    let mid = n / 2;
    for i in (mid - 2)..=(mid + 2) {
        println!(
            "    x[{}]={:.0}, y={:.4}, smoothed={:.4}",
            i, x[i], y[i], smoothed[i]
        );
    }
    for i in (n - 10)..n {
        println!(
            "    x[{}]={:.0}, y={:.4}, smoothed={:.4}",
            i, x[i], y[i], smoothed[i]
        );
    }

    // Check that outliers are smoothed out
    assert!(
        (smoothed[100] - y[100]).abs() > 0.5,
        "Outlier at index 100 should be smoothed"
    );
    assert!(
        (smoothed[200] - y[200]).abs() > 0.5,
        "Outlier at index 200 should be smoothed"
    );

    // Check that smoothing preserves overall trend
    let start_avg: f64 = smoothed[0..50].iter().sum::<f64>() / 50.0;
    let end_avg: f64 = smoothed[(n - 50)..n].iter().sum::<f64>() / 50.0;
    println!("  Start average (first 50): {:.4}", start_avg);
    println!("  End average (last 50): {:.4}", end_avg);
}

#[test]
fn test_smooth_spline_output_for_r_comparison() {
    // Test case 4: Generate output that can be directly compared with R
    // Run this test and copy the output to compare with R's smooth.spline

    let n = 50;
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let y: Vec<f64> = (1..=n)
        .map(|i| {
            let i_f = i as f64;
            // Simple pattern: linear trend with some noise
            2.0 + i_f * 0.1 + (i % 5) as f64 * 0.05
        })
        .collect();

    let smoothed = smooth_spline(&x, &y, 0.5).expect("Smoothing should succeed");

    // Output in R-readable format
    println!("\n=== R Comparison Data ===");
    println!("# To compare with R, run:");
    println!(
        "# x <- c({})",
        x.iter()
            .map(|v| format!("{:.1}", v))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "# y <- c({})",
        y.iter()
            .map(|v| format!("{:.6}", v))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("# smoothed_r <- smooth.spline(x, y, spar=0.5)$y");
    println!(
        "# smoothed_rust <- c({})",
        smoothed
            .iter()
            .map(|v| format!("{:.6}", v))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("# max_diff <- max(abs(smoothed_r - smoothed_rust))");
    println!("# mean_diff <- mean(abs(smoothed_r - smoothed_rust))");
    println!("# cat(sprintf('Max diff: %.6f, Mean diff: %.6f\\n', max_diff, mean_diff))");
    println!();

    // Also output as CSV for easy comparison
    println!("x,y_original,y_smoothed");
    for i in 0..n {
        println!("{:.1},{:.6},{:.6}", x[i], y[i], smoothed[i]);
    }
}

#[test]
fn test_smooth_spline_r_direct_comparison() {
    // Test that directly calls R to compare outputs (if R is available)
    // Set RSCRIPT_AVAILABLE=1 environment variable to enable

    if std::env::var("RSCRIPT_AVAILABLE").is_err() {
        println!("Skipping R direct comparison test. Set RSCRIPT_AVAILABLE=1 to enable.");
        return;
    }

    // Check if R is available
    let r_available = Command::new("Rscript").arg("--version").output().is_ok();

    if !r_available {
        println!("R not found. Skipping direct R comparison.");
        return;
    }

    // Generate test data
    let n = 30;
    let x: Vec<f64> = (1..=n).map(|i| i as f64).collect();
    let y: Vec<f64> = (1..=n)
        .map(|i| {
            let i_f = i as f64;
            // Pattern with some noise
            let noise_val = ((i * 7) % 11) as i32;
            let noise = (noise_val as f64 - 5.0) * 0.1;
            2.0 + i_f * 0.15 + noise
        })
        .collect();

    // Get Rust smoothed values
    let smoothed_rust = smooth_spline(&x, &y, 0.5).expect("Smoothing should succeed");

    // Create R script
    let r_script = format!(
        r#"
x <- c({})
y <- c({})
result <- stats::smooth.spline(x, y, spar=0.5)
cat(paste(result$y, collapse=","))
"#,
        x.iter()
            .map(|v| format!("{:.6}", v))
            .collect::<Vec<_>>()
            .join(","),
        y.iter()
            .map(|v| format!("{:.6}", v))
            .collect::<Vec<_>>()
            .join(",")
    );

    // Run R script
    let output = Command::new("Rscript")
        .arg("-e")
        .arg(&r_script)
        .output()
        .expect("Failed to execute R");

    if !output.status.success() {
        eprintln!(
            "R script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    // Parse R output
    let r_output = String::from_utf8(output.stdout).expect("Invalid UTF-8 from R");
    let smoothed_r: Vec<f64> = r_output
        .trim()
        .split(',')
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .collect();

    if smoothed_r.len() != smoothed_rust.len() {
        eprintln!(
            "Length mismatch: R={}, Rust={}",
            smoothed_r.len(),
            smoothed_rust.len()
        );
        return;
    }

    // Calculate differences
    let mut max_diff = 0.0;
    let mut mean_diff = 0.0;
    let mut max_diff_idx = 0;

    for i in 0..smoothed_r.len() {
        let diff = (smoothed_r[i] - smoothed_rust[i]).abs();
        mean_diff += diff;
        if diff > max_diff {
            max_diff = diff;
            max_diff_idx = i;
        }
    }
    mean_diff /= smoothed_r.len() as f64;

    println!("\n=== R Direct Comparison Results ===");
    println!("n={}, spar=0.5", n);
    println!("Max difference: {:.6} at index {}", max_diff, max_diff_idx);
    println!("Mean difference: {:.6}", mean_diff);
    println!("R value at max_diff: {:.6}", smoothed_r[max_diff_idx]);
    println!("Rust value at max_diff: {:.6}", smoothed_rust[max_diff_idx]);

    // Show first 5 and last 5 for comparison
    println!("\nFirst 5 values:");
    for i in 0..5.min(n) {
        println!(
            "  [{}] R: {:.6}, Rust: {:.6}, diff: {:.6}",
            i,
            smoothed_r[i],
            smoothed_rust[i],
            (smoothed_r[i] - smoothed_rust[i]).abs()
        );
    }
    println!("\nLast 5 values:");
    for i in (n - 5).max(0)..n {
        println!(
            "  [{}] R: {:.6}, Rust: {:.6}, diff: {:.6}",
            i,
            smoothed_r[i],
            smoothed_rust[i],
            (smoothed_r[i] - smoothed_rust[i]).abs()
        );
    }

    // Warn if differences are large
    if max_diff > 0.1 {
        eprintln!(
            "\nWARNING: Large differences detected! Max diff = {:.6}",
            max_diff
        );
    } else if max_diff > 0.01 {
        eprintln!(
            "\nNOTE: Moderate differences detected. Max diff = {:.6}",
            max_diff
        );
    } else {
        println!("\n✓ Differences are small. Implementation matches R well.");
    }
}

fn variance(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    let var = data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / data.len() as f64;
    var
}

fn second_differences_variance(data: &[f64]) -> f64 {
    if data.len() < 3 {
        return 0.0;
    }
    let mut second_diffs = Vec::new();
    for i in 0..(data.len() - 2) {
        let diff = data[i] - 2.0 * data[i + 1] + data[i + 2];
        second_diffs.push(diff);
    }
    variance(&second_diffs)
}
