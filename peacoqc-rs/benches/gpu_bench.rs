//! Benchmark GPU vs CPU performance for PeacoQC operations

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use peacoqc_rs::stats::density::KernelDensity;
use peacoqc_rs::qc::isolation_tree::build_feature_matrix;
use peacoqc_rs::qc::peaks::{ChannelPeakFrame, PeakInfo};
use std::collections::HashMap;
use std::hint::black_box;
use rand::Rng;

fn generate_random_data(n: usize) -> Vec<f64> {
    let mut rng = rand::rng();
    (0..n).map(|_| rng.random_range(0.0..1000.0)).collect()
}

fn generate_peak_results(n_channels: usize, n_bins: usize) -> HashMap<String, ChannelPeakFrame> {
    let mut rng = rand::rng();
    let mut peak_results = HashMap::new();
    
    for ch in 0..n_channels {
        let channel_name = format!("FL{}-A", ch + 1);
        let mut peaks = Vec::new();
        
        // Generate peaks for some bins
        for bin in 0..n_bins {
            if rng.random_bool(0.3) { // 30% of bins have peaks
                peaks.push(PeakInfo {
                    bin,
                    peak_value: rng.random_range(100.0..1000.0),
                    cluster: rng.random_range(0..3),
                });
            }
        }
        
        peak_results.insert(channel_name, ChannelPeakFrame { peaks });
    }
    
    peak_results
}

fn bench_kde_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("KDE CPU vs GPU (Threshold Analysis)");
    
    // Test around threshold to verify 500K threshold is correct
    // Include 50K and 100K to see if GPU should be used earlier
    for size in [50_000, 100_000, 500_000, 1_000_000, 2_000_000].iter() {
        let data = generate_random_data(*size);
        
        // CPU benchmark
        group.bench_with_input(
            BenchmarkId::new("CPU", size),
            &data,
            |b, data| {
                b.iter(|| {
                    KernelDensity::estimate(black_box(data), 1.0, 512).unwrap()
                })
            },
        );
        
        // GPU benchmark (if available)
        #[cfg(feature = "gpu")]
        {
            use peacoqc_rs::gpu::is_gpu_available;
            if is_gpu_available() {
                group.bench_with_input(
                    BenchmarkId::new("GPU", size),
                    &data,
                    |b, data| {
                        b.iter(|| {
                            KernelDensity::estimate(black_box(data), 1.0, 512).unwrap()
                        })
                    },
                );
            }
        }
    }
    
    group.finish();
}

fn bench_kde_batched_gpu(c: &mut Criterion) {
    #[cfg(feature = "gpu")]
    {
        use peacoqc_rs::gpu::{is_gpu_available, GpuContext, kde_fft_batched_gpu, KdeContext};
        
        if !is_gpu_available() {
            return;
        }
        
        let mut group = c.benchmark_group("KDE Batched GPU (Multiple Channels)");
        
        // Test batched processing with multiple channels
        // Test smaller sizes too - batching may amortize overhead even for small datasets
        for n_channels in [5, 10].iter() {
            for size_per_channel in [50_000, 100_000, 500_000, 1_000_000].iter() {
                // Pre-compute grids and bandwidths for all channels
                let mut all_data = Vec::new();
                let mut all_grids = Vec::new();
                let mut all_bandwidths = Vec::new();
                let mut all_ns = Vec::new();
                
                for _ in 0..*n_channels {
                    let data = generate_random_data(*size_per_channel);
                    // Pre-compute KDE once to get grid and bandwidth
                    let kde = KernelDensity::estimate(&data, 1.0, 512).unwrap();
                    all_data.push(data);
                    all_grids.push(kde.x.clone()); // Clone to avoid lifetime issues
                    // Estimate bandwidth (simplified)
                    all_bandwidths.push(1.0);
                    all_ns.push(*size_per_channel as f64);
                }
                
                // Batched GPU benchmark
                group.bench_with_input(
                    BenchmarkId::new("Batched GPU", format!("{}ch_{}events", n_channels, size_per_channel)),
                    &(all_data.clone(), all_grids.clone(), all_bandwidths.clone(), all_ns.clone()),
                    |b, (all_data, all_grids, all_bandwidths, all_ns)| {
                        let mut gpu_ctx = GpuContext::new().unwrap();
                        b.iter(|| {
                            let contexts: Vec<KdeContext> = all_data.iter()
                                .zip(all_grids.iter())
                                .zip(all_bandwidths.iter())
                                .zip(all_ns.iter())
                                .map(|(((data, grid), bw), n)| KdeContext {
                                    data: black_box(data),
                                    grid: black_box(grid),
                                    bandwidth: *bw,
                                    n: *n,
                                })
                                .collect();
                            kde_fft_batched_gpu(&contexts, &mut gpu_ctx).unwrap()
                        })
                    },
                );
                
                // Sequential CPU benchmark (for comparison)
                group.bench_with_input(
                    BenchmarkId::new("Sequential CPU", format!("{}ch_{}events", n_channels, size_per_channel)),
                    &all_data,
                    |b, all_data| {
                        b.iter(|| {
                            for data in all_data {
                                KernelDensity::estimate(black_box(data), 1.0, 512).unwrap();
                            }
                        })
                    },
                );
            }
        }
        
        group.finish();
    }
}

fn bench_feature_matrix_cpu_vs_gpu(c: &mut Criterion) {
    let mut group = c.benchmark_group("Feature Matrix CPU vs GPU");
    
    for (n_bins, n_channels) in [
        (1000, 5),
        (5000, 5),
        (10000, 5),
        (50000, 5),
        (1000, 10),
        (5000, 10),
    ].iter() {
        let peak_results = generate_peak_results(*n_channels, *n_bins);
        
        // CPU benchmark
        group.bench_with_input(
            BenchmarkId::new("CPU", format!("{}bins_{}ch", n_bins, n_channels)),
            &peak_results,
            |b, pr| {
                b.iter(|| {
                    build_feature_matrix(black_box(pr), *n_bins).unwrap()
                })
            },
        );
        
        // GPU benchmark (if available)
        #[cfg(feature = "gpu")]
        {
            use peacoqc_rs::gpu::{is_gpu_available, build_feature_matrix_gpu};
            if is_gpu_available() {
                group.bench_with_input(
                    BenchmarkId::new("GPU", format!("{}bins_{}ch", n_bins, n_channels)),
                    &peak_results,
                    |b, pr| {
                        b.iter(|| {
                            build_feature_matrix_gpu(black_box(pr), *n_bins).unwrap()
                        })
                    },
                );
            }
        }
    }
    
    group.finish();
}

criterion_group!(benches, bench_kde_cpu_vs_gpu, bench_kde_batched_gpu, bench_feature_matrix_cpu_vs_gpu);
criterion_main!(benches);
