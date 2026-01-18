use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use polars::prelude::*;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Duration;

// Import peacoqc types
use peacoqc_rs::{PeacoQCConfig, PeacoQCData, QCMode, peacoqc};

/// In-memory FCS-like structure for benchmarking
/// This avoids needing to create actual FCS files on disk
struct BenchmarkFcs {
    data_frame: Arc<DataFrame>,
    channel_names: Vec<String>,
}

impl PeacoQCData for BenchmarkFcs {
    fn n_events(&self) -> usize {
        self.data_frame.height()
    }

    fn channel_names(&self) -> Vec<String> {
        self.channel_names.clone()
    }

    fn get_channel_range(&self, channel: &str) -> Option<(f64, f64)> {
        let series = self.data_frame.column(channel).ok()?;
        let f32_series = series.f32().ok()?;
        let slice = f32_series.cont_slice().ok()?;
        let min = *slice.iter().min_by(|a, b| a.partial_cmp(b).unwrap())? as f64;
        let max = *slice.iter().max_by(|a, b| a.partial_cmp(b).unwrap())? as f64;
        Some((min, max))
    }

    fn get_channel_f64(&self, channel: &str) -> peacoqc_rs::Result<Vec<f64>> {
        let series = self.data_frame.column(channel).map_err(|e| {
            peacoqc_rs::PeacoQCError::StatsError(format!("Channel {} not found: {}", channel, e))
        })?;
        let f32_series = series.f32().map_err(|e| {
            peacoqc_rs::PeacoQCError::StatsError(format!("Channel {} is not f32: {}", channel, e))
        })?;
        let slice = f32_series.cont_slice().map_err(|e| {
            peacoqc_rs::PeacoQCError::StatsError(format!(
                "Channel {} data not contiguous: {}",
                channel, e
            ))
        })?;
        Ok(slice.iter().map(|&x| x as f64).collect())
    }
}

/// Generate synthetic FCS-like data for benchmarking
fn generate_benchmark_fcs(num_events: usize, num_channels: usize) -> BenchmarkFcs {
    use rand::Rng;
    let mut rng = rand::rng();

    let mut columns = Vec::new();
    let mut channel_names = Vec::new();

    // Generate channels with realistic flow cytometry data patterns
    for i in 0..num_channels {
        let channel_name = if i == 0 {
            "FSC-A".to_string()
        } else if i == 1 {
            "SSC-A".to_string()
        } else {
            format!("FL{}-A", i - 1)
        };
        channel_names.push(channel_name.clone());

        // Generate data with realistic distributions
        let values: Vec<f32> = (0..num_events)
            .map(|_| {
                // Mix of low and high populations for some channels
                if i >= 2 && rng.random::<f64>() < 0.3 {
                    // High population
                    5000.0 + rng.random::<f32>() * 2000.0
                } else {
                    // Low population
                    100.0 + rng.random::<f32>() * 500.0
                }
            })
            .collect();

        columns.push(Column::new(channel_name.into(), values));
    }

    let df = DataFrame::new(columns).expect("Failed to create DataFrame");

    BenchmarkFcs {
        data_frame: Arc::new(df),
        channel_names,
    }
}

fn bench_qc_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("qc_throughput");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    // Test configurations
    let event_counts = vec![10_000, 25_000, 50_000, 100_000, 250_000, 500_000, 1_000_000];

    let channel_counts = vec![4, 8, 12, 16, 20];

    // Benchmark 1: Events vs Time (with varying channel counts)
    for num_channels in [4, 8, 16].iter() {
        for &num_events in &event_counts {
            let fcs = generate_benchmark_fcs(num_events, *num_channels);
            let channels = fcs.channel_names[2..].to_vec(); // Use fluorescence channels

            let config = PeacoQCConfig {
                channels: channels.clone(),
                determine_good_cells: QCMode::All,
                mad: 6.0,
                it_limit: 0.6,
                consecutive_bins: 5,
                ..Default::default()
            };

            let num_observations = num_events * channels.len();

            let fcs_clone1 = BenchmarkFcs {
                data_frame: Arc::clone(&fcs.data_frame),
                channel_names: fcs.channel_names.clone(),
            };
            let config_clone1 = config.clone();

            group.throughput(Throughput::Elements(num_events as u64));
            group.bench_with_input(
                BenchmarkId::new(
                    format!("events_vs_time_{}channels", num_channels),
                    format!("{}_events", num_events),
                ),
                &(),
                |b, _| {
                    b.iter(|| {
                        let result =
                            peacoqc(black_box(&fcs_clone1), black_box(&config_clone1)).unwrap();
                        black_box(result);
                    })
                },
            );

            // Also benchmark with observations throughput
            let fcs_clone2 = BenchmarkFcs {
                data_frame: Arc::clone(&fcs.data_frame),
                channel_names: fcs.channel_names.clone(),
            };
            let config_clone2 = config.clone();

            group.throughput(Throughput::Elements(num_observations as u64));
            group.bench_with_input(
                BenchmarkId::new(
                    format!("observations_vs_time_{}channels", num_channels),
                    format!("{}_events", num_events),
                ),
                &(),
                |b, _| {
                    b.iter(|| {
                        let result =
                            peacoqc(black_box(&fcs_clone2), black_box(&config_clone2)).unwrap();
                        black_box(result);
                    })
                },
            );
        }
    }

    // Benchmark 2: Channels vs Time (with varying event counts)
    for num_events in [50_000, 100_000, 500_000].iter() {
        for &num_channels in &channel_counts {
            let fcs = generate_benchmark_fcs(*num_events, num_channels);
            let channels = fcs.channel_names[2..].to_vec(); // Use fluorescence channels

            let config = PeacoQCConfig {
                channels: channels.clone(),
                determine_good_cells: QCMode::All,
                mad: 6.0,
                it_limit: 0.6,
                consecutive_bins: 5,
                ..Default::default()
            };

            let num_observations = *num_events * channels.len();

            let fcs_clone1 = BenchmarkFcs {
                data_frame: Arc::clone(&fcs.data_frame),
                channel_names: fcs.channel_names.clone(),
            };
            let config_clone1 = config.clone();

            group.throughput(Throughput::Elements(num_channels as u64));
            group.bench_with_input(
                BenchmarkId::new(
                    format!("channels_vs_time_{}events", num_events),
                    format!("{}_channels", num_channels),
                ),
                &(),
                |b, _| {
                    b.iter(|| {
                        let result =
                            peacoqc(black_box(&fcs_clone1), black_box(&config_clone1)).unwrap();
                        black_box(result);
                    })
                },
            );

            // Also benchmark with observations throughput
            let fcs_clone2 = BenchmarkFcs {
                data_frame: Arc::clone(&fcs.data_frame),
                channel_names: fcs.channel_names.clone(),
            };
            let config_clone2 = config.clone();

            group.throughput(Throughput::Elements(num_observations as u64));
            group.bench_with_input(
                BenchmarkId::new(
                    format!("observations_vs_time_{}events", num_events),
                    format!("{}_channels", num_channels),
                ),
                &(),
                |b, _| {
                    b.iter(|| {
                        let result =
                            peacoqc(black_box(&fcs_clone2), black_box(&config_clone2)).unwrap();
                        black_box(result);
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench_qc_throughput
}
criterion_main!(benches);
