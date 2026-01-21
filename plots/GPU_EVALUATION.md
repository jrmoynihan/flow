# GPU Acceleration Evaluation for Density Plots

## Executive Summary

After extensive implementation and benchmarking, **GPU acceleration is NOT worthwhile** for density plot calculations in this crate. Sequential CPU processing provides the best performance, and batching (CPU or GPU) adds overhead without benefit for typical use cases.

**Recommendation**: Use sequential processing for all batch sizes. GPU code has been removed from this crate. Downstream consumers seeking additional parallelism should explore alternative approaches (see "Future Parallelism Opportunities" below).

## Benchmark Results

### Performance Comparison (5 plots, 50k events each)

| Method | Time | vs Sequential | Notes |
|--------|------|---------------|-------|
| **Sequential** | **32.9 ms** | 1.0x (baseline) | **Fastest** |
| CPU Batched | 42.3 ms | 0.78x | 29% slower |
| GPU Batched | 42.5 ms | 0.77x | 29% slower |

**Key Finding**: Sequential processing is 29% faster than batched approaches.

### Scaling Analysis

**With Plot Count (50k events each):**
- 5 plots: Sequential 32.9 ms, Batched 42.3 ms
- 10 plots: Sequential ~66 ms (estimated), Batched 84.7 ms
- 20 plots: Sequential ~132 ms (estimated), Batched 172.2 ms

**With Event Count (10 plots):**
- 50k events: Batched 84.7 ms
- 100k events: Batched 141.0 ms
- 500k events: Batched 444.9 ms

**Conclusion**: Sequential scales linearly and consistently outperforms batched approaches.

## Why GPU Doesn't Help

### 1. Density Accumulation Bottleneck

The core density accumulation step requires atomic operations that were difficult to implement efficiently on GPU:

- **CPU fallback required**: cubeCL kernel for atomic operations was incomplete due to type conversion limitations
- **Dominates pipeline**: Density accumulation takes ~60% of total time
- **GPU gains offset**: Even if GPU accumulation worked, transfer overhead would negate benefits

### 2. Memory Transfer Overhead

Multiple GPU↔CPU transfers add significant overhead:

```
Data → GPU (coordinate calculation)
GPU → CPU (density accumulation - CPU fallback)
CPU → GPU (log transform)
GPU → CPU (colormap application)
```

**Estimated overhead**: ~4 ms per batch, offsetting ~2 ms GPU compute gains.

### 3. Small Batch Overhead

- GPU initialization and kernel launch overhead
- Doesn't amortize well for typical batch sizes (5-20 plots)
- Sequential processing avoids all overhead

### 4. Colormap Limitation

- Colormap application runs on CPU (library limitation)
- Final step requires CPU, adding transfer overhead
- Would need GPU-compatible colormap library for full GPU pipeline

## Comparison with Other Crates

### gates Crate (Similar Findings)

From `gates/GPU_PERFORMANCE_FINDINGS.md`:
- GPU was **2-10x slower** than CPU even with proper cubeCL kernels
- Data transfer overhead dominated
- GPU overhead didn't amortize

**Similar pattern**: GPU overhead dominates even with proper implementation.

### peacoqc-rs (Successful GPU)

- **20-32x speedup** for batched multi-channel operations
- **Key difference**: All operations on GPU (no CPU fallbacks)
- Proper cubeCL implementation with atomic operations

**For plots crate**: CPU fallback for density accumulation prevents GPU benefit.

## Implementation Attempts

### What Was Tried

1. **burn Framework**: GPU tensor operations for coordinate calculation, log transform, max reduction
2. **cubeCL Kernels**: Attempted atomic operations for density accumulation
3. **Memory Optimizations**: Reduced CPU↔GPU transfers
4. **Batching**: Grouped plots by size for efficiency

### Technical Challenges

1. **cubeCL Atomic Operations**: Type conversion limitations (Line<F> to usize for array indexing)
2. **Multiple Transfers**: Pipeline requires 4+ GPU↔CPU transfers
3. **Colormap Library**: CPU-only, no GPU alternative available
4. **Small Batches**: Overhead dominates for typical use cases

### What Worked

- GPU coordinate calculation (faster than CPU)
- GPU log transform (faster than CPU)
- Memory transfer optimizations (reduced overhead)

### What Didn't Work

- GPU density accumulation (CPU fallback required)
- Overall GPU pipeline (overhead > gains)
- Batching (adds overhead, slower than sequential)

## Performance Breakdown

