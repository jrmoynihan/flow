//! Criterion: peak detection bin-window alloc A/B.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use peacoqc_rs::qc::peaks::{PeakDetectionConfig, determine_channel_peaks_bench};
use std::hint::black_box;
use std::time::Duration;

fn synth(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let x = (i as f64) * 0.001;
            x.sin() * 100.0 + (i % 17) as f64
        })
        .collect()
}

fn bench_peaks(c: &mut Criterion) {
    let n = 100_000usize;
    let data = synth(n);
    let config = PeakDetectionConfig {
        remove_zeros: false,
        ..PeakDetectionConfig::default()
    };

    let mut group = c.benchmark_group("peaks_alloc_micro");
    group.warm_up_time(Duration::from_millis(400));
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(25);
    group.throughput(Throughput::Elements(n as u64));
    group.bench_function("100k_events", |b| {
        b.iter(|| {
            black_box(determine_channel_peaks_bench(
                black_box(&data),
                500,
                black_box(&config),
            ))
        });
    });
    group.finish();
}

criterion_group!(benches, bench_peaks);
criterion_main!(benches);
