use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use flow_fcs_compress::codec::lossless_f32::{BssZstd, RawZstd};
use flow_fcs_compress::{ChannelParams, ColumnCodec};
use std::hint::black_box;

fn synth(n: usize) -> Vec<f32> {
    (0..n).map(|i| (i as f32).sin() * 1000.0 + (i as f32) * 0.1).collect()
}

fn bench_codecs(c: &mut Criterion) {
    let n = 65_536;
    let input = synth(n);
    let params = ChannelParams::linear_unsigned("synthetic", 262_144);

    let mut group = c.benchmark_group("encode_64k_events");
    group.throughput(Throughput::Bytes((n * 4) as u64));

    group.bench_function("bss_zstd", |b| {
        let codec = BssZstd::default();
        b.iter(|| {
            let mut out = Vec::with_capacity(n * 4);
            codec
                .encode_chunk(black_box(&input), &params, &mut out)
                .unwrap();
            black_box(out);
        });
    });
    group.bench_function("raw_zstd", |b| {
        let codec = RawZstd::default();
        b.iter(|| {
            let mut out = Vec::with_capacity(n * 4);
            codec
                .encode_chunk(black_box(&input), &params, &mut out)
                .unwrap();
            black_box(out);
        });
    });
    group.finish();

    // Decode benches: pre-encode once, time the decode.
    let mut bss_payload = Vec::new();
    BssZstd::default()
        .encode_chunk(&input, &params, &mut bss_payload)
        .unwrap();
    let mut raw_payload = Vec::new();
    RawZstd::default()
        .encode_chunk(&input, &params, &mut raw_payload)
        .unwrap();

    let mut group = c.benchmark_group("decode_64k_events");
    group.throughput(Throughput::Bytes((n * 4) as u64));
    group.bench_function("bss_zstd", |b| {
        let codec = BssZstd::default();
        let mut out = vec![0.0f32; n];
        b.iter(|| {
            codec
                .decode_chunk(black_box(&bss_payload), &params, &mut out)
                .unwrap();
            black_box(&out);
        });
    });
    group.bench_function("raw_zstd", |b| {
        let codec = RawZstd::default();
        let mut out = vec![0.0f32; n];
        b.iter(|| {
            codec
                .decode_chunk(black_box(&raw_payload), &params, &mut out)
                .unwrap();
            black_box(&out);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_codecs);
criterion_main!(benches);
