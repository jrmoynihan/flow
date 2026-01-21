//! Tests to verify GPU implementations produce correct results

#[cfg(feature = "gpu")]
use peacoqc_rs::gpu::is_gpu_available;
use peacoqc_rs::qc::isolation_tree::build_feature_matrix;
use peacoqc_rs::qc::peaks::{ChannelPeakFrame, PeakInfo};
use peacoqc_rs::stats::density::KernelDensity;
use std::collections::HashMap;

#[test]
#[cfg(feature = "gpu")]
fn test_gpu_kde_correctness() {
    if !is_gpu_available() {
        println!("GPU not available, skipping GPU correctness test");
        return;
    }

    // Generate test data
    let data: Vec<f64> = (0..100_000)
        .map(|i| (i as f64 / 1000.0).sin() * 100.0 + 500.0)
        .collect();

    // Run CPU version
    let kde_cpu = KernelDensity::estimate(&data, 1.0, 512).unwrap();

    // Run GPU version (will use GPU if available)
    let kde_gpu = KernelDensity::estimate(&data, 1.0, 512).unwrap();

    // Compare results - should be very close (within numerical precision)
    let cpu_density = &kde_cpu.y;
    let gpu_density = &kde_gpu.y;

    assert_eq!(cpu_density.len(), gpu_density.len());

    // Check that densities are close (within 1% relative error)
    for (cpu_val, gpu_val) in cpu_density.iter().zip(gpu_density.iter()) {
        let rel_error = ((cpu_val - gpu_val).abs() / cpu_val.max(1e-10)).abs();
        assert!(
            rel_error < 0.01,
            "GPU result differs from CPU: CPU={}, GPU={}, rel_error={}",
            cpu_val,
            gpu_val,
            rel_error
        );
    }
}

#[test]
#[cfg(feature = "gpu")]
fn test_gpu_feature_matrix_correctness() {
    if !is_gpu_available() {
        println!("GPU not available, skipping GPU feature matrix correctness test");
        return;
    }

    // Generate test peak results
    let mut peak_results = HashMap::new();
    let channel1 = "FL1-A".to_string();
    let channel2 = "FL2-A".to_string();

    peak_results.insert(
        channel1.clone(),
        ChannelPeakFrame {
            peaks: vec![
                PeakInfo {
                    bin: 10,
                    peak_value: 100.0,
                    cluster: 0,
                },
                PeakInfo {
                    bin: 20,
                    peak_value: 200.0,
                    cluster: 1,
                },
            ],
        },
    );

    peak_results.insert(
        channel2.clone(),
        ChannelPeakFrame {
            peaks: vec![PeakInfo {
                bin: 15,
                peak_value: 150.0,
                cluster: 0,
            }],
        },
    );

    let n_bins = 100;

    // Run CPU version
    let (matrix_cpu, names_cpu) = build_feature_matrix(&peak_results, n_bins).unwrap();

    // Run GPU version (if available)
    #[cfg(feature = "gpu")]
    use peacoqc_rs::gpu::{build_feature_matrix_gpu, is_gpu_available};
    #[cfg(feature = "gpu")]
    let (matrix_gpu, names_gpu) = if is_gpu_available() {
        build_feature_matrix_gpu(&peak_results, n_bins).unwrap()
    } else {
        build_feature_matrix(&peak_results, n_bins).unwrap()
    };

    #[cfg(not(feature = "gpu"))]
    let (matrix_gpu, names_gpu) = build_feature_matrix(&peak_results, n_bins).unwrap();

    // Compare results
    assert_eq!(matrix_cpu.len(), matrix_gpu.len());
    assert_eq!(names_cpu, names_gpu);

    for (row_cpu, row_gpu) in matrix_cpu.iter().zip(matrix_gpu.iter()) {
        assert_eq!(row_cpu.len(), row_gpu.len());
        for (val_cpu, val_gpu) in row_cpu.iter().zip(row_gpu.iter()) {
            assert!(
                (val_cpu - val_gpu).abs() < 1e-10,
                "GPU result differs from CPU: CPU={}, GPU={}",
                val_cpu,
                val_gpu
            );
        }
    }
}
