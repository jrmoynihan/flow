---
name: GPU Integration - fcs crate
overview: Integrate burn for GPU-accelerated matrix operations in compensation and spectral unmixing, with CPU fallback using existing ndarray/OpenBLAS implementation.
todos: []
---

# GPU Integration Plan: fcs crate

## Overview

Add GPU acceleration to matrix operations in the `fcs` crate, focusing on compensation matrix inversion and matrix-vector multiplication. Maintain compatibility with existing ndarray/OpenBLAS CPU implementation.

## Current Implementation Analysis

### Key Operations to Accelerate

1. **Compensation Matrix Operations** (`fcs/src/file.rs:1466-1624`)

   - Matrix inversion: `ndarray-linalg::Inverse` (OpenBLAS)
   - Matrix-vector multiplication: Sequential dot products per event
   - Current bottleneck: O(n²) matrix inversion, O(n×m) per-event multiplication

2. **Spectral Unmixing** (`fcs/src/file.rs:1720-1775`)

   - Similar to compensation (matrix operations)
   - Inverse transform + compensation + re-transform

3. **Parameter Statistics** (`fcs/src/file.rs:1105-1142`)

   - Min/max/mean/std calculations using Polars
   - Could benefit from GPU reduction operations

## Architecture

### New Module Structure

```
fcs/src/
├── gpu/
│   ├── mod.rs              # GPU backend & exports
│   ├── backend.rs          # Backend detection
│   ├── matrix.rs           # GPU matrix operations
│   ├── compensation.rs     # GPU-accelerated compensation
│   └── fallback.rs         # CPU fallback wrappers
```

## Implementation Details

### 1. GPU Matrix Operations

**File**: `fcs/src/gpu/matrix.rs`

```rust
use burn::tensor::{Tensor, Device};

pub struct GpuMatrixOps {
    device: Device,
}

impl GpuMatrixOps {
    /// Invert matrix on GPU
    pub fn invert_matrix(&self, matrix: &Array2<f32>) -> Result<Array2<f32>> {
        // Convert ndarray to burn tensor
        // Use burn's matrix inversion
        // Convert back to ndarray
    }
    
    /// Batch matrix-vector multiplication
    /// Input: matrix [n×n], vectors [m×n]
    /// Output: [m×n] result vectors
    pub fn batch_matvec(
        &self,
        matrix: &Array2<f32>,
        vectors: &Array2<f32>,
    ) -> Result<Array2<f32>>;
}
```

### 2. GPU-Accelerated Compensation

**File**: `fcs/src/gpu/compensation.rs`

```rust
pub struct GpuCompensation {
    matrix_ops: GpuMatrixOps,
}

impl GpuCompensation {
    /// Compensate multiple channels in parallel
    pub fn compensate_parameters(
        &self,
        comp_matrix: &Array2<f32>,
        channel_data: &[Vec<f32>],
    ) -> Result<Vec<Vec<f32>>> {
        // 1. Invert matrix on GPU
        // 2. Batch matrix-vector multiplication for all events
        // 3. Return compensated data
    }
}
```

### 3. Enhanced get_compensated_parameters()

**File**: `fcs/src/file.rs`

Modify `Fcs::get_compensated_parameters()`:

```rust
pub fn get_compensated_parameters(
    &self,
    channels_needed: &[&str],
) -> Result<HashMap<String, Vec<f32>>> {
    // Check if GPU available and batch size warrants GPU
    if self.should_use_gpu(channels_needed) {
        self.get_compensated_parameters_gpu(channels_needed)
    } else {
        self.get_compensated_parameters_cpu(channels_needed)  // Existing impl
    }
}
```

### 4. CPU Fallback

**File**: `fcs/src/gpu/fallback.rs`

- Wrap existing ndarray-linalg operations
- Same API as GPU version
- Transparent fallback when GPU unavailable

### 5. Backend Detection

**File**: `fcs/src/gpu/backend.rs`

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
    
    pub fn is_available(&self) -> bool {
        self.available
    }
}
```

## Integration Points

### Modify Existing Functions

1. **`Fcs::get_compensated_parameters()`** (`fcs/src/file.rs:1466-1624`)

   - Add GPU path alongside existing CPU path
   - Use GPU for: >10K events OR >5 channels
   - Keep existing optimizations (identity matrix check, sparsity analysis)

2. **`Fcs::apply_compensation()`** (`fcs/src/file.rs:1647-1720`)

   - Similar GPU integration
   - Batch matrix operations

3. **`Fcs::apply_spectral_unmixing()`** (`fcs/src/file.rs:1720-1775`)

   - GPU for matrix operations
   - Keep transform operations on CPU (element-wise, already fast)

## Performance Optimizations

### Batch Processing Strategy

- **Small batches** (<10K events): CPU (overhead not worth it)
- **Medium batches** (10K-100K): GPU with chunking
- **Large batches** (>100K): Full GPU, optimal performance

### Memory Management

- Transfer data to GPU once per operation
- Reuse GPU buffers when possible
- Batch multiple channels together

### Matrix Inversion Caching

- Cache inverted matrices (same compensation matrix reused)
- Store in GPU memory if available
- Invalidate on matrix change

## Dependencies

Add to `fcs/Cargo.toml`:

```toml
[dependencies]
burn = { version = "0.15", features = ["wgpu", "candle"] }
candle-core = "0.4"

[features]
default = []
gpu = ["burn/wgpu", "burn/candle"]
```

Keep existing:

- `ndarray` (for CPU fallback and data structures)
- `ndarray-linalg` (for CPU matrix inversion)

## CPU Fallback Strategy

1. **Automatic**: Detect GPU availability at runtime
2. **Graceful**: Fall back to existing CPU implementation
3. **Transparent**: Same API, no code changes needed
4. **Performance**: CPU path already optimized (OpenBLAS, Rayon)

## Testing Strategy

1. **Correctness**: Verify GPU results match CPU exactly (within floating-point tolerance)
2. **Performance**: Benchmark GPU vs CPU for various dataset sizes
3. **Fallback**: Test CPU path when GPU disabled/unavailable
4. **Edge cases**: Identity matrices, sparse matrices, small batches

## Migration Path

1. **Phase 1**: Add GPU module, keep CPU as default
2. **Phase 2**: Add feature flag `gpu` (opt-in)
3. **Phase 3**: Enable GPU by default with auto-detection
4. **Phase 4**: Optimize based on real-world benchmarks

## Considerations

### Matrix Size Impact

- Small matrices (<10×10): CPU faster (GPU overhead)
- Medium matrices (10×10 to 50×50): GPU beneficial for large batches
- Large matrices (>50×50): GPU significantly faster

### Event Count Impact

- <1K events: CPU (GPU overhead)
- 1K-10K: Depends on matrix size
- >10K: GPU almost always faster

### Multi-Channel Optimization

- Process all channels in single GPU batch
- Reduce GPU memory transfers
- Maximize GPU utilization