---
name: rust-clippy
description: 'Use Clippy lints effectively, understand common warnings, and apply appropriate fixes. Use when running clippy, fixing lint warnings, understanding when to allow/deny lints, or improving code quality. Handles common lints, performance suggestions, style improvements, and lint configuration.'
---

# Clippy Linting

Guidelines for using Clippy effectively to improve Rust code quality.

## When to Use This Skill

- Running `cargo clippy` and fixing warnings
- Understanding what Clippy warnings mean
- Deciding when to allow/deny specific lints
- Improving code quality and performance
- Configuring Clippy for a project

## Running Clippy

### Basic Commands

```bash
# Run clippy
cargo clippy

# Run with all targets and features
cargo clippy --all-targets --all-features

# Fix issues automatically where possible
cargo clippy --fix --allow-dirty

# Deny warnings (treat as errors)
cargo clippy -- -D warnings

# Allow specific lint
cargo clippy -- -A clippy::lint_name

# Deny specific lint
cargo clippy -- -D clippy::lint_name
```

## Common Clippy Lints

### Performance Lints

#### `needless_collect`

```rust
// ❌ Bad: Unnecessary collect
let items: Vec<_> = iterator.collect();
for item in items.iter() {
    process(item);
}

// ✅ Good: Use iterator directly
for item in iterator {
    process(item);
}
```

#### `unnecessary_to_owned`

```rust
// ❌ Bad: Unnecessary clone
let owned = string.to_owned();

// ✅ Good: Use reference or clone only when needed
let borrowed = &string;
```

#### `redundant_clone`

```rust
// ❌ Bad: Redundant clone
let cloned = vec.clone();
process(cloned);

// ✅ Good: Use reference
process(&vec);
```

### Style Lints

#### `single_char_pattern`

```rust
// ❌ Bad: Single character string
if s.contains("x") { }

// ✅ Good: Use char
if s.contains('x') { }
```

#### `len_zero`

```rust
// ❌ Bad: Using len() == 0
if vec.len() == 0 { }

// ✅ Good: Use is_empty()
if vec.is_empty() { }
```

#### `bool_comparison`

```rust
// ❌ Bad: Comparing bool to true/false
if condition == true { }

// ✅ Good: Use bool directly
if condition { }
```

### Correctness Lints

#### `unwrap_used`

```rust
// ❌ Bad: Using unwrap()
let value = option.unwrap();

// ✅ Good: Handle properly
let value = option.ok_or(Error::MissingValue)?;
```

#### `expect_used`

```rust
// ❌ Bad: Using expect() without context
let value = option.expect("failed");

// ✅ Good: Provide meaningful message or handle error
let value = option.expect("Failed to get value: context");
```

#### `panic`

```rust
// ❌ Bad: Unnecessary panic
if condition {
    panic!("error");
}

// ✅ Good: Return error
if condition {
    return Err(Error::InvalidInput);
}
```

### Complexity Lints

#### `too_many_arguments`

```rust
// ❌ Bad: Too many arguments
fn process(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) { }

// ✅ Good: Use struct
struct Params {
    a: i32,
    b: i32,
    c: i32,
    d: i32,
    e: i32,
    f: i32,
}
fn process(params: Params) { }
```

#### `cognitive_complexity`

```rust
// ❌ Bad: Too complex
fn complex() {
    if a {
        if b {
            if c {
                if d {
                    // Too nested
                }
            }
        }
    }
}

// ✅ Good: Extract functions
fn complex() {
    if a && b && c && d {
        handle_case();
    }
}
```

## Allowing/Denying Lints

### In Code

```rust
// Allow specific lint for this item
#[allow(clippy::lint_name)]
fn function() {
    // Code that triggers lint
}

// Deny lint for this scope
#[deny(clippy::lint_name)]
mod module {
    // All code in module must not trigger lint
}
```

### In Configuration

```toml
# In Cargo.toml or clippy.toml
[lints.clippy]
unwrap_used = "deny"
expect_used = "warn"
```

### In Command Line

```bash
# Allow specific lint
cargo clippy -- -A clippy::unwrap_used

# Deny specific lint
cargo clippy -- -D clippy::unwrap_used
```

## Clippy Configuration

### clippy.toml

```toml
# Allow specific lints
avoid-breaking-exported-api = false

# Set complexity limits
too-many-arguments-threshold = 10
```

### In Cargo.toml

```toml
[lints.clippy]
# Treat warnings as errors
unwrap_used = "deny"
expect_used = "deny"
```

## Important Rules

1. **Fix warnings when possible**: Don't suppress without good reason
2. **Use meaningful allow comments**: Explain why lint is allowed
3. **Consider performance lints**: They often improve code efficiency
4. **Don't ignore correctness lints**: They catch real bugs
5. **Use `--fix` carefully**: Review auto-fixes before committing
6. **Configure project-wide**: Set lint levels in `Cargo.toml` or `clippy.toml`

## Common Patterns

### ✅ Good

```rust
// Use iterator methods
let sum: i32 = items.iter().sum();

// Use is_empty()
if vec.is_empty() { }

// Handle errors properly
let value = option.ok_or(Error::Missing)?;
```

### ❌ Avoid

```rust
// Don't collect unnecessarily
let vec: Vec<_> = iterator.collect();
for item in vec.iter() { }

// Don't use unwrap() without good reason
let value = option.unwrap();

// Don't compare bools
if condition == true { }
```

## Examples from Project

Run `cargo clippy --all-targets --all-features` to see:
- Performance suggestions
- Style improvements
- Correctness warnings
- Complexity issues

## When to Allow Lints

- **Performance**: When the "better" way is actually slower
- **Readability**: When the lint makes code less readable
- **API compatibility**: When changing would break public API
- **Test code**: Some lints are less important in tests
- **Unsafe code**: Some patterns are necessary in unsafe blocks
