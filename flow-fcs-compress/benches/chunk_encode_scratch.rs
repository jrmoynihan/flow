//! Criterion: multi-chunk encode with payload scratch reuse A/B.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use flow_fcs_compress::container::inline::encode_inline;
use flow_fcs_compress::{ChannelParams, CodecId};
use std::hint::black_box;
use std::time::Duration;

fn synth(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (i as f32).sin() * 1000.0 + (i as f32) * 0.1)
        .collect()
}

fn bench_chunk_encode(c: &mut Criterion) {
    // 16 chunks × 64k events (primary size for Campaign 2 item 4).
    let events_per_chunk = 65_536u32;
    let n_chunks = 16usize;
    let total = events_per_chunk as usize * n_chunks;
    let data = synth(total);
    let params = ChannelParams::linear_unsigned("synthetic", 262_144);
    let columns = [(
        "FL1".to_string(),
        params,
        data.as_slice(),
        CodecId::LosslessF32BssZstd,
    )];

    let mut group = c.benchmark_group("chunk_encode_scratch");
    group.warm_up_time(Duration::from_millis(400));
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(20);
    group.throughput(Throughput::Bytes((total * 4) as u64));
    group.bench_function("16x64k_bss_zstd_inline", |b| {
        b.iter(|| {
            black_box(encode_inline(black_box(&columns), events_per_chunk).unwrap());
        });
    });
    group.finish();
}

criterion_group!(benches, bench_chunk_encode);
criterion_main!(benches);
