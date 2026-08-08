---
name: rust-features
description: 'Work with Cargo features and conditional compilation. Use when adding optional dependencies, using #[cfg(feature = "...")], conditional compilation, or organizing optional functionality. Handles feature flags, optional dependencies, and cfg attributes.'
---

# Cargo Features and Conditional Compilation

Guidelines for working with Cargo features and conditional compilation in Rust.

## When to Use This Skill

- Adding optional functionality
- Using `#[cfg(feature = "...")]` attributes
- Organizing optional dependencies
- Conditional compilation based on features
- Creating feature gates for functionality

## Defining Features in Cargo.toml

### Basic Feature

```toml
[features]
default = []
gpu = []
```

### Feature with Dependencies

```toml
[features]
default = []
gpu = ["burn", "burn_cubecl"]

[dependencies]
# Always included
ndarray = { workspace = true }

# Optional dependencies
burn = { version = "0.14", optional = true }
burn_cubecl = { version = "0.14", optional = true }
```

### Default Features

```toml
[features]
default = ["gpu"]
gpu = []
```

## Using #[cfg(feature = "...")]

### Conditional Module

```rust
#[cfg(feature = "gpu")]
pub mod gpu;

#[cfg(feature = "gpu")]
pub use gpu::GpuMatrixOps;
```

### Conditional Function

```rust
#[cfg(feature = "gpu")]
pub fn gpu_function() -> Result<()> {
    // GPU-specific implementation
    Ok(())
}

#[cfg(not(feature = "gpu"))]
pub fn gpu_function() -> Result<()> {
    Err(Error::GpuNotAvailable)
}
```

### Conditional Implementation

```rust
#[cfg(feature = "gpu")]
impl GpuOps for MyStruct {
    fn gpu_method(&self) -> Result<()> {
        // Implementation
        Ok(())
    }
}
```

### Conditional Benchmark

```rust
#[cfg(feature = "gpu")]
fn bench_gpu(c: &mut Criterion) {
    // GPU benchmark
}

#[cfg(not(feature = "gpu"))]
criterion_group!(benches, bench_cpu_only);

#[cfg(feature = "gpu")]
criterion_group!(
    benches,
    bench_cpu_only,
    bench_gpu,
    bench_comparison
);
```

## Common Patterns

### Feature-Gated API

```rust
#[cfg(feature = "gpu")]
pub struct GpuMatrixOps {
    // GPU implementation
}

#[cfg(feature = "gpu")]
impl GpuMatrixOps {
    pub fn new() -> Self {
        // Initialize GPU
    }
    
    pub fn is_available() -> bool {
        // Check GPU availability
    }
}
```

### Fallback Implementation

```rust
#[cfg(feature = "gpu")]
pub fn matrix_multiply(a: &Matrix, b: &Matrix) -> Result<Matrix> {
    if GpuOps::is_available() {
        gpu_multiply(a, b)
    } else {
        cpu_multiply(a, b)
    }
}

#[cfg(not(feature = "gpu"))]
pub fn matrix_multiply(a: &Matrix, b: &Matrix) -> Result<Matrix> {
    cpu_multiply(a, b)
}
```

### Conditional Compilation in Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic() {
        // Always available
    }
    
    #[cfg(feature = "gpu")]
    #[test]
    fn test_gpu() {
        // Only with gpu feature
    }
}
```

### Feature Detection at Runtime

```rust
#[cfg(feature = "gpu")]
pub fn try_gpu_operation() -> Option<Result<()>> {
    if GpuOps::is_available() {
        Some(gpu_operation())
    } else {
        None
    }
}
```

## Multiple Features

### Feature Combinations

```rust
#[cfg(all(feature = "gpu", feature = "cuda"))]
pub mod cuda_gpu;

#[cfg(any(feature = "gpu", feature = "opencl"))]
pub mod gpu_common;
```

### Feature Exclusions

```rust
#[cfg(all(feature = "gpu", not(feature = "cpu-only")))]
pub fn gpu_function() {
    // GPU function, but not if cpu-only is enabled
}
```

## Important Rules

1. **Use features for optional functionality**: Don't use features for different implementations of the same API
2. **Document features**: Add `#![doc(cfg(feature = "..."))]` to document feature-gated items
3. **Default features**: Be careful with default features - they're always enabled unless explicitly disabled
4. **Optional dependencies**: Mark optional dependencies with `optional = true` in `Cargo.toml`
5. **Feature combinations**: Use `all()`, `any()`, and `not()` for complex feature logic
6. **Runtime checks**: Use runtime checks when feature availability needs to be detected at runtime

## Documenting Features

```rust
/// GPU-accelerated matrix operations
///
/// This module requires the `gpu` feature to be enabled.
#[cfg(feature = "gpu")]
#[doc(cfg(feature = "gpu"))]
pub mod gpu;
```

## Building with Features

```bash
# Build with default features
cargo build

# Build with specific feature
cargo build --features gpu

# Build without default features
cargo build --no-default-features

# Build with multiple features
cargo build --features "gpu,cuda"

# Run tests with feature
cargo nextest run --features gpu

# Run benchmarks with feature
cargo bench --features gpu
```

## Examples from Project

See `fcs/Cargo.toml` and `fcs/src/gpu/` for examples of:
- Feature definitions
- Optional dependencies
- Conditional compilation
- Feature-gated modules

## Common Patterns

### ✅ Good

```toml
[features]
default = []
gpu = ["burn"]

[dependencies]
burn = { version = "0.14", optional = true }
```

```rust
#[cfg(feature = "gpu")]
pub mod gpu;
```

### ❌ Avoid

```toml
# Don't make everything a feature
[features]
everything = []
nothing = []
```

```rust
// Don't use features for different API versions
#[cfg(feature = "v1")]
pub fn api() {}
#[cfg(feature = "v2")]
pub fn api() {}  // BAD: Same function name
```
