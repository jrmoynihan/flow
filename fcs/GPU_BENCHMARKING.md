# GPU Acceleration Benchmarking Results

## Summary

GPU implementations for matrix operations were developed and benchmarked but found to be **slower than CPU** for typical flow cytometry workloads. As a result, GPU acceleration has been removed from the codebase in favor of CPU-only implementations.

## Benchmark Results

Benchmarks were conducted comparing CPU (OpenBLAS/LAPACK) vs GPU (WGPU backend) implementations for:

1. **Matrix inversion** (5×5 to 30×30 matrices typical in flow cytometry)
2. **Batch matrix-vector multiplication** (compensation operations on 10K-1M events with 5-30 channels)

### Key Findings

- **CPU was 1.2-21× faster** than GPU across typical flow cytometry workloads
- **GPU only showed advantage** in one narrow case: 10ch × 100K events (~1.25× faster)
- **Small datasets (< 50K events)**: CPU was 2-21× faster due to GPU overhead
- **Medium datasets (50K-250K events)**: CPU was 1.2-1.9× faster
- **Large datasets (> 500K events)**: CPU remained 1.5-1.75× faster

### Matrix Inversion Performance

CPU LAPACK performance for typical flow cytometry matrix sizes:
- 5×5: ~630ns
- 10×10: ~1.8µs
- 15×15: ~3.8µs
- 20×20: ~5.2µs
- 30×30: ~13µs

GPU was not benchmarked for matrix inversion as CPU performance is already in the microsecond range, making GPU overhead clearly prohibitive.

## Why GPU Was Slower

### 1. Transfer Overhead
CPU↔GPU data transfers have microsecond-level latency that dominates for small-to-medium datasets:
- **PCIe latency**: 3-5µs for small buffers, even on high-speed interconnects
- **Bandwidth scaling**: Transfers under ~1MB have much lower effective bandwidth
- **Fixed costs**: Transfer overhead doesn't scale down with data size

### 2. Kernel Launch Overhead
GPU operations have fixed startup costs:
- Kernel preparation and scheduling
- Queue management and synchronization
- These fixed costs exceed compute time for small matrices

### 3. CPU BLAS Optimization
Modern CPU BLAS/LAPACK implementations are highly optimized:
- **OpenBLAS/Intel MKL**: Highly tuned for small-to-medium matrices
- **Cache locality**: CPU benefits from data already in cache
- **SIMD vectorization**: Modern CPUs have excellent vector units
- **Multi-threading**: Efficient parallelization for these workloads

### 4. Matrix Size Characteristics
Flow cytometry uses small matrices (5×5 to 30×30):
- Well below typical GPU crossover points (256×256 to 1024×1024)
- GPU cores are underutilized for such small matrices
- CPU cache can hold entire matrices, eliminating memory bandwidth bottlenecks

## Architecture Considerations

This pattern is expected to hold across **most laptop architectures**:

- **Discrete GPUs**: Higher transfer overhead via PCIe, but still slower due to overhead
- **Integrated GPUs**: Lower transfer overhead (unified memory), but still kernel launch overhead
- **Apple Silicon**: Unified memory helps, but still overhead and CPU BLAS is highly optimized

The fundamental issue is that **transfer and kernel launch overhead are universal**, and flow cytometry matrices are simply too small to amortize these costs.

## When GPU Might Be Beneficial

GPU acceleration could potentially be beneficial for:

1. **Very large datasets**: >1M events with >30 channels (but benchmarks showed CPU still faster)
2. **Batch processing**: Processing many files simultaneously where data can stay on GPU
3. **Pipeline operations**: When data is already on GPU from previous operations
4. **Very large matrices**: >50×50 (not typical in flow cytometry)

However, none of these scenarios are common in typical flow cytometry workflows.

## Implementation History

- GPU implementations were developed using:
  - `burn` framework with WGPU backend
  - Custom cubeCL kernels for triangular solve operations
- Benchmarks were run on laptop architectures with both discrete and integrated GPUs
- Results consistently showed CPU superiority across all tested configurations

## Conclusion

CPU-only implementations provide the best performance for flow cytometry workloads. The added complexity, dependencies, and maintenance burden of GPU code is not justified by performance gains. The codebase now uses CPU-only matrix operations with highly optimized BLAS/LAPACK backends.
