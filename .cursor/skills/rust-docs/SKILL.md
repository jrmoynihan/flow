---
name: rust-docs
description: 'Write Rust documentation comments following project conventions. Use when writing doc comments, documenting functions, structs, modules, or creating crate-level documentation. Handles /// for items, //! for crate/module docs, markdown formatting, code examples, and panic/safety sections.'
---

# Rust Documentation Comments

Guidelines for writing Rust documentation comments that match this project's conventions.

## When to Use This Skill

- Writing documentation for public functions, structs, enums, traits, or modules
- Creating crate-level documentation (`//!`)
- Adding code examples to documentation
- Documenting safety requirements or panic conditions
- Writing module-level documentation

## Documentation Comment Types

### Item Documentation (`///`)

Use `///` for documenting public items (functions, structs, enums, traits, etc.):

```rust
/// Brief one-line description.
///
/// Optional longer description explaining what the function does,
/// its purpose, and important details.
///
/// # Arguments
///
/// * `param1` - Description of parameter 1
/// * `param2` - Description of parameter 2
///
/// # Returns
///
/// Description of return value, including error conditions.
///
/// # Errors
///
/// When this function will return an error.
///
/// # Panics
///
/// When this function might panic.
///
/// # Safety
///
/// Safety requirements if this is an unsafe function.
///
/// # Examples
///
/// ```rust
/// use crate::module::function;
///
/// let result = function(arg1, arg2)?;
/// ```
pub fn function(param1: Type1, param2: Type2) -> Result<ReturnType, Error> {
    // implementation
}
```

### Crate/Module Documentation (`//!`)

Use `//!` for crate-level or module-level documentation at the top of the file:

```rust
//! Brief description of the module/crate
//!
//! Longer description explaining the purpose, design decisions,
//! and important notes about the module.
//!
//! # Key Features
//!
//! - Feature 1
//! - Feature 2
//!
//! # Examples
//!
//! ```rust
//! use crate::module;
//! ```
```

## Project Conventions

1. **Start with a brief summary**: First line should be a concise description
2. **Use markdown**: Format with headers, lists, code blocks, and emphasis
3. **Include examples**: Add `# Examples` section with runnable code when helpful
4. **Document errors**: Use `# Errors` section for `Result`-returning functions
5. **Note important details**: Use `**Note**` or `**Warning**` for important information
6. **Code examples**: Use `no_run` when examples can't compile standalone: ` ```rust,no_run`

## Common Patterns

### Documenting Structs

```rust
/// Description of what this struct represents.
///
/// Additional details about the struct's purpose, invariants,
/// or usage patterns.
///
/// # Examples
///
/// ```rust
/// let instance = StructName::new();
/// ```
pub struct StructName {
    /// Description of this field's purpose
    pub field1: Type1,
}
```

### Documenting Enums

```rust
/// Description of what this enum represents.
///
/// # Variants
///
/// - `Variant1` - Description of variant 1
/// - `Variant2` - Description of variant 2
pub enum EnumName {
    /// Description of this variant
    Variant1,
    /// Description of this variant
    Variant2,
}
```

### Documenting Modules

```rust
//! Module description
//!
//! Explains the module's purpose and key concepts.
//!
//! **Note**: Important information about the module.
```

## Markdown Formatting

- Use `**bold**` for emphasis
- Use `*italic*` for subtle emphasis
- Use `` `code` `` for inline code
- Use `# Section` for major sections
- Use `## Subsection` for subsections
- Use `- Item` for bullet lists
- Use `1. Item` for numbered lists

## Examples from Project

See `fcs/src/gpu/matrix.rs` and `fcs/src/lib.rs` for examples of module-level documentation.
