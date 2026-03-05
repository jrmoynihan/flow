---
name: GPU Integration - peacoqc-rs crate
overview: Integrate burn and cubeCL for GPU-accelerated FFT operations, feature matrix construction, and statistical calculations in quality control algorithms.
todos: []
---

# GPU Integration Plan: peacoqc-rs crate

## Overview

Add GPU acceleration to computationally intensive operations in the `peacoqc-rs` crate, focusing on FFT-based KDE, feature matrix operations, and statistical calculations. Maintain CPU fallback for all operations.

## Current Implementation Analysis

### Key Operations to Accelerate

1. **FFT-based Kernel Density Estimation** (`peacoqc-rs/src/stats/density.rs:111-209`)

   - Current: `realfft` crate for CPU FFT
   - FFT convolution for KDE (lines 145-209)
   - Bottleneck: FFT operations, especially for large datasets

2. **Feature Matrix Construction** (`peacoqc-rs/src/qc/isolation_tree.rs:211-271`)

   - Current: Sequential matrix building
   - Builds bins × features matrix
   - Cluster median calculations

3. **Isolation Tree Operations** (`peacoqc-rs/src/qc/isolation_tree.rs`)

   - Standard deviation calculations (lines 110-118, 466-516)
   - Feature matrix operations (covered above)
   - Tree building remains CPU (inherently sequential)

4. **Statistical Calculations**

   - Standard deviation, variance
   - Percentile calculations
   - Median calculations

## Architecture

### New Module Structure

```
peacoqc-rs/src/
├── gpu/
│   ├── mod.rs              # GPU backend & exports
│   ├── backend.rs          # Backend detection
│   ├── fft.rs              # GPU FFT operations
│   ├── matrix.rs           # GPU matrix operations
│   ├── stats.rs            # GPU statistical operations
│   └── fallback.rs         # CPU fallback wrappers
```

## Implementation Details

### 1. GPU FFT Operations

**File**: `peacoqc-rs/src/gpu/fft.rs`

```rust
use burn::tensor::{Tensor, Device};

pub struct GpuFft {
    device: Device,
}

impl GpuFft {
    /// FFT-based KDE on GPU
    pub fn kde_fft(
        &self,
        data: &[f64],
        grid: &[f64],
        bandwidth: f64,
    ) -> Result<Vec<f64>> {
        // 1. Bin data onto grid (GPU)
        // 2. Create kernel values (GPU)
        // 3. FFT both (GPU-accelerated FFT)
        // 4. Multiply in frequency domain (GPU)
        // 5. Inverse FFT (GPU)
        // 6. Extract and normalize (GPU)
    }
    
    /// Forward FFT
    pub fn fft_forward(&self, data: &[f64]) -> Result<Vec<Complex<f64>>>;
    
    /// Inverse FFT
    pub fn fft_inverse(&self, spectrum: &[Complex<f64>]) -> Result<Vec<f64>>;
}
```

### 2. GPU Feature Matrix Operations

**File**: `peacoqc-rs/src/gpu/matrix.rs`

```rust
pub struct GpuMatrixOps {
    device: Device,
}

impl GpuMatrixOps {
    /// Build feature matrix on GPU
    pub fn build_feature_matrix(
        &self,
        peak_results: &HashMap<String, ChannelPeakFrame>,
        n_bins: usize,
    ) -> Result<(Vec<Vec<f64>>, Vec<String>)> {
        // Parallel matrix construction
        // Vectorized cluster median calculations
        // Batch peak value assignments
    }
}
```

### 3. GPU Statistical Operations

**File**: `peacoqc-rs/src/gpu/stats.rs`

```rust
pub struct GpuStats {
    device: Device,
}

impl GpuStats {
    /// Parallel standard deviation calculation
    pub fn standard_deviation(&self, data: &[f64]) -> Result<f64>;
    
    /// Parallel percentile calculation
    pub fn percentile(&self, data: &[f64], p: f64) -> Result<f64>;
    
    /// Parallel median calculation
    pub fn median(&self, data: &[f64]) -> Result<f64>;
}
```

### 4. Enhanced KDE Implementation

**File**: `peacoqc-rs/src/stats/density.rs`

Modify `KernelDensity::estimate()`:

```rust
pub fn estimate(data: &[f64], adjust: f64, n_points: usize) -> Result<Self> {
    // Check if GPU available and dataset warrants GPU
    if data.len() > GPU_THRESHOLD && gpu_backend::is_available() {
        Self::estimate_gpu(data, adjust, n_points)
    } else {
        Self::estimate_cpu(data, adjust, n_points)  // Existing realfft impl
    }
}
```

