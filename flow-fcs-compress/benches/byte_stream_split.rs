//! Isolated Criterion benches for BSS plane shuffle (no zstd).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_fcs_compress::transform::byte_stream_split::{split_f32_le, unsplit_f32_le};
use std::hint::black_box;
use std::time::Duration;

fn synth(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (i as f32).sin() * 1000.0 + (i as f32) * 0.1)
        .collect()
}

fn bench_bss(c: &mut Criterion) {
    let sizes = [65_536usize, 262_144, 1_048_576];

    let mut split_group = c.benchmark_group("bss_split");
    split_group.warm_up_time(Duration::from_millis(300));
    split_group.measurement_time(Duration::from_secs(2));
    split_group.sample_size(40);

    for &n in &sizes {
        let input = synth(n);
        split_group.throughput(Throughput::Bytes((n * 4) as u64));
        split_group.bench_with_input(BenchmarkId::from_parameter(n), &input, |b, input| {
            let mut out = Vec::with_capacity(n * 4);
            b.iter(|| {
                out.clear();
                split_f32_le(black_box(input), &mut out);
                black_box(&out);
            });
        });
    }
    split_group.finish();

    let mut unsplit_group = c.benchmark_group("bss_unsplit");
    unsplit_group.warm_up_time(Duration::from_millis(300));
    unsplit_group.measurement_time(Duration::from_secs(2));
    unsplit_group.sample_size(40);

    for &n in &sizes {
        let input = synth(n);
        let mut planes = Vec::new();
        split_f32_le(&input, &mut planes);
        unsplit_group.throughput(Throughput::Bytes((n * 4) as u64));
        unsplit_group.bench_with_input(BenchmarkId::from_parameter(n), &planes, |b, planes| {
            let mut out = vec![0.0f32; n];
            b.iter(|| {
                unsplit_f32_le(black_box(planes), &mut out);
                black_box(&out);
            });
        });
    }
    unsplit_group.finish();
}

criterion_group!(benches, bench_bss);
criterion_main!(benches);
