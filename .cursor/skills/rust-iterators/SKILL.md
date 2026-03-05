---
name: rust-iterators
description: 'Use Rust iterators effectively, understand iterator adaptors, and write efficient iterator chains. Use when working with collections, transforming data, filtering, mapping, or collecting results. Handles iterator adaptors, lazy evaluation, collecting strategies, and performance considerations.'
---

# Rust Iterators

Guidelines for using Rust iterators effectively and efficiently.

## When to Use This Skill

- Working with collections
- Transforming data
- Filtering and mapping
- Collecting results
- Writing iterator chains
- Understanding lazy evaluation

## Basic Iterator Usage

### Creating Iterators

```rust
// From vector
let vec = vec![1, 2, 3];
let iter = vec.iter();  // Iterator over &T

// From mutable vector
let mut vec = vec![1, 2, 3];
let iter = vec.iter_mut();  // Iterator over &mut T

// Consuming iterator
let iter = vec.into_iter();  // Iterator over T (consumes vec)
```

### Basic Operations

```rust
// Iterate
for item in vec.iter() {
    println!("{}", item);
}

// Collect
let doubled: Vec<i32> = vec.iter().map(|x| x * 2).collect();

// Sum
let sum: i32 = vec.iter().sum();

// Count
let count = vec.iter().count();
```

## Iterator Adaptors

### Map

```rust
// Transform each element
let doubled: Vec<i32> = vec![1, 2, 3]
    .iter()
    .map(|x| x * 2)
    .collect();
// Result: [2, 4, 6]
```

### Filter

```rust
// Keep only elements matching predicate
let evens: Vec<i32> = vec![1, 2, 3, 4, 5]
    .iter()
    .filter(|x| x % 2 == 0)
    .collect();
// Result: [2, 4]
```

### Filter Map

```rust
// Filter and map in one step
let results: Vec<i32> = vec![Some(1), None, Some(3)]
    .iter()
    .filter_map(|x| *x)
    .collect();
// Result: [1, 3]
```

### Flat Map

```rust
// Flatten nested iterators
let nested = vec![vec![1, 2], vec![3, 4]];
let flat: Vec<i32> = nested
    .iter()
    .flat_map(|v| v.iter())
    .collect();
// Result: [1, 2, 3, 4]
```

### Take and Skip

```rust
// Take first n elements
let first_three: Vec<i32> = vec![1, 2, 3, 4, 5]
    .iter()
    .take(3)
    .collect();
// Result: [1, 2, 3]

// Skip first n elements
let rest: Vec<i32> = vec![1, 2, 3, 4, 5]
    .iter()
    .skip(2)
    .collect();
// Result: [3, 4, 5]
```

### Zip

```rust
// Combine two iterators
let a = vec![1, 2, 3];
let b = vec![4, 5, 6];
let zipped: Vec<(i32, i32)> = a.iter()
    .zip(b.iter())
    .map(|(x, y)| (*x, *y))
    .collect();
// Result: [(1, 4), (2, 5), (3, 6)]
```

### Enumerate

```rust
// Add index to iterator
let enumerated: Vec<(usize, i32)> = vec![10, 20, 30]
    .iter()
    .enumerate()
    .map(|(i, x)| (i, *x))
    .collect();
// Result: [(0, 10), (1, 20), (2, 30)]
```

### Chain

```rust
// Combine multiple iterators
let chained: Vec<i32> = vec![1, 2].iter()
    .chain(vec![3, 4].iter())
    .collect();
// Result: [1, 2, 3, 4]
```

## Collecting Strategies

### Vec

```rust
let vec: Vec<i32> = (0..10).collect();
```

### HashMap

```rust
use std::collections::HashMap;

let map: HashMap<i32, i32> = vec![(1, 2), (3, 4)]
    .into_iter()
    .collect();
```

### HashSet

```rust
use std::collections::HashSet;

let set: HashSet<i32> = vec![1, 2, 2, 3]
    .into_iter()
    .collect();
// Result: {1, 2, 3}
```

### With Capacity

```rust
// Pre-allocate capacity
let mut vec = Vec::with_capacity(100);
vec.extend((0..100).map(|x| x * 2));
```

## Lazy Evaluation

### Iterators Are Lazy

```rust
// Nothing happens until collected
let iterator = (0..1000)
    .map(|x| x * 2)
    .filter(|x| x % 3 == 0);

// Still nothing happens
let doubled = iterator.clone();

// Now it executes
let result: Vec<i32> = iterator.collect();
```

### Early Termination

```rust
// Stops when condition met
let result: Option<i32> = (0..100)
    .map(|x| x * 2)
    .find(|x| *x > 50);
// Stops at first value > 50
```

## Performance Considerations

### Avoid Unnecessary Collects

```rust
// ❌ Bad: Multiple collects
let doubled: Vec<i32> = vec.iter().map(|x| x * 2).collect();
let filtered: Vec<i32> = doubled.iter().filter(|x| *x > 10).collect();

// ✅ Good: Single chain
let result: Vec<i32> = vec.iter()
    .map(|x| x * 2)
    .filter(|x| *x > 10)
    .collect();
```

### Use Iterators Instead of Loops

```rust
// ❌ Bad: Manual loop
let mut result = Vec::new();
for item in &vec {
    if item % 2 == 0 {
        result.push(item * 2);
    }
}

// ✅ Good: Iterator
let result: Vec<i32> = vec.iter()
    .filter(|x| x % 2 == 0)
    .map(|x| x * 2)
    .collect();
```

### Pre-allocate When Size Known

```rust
// ✅ Good: Pre-allocate
let mut result = Vec::with_capacity(vec.len());
result.extend(vec.iter().map(|x| x * 2));
```

## Common Patterns

### Sum and Product

```rust
let sum: i32 = vec.iter().sum();
let product: i32 = vec.iter().product();
```

### Min and Max

```rust
let min = vec.iter().min();
let max = vec.iter().max();
```

### Any and All

```rust
let has_positive = vec.iter().any(|x| *x > 0);
let all_positive = vec.iter().all(|x| *x > 0);
```

### Fold

```rust
// Reduce iterator to single value
let sum = vec.iter().fold(0, |acc, x| acc + x);
```

### Reduce

```rust
// Like fold but uses first element as initial
let max = vec.iter().reduce(|acc, x| if *x > acc { *x } else { acc });
```

## Custom Iterators

### Implementing Iterator Trait

```rust
struct Counter {
    current: i32,
    max: i32,
}

impl Iterator for Counter {
    type Item = i32;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.max {
            let value = self.current;
            self.current += 1;
            Some(value)
        } else {
            None
        }
    }
}
```

### Using Custom Iterator

```rust
let counter = Counter { current: 0, max: 5 };
let values: Vec<i32> = counter.collect();
// Result: [0, 1, 2, 3, 4]
```

## Important Rules

1. **Iterators are lazy**: They don't execute until consumed
2. **Chain operations**: Combine multiple adaptors in one chain
3. **Avoid unnecessary collects**: Only collect when needed
4. **Use appropriate iterator**: `iter()`, `iter_mut()`, or `into_iter()`
5. **Pre-allocate when possible**: Use `with_capacity()` when size is known
6. **Prefer iterators over loops**: More idiomatic and often faster

## Examples from Project

Look for iterator usage in:
- Data processing pipelines
- Collection transformations
- Filtering and mapping operations
- Performance-critical sections

## Common Patterns

### ✅ Good

```rust
// Single chain, efficient
let result: Vec<i32> = data.iter()
    .filter(|x| x > 0)
    .map(|x| x * 2)
    .collect();

// Early termination
let found = data.iter().find(|x| *x > 100);
```

### ❌ Avoid

```rust
// Multiple collects
let step1: Vec<i32> = data.iter().map(|x| x * 2).collect();
let step2: Vec<i32> = step1.iter().filter(|x| *x > 10).collect();

// Manual loops when iterators work
for item in &data {
    // Prefer iterator methods
}
```