### 5. Enhanced Feature Matrix Building

**File**: `peacoqc-rs/src/qc/isolation_tree.rs`

Modify `build_feature_matrix()`:

```rust
pub fn build_feature_matrix(
    peak_results: &HashMap<String, ChannelPeakFrame>,
    n_bins: usize,
) -> Result<(Vec<Vec<f64>>, Vec<String>)> {
    // Use GPU if available and matrix large enough
    if n_bins > GPU_THRESHOLD && gpu_backend::is_available() {
        build_feature_matrix_gpu(peak_results, n_bins)
    } else {
        build_feature_matrix_cpu(peak_results, n_bins)  // Existing impl
    }
}
```

### 6. CPU Fallback

**File**: `peacoqc-rs/src/gpu/fallback.rs`

- Wrap existing CPU implementations
- Same API as GPU versions
- Transparent fallback

### 7. Backend Detection

**File**: `peacoqc-rs/src/gpu/backend.rs`

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

1. **`KernelDensity::estimate()`** (`peacoqc-rs/src/stats/density.rs:22-60`)

   - Add GPU path for FFT-based KDE
   - Keep CPU path with `realfft`

2. **`kde_fft()`** (`peacoqc-rs/src/stats/density.rs:111-209`)

   - Replace `realfft` with GPU-accelerated FFT
   - Use burn's FFT operations or cuFFT/ROCm FFT

3. **`build_feature_matrix()`** (`peacoqc-rs/src/qc/isolation_tree.rs:211-271`)

   - GPU-accelerated matrix construction
   - Parallel cluster median calculations

4. **Standard Deviation Calculations** (`peacoqc-rs/src/qc/isolation_tree.rs:110-118`)

   - Use GPU for large arrays
   - Parallel variance calculation

## Performance Optimizations

### FFT Operations

- **GPU FFT libraries**: Use cuFFT (CUDA) or rocFFT (ROCm) via burn
- **Batch FFT**: Process multiple channels simultaneously
- **Memory**: Minimize GPU-CPU transfers

### Feature Matrix

- **Parallel construction**: Build multiple columns simultaneously
- **Vectorized operations**: Cluster median calculations
- **Memory access**: Coalesced access patterns

### Statistical Operations

- **Reduction operations**: Parallel sum, min, max
- **Sorting**: GPU-accelerated sorting for percentiles
- **Batch processing**: Calculate stats for multiple channels

## Dependencies

Add to `peacoqc-rs/Cargo.toml`:

```toml
[dependencies]
burn = { version = "0.15", features = ["wgpu", "candle"] }
candle-core = "0.4"
candle-nn = "0.4"

[features]
default = []
gpu = ["burn/wgpu", "burn/candle"]
```

Keep existing:

- `realfft` (for CPU fallback)

## CPU Fallback Strategy

1. **Automatic**: Detect GPU availability
2. **Threshold-based**: Use CPU for small datasets
3. **Library fallback**: Keep `realfft` for CPU FFT
4. **Transparent**: Same API, no breaking changes

## Testing Strategy

1. **Correctness**: Verify GPU results match CPU (within numerical tolerance)
2. **Performance**: Benchmark GPU vs CPU for various dataset sizes
3. **FFT accuracy**: Compare FFT results (should match within floating-point precision)
4. **Integration**: Test with real FCS files

## Migration Path

1. **Phase 1**: Add GPU module, keep CPU as default
2. **Phase 2**: Add feature flag for GPU support
3. **Phase 3**: Enable GPU by default with auto-detection
4. **Phase 4**: Optimize based on benchmarks

## Considerations

### FFT Library Selection

- **cuFFT** (CUDA): Best performance on NVIDIA GPUs
- **rocFFT** (ROCm): For AMD GPUs
- **WGPU FFT**: Cross-platform but potentially slower
- **Recommendation**: Use burn's backend abstraction, let it choose

### Numerical Precision

- FFT operations sensitive to floating-point precision
- Verify GPU results match CPU within acceptable tolerance
- May need to adjust tolerance for GPU vs CPU comparisons

### Isolation Tree Limitations

- Tree building inherently sequential
- GPU acceleration limited to:
  - Feature matrix construction
  - Statistical calculations within nodes
  - Batch processing of multiple trees (if applicable)

### Memory Management

- Feature matrices can be large (bins × features)
- Monitor GPU memory usage
- Consider chunking for very large matrices