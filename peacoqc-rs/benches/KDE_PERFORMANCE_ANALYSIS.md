# KDE Performance Analysis

## Benchmark Results

Benchmarks were run to determine if FFT-based KDE is necessary for the PeacoQC implementation.

### Full Pipeline Performance (estimate + find_peaks)

The `kde_full_pipeline` benchmark measures the complete KDE workflow that gets called during peak detection:

| Data Size     | Grid Size | Time (mean) | Notes                |
| ------------- | --------- | ----------- | -------------------- |
| 500 events    | 512       | 0.71 ms     | Small bin            |
| 1,000 events  | 512       | 1.42 ms     | **Default bin size** |
| 2,500 events  | 512       | 3.54 ms     | Medium-small         |
| 5,000 events  | 512       | 7.17 ms     | Medium               |
| 10,000 events | 512       | 14.48 ms    | Large                |
| 25,000 events | 512       | 35.74 ms    | Very large           |
| 50,000 events | 512       | 72.11 ms    | Extreme (unlikely)   |

### Performance Characteristics

**Scaling**: The naive implementation scales as O(n × m) where:

- `n` = number of data points (events per bin)
- `m` = number of grid points (512 by default)

**Typical Use Case** (50k-1M events total):

- Default bin size: 1,000 events → ~1.4ms per bin
- 50 bins → ~70ms total KDE time
- 500 bins → ~700ms total KDE time

### When FFT Would Be Beneficial

FFT-based KDE has O(n log n) complexity for convolution, but has setup overhead. Based on benchmarks:

**Current Implementation is Sufficient When:**

- ✅ Bin sizes ≤ 10,000 events (typical: 1,000)
- ✅ Grid size is 512-1024 points (default: 512)
- ✅ Total dataset is < 1M events (typical: 50k-1M)

**FFT Would Help When:**

- ⚠️ Bin sizes > 50,000 events (rare in practice)
- ⚠️ Grid sizes > 2048 points (unlikely needed)
- ⚠️ Processing many very large files (>10M events)

### Recommendation

**Keep the naive implementation** for the following reasons:

1. **Default bin size (1,000 events)** is well within the fast range (<2ms per bin)
2. **Total processing time** for typical datasets is acceptable (<1s for KDE)
3. **No dependency overhead** - FFT libraries add complexity and dependencies
4. **Code simplicity** - easier to maintain and debug
5. **Early optimization is premature** - optimize if profiling shows KDE is a bottleneck

### Future Considerations

If profiling reveals KDE is a bottleneck in production:

- Add FFT-based KDE as an optional feature behind a feature flag
- Use automatic switching: naive for n < 10k, FFT for n ≥ 10k
- Consider using `rustfft` or similar pure Rust FFT library

### Benchmark Command

Run benchmarks with:

```bash
cargo bench -p peacoqc-rs --bench kde_bench
```

For quick tests:

```bash
cargo bench -p peacoqc-rs --bench kde_bench -- --quick
```
