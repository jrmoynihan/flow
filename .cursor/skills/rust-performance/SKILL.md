---
name: rust-performance
description: 'Optimize Rust code for performance, understand profiling, memory layout, and zero-cost abstractions. Use when optimizing code, profiling performance, understanding memory usage, or implementing performance-critical code. Handles profiling techniques, memory optimization, SIMD usage, and performance best practices.'
---

# Rust Performance Optimization

Guidelines for optimizing Rust code for performance.

## When to Use This Skill

- Optimizing performance-critical code
- Profiling applications
- Understanding memory usage
- Using SIMD for acceleration
- Implementing zero-cost abstractions
- Benchmarking improvements

## Profiling

### Using perf (Linux)

```bash
# Profile with perf
perf record --call-graph=dwarf ./target/release/my_program
perf report
```

### Using cargo-flamegraph

```bash
# Install cargo-flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin my_program
```

### Using criterion

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_function(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| black_box(my_function()))
    });
}

criterion_group!(benches, bench_function);
criterion_main!(benches);
```

## Memory Optimization

### Avoid Unnecessary Allocations

```rust
// ❌ Bad: Unnecessary allocation
fn bad_example() -> String {
    let mut result = String::new();
    result.push_str("prefix");
    result.push_str("suffix");
    result
}

// ✅ Good: Pre-allocate
fn good_example() -> String {
    let mut result = String::with_capacity(13);  // "prefixsuffix".len()
    result.push_str("prefix");
    result.push_str("suffix");
    result
}
```

### Use References Instead of Clones

```rust
// ❌ Bad: Unnecessary clone
fn process(data: Vec<i32>) {
    let cloned = data.clone();
    use_data(cloned);
}

// ✅ Good: Use reference
fn process(data: &Vec<i32>) {
    use_data(data);
}
```

### Use SmallVec for Small Collections

```rust
use smallvec::SmallVec;

// Use SmallVec for small collections that are usually stack-allocated
fn example() {
    let mut vec: SmallVec<[i32; 8]> = SmallVec::new();
    // Stored on stack if <= 8 elements, heap if larger
}
```

## Zero-Cost Abstractions

### Iterators Are Zero-Cost

```rust
// Iterators compile to efficient code
let sum: i32 = vec.iter().sum();
// Often as fast as manual loop
```

### Option/Result Are Zero-Cost

```rust
// Option<T> has same size as T when T is non-zero-sized
let opt: Option<i32> = Some(42);
// No overhead compared to sentinel values
```

## SIMD

### Using SIMD

```rust
use std::arch::x86_64::*;

#[target_feature(enable = "avx2")]
unsafe fn simd_add(a: &[f32], b: &[f32], result: &mut [f32]) {
    // SIMD-accelerated addition
    // Process multiple elements at once
}
```

### Using crates like packed_simd

```rust
use packed_simd::f32x4;

fn simd_example() {
    let a = f32x4::new(1.0, 2.0, 3.0, 4.0);
    let b = f32x4::new(5.0, 6.0, 7.0, 8.0);
    let sum = a + b;
}
```

## Inlining

### When to Inline

```rust
// Small, hot functions should be inlined
#[inline]
fn small_function(x: i32) -> i32 {
    x * 2
}

// Very small functions should always inline
#[inline(always)]
fn tiny_function(x: i32) -> i32 {
    x + 1
}

// Never inline large functions
#[inline(never)]
fn large_function() {
    // Large function body
}
```

## Cache-Friendly Code

### Sequential Access

```rust
// ✅ Good: Sequential access
fn good_example(data: &[i32]) -> i32 {
    let mut sum = 0;
    for &value in data {
        sum += value;  // Sequential memory access
    }
    sum
}

// ❌ Bad: Random access
fn bad_example(data: &[i32], indices: &[usize]) -> i32 {
    let mut sum = 0;
    for &index in indices {
        sum += data[index];  // Random memory access
    }
    sum
}
```

### Structure of Arrays vs Array of Structures

```rust
// Array of Structures (AoS) - cache-unfriendly
struct Point {
    x: f32,
    y: f32,
    z: f32,
}
let points: Vec<Point> = vec![];

// Structure of Arrays (SoA) - cache-friendly
struct Points {
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
}
```

## Parallelization

### Using Rayon

```rust
use rayon::prelude::*;

// Parallel iterator
let sum: i32 = data.par_iter().sum();

// Parallel processing
data.par_iter_mut().for_each(|x| {
    *x = process(*x);
});
```

### When to Parallelize

- Large datasets (>1000 elements)
- CPU-bound operations
- Independent operations
- Overhead is worth it

## Important Rules

1. **Measure first**: Profile before optimizing
2. **Optimize hot paths**: Focus on code that runs frequently
3. **Use zero-cost abstractions**: Iterators, Option, Result
4. **Avoid premature optimization**: Write clear code first
5. **Use appropriate data structures**: Choose based on access patterns
6. **Consider cache locality**: Sequential access is faster

## Performance Checklist

- [ ] Profile to find bottlenecks
- [ ] Avoid unnecessary allocations
- [ ] Use references instead of clones
- [ ] Pre-allocate collections when size known
- [ ] Use iterators for transformations
- [ ] Consider SIMD for numeric operations
- [ ] Parallelize when appropriate
- [ ] Optimize hot paths, not cold paths

## Common Patterns

### ✅ Good

```rust
// Pre-allocated, efficient
fn process(data: &[i32]) -> Vec<i32> {
    let mut result = Vec::with_capacity(data.len());
    result.extend(data.iter().map(|x| x * 2));
    result
}

// Iterator chain, lazy evaluation
let sum: i32 = data.iter()
    .filter(|x| *x > 0)
    .map(|x| x * 2)
    .sum();
```

### ❌ Avoid

```rust
// Unnecessary allocations
fn bad(data: Vec<i32>) -> Vec<i32> {
    let step1: Vec<i32> = data.iter().map(|x| x * 2).collect();
    let step2: Vec<i32> = step1.iter().filter(|x| *x > 10).collect();
    step2
}

// Unnecessary clones
fn bad(data: Vec<i32>) {
    let cloned = data.clone();
    process(cloned);
}
```

## Examples from Project

Look for performance optimizations in:
- Matrix operations
- Data processing pipelines
- GPU operations
- Benchmark results
