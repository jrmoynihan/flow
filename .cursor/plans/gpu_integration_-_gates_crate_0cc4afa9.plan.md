---
name: GPU Integration - gates crate
overview: Integrate burn/cubeCL for GPU-accelerated gate filtering operations, focusing on batch point-in-polygon queries with custom kernels and CPU fallback support.
todos: []
---

# GPU Integration Plan: gates crate

## Overview

Add GPU acceleration to gate filtering operations in the `gates` crate, with special focus on optimizing point-in-polygon queries using custom cubeCL kernels. Maintain full CPU fallback compatibility.

## Current Implementation Analysis

### Key Operations to Accelerate

1. **Point-in-polygon queries** (`gates/src/types.rs:651-665`)

   - Current: Sequential ray-casting algorithm per point
   - Used in: `contains_point()`, `filter_by_polygon()`
   - Bottleneck: O(n) per point, sequential execution

2. **Point-in-rectangle queries** (`gates/src/filtering/mod.rs:302-333`)

   - Current: Simple bounds checking
   - Easy to vectorize

3. **Point-in-ellipse queries** (`gates/src/filtering/mod.rs:196-250`)

   - Current: Coordinate rotation + ellipse equation
   - Good candidate for batch processing

4. **Spatial indexing** (`gates/src/filtering/mod.rs:EventIndex`)

   - Current: R*-tree on CPU
   - Keep CPU-based (tree building not GPU-friendly)
   - GPU acceleration for batch queries after spatial filtering

## Architecture

### New Module Structure

```
gates/src/
├── gpu/
│   ├── mod.rs              # GPU backend abstraction
│   ├── backend.rs          # Backend detection & initialization
│   ├── kernels/
│   │   ├── mod.rs
│   │   ├── point_in_polygon.cu  # Custom cubeCL kernel
│   │   ├── point_in_ellipse.cu
│   │   └── point_in_rectangle.cu
│   ├── filter.rs           # GPU-accelerated filtering
│   └── fallback.rs         # CPU fallback implementations
```

### GPU Backend Abstraction

- Use `burn` for tensor operations and backend management
- Use `cubeCL` for custom point-in-polygon kernel
- Auto-detect best backend (WGPU → Candle → CPU)
- Runtime backend selection with CPU fallback

## Implementation Details

### 1. Custom Point-in-Polygon Kernel (cubeCL)

**File**: `gates/src/gpu/kernels/point_in_polygon.cu`

Create optimized GPU kernel for batch point-in-polygon:

- Process multiple points in parallel
- Shared memory for polygon edges
- Optimized ray-casting with early exit strategies
- Handle polygons with varying vertex counts

**Performance Target**: 10-100x speedup for batches of 10K+ points

### 2. GPU Filtering API

**File**: `gates/src/gpu/filter.rs`

```rust
pub struct GpuFilterBackend {
    backend: burn::backend::Backend, // Auto-detected
    device: Device,
}

impl GpuFilterBackend {
    /// Batch point-in-polygon query
    pub fn filter_by_polygon_batch(
        &self,
        points: &[(f32, f32)],
        polygon: &[(f32, f32)],
    ) -> Result<Vec<bool>>;
    
    /// Batch point-in-rectangle query
    pub fn filter_by_rectangle_batch(
        &self,
        points: &[(f32, f32)],
        bounds: (f32, f32, f32, f32),
    ) -> Result<Vec<bool>>;
    
    /// Batch point-in-ellipse query
    pub fn filter_by_ellipse_batch(
        &self,
        points: &[(f32, f32)],
        center: (f32, f32),
        radius_x: f32,
        radius_y: f32,
        angle: f32,
    ) -> Result<Vec<bool>>;
}
```

### 3. Enhanced EventIndex

**File**: `gates/src/filtering/mod.rs`

Modify `EventIndex::filter_by_gate()` to:

1. Use R*-tree for spatial filtering (CPU, keep as-is)
2. For batches > threshold (e.g., 1000 points), use GPU for precise tests
3. Fallback to CPU if GPU unavailable or batch too small
```rust
impl EventIndex {
    pub fn filter_by_gate(&self, gate: &Gate) -> Result<Vec<usize>> {
        // 1. Spatial filter with R*-tree (CPU)
        let candidates = self.spatial_filter(gate)?;
        
        // 2. Precise filtering (GPU if available, else CPU)
        if candidates.len() > GPU_THRESHOLD {
            self.precise_filter_gpu(gate, &candidates)
        } else {
            self.precise_filter_cpu(gate, &candidates)
        }
    }
}
```


### 4. CPU Fallback Strategy

**File**: `gates/src/gpu/fallback.rs`

- Always provide CPU implementation
- Use existing `point_in_polygon()` function
- Parallelize with Rayon for CPU fallback
- Seamless API - caller doesn't know if GPU or CPU used

### 5. Backend Detection

**File**: `gates/src/gpu/backend.rs`

```rust
pub enum ComputeBackend {
    Wgpu,
    CandleCuda,
    CandleMetal,
    Cpu,
}

pub fn detect_best_backend() -> ComputeBackend {
    // Try WGPU (Metal/Vulkan/DX12)
    // Fallback to Candle (CUDA/Metal)
    // Final fallback to CPU
}
```

## Integration Points

### Modify Existing Functions

1. **`GateGeometry::contains_point()`** (`gates/src/types.rs:305-376`)

   - Add batch version: `contains_points_batch()`
   - Use GPU for batches, CPU for single points

2. **`EventIndex::filter_by_polygon()`** (`gates/src/filtering/mod.rs:268-300`)

   - After R*-tree spatial filtering, use GPU for precise tests
   - Threshold-based: GPU for >1000 candidates, CPU otherwise

3. **`filter_events_by_gate()`** (`gates/src/filtering/mod.rs:347-373`)

   - Detect if GPU available
   - Route to GPU path for large datasets

## Dependencies

Add to `gates/Cargo.toml`:

```toml
[dependencies]
burn = { version = "0.15", features = ["wgpu", "candle"] }
candle-core = "0.4"
candle-nn = "0.4"
cubecl = "0.1"  # For custom kernels

[features]
default = []
gpu = ["burn/wgpu", "burn/candle", "cubecl"]
```

## Performance Considerations

### GPU Thresholds

- **Minimum batch size**: 1000 points (GPU overhead not worth it for smaller batches)
- **Optimal batch size**: 10K-1M points
- **Memory management**: Batch large datasets into chunks

### CPU Fallback Triggers

- GPU unavailable
- Batch size < threshold
- GPU memory insufficient
- GPU error during execution

## Testing Strategy

1. **Unit tests**: Verify GPU results match CPU exactly
2. **Performance benchmarks**: Compare GPU vs CPU for various batch sizes
3. **Fallback tests**: Ensure CPU path works when GPU disabled
4. **Integration tests**: Test with real FCS files

## Migration Path

1. Phase 1: Add GPU module alongside existing code (non-breaking)
2. Phase 2: Add feature flag for GPU support
3. Phase 3: Enable GPU by default with CPU fallback
4. Phase 4: Optimize based on benchmarks

## Custom Kernel Optimization Opportunities

### Point-in-Polygon Kernel Optimizations

1. **Coalesced memory access**: Structure point arrays for optimal GPU memory access
2. **Shared memory**: Cache polygon edges in shared memory per block
3. **Early exit**: For simple polygons, use bounding box pre-check
4. **Warp-level reductions**: Parallelize ray intersection tests
5. **Adaptive batching**: Adjust batch size based on polygon complexity

### Rectangle/Ellipse Kernels

- Simple vectorized operations
- Use burn tensor operations (no custom kernel needed)