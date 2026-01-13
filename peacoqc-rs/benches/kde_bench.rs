use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use peacoqc_rs::stats::density::KernelDensity;
use rand::Rng;
use std::hint::black_box;

/// Generate synthetic flow cytometry data with multiple peaks
fn generate_test_data(n_points: usize, n_peaks: usize) -> Vec<f64> {
    let mut rng = rand::thread_rng();
    let mut data = Vec::with_capacity(n_points);

    // Create data with n_peaks distinct populations
    let peak_positions: Vec<f64> = (0..n_peaks).map(|i| (i as f64 + 1.0) * 1000.0).collect();

    for _ in 0..n_points {
        // Pick a random peak
        let peak_idx = rng.random_range(0..n_peaks);
        let peak_center = peak_positions[peak_idx];

        // Add Gaussian noise around the peak
        let value = peak_center + rng.random_range(-200.0..200.0);
        data.push(value);
    }

    data
}

fn benchmark_kde_estimate(c: &mut Criterion) {
    let mut group = c.benchmark_group("kde_estimate");

    // Test different data sizes (representing different bin sizes)
    // Typical bin sizes: 1000 (default), 5000, 10000, 50000, 100000
    let data_sizes = vec![
        500,   // Small bin
        1000,  // Default bin size
        2500,  // Medium-small
        5000,  // Medium
        10000, // Large
        25000, // Very large
        50000, // Extreme (unlikely in practice, but tests scalability)
    ];

    // Test different grid sizes (number of evaluation points)
    let grid_sizes = vec![256, 512, 1024, 2048];

    for &n_data in &data_sizes {
        for &n_grid in &grid_sizes {
            let data = generate_test_data(n_data, 3); // 3 peaks

            group.bench_with_input(
                BenchmarkId::new("naive", format!("n_data={},n_grid={}", n_data, n_grid)),
                &(data, n_grid),
                |b, (data, n_grid)| {
                    b.iter(|| {
                        KernelDensity::estimate(
                            black_box(data),
                            black_box(1.0), // adjust factor
                            black_box(*n_grid),
                        )
                    })
                },
            );
        }
    }

    group.finish();
}

fn benchmark_kde_find_peaks(c: &mut Criterion) {
    let mut group = c.benchmark_group("kde_find_peaks");

    // Test peak finding on different density estimate sizes
    let grid_sizes = vec![256, 512, 1024, 2048];

    for &n_grid in &grid_sizes {
        // Create a realistic density estimate with 3 peaks
        let mut x = Vec::with_capacity(n_grid);
        let mut y = Vec::with_capacity(n_grid);

        let x_min = 0.0;
        let x_max = 5000.0;

        for i in 0..n_grid {
            let xi = x_min + (x_max - x_min) * (i as f64) / (n_grid - 1) as f64;
            x.push(xi);

            // Create a density with peaks at 1000, 2500, 4000
            let mut density = 0.0;
            for &peak_pos in &[1000.0, 2500.0, 4000.0] {
                let dist = (xi - peak_pos).abs();
                density += (-dist * dist / (2.0 * 400.0 * 400.0)).exp();
            }
            y.push(density);
        }

        let kde = KernelDensity { x, y };

        group.bench_with_input(BenchmarkId::from_parameter(n_grid), &kde, |b, kde| {
            b.iter(|| {
                kde.find_peaks(black_box(0.3));
            })
        });
    }

    group.finish();
}

fn benchmark_full_kde_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("kde_full_pipeline");

    // Test the complete pipeline: estimate + find_peaks
    // This is what gets called in actual peak detection
    let data_sizes = vec![500, 1000, 2500, 5000, 10000, 25000, 50000];
    let n_grid = 512; // Default grid size used in peaks.rs

    for &n_data in &data_sizes {
        let data = generate_test_data(n_data, 3);

        group.bench_with_input(BenchmarkId::from_parameter(n_data), &data, |b, data| {
            b.iter(|| {
                let kde =
                    KernelDensity::estimate(black_box(data), black_box(1.0), black_box(n_grid))
                        .unwrap();
                black_box(kde.find_peaks(0.3));
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_kde_estimate,
    benchmark_kde_find_peaks,
    benchmark_full_kde_pipeline
);
criterion_main!(benches);
