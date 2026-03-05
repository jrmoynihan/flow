---
name: rust-black-box
description: 'Use std::hint::black_box correctly in benchmarks and tests. Use when writing benchmarks, preventing compiler optimizations, or ensuring code actually executes. Handles proper placement, when to use it, and common patterns for benchmarking.'
---

# Using std::hint::black_box

Guidelines for using `std::hint::black_box` in benchmarks and tests.

## When to Use This Skill

- Writing Criterion benchmarks
- Preventing compiler optimizations in benchmarks
- Ensuring benchmarked code actually executes
- Testing performance-sensitive code

## Basic Usage

### Required Import

```rust
use std::hint::black_box;
```

### In Benchmarks

Always wrap the function call being benchmarked:

```rust
use std::hint::black_box;

c.bench_function("my_bench", |b| {
    b.iter(|| black_box(function_to_benchmark()));
});
```

### With Input Data

When benchmarking with input:

```rust
use std::hint::black_box;

group.bench_with_input(
    BenchmarkId::new("bench_name", size),
    &input_data,
    |b, data| {
        b.iter(|| black_box(function_to_benchmark(data)));
    },
);
```

### With Multiple Arguments

```rust
use std::hint::black_box;

b.iter(|| {
    black_box(function_to_benchmark(
        black_box(arg1),
        black_box(arg2)
    ))
});
```

## Common Patterns

### Simple Function Call

```rust
use std::hint::black_box;

c.bench_function("simple", |b| {
    b.iter(|| black_box(2 + 2));
});
```

### Function with Return Value

```rust
use std::hint::black_box;

group.bench_with_input(
    BenchmarkId::new("operation", size),
    &input,
    |b, input| {
        b.iter(|| {
            let result = black_box(operation(input));
            black_box(result)  // Also black_box the result
        });
    },
);
```

### Method Calls

```rust
use std::hint::black_box;

b.iter(|| {
    black_box(instance.method(black_box(arg)));
});
```

### With Error Handling

```rust
use std::hint::black_box;

b.iter(|| {
    black_box(function_that_returns_result()).unwrap();
});
```

### Multiple Operations

```rust
use std::hint::black_box;

b.iter(|| {
    let result1 = black_box(operation1());
    let result2 = black_box(operation2(result1));
    black_box(result2)
});
```

## Important Rules

1. **Always use in benchmarks**: Wrap the function/operation being measured
2. **Wrap inputs**: Also wrap input arguments if they might be optimized away
3. **Wrap results**: Wrap return values to prevent optimization of unused results
4. **Don't overuse**: Only use where needed to prevent optimizations
5. **Import explicitly**: Use `use std::hint::black_box;` at the top of the file

## What black_box Does

`black_box` tells the compiler:
- "Don't optimize this value away"
- "Assume this value might be used later"
- "Don't eliminate this computation"

This ensures benchmarks measure actual execution time, not optimized-away code.

## Examples from Project

See `fcs/benches/matrix_operations.rs` for examples:

```rust
// Simple usage
b.iter(|| black_box(CpuFallback::invert_matrix(m)).unwrap());

// With multiple arguments
b.iter(|| black_box(CpuFallback::batch_matvec(*m, *d)).unwrap());

// GPU operations
b.iter(|| black_box(gpu_ops.batch_matvec(*m, *d)).unwrap());
```

## Anti-patterns to Avoid

❌ **Don't forget black_box**:
```rust
// BAD: Compiler might optimize this away
b.iter(|| function_to_benchmark());
```

✅ **Do use black_box**:
```rust
// GOOD: Ensures code actually runs
b.iter(|| black_box(function_to_benchmark()));
```

❌ **Don't black_box everything**:
```rust
// BAD: Unnecessary
let x = black_box(1);
let y = black_box(2);
let z = black_box(x + y);
```

✅ **Do black_box strategically**:
```rust
// GOOD: Only where needed
b.iter(|| black_box(1 + 2));
```
