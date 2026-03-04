---
name: criterion-benchmarks
description: 'Create Criterion benchmarks with proper group macros and configuration. Use when writing benchmarks, creating benchmark groups, using criterion_group! and criterion_main! macros, setting throughput, or benchmarking multiple implementations. Handles benchmark groups, throughput metrics, conditional compilation, and proper black_box usage.'
---

# Criterion Benchmarking

Guidelines for writing Criterion benchmarks following this project's conventions.

## When to Use This Skill

- Creating new benchmark files
- Writing benchmark functions
- Setting up benchmark groups
- Configuring throughput metrics
- Using `criterion_group!` and `criterion_main!` macros
- Benchmarking multiple implementations (CPU vs GPU, etc.)

## Basic Structure

### Required Imports

```rust
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
```

### Basic Benchmark Function

```rust
fn bench_function_name(c: &mut Criterion) {
    let mut group = c.benchmark_group("group_name");
    
    // Setup test data
    let test_data = setup_test_data();
    
    // Optional: Set throughput
    group.throughput(Throughput::Elements(test_data.len() as u64));
    
    // Benchmark with input
    group.bench_with_input(
        BenchmarkId::new("benchmark_name", input_size),
        &test_data,
        |b, data| {
            b.iter(|| black_box(function_to_benchmark(data)));
        },
    );
    
    group.finish();
}
```

## Benchmark Groups

### Simple Group

For benchmarks with a single test case:

```rust
fn bench_simple(c: &mut Criterion) {
    c.bench_function("simple_bench", |b| {
        b.iter(|| black_box(function_to_bench()));
    });
}
```

### Group with Multiple Test Cases

```rust
fn bench_multiple_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation_name");
    
    let sizes = vec![5, 10, 15, 20, 30];
    
    for &size in &sizes {
        let test_data = generate_test_data(size);
        
        group.throughput(Throughput::Elements((size * size) as u64));
        group.bench_with_input(
            BenchmarkId::new("implementation", size),
            &test_data,
            |b, data| {
                b.iter(|| black_box(implementation(data)));
            },
        );
    }
    
    group.finish();
}
```

### Comparison Benchmarks (CPU vs GPU)

```rust
#[cfg(feature = "gpu")]
fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation_comparison");
    
    let gpu_ops = GpuOps::new();
    if !gpu_ops.is_available() {
        eprintln!("⚠️  GPU not available, skipping GPU benchmarks");
        return;
    }
    
    let test_cases = vec![(10, 100_000), (15, 250_000)];
    
    for &(channels, events) in &test_cases {
        let data = generate_data(channels, events);
        
        group.throughput(Throughput::Elements((channels * events) as u64));
        
        // CPU benchmark
        group.bench_with_input(
            BenchmarkId::new("CPU", format!("{}ch_{}ev", channels, events)),
            &data,
            |b, d| {
                b.iter(|| black_box(cpu_implementation(d)));
            },
        );
        
        // GPU benchmark
        group.bench_with_input(
            BenchmarkId::new("GPU", format!("{}ch_{}ev", channels, events)),
            &data,
            |b, d| {
                b.iter(|| black_box(gpu_ops.implementation(d)));
            },
        );
    }
    
    group.finish();
}
```

## Using criterion_group! Macro

### Simple Group Registration

```rust
criterion_group!(benches, bench_function1, bench_function2);
criterion_main!(benches);
```

### Conditional Compilation

```rust
#[cfg(not(feature = "gpu"))]
criterion_group!(benches, bench_cpu_only);

#[cfg(feature = "gpu")]
criterion_group!(
    benches,
    bench_cpu_only,
    bench_gpu_only,
    bench_comparison
);

criterion_main!(benches);
```

### Advanced Configuration

```rust
criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    targets = bench_function1, bench_function2
}
criterion_main!(benches);
```

## Throughput Metrics

Set throughput to help Criterion normalize results:

```rust
// For element-based operations
group.throughput(Throughput::Elements(num_elements as u64));

// For byte-based operations
group.throughput(Throughput::Bytes(num_bytes as u64));
```

## Important Rules

1. **Always use `black_box`**: Wrap the function call in `std::hint::black_box()` to prevent compiler optimizations
2. **Use benchmark groups**: For related benchmarks, use `c.benchmark_group()` instead of individual `c.bench_function()` calls
3. **Set throughput**: When benchmarking operations on data, set throughput to normalize results
4. **Use descriptive names**: Group names should describe the operation, benchmark IDs should describe the variant
5. **Handle errors**: If benchmarking can fail, use `.unwrap()` or handle errors appropriately
6. **Conditional compilation**: Use `#[cfg(feature = "...")]` for optional benchmarks

## Examples from Project

See `fcs/benches/matrix_operations.rs` for comprehensive examples including:
- Multiple benchmark functions
- Throughput configuration
- Comparison benchmarks (CPU vs GPU)
- Conditional compilation with `criterion_group!`