For 5 plots, 50k events each:

| Step | Sequential | Batched (CPU/GPU) | Notes |
|------|------------|-------------------|-------|
| Coordinate calculation | ~5 ms | ~5 ms | Similar |
| Density accumulation | ~25 ms | ~25 ms | CPU fallback in GPU path |
| Log transform | ~1 ms | ~1 ms | Similar |
| Max reduction | ~0.1 ms | ~0.1 ms | Similar |
| Colormap | ~10 ms | ~10 ms | CPU only |
| **Batching overhead** | 0 ms | **~11 ms** | Coordination, allocation |
| **Transfer overhead** | 0 ms | **~4 ms** | GPU↔CPU transfers (GPU only) |
| **Total** | **~41 ms** | **~42-43 ms** | Batched slower |

**Key insight**: Batching overhead (~11 ms) exceeds any potential gains.

## Recommendations

### For This Crate

1. ✅ **Use Sequential Processing**: Fastest for all batch sizes
2. ✅ **Removed GPU Code**: No performance benefit, adds complexity
3. ✅ **Keep Batch API**: Useful for applications that need batch semantics (even if slower)

### For Downstream Consumers

If you need additional parallelism for plotting:

1. **Parallelize Across Plots**: Use `rayon` or similar to process multiple plots in parallel
   ```rust
   use rayon::prelude::*;
   let results: Vec<_> = plot_requests.par_iter()
       .map(|(data, options)| {
           plot.render(data, options, &mut render_config)
       })
       .collect();
   ```

2. **Parallelize Within Plots**: For very large datasets, parallelize density accumulation
   - Split data into chunks
   - Process chunks in parallel
   - Merge density maps

3. **Consider Alternative Approaches**:
   - **WebGPU/WebGL**: For web-based applications, browser GPU may be more efficient
   - **Specialized Libraries**: Consider libraries designed for GPU-accelerated visualization
   - **Precomputation**: Pre-compute density maps for common plot configurations

4. **Profile Your Workload**: 
   - Measure actual performance in your application
   - GPU may help for very large batches (>100 plots) or very large datasets (>1M events)
   - Sequential is best for typical flow cytometry workflows

## Future Parallelism Opportunities

### 1. Plot-Level Parallelism

**Approach**: Process multiple plots concurrently using `rayon` or async tasks.

**Benefits**:
- No GPU overhead
- Simple implementation
- Scales with CPU cores
- Works well for independent plots

**Example**:
```rust
use rayon::prelude::*;
let results: Vec<_> = requests.par_iter()
    .map(|(data, options)| {
        DensityPlot::new().render(data, options, &mut render_config)
    })
    .collect();
```

### 2. Data Chunking

**Approach**: Split large datasets into chunks, process chunks in parallel, merge results.

**Benefits**:
- Parallelizes density accumulation (current bottleneck)
- No GPU dependencies
- Scales with dataset size

**Challenges**:
- Requires merging density maps
- May have cache locality issues

### 3. GPU with Complete Pipeline

**Approach**: If GPU acceleration is desired, implement complete GPU pipeline:
1. Complete cubeCL kernel for density accumulation
2. GPU-compatible colormap library
3. Minimize CPU↔GPU transfers
4. Benchmark to verify benefit

**Note**: Based on gates crate experience, GPU may still not provide benefit even with proper implementation.

### 4. Specialized Hardware

**Approach**: Consider specialized hardware or libraries:
- **CUDA**: For NVIDIA GPUs, native CUDA may be faster than WGPU
- **Metal**: For Apple Silicon, native Metal may be faster
- **Specialized Libraries**: Libraries designed for GPU visualization

## Conclusion

**GPU acceleration is NOT worthwhile** for density plot calculations:

1. **No performance benefit**: GPU is identical to CPU batched
2. **Slower than sequential**: Both batched approaches are slower
3. **Complexity cost**: GPU code adds maintenance burden
4. **CPU is excellent**: Sequential processing is fast and simple

**Final Recommendation**: 
- ✅ Use sequential processing for all batch sizes
- ✅ Removed GPU code from this crate
- ✅ Document findings for downstream consumers
- ✅ Provide guidance for alternative parallelism approaches

## References

- `gates/GPU_PERFORMANCE_FINDINGS.md`: Similar findings for gate filtering
- `peacoqc-rs`: Successful GPU implementation (all operations on GPU)
- Benchmark results: See `BENCHMARK_ANALYSIS.md` for detailed data
