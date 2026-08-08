---
name: rust-cargo
description: 'Work with Cargo commands, workspace configuration, dependency management, and build systems. Use when managing dependencies, configuring workspaces, building projects, running tests, or publishing crates. Handles Cargo.toml configuration, workspace setup, dependency resolution, and build commands.'
---

# Cargo and Workspace Management

Guidelines for working with Cargo, workspaces, dependencies, and build configuration.

## When to Use This Skill

- Managing dependencies in `Cargo.toml`
- Configuring Rust workspaces
- Understanding dependency resolution
- Building and testing projects
- Publishing crates
- Working with feature flags
- Understanding workspace inheritance

## Workspace Configuration

### Basic Workspace

```toml
[workspace]
members = [
    "crate1",
    "crate2",
    "crate3",
]
resolver = "2"  # or "3" for newer editions
```

### Workspace Package Inheritance

```toml
[workspace]
members = ["crate1", "crate2"]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Author Name"]
license = "MIT"

# Individual crates inherit these values
[package]
name = "crate1"
# version, edition, authors, license inherited from workspace
```

### Workspace Dependencies

```toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }

# In member crates, use workspace = true
[dependencies]
serde = { workspace = true }
tokio = { workspace = true }
```

## Dependency Management

### Version Specifiers

```toml
[dependencies]
# Exact version
crate = "1.2.3"

# Compatible version (^1.2.3 = >=1.2.3, <2.0.0)
crate = "^1.2.3"

# Patch version (~1.2.3 = >=1.2.3, <1.3.0)
crate = "~1.2.3"

# Wildcard (any version)
crate = "*"

# Version range
crate = ">=1.2.0, <2.0.0"
```

### Dependency Sources

```toml
# From crates.io (default)
crate = "1.0"

# From git repository
crate = { git = "https://github.com/user/repo", branch = "main" }
crate = { git = "https://github.com/user/repo", tag = "v1.0.0" }
crate = { git = "https://github.com/user/repo", rev = "abc123" }

# From local path
crate = { path = "../local-crate" }

# From workspace
crate = { workspace = true }

# With features
crate = { version = "1.0", features = ["feature1", "feature2"] }

# Optional dependency
crate = { version = "1.0", optional = true }
```

### Dev Dependencies

```toml
[dev-dependencies]
# Only included when building tests/examples/benchmarks
criterion = "0.5"
tempfile = "3.0"
```

### Build Dependencies

```toml
[build-dependencies]
# Only included when building build.rs
cc = "1.0"
```

## Common Cargo Commands

### Building

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Build specific package
cargo build --package <package-name>

# Build with features
cargo build --features <feature-name>

# Build without default features
cargo build --no-default-features

# Build all workspace members
cargo build --workspace
```

### Testing

Tests run under [cargo-nextest](https://nexte.st), aliased to `cargo nt` in this
workspace. Doctests are the one exception — nextest cannot run them.

```bash
# Run all tests
cargo nextest run

# Run tests for specific package
cargo nextest run --package <package-name>

# Run specific test
cargo nextest run test_name

# Run tests with output
cargo nextest run --no-capture

# Run tests with features
cargo nextest run --features <feature-name>

# Doctests (built-in harness)
cargo test --doc
```

### Checking

```bash
# Check code without building
cargo check

# Check all targets
cargo check --all-targets

# Check with features
cargo check --features <feature-name>
```

### Formatting

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Format workspace
cargo fmt --workspace
```

### Clippy

```bash
# Run clippy
cargo clippy

# Run clippy with all targets
cargo clippy --all-targets --all-features

# Fix issues automatically
cargo clippy --fix --allow-dirty

# Deny warnings
cargo clippy -- -D warnings
```

### Documentation

```bash
# Generate documentation
cargo doc

# Open documentation in browser
cargo doc --open

# Generate docs for specific package
cargo doc --package <package-name>

# Generate docs without dependencies
cargo doc --no-deps

# Generate docs for workspace
cargo doc --workspace
```

### Dependency Management

```bash
# Update dependencies
cargo update

# Update specific dependency
cargo update -p <crate-name>

# Show dependency tree
cargo tree

# Show dependency tree for specific package
cargo tree --package <package-name>

# Show only direct dependencies
cargo tree --depth 1

# Show what depends on a crate
cargo tree --invert -p <crate-name>
```

## Workspace Metadata Scripts

```toml
[workspace.metadata.scripts]
build = "cargo build --workspace"
test = "cargo nextest run --workspace"
check = "cargo check --workspace"
clippy = "cargo clippy --workspace"
fmt = "cargo fmt --workspace"
```

## Package Metadata

```toml
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
authors = ["Author"]
license = "MIT"
description = "Description of the crate"
repository = "https://github.com/user/repo"
keywords = ["keyword1", "keyword2"]
categories = ["category1"]
readme = "README.md"
```

## Feature Flags

See `rust-features` skill for detailed feature flag usage.

## Important Rules

1. **Use workspace dependencies**: Share common dependencies via `[workspace.dependencies]`
2. **Inherit workspace values**: Use `workspace = true` for version, edition, authors, license
3. **Pin versions carefully**: Use `^` for compatible updates, exact versions sparingly
4. **Document features**: Clearly document what each feature enables
5. **Use resolver = "2" or "3"**: Specify resolver version for workspace
6. **Organize dependencies**: Group by purpose (core, error handling, serialization, etc.)

## Examples from Project

See `Cargo.toml` for workspace configuration:
- Workspace members
- Workspace package inheritance
- Workspace dependencies
- Workspace metadata scripts

See individual crate `Cargo.toml` files for:
- Package configuration
- Feature definitions
- Dependency usage with `workspace = true`

## Common Patterns

### ✅ Good

```toml
[workspace]
members = ["crate1", "crate2"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }

[package]
name = "crate1"
serde = { workspace = true }
```

### ❌ Avoid

```toml
# Don't duplicate versions
[package]
serde = "1.0"  # BAD: Should use workspace

# Don't use wildcard versions in production
[dependencies]
crate = "*"  # BAD: Unpredictable updates
```
