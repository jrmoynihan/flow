use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use flow_plots::density_calc::calculate_density_per_pixel_batch;
use flow_plots::options::{AxisOptions, BasePlotOptions, DensityPlotOptions, PlotOptions};
use rand::Rng;
use std::hint::black_box;

fn generate_test_data(n_points: usize) -> Vec<(f32, f32)> {
    let mut rng = rand::thread_rng();
    (0..n_points)
        .map(|_| (rng.gen_range(0.0..200_000.0), rng.gen_range(0.0..200_000.0)))
        .collect()
}

fn create_test_options(width: u32, height: u32) -> DensityPlotOptions {
    let base = BasePlotOptions::new()
        .width(width)
        .height(height)
        .build()
        .unwrap();
    let x_axis = AxisOptions::default();
    let y_axis = AxisOptions::default();

    DensityPlotOptions::new()
        .base(base)
        .x_axis(x_axis)
        .y_axis(y_axis)
        .build()
        .unwrap()
}

fn benchmark_batch_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_density");

    // Scaling benchmarks: Vary plot count (hold events constant at 50k)
    for plot_count in [5, 10, 20] {
        let event_count = 50_000;
        let requests: Vec<(Vec<(f32, f32)>, DensityPlotOptions)> = (0..plot_count)
            .map(|_| {
                let data = generate_test_data(event_count);
                let options = create_test_options(800, 600);
                (data, options)
            })
            .collect();

        group.throughput(Throughput::Elements(plot_count as u64));
        group.bench_with_input(
            BenchmarkId::new("cpu_batch", format!("vary_plots_{}_plots_{}k_each", plot_count, event_count / 1000)),
            &requests,
            |b, requests| {
                b.iter(|| {
                    black_box(calculate_density_per_pixel_batch(black_box(requests)))
                });
            },
        );
    }

    // Scaling benchmarks: Vary event count (hold plot count constant at 10)
    for event_count in [50_000, 100_000, 500_000] {
        let plot_count = 10;
        let requests: Vec<(Vec<(f32, f32)>, DensityPlotOptions)> = (0..plot_count)
            .map(|_| {
                let data = generate_test_data(event_count);
                let options = create_test_options(800, 600);
                (data, options)
            })
            .collect();

        group.throughput(Throughput::Elements(plot_count as u64));
        group.bench_with_input(
            BenchmarkId::new("cpu_batch", format!("vary_events_{}_plots_{}k_each", plot_count, event_count / 1000)),
            &requests,
            |b, requests| {
                b.iter(|| {
                    black_box(calculate_density_per_pixel_batch(black_box(requests)))
                });
            },
        );
    }

    // Mixed scenarios
    let scenarios = vec![
        (5, 100_000),
        (10, 50_000),
        (20, 100_000),
    ];

    for (plot_count, event_count) in scenarios {
        let requests: Vec<(Vec<(f32, f32)>, DensityPlotOptions)> = (0..plot_count)
            .map(|_| {
                let data = generate_test_data(event_count);
                let options = create_test_options(800, 600);
                (data, options)
            })
            .collect();

        group.throughput(Throughput::Elements(plot_count as u64));
        group.bench_with_input(
            BenchmarkId::new("cpu_batch", format!("{}_plots_{}k_each", plot_count, event_count / 1000)),
            &requests,
            |b, requests| {
                b.iter(|| {
                    black_box(calculate_density_per_pixel_batch(black_box(requests)))
                });
            },
        );
    }

    group.finish();
}

fn benchmark_mixed_vs_constant_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_density_sizes");

    // Constant size: 10 plots, all 800×600
    let constant_requests: Vec<(Vec<(f32, f32)>, DensityPlotOptions)> = (0..10)
        .map(|_| {
            let data = generate_test_data(100_000);
            let options = create_test_options(800, 600);
            (data, options)
        })
        .collect();

    group.bench_with_input(
        BenchmarkId::new("cpu_batch", "constant_size_10_plots_800x600"),
        &constant_requests,
        |b, requests| {
            b.iter(|| {
                black_box(calculate_density_per_pixel_batch(black_box(requests)))
            });
        },
    );

    // Mixed sizes: 10 plots, mix of 800×600, 1024×768, 640×480
    let mixed_requests: Vec<(Vec<(f32, f32)>, DensityPlotOptions)> = (0..10)
        .map(|i| {
            let data = generate_test_data(100_000);
            let (width, height) = match i % 3 {
                0 => (800, 600),
                1 => (1024, 768),
                _ => (640, 480),
            };
            let options = create_test_options(width, height);
            (data, options)
        })
        .collect();

    group.bench_with_input(
        BenchmarkId::new("cpu_batch", "mixed_size_10_plots"),
        &mixed_requests,
        |b, requests| {
            b.iter(|| {
                black_box(calculate_density_per_pixel_batch(black_box(requests)))
            });
        },
    );

    group.finish();
}

fn benchmark_sequential_vs_batch(c: &mut Criterion) {
    use flow_plots::density_calc::calculate_density_per_pixel;

    let mut group = c.benchmark_group("batch_density_comparison");

    // Test with 5 plots, 50k events each
    let plot_count = 5;
    let event_count = 50_000;
    let requests: Vec<(Vec<(f32, f32)>, DensityPlotOptions)> = (0..plot_count)
        .map(|_| {
            let data = generate_test_data(event_count);
            let options = create_test_options(800, 600);
            (data, options)
        })
        .collect();

    // Sequential (baseline)
    group.bench_with_input(
        BenchmarkId::new("sequential", format!("{}_plots_{}k_each", plot_count, event_count / 1000)),
        &requests,
        |b, requests| {
            b.iter(|| {
                for (data, options) in requests {
                    let base = options.base();
                    black_box(calculate_density_per_pixel(
                        black_box(data),
                        base.width as usize,
                        base.height as usize,
                        black_box(options),
                    ));
                }
            });
        },
    );

    // CPU Batched
    group.bench_with_input(
        BenchmarkId::new("cpu_batched", format!("{}_plots_{}k_each", plot_count, event_count / 1000)),
        &requests,
        |b, requests| {
            b.iter(|| {
                black_box(calculate_density_per_pixel_batch(black_box(requests)))
            });
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    benchmark_batch_density,
    benchmark_mixed_vs_constant_sizes,
    benchmark_sequential_vs_batch
);

criterion_main!(benches);
