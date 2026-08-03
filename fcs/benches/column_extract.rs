//! Criterion: row-major → column de-interleave (post-parse FCS path).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_fcs::file::extract_all_param_columns;
use std::hint::black_box;
use std::time::Duration;

fn synth(n_events: usize, n_params: usize) -> Vec<f32> {
    let total = n_events * n_params;
    (0..total).map(|i| (i as f32) * 0.001).collect()
}

fn bench_column_extract(c: &mut Criterion) {
    let cases = [(100_000usize, 20usize), (1_000_000, 20), (1_000_000, 40)];

    let mut group = c.benchmark_group("column_extract");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(30);

    for &(n_events, n_params) in &cases {
        let data = synth(n_events, n_params);
        let id = format!("{n_events}x{n_params}");
        group.throughput(Throughput::Elements((n_events * n_params) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(id), &data, |b, data| {
            b.iter(|| {
                black_box(extract_all_param_columns(
                    black_box(data),
                    n_events,
                    n_params,
                ))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_column_extract);
criterion_main!(benches);
