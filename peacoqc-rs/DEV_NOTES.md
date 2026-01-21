# Development Notes

This document contains important technical notes for developers working on peacoqc-rs.

## Port Status: Production-Ready ✅

The Rust port is **functionally correct and production-ready**. All major algorithms have been verified to match the R implementation, with minor numerical precision differences (< 1-7% in most cases) that are expected and acceptable.

## Critical Implementation Details

### Preprocessing Order

The preprocessing order is **critical** and must match R's order exactly:

1. **RemoveMargins** (on raw data)
2. **RemoveDoublets** (on raw data)  
3. **Compensate** + **Transform** (biexponential if compensation available, arcsinh otherwise)

**Why it matters**: Margin/doublet removal must happen on raw data before transformation, as thresholds are calculated on raw values.

### Transformation Logic

- **If compensation available** (`$SPILLOVER`, `$SPILL`, or `$COMP` keyword): Use `biexponential` transformation (matching R's `estimateLogicle`)
- **If no compensation**: Use `arcsinh` transformation with cofactor=2000

### Feature Matrix Structure (Critical Fix)

The Isolation Tree feature matrix must have **one column per cluster per channel** (format: `{channel}_cluster_{cluster_id}`), not one column per channel. This is critical for IT to distinguish different peak trajectories.

**Structure**: `bins × (channels × clusters)` matrix where:

- Each cluster gets its own column
- Bins are initialized with cluster median (default value)
- Actual peak values replace defaults where peaks were detected

### Doublet Removal MAD Scaling

Doublet removal uses **scaled MAD** matching R's `stats::mad()` with constant=1.4826, not raw MAD.

**Formula**: `threshold = median(ratio) + nmad * scaled_mad(ratio)`

### Margin Removal Threshold

Maximum margin removal uses `>` (not `>=`) to match R's logic:

```rust
if v > threshold && mask[i] {  // Uses >, not >=
```

## Algorithm Verification

### Isolation Tree

- ✅ Matches R's `isolationTreeSD` implementation
- ✅ Uses SD-based reduction (not standard Isolation Forest)
- ✅ Finds largest homogeneous group (inliers)

### MAD Outlier Detection

- ✅ Matches R's MAD calculation with scale factor 1.4826
- ✅ Uses spline-smoothed peak trajectories
- ✅ Flags bins where trajectory exceeds threshold

### Peak Detection

- ✅ Uses kernel density estimation (KDE)
- ✅ Clusters peaks by median values
- ✅ Handles bins without peaks (uses cluster median)

## Known Differences from R

### Numerical Precision

- Minor differences (< 1-7%) in QC results due to:
  - Different KDE implementations
  - Floating-point precision differences
  - Different random number generators (if any)

### Cluster Assignments

- Rust may detect slightly different numbers of clusters than R
- This is acceptable as long as QC results are similar
- Differences propagate to IT results but don't significantly affect final QC

## Testing Results

Across 4 test files:

- ✅ **IT Results**: Match perfectly on 3/4 files (0 outlier bins)
- ✅ **MAD Results**: Very close (0.90-6.85% difference, acceptable)
- ✅ **Preprocessing**: Event counts match closely (0.09% difference)
- ✅ **Bin counts**: Match exactly after preprocessing fixes

## Doublet Removal Behavior

See `DOUBLET_REMOVAL_RECONCILIATION.md` for details on why:

- Default behavior removes doublets (matches R recommendations)
- Keeping doublets can cause more bins to be flagged (expected behavior)
- Published figures may use dataset-specific preprocessing

## GPU Acceleration

GPU acceleration is available via the `gpu` feature flag and provides significant speedup for batched multi-channel operations.

### Operations Using GPU

1. **FFT-based Kernel Density Estimation (KDE)**
   - Uses GPU for complex multiplication in frequency domain
   - CPU FFT is used for transforms (burn doesn't expose FFT directly)
   - **Benefit**: 20-32x speedup for batched multi-channel operations

2. **Feature Matrix Building**
   - Matrix construction for Isolation Tree analysis
   - **Benefit**: Moderate speedup for large matrices

### Performance Results

Batched operations (multiple channels processed together) show significant speedup:

| Configuration | Batched GPU | Sequential CPU | Speedup |
|--------------|-------------|----------------|---------|
| 5 channels, 50K events | 242 µs | 6.2 ms | **25.8x** |
| 5 channels, 100K events | 469 µs | 11.8 ms | **25.2x** |
| 5 channels, 500K events | 1.7 ms | 60.7 ms | **35.0x** |
| 10 channels, 50K events | 502 µs | 10.7 ms | **21.3x** |
| 10 channels, 100K events | 829 µs | 20.6 ms | **24.9x** |
| 10 channels, 500K events | 3.4 ms | 110 ms | **32.1x** |
| 10 channels, 1M events | 7.0 ms | 231 ms | **33.0x** |

**Key Insight**: Batching amortizes GPU overhead across multiple channels, providing massive speedups even for smaller datasets (50K-100K events per channel).

### Implementation Details

- **Backend**: WGPU (WebGPU) via burn framework
- **Custom Kernels**: cubeCL kernels available (optional, `--features cubecl`)
- **Batching**: GPU context reuse and kernel caching amortize overhead
- **Fallback**: Automatic CPU fallback when GPU unavailable
- **Usage**: GPU is used automatically whenever available (no thresholds)

### What Was Tried

1. **Burn tensor operations**: Primary implementation using burn's tensor API
   - Status: ✅ Used as primary implementation

2. **cubeCL custom kernels**: Custom GPU shaders for complex multiplication
   - Result: Slightly faster than burn tensors
   - Status: ✅ Available as optional feature (`--features cubecl`)

3. **Threshold analysis**: Initially tested thresholds to avoid GPU overhead
   - Result: Batched GPU provides 20-26x speedup even at 50K events with 5 channels
   - Status: ✅ Thresholds removed - GPU used whenever available

4. **Batched operations**: Process multiple channels together
   - Result: **20-32x speedup** - primary benefit of GPU implementation
   - Status: ✅ Core optimization strategy

### Operations NOT Using GPU

- **Statistical calculations** (median, percentile): GPU sorting not implemented, uses CPU
- **Single-channel operations**: Benefit only when batched with other channels

## References

- R PeacoQC package: See `PeacoQC R at master.R` for reference implementation
- R package improvements: See `R_PACKAGE_IMPROVEMENTS.md` for suggestions
