---
name: rust-rand
description: 'Use rand crate correctly for generating random numbers. Use when creating random data, using Rng::gen_range, seeding RNGs, or generating test data. Handles proper range syntax, seeding with SeedableRng, choosing appropriate RNG types, and generating random values for different types.'
---

# Rust Random Number Generation

Guidelines for using the `rand` crate correctly in this project.

## When to Use This Skill

- Generating random test data
- Creating random values for benchmarks
- Seeding random number generators
- Using `Rng::gen_range` with proper range syntax
- Choosing appropriate RNG types

## Basic Usage

### Required Imports

```rust
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
```

### Creating a Seeded RNG

Always seed RNGs for reproducible results in tests and benchmarks:

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;

let mut rng = StdRng::seed_from_u64(42);
```

### Using gen_range

The `gen_range` method uses range syntax (`..` or `..=`):

```rust
use rand::Rng;

let mut rng = StdRng::seed_from_u64(42);

// Integer ranges (exclusive end)
let value: i32 = rng.gen_range(0..100);        // 0 to 99
let value: u32 = rng.gen_range(0..=100);       // 0 to 100 (inclusive)

// Float ranges (exclusive end)
let value: f32 = rng.gen_range(0.0..1.0);      // 0.0 to 1.0 (exclusive)
let value: f64 = rng.gen_range(-1.0..1.0);     // -1.0 to 1.0 (exclusive)

// Note: Float ranges with ..= are not supported, use exclusive ranges
```

## Common Patterns

### Generating Random Matrices

```rust
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use ndarray::Array2;

fn generate_random_matrix(n: usize) -> Array2<f32> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut matrix = Array2::<f32>::zeros((n, n));
    
    for i in 0..n {
        for j in 0..n {
            if i == j {
                // Diagonal: make it dominant
                matrix[[i, j]] = 1.0 + rng.gen_range(0.0..0.1);
            } else {
                // Off-diagonal: small values
                matrix[[i, j]] = rng.gen_range(-0.1..0.1);
            }
        }
    }
    
    matrix
}
```

### Generating Random Vectors

```rust
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn generate_random_vector(n: usize) -> Vec<f32> {
    let mut rng = StdRng::seed_from_u64(123);
    (0..n).map(|_| rng.gen_range(0.0..1000.0)).collect()
}
```

### Generating Random Channel Data

```rust
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn generate_channel_data(n_channels: usize, n_events: usize) -> Vec<Vec<f32>> {
    let mut rng = StdRng::seed_from_u64(123);
    let mut data = Vec::with_capacity(n_channels);
    
    for _ in 0..n_channels {
        let channel: Vec<f32> = (0..n_events)
            .map(|_| rng.gen_range(0.0..1000.0))
            .collect();
        data.push(channel);
    }
    
    data
}
```

## RNG Types

### StdRng (Recommended for Tests/Benchmarks)

- Fast and good quality
- Seedable for reproducibility
- Use `StdRng::seed_from_u64(seed)` to create

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;

let mut rng = StdRng::seed_from_u64(42);
```

### ThreadRng (For Production)

- Thread-local, automatically seeded
- Use `rand::thread_rng()` to get
- Not seedable (for security)

```rust
use rand::Rng;

let mut rng = rand::thread_rng();
let value = rng.gen_range(0..100);
```

## Important Rules

1. **Always seed in tests/benchmarks**: Use `StdRng::seed_from_u64()` for reproducible results
2. **Use proper range syntax**: 
   - `..` for exclusive end (most common)
   - `..=` for inclusive end (integers only)
   - Float ranges must use `..` (exclusive)
3. **Import Rng trait**: Must `use rand::Rng;` to call `gen_range`
4. **Type inference**: Rust can infer numeric types from the range, but be explicit if needed
5. **Reuse RNG**: Create one RNG and reuse it rather than creating new ones

## Range Syntax Reference

| Syntax | Type | Range | Example |
|--------|------|-------|---------|
| `a..b` | Integer | `[a, b)` exclusive | `0..100` → 0 to 99 |
| `a..=b` | Integer | `[a, b]` inclusive | `0..=100` → 0 to 100 |
| `a..b` | Float | `[a, b)` exclusive | `0.0..1.0` → 0.0 to 1.0 (exclusive) |
| `..b` | Any | Start to `b` | `..100` → up to 99 |
| `a..` | Any | `a` to end | `10..` → 10 onwards |

## Examples from Project

See `fcs/benches/matrix_operations.rs` for examples of:
- Seeded RNG usage
- Generating random matrices
- Generating random channel data
- Using `gen_range` with float ranges
