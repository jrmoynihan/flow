//! Criterion: KnnGraph on-disk load/write (syscall/alloc A/B).

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use flow_knn::{DistanceMetric, KnnGraph, NeighborList, read_knn_graph, write_knn_graph};
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

fn synth_graph(n: usize, k: usize) -> KnnGraph {
    let neighbors = (0..n)
        .map(|i| NeighborList {
            indices: (0..k as u32)
                .map(|j| (i as u32 + j + 1) % n as u32)
                .collect(),
            distances: (0..k).map(|j| j as f32 * 0.01).collect(),
        })
        .collect();
    KnnGraph {
        neighbors,
        n,
        k,
        metric: DistanceMetric::Euclidean,
        provenance: Some("bench".into()),
    }
}

fn bench_graph_io(c: &mut Criterion) {
    let cases = [(50_000usize, 60usize), (100_000, 60)];
    let dir = std::env::temp_dir();

    let mut load_group = c.benchmark_group("knn_graph_io_load");
    load_group.warm_up_time(Duration::from_millis(300));
    load_group.measurement_time(Duration::from_secs(4));
    load_group.sample_size(20);

    for &(n, k) in &cases {
        let path: PathBuf = dir.join(format!(
            "flow-knn-io-load-bench-{}-{}-{}.bin",
            n,
            k,
            std::process::id()
        ));
        let graph = synth_graph(n, k);
        write_knn_graph(&path, &graph).expect("write graph");
        let bytes = (n * k * 8) as u64;
        load_group.throughput(Throughput::Bytes(bytes));
        load_group.bench_with_input(BenchmarkId::from_parameter(format!("{n}x{k}")), &path, |b, path| {
            b.iter(|| black_box(read_knn_graph(black_box(path)).unwrap()));
        });
        let _ = std::fs::remove_file(&path);
    }
    load_group.finish();

    let mut write_group = c.benchmark_group("knn_graph_io_write");
    write_group.warm_up_time(Duration::from_millis(300));
    write_group.measurement_time(Duration::from_secs(4));
    write_group.sample_size(20);

    for &(n, k) in &cases {
        let path: PathBuf = dir.join(format!(
            "flow-knn-io-write-bench-{}-{}-{}.bin",
            n,
            k,
            std::process::id()
        ));
        let graph = synth_graph(n, k);
        let bytes = (n * k * 8) as u64;
        write_group.throughput(Throughput::Bytes(bytes));
        write_group.bench_with_input(
            BenchmarkId::from_parameter(format!("{n}x{k}")),
            &(graph, path.clone()),
            |b, (graph, path)| {
                b.iter(|| {
                    write_knn_graph(black_box(path), black_box(graph)).unwrap();
                    black_box(());
                });
            },
        );
        let _ = std::fs::remove_file(&path);
    }
    write_group.finish();
}

criterion_group!(benches, bench_graph_io);
criterion_main!(benches);
