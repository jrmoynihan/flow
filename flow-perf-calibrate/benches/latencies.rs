//! Criterion counterpart to `examples/snapshot_host`. Prefer the example to fill
//! `docs/dev/PERF_HOST.md`; this bench is for HTML reports and regressions.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_perf_calibrate::{
    F32_SCAN_BYTES, GATHER_F32_ELEMS, GATHER_PROBES, MEMCPY_LARGE, MEMCPY_SMALL, WIDTH_ELEMS,
    filled_bytes, filled_f32, filled_f64, filled_u16, gather_sum_f32, hashmap_from_keys, hashmap_sum,
    memcpy_bytes, n_f32, random_indices, rayon_scale_sum, seq_scale_sum, sequential_indices,
    slice_sum_f32, sort_f32_clone, sum_f32, sum_f64, sum_u16, vec_push_f32,
};
use std::hint::black_box;
use std::time::Duration;

fn bench_f32_scans(c: &mut Criterion) {
    let mut group = c.benchmark_group("seq_sum_f32");
    for &bytes in &F32_SCAN_BYTES {
        let n = n_f32(bytes);
        let data = filled_f32(n, 1);
        group.throughput(Throughput::Bytes(bytes as u64));
        group.bench_with_input(BenchmarkId::from_parameter(bytes), &data, |b, data| {
            b.iter(|| black_box(sum_f32(data)));
        });
    }
    group.finish();
}

fn bench_width(c: &mut Criterion) {
    let mut group = c.benchmark_group("width_scan");
    let n = WIDTH_ELEMS;
    let u16s = filled_u16(n, 2);
    let f32s = filled_f32(n, 2);
    let f64s = filled_f64(n, 2);
    group.throughput(Throughput::Elements(n as u64));
    group.bench_function("u16", |b| b.iter(|| black_box(sum_u16(&u16s))));
    group.bench_function("f32", |b| b.iter(|| black_box(sum_f32(&f32s))));
    group.bench_function("f64", |b| b.iter(|| black_box(sum_f64(&f64s))));
    group.finish();
}

fn bench_gather(c: &mut Criterion) {
    let mut group = c.benchmark_group("gather_64mib");
    let data = filled_f32(GATHER_F32_ELEMS, 3);
    let seq = sequential_indices(GATHER_PROBES.min(GATHER_F32_ELEMS));
    let rnd = random_indices(GATHER_F32_ELEMS, GATHER_PROBES, 4);
    group.throughput(Throughput::Elements(seq.len() as u64));
    group.bench_function("sequential", |b| {
        b.iter(|| black_box(gather_sum_f32(&data, &seq)))
    });
    group.throughput(Throughput::Elements(rnd.len() as u64));
    group.bench_function("random", |b| {
        b.iter(|| black_box(gather_sum_f32(&data, &rnd)))
    });
    group.finish();
}

fn bench_memcpy(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy");
    for &n in &[MEMCPY_SMALL, MEMCPY_LARGE] {
        let src = filled_bytes(n, 5);
        let mut dst = vec![0u8; n];
        group.throughput(Throughput::Bytes(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &src, |b, src| {
            b.iter(|| {
                memcpy_bytes(&mut dst, src);
                black_box(&dst);
            });
        });
    }
    group.finish();
}

fn bench_vec_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_push_f32");
    for n in [10_000usize, 100_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("with_capacity", n), &n, |b, &n| {
            b.iter(|| black_box(vec_push_f32(n, true)));
        });
        group.bench_with_input(BenchmarkId::new("grow", n), &n, |b, &n| {
            b.iter(|| black_box(vec_push_f32(n, false)));
        });
    }
    group.finish();
}

fn bench_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");
    for n in [1_000usize, 100_000] {
        let (map, keys) = hashmap_from_keys(n);
        let slice: Vec<f32> = (0..n).map(|i| i as f32).collect();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("hashmap_get", n), &n, |b, _| {
            b.iter(|| black_box(hashmap_sum(&map, &keys)));
        });
        group.bench_with_input(BenchmarkId::new("slice_index", n), &n, |b, _| {
            b.iter(|| black_box(slice_sum_f32(&slice)));
        });
    }
    group.finish();
}

fn bench_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("sort_unstable_f32");
    for n in [10_000usize, 100_000] {
        let data = filled_f32(n, 6);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &data, |b, data| {
            b.iter(|| black_box(sort_f32_clone(data.clone())));
        });
    }
    group.finish();
}

fn bench_rayon(c: &mut Criterion) {
    let mut group = c.benchmark_group("rayon_scale");
    for n in [256usize, 10_000] {
        let data = filled_f32(n, 7);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("seq", n), &data, |b, data| {
            b.iter(|| black_box(seq_scale_sum(data)));
        });
        group.bench_with_input(BenchmarkId::new("par", n), &data, |b, data| {
            b.iter(|| black_box(rayon_scale_sum(data)));
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = bench_f32_scans, bench_width, bench_gather, bench_memcpy, bench_vec_push, bench_lookup, bench_sort, bench_rayon
}
criterion_main!(benches);
