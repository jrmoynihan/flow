---
name: GPU Integration - plots crate
overview: Integrate burn for GPU-accelerated density calculations and pixel operations, focusing on parallel density accumulation and vectorized transformations.
todos: []
---

# GPU Integration Plan: plots crate

## Overview

Add GPU acceleration to density plot calculations in the `plots` crate, focusing on pixel-based density accumulation and color mapping operations. Maintain CPU fallback for compatibility.

## Current Implementation Analysis

### Key Operations to Accelerate

1. **Density Calculation** (`plots/src/density_calc.rs:115-245`)

   - Current: Array-based density building (lines 144-179)
   - Sequential pixel accumulation: `density[idx] += 1.0`
   - Log transformation: Sequential log10 operations (lines 206-209)
   - Color mapping: Sequential colormap lookups (lines 223-241)

2. **Performance Characteristics**

   - Already optimized for CPU (7x faster than HashMap approach)
   - Sequential processing chosen over parallel (overhead dominates)
   - GPU could change this tradeoff significantly

## Architecture

### New Module Structure

```
plots/src/
├── gpu/
│   ├── mod.rs              # GPU backend & exports
│   ├── backend.rs          # Backend detection
│   ├── density.rs          # GPU density calculations
│   └── fallback.rs         # CPU fallback
```

## Implementation Details

### 1. GPU Density Calculation

**File**: `plots/src/gpu/density.rs`

```rust
use burn::tensor::{Tensor, Device};

pub struct GpuDensityCalc {
    device: Device,
}

impl GpuDensityCalc {
    /// Calculate density map on GPU
    pub fn calculate_density(
        &self,
        data: &[(f32, f32)],
        width: usize,
        height: usize,
        x_range: (f32, f32),
        y_range: (f32, f32),
    ) -> Result<Vec<f32>> {
        // 1. Convert data to GPU tensor
        // 2. Parallel pixel coordinate calculation
        // 3. Atomic accumulation in density map (or use reduction)
        // 4. Return density array
    }
    
    /// Vectorized log transformation
    pub fn log_transform(&self, density: &[f32]) -> Result<Vec<f32>> {
        // GPU-accelerated log10(x + 1.0)
    }
    
    /// Vectorized color mapping
    pub fn apply_colormap(
        &self,
        density: &[f32],
        colormap: &Colormap,
    ) -> Result<Vec<RawPixelData>>;
}
```

### 2. Enhanced calculate_density_per_pixel()

**File**: `plots/src/density_calc.rs`

Modify `calculate_density_per_pixel_cancelable()`:

```rust
pub fn calculate_density_per_pixel_cancelable(
    data: &[(f32, f32)],
    width: usize,
    height: usize,
    options: &DensityPlotOptions,
    should_cancel: impl FnMut() -> bool,
) -> Option<Vec<RawPixelData>> {
    // Check if GPU available and dataset warrants GPU
    if data.len() > GPU_THRESHOLD && gpu_backend::is_available() {
        calculate_density_per_pixel_gpu(data, width, height, options, should_cancel)
    } else {
        calculate_density_per_pixel_cpu(data, width, height, options, should_cancel)  // Existing
    }
}
```

### 3. GPU Density Accumulation Strategy

**Approach 1: Atomic Operations**

- Use GPU atomics for parallel accumulation
- Each thread processes one data point
- Accumulate into shared memory density map

**Approach 2: Reduction-Based**

- Sort points by pixel coordinates
- Use parallel reduction to count per pixel
- More complex but potentially faster

**Recommendation**: Start with atomic operations (simpler), optimize to reduction if needed.

### 4. CPU Fallback

**File**: `plots/src/gpu/fallback.rs`

- Use existing `calculate_density_per_pixel_cancelable()` implementation
- Same API as GPU version
- Transparent fallback

### 5. Backend Detection

**File**: `plots/src/gpu/backend.rs`

```rust
pub struct GpuBackend {
    backend: burn::backend::Backend,
    device: Device,
    available: bool,
}

impl GpuBackend {
    pub fn new() -> Self {
        // Auto-detect best backend
        // Fallback to CPU if none available
    }
}
```

## Integration Points

### Modify Existing Functions

1. **`calculate_density_per_pixel_cancelable()`** (`plots/src/density_calc.rs:126-245`)

   - Add GPU path for large datasets (>50K points)
   - Keep CPU path for smaller datasets and fallback

2. **Density Building** (lines 144-179)

   - GPU: Parallel pixel coordinate calculation
   - GPU: Atomic accumulation or reduction

3. **Log Transformation** (lines 206-209)

   - GPU: Vectorized log10 operations
   - Simple element-wise operation, perfect for GPU

4. **Color Mapping** (lines 223-241)

   - GPU: Parallel colormap lookups
   - Vectorized operations

## Performance Optimizations

### GPU Thresholds

- **Minimum**: 50K data points (GPU overhead for smaller datasets)
- **Optimal**: 100K-10M points
- **Memory**: Chunk very large datasets

### Density Map Size Impact

- Small maps (<500×500): CPU faster
- Medium maps (500×500 to 2000×2000): GPU beneficial
- Large maps (>2000×2000): GPU significantly faster

### Memory Access Patterns

- Coalesced memory access for data points
- Shared memory for density map tiles
- Minimize GPU-CPU transfers

## Dependencies

Add to `plots/Cargo.toml`:

```toml
[dependencies]
burn = { version = "0.15", features = ["wgpu", "candle"] }
candle-core = "0.4"

[features]
default = []
gpu = ["burn/wgpu", "burn/candle"]
```

## CPU Fallback Strategy

1. **Automatic**: Detect GPU availability
2. **Threshold-based**: Use CPU for small datasets
3. **Graceful**: Fall back on GPU errors
4. **Transparent**: Same API, no breaking changes

## Testing Strategy

1. **Correctness**: Verify GPU density maps match CPU exactly
2. **Performance**: Benchmark GPU vs CPU for various dataset sizes
3. **Visual**: Compare rendered plots (should be identical)
4. **Edge cases**: Empty data, single point, very large datasets

## Migration Path

1. **Phase 1**: Add GPU module alongside existing code
2. **Phase 2**: Add feature flag for GPU support
3. **Phase 3**: Enable GPU by default with auto-detection
4. **Phase 4**: Optimize based on benchmarks

## Considerations

### Cancellation Support

- GPU operations harder to cancel mid-execution
- Options:

  1. Check cancellation between GPU kernel launches
  2. Use smaller GPU batches for cancellation points
  3. Accept that GPU path may be less responsive to cancellation

### Progress Reporting

- GPU operations are asynchronous
- Report progress after kernel completion
- Less granular than CPU (check every 250K points)

### Memory Management

- Large density maps (e.g., 4K×4K = 16M pixels)
- Consider chunking or streaming for very high resolution
- Monitor GPU memory usage