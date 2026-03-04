---
name: rust-crate-docs
description: 'Read and understand Rust crate documentation from crates.io or local crates. Use when needing to understand how to use a crate, find API documentation, check version compatibility, or understand crate features. Handles cargo doc, docs.rs, local documentation, and dependency information.'
---

# Reading Rust Crate Documentation

Guidelines for accessing and understanding Rust crate documentation from various sources.

## When to Use This Skill

- Understanding how to use a crate's API
- Finding documentation for a dependency
- Checking version compatibility
- Understanding crate features
- Looking up function signatures and examples
- Checking for breaking changes between versions

## Methods for Accessing Documentation

### 1. Generate Local Documentation

Build documentation for all dependencies:

```bash
# Generate docs for all dependencies
cargo doc --open

# Generate docs for specific crate
cargo doc --package <crate-name> --open

# Generate docs without opening browser
cargo doc --no-deps

# Generate docs for workspace crates only
cargo doc --workspace
```

The documentation will be available at `target/doc/<crate-name>/index.html`.

### 2. Online Documentation (docs.rs)

For published crates, documentation is available at:
- `https://docs.rs/<crate-name>/<version>/`
- `https://docs.rs/<crate-name>/` (latest version)

Example:
- `https://docs.rs/serde/1.0/`
- `https://docs.rs/polars/`

### 3. Crate Information

Get information about dependencies:

```bash
# List all dependencies
cargo tree

# Show dependency tree for specific crate
cargo tree --package <crate-name>

# Show dependency information
cargo metadata --format-version 1

# Check what version is being used
cargo tree --package <crate-name> --depth 0
```

### 4. Read Local Crate Documentation

For workspace crates or local dependencies:

```bash
# Generate docs for workspace crate
cargo doc --package <workspace-crate> --open

# Read from target/doc after building
# Files are at: target/doc/<crate-name>/index.html
```

## Reading Documentation Effectively

### Understanding API Structure

1. **Start with the crate root** - Check `index.html` or main module docs
2. **Look for examples** - Many crates have examples in their docs
3. **Check feature flags** - Understand what features enable what functionality
4. **Read trait documentation** - Traits often define the main API patterns

### Finding Specific Information

- **Functions**: Look in module documentation or search the docs
- **Types**: Check struct/enum documentation
- **Traits**: Look for trait definitions and implementations
- **Examples**: Often in `examples/` section or inline in docs

### Common Documentation Patterns

```rust
// Crate-level docs often show:
//! # Crate Name
//! 
//! Brief description
//! 
//! ## Features
//! 
//! - `feature1` - Description
//! - `feature2` - Description
//! 
//! ## Examples
//! 
//! ```rust
//! use crate_name::Type;
//! ```

// Module docs show organization:
/// Module for handling X
/// 
/// This module provides...

// Function docs show usage:
/// Does something
/// 
/// # Examples
/// 
/// ```rust
/// use crate::function;
/// function();
/// ```
```

## Checking Version Compatibility

### From Cargo.toml

```toml
[dependencies]
crate-name = "1.0"  # Exact version
crate-name = "^1.0" # Compatible version (1.0.0 to <2.0.0)
crate-name = "~1.0" # Patch version (1.0.0 to <1.1.0)
crate-name = "1.0.0" # Exact version
```

### Check Available Versions

```bash
# Search for crate versions
cargo search <crate-name>

# Check what's available on crates.io
# Visit: https://crates.io/crates/<crate-name>
```

## Understanding Feature Flags

Many crates use feature flags to enable optional functionality:

```toml
[dependencies]
crate-name = { version = "1.0", features = ["feature1", "feature2"] }
```

Check the crate's `Cargo.toml` or documentation for available features:
- Often documented in crate-level docs
- May be in a `FEATURES.md` file
- Check `Cargo.toml` in the crate source

## Reading Documentation in Code

When reading code that uses a crate:

1. **Check imports** - See what's being imported
2. **Follow the types** - Understand the type signatures
3. **Look for examples** - In the crate's examples directory
4. **Check tests** - Tests often show usage patterns

## Important Rules

1. **Use local docs first** - `cargo doc --open` is fastest for dependencies
2. **Check version** - Make sure you're reading docs for the correct version
3. **Read examples** - Examples show real usage patterns
4. **Check features** - Some APIs require specific features to be enabled
5. **Understand traits** - Many Rust APIs are trait-based

## Examples from Project

For this workspace, documentation is available for:

- `flow-fcs`: `cargo doc --package fcs --open`
- `peacoqc-rs`: `cargo doc --package peacoqc-rs --open`
- `plots`: `cargo doc --package plots --open`

## Troubleshooting

### Docs Not Generating

```bash
# Clean and rebuild
cargo clean
cargo doc

# Check if crate is in dependencies
cargo tree --package <crate-name>
```

### Wrong Version in Docs

```bash
# Regenerate docs
cargo doc --package <crate-name> --open

# Check what version is actually used
cargo tree --package <crate-name> --depth 0
```

### Missing Features

If documentation shows features you don't have access to:
- Check `Cargo.toml` for feature flags
- Enable features: `cargo doc --features <feature> --open`
- Check crate's feature documentation
