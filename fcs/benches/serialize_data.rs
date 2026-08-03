//! Criterion: FCS float32 DATA segment serialization (columnar → row-major bytes).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_fcs::write::serialize_f32_columns;
use std::hint::black_box;
use std::time::Duration;

fn synth_columns(n_events: usize, n_params: usize) -> Vec<Vec<f32>> {
    (0..n_params)
        .map(|p| {
            (0..n_events)
                .map(|e| (e * n_params + p) as f32 * 0.001)
                .collect()
        })
        .collect()
}

fn bench_serialize(c: &mut Criterion) {
    let cases = [(100_000usize, 20usize), (1_000_000, 20)];

    let mut group = c.benchmark_group("serialize_data_le");
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(25);

    for &(n_events, n_params) in &cases {
        let owned = synth_columns(n_events, n_params);
        let refs: Vec<&[f32]> = owned.iter().map(|v| v.as_slice()).collect();
        let id = format!("{n_events}x{n_params}");
        group.throughput(Throughput::Bytes((n_events * n_params * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(id), &refs, |b, cols| {
            b.iter(|| black_box(serialize_f32_columns(black_box(cols), true).unwrap()));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_serialize);
criterion_main!(benches);
