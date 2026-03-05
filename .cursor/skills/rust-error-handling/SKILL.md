---
name: rust-error-handling
description: 'Handle Rust errors using Result types, thiserror, and error propagation. Use when writing functions that can fail, creating error types, propagating errors with ?, or using custom Result aliases. Handles thiserror derive macros, error conversion, and proper error context.'
---

# Rust Error Handling

Guidelines for error handling using `Result` types, `thiserror`, and error propagation.

## When to Use This Skill

- Writing functions that can fail
- Creating custom error types
- Propagating errors with `?` operator
- Using `thiserror` for error definitions
- Converting between error types
- Adding error context

## Basic Result Usage

### Function That Returns Result

```rust
use std::io;

fn function_that_can_fail() -> Result<String, io::Error> {
    // Operations that might fail
    let content = std::fs::read_to_string("file.txt")?;
    Ok(content)
}
```

### Using the ? Operator

```rust
fn process_data() -> Result<(), MyError> {
    let data = read_file()?;        // Propagate error if read fails
    let parsed = parse_data(data)?; // Propagate error if parse fails
    process(parsed)?;                 // Propagate error if process fails
    Ok(())
}
```

## Custom Error Types with thiserror

### Required Dependency

```toml
[dependencies]
thiserror = { workspace = true }
```

### Basic Error Enum

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Parse error: {message}")]
    ParseError { message: String },
}
```

### Error with Context

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcessingError {
    #[error("Failed to process {item}: {reason}")]
    ProcessingFailed {
        item: String,
        reason: String,
    },
    
    #[error("Insufficient data: need at least {min}, got {actual}")]
    InsufficientData {
        min: usize,
        actual: usize,
    },
}
```

### Automatic Error Conversion

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_json::Error),
    
    #[error("Custom error: {0}")]
    Custom(String),
}
```

## Custom Result Type Alias

### Define Result Alias

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Error: {0}")]
    Error(String),
}

pub type Result<T> = std::result::Result<T, MyError>;
```

### Use Custom Result

```rust
fn function() -> Result<String> {
    // Can return MyError directly
    Ok("success".to_string())
}
```

## Error Propagation Patterns

### Early Return on Error

```rust
fn process() -> Result<()> {
    let data = read_data()?;
    let validated = validate(data)?;
    let processed = process(validated)?;
    Ok(())
}
```

### Handling Specific Errors

```rust
fn process() -> Result<()> {
    match operation() {
        Ok(value) => Ok(value),
        Err(MyError::SpecificError) => {
            // Handle specific error
            Ok(default_value())
        },
        Err(e) => Err(e),  // Propagate other errors
    }
}
```

### Converting Errors

```rust
fn process() -> Result<()> {
    let result = external_function()
        .map_err(|e| MyError::Custom(format!("Failed: {}", e)))?;
    Ok(result)
}
```

## Common Patterns

### Error with Source

```rust
use thiserror::Error;
use std::error::Error as StdError;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Operation failed: {message}")]
    OperationFailed {
        message: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}
```

### Multiple Error Variants

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcessingError {
    #[error("Invalid channel: {0}")]
    InvalidChannel(String),
    
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),
    
    #[error("Insufficient data: need {min}, got {actual}")]
    InsufficientData { min: usize, actual: usize },
    
    #[error("External error: {0}")]
    ExternalError(#[from] polars::error::PolarsError),
}
```

### Error Display

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Error with value: {value}")]
    ErrorWithValue { value: i32 },
}

// Usage
let err = MyError::ErrorWithValue { value: 42 };
println!("{}", err);  // Prints: "Error with value: 42"
```

## Important Rules

1. **Use Result for fallible operations**: Functions that can fail should return `Result<T, E>`
2. **Use `?` for propagation**: Use `?` operator to propagate errors up the call stack
3. **Use thiserror for custom errors**: Derive `Error` trait with `thiserror::Error`
4. **Use `#[from]` for conversions**: Automatically convert from other error types
5. **Add context**: Include relevant information in error messages
6. **Define Result alias**: Create `pub type Result<T> = std::result::Result<T, MyError>` for convenience
7. **Use descriptive error messages**: Error messages should help debug the issue

## Examples from Project

See error definitions in:
- `peacoqc-rs/src/error.rs` - Uses `thiserror` with multiple variants
- `gates/src/error.rs` - Comprehensive error type with source tracking

## Error Handling Best Practices

### ✅ Good

```rust
#[derive(Error, Debug)]
pub enum MyError {
    #[error("Failed to process {item}: {reason}")]
    ProcessingFailed { item: String, reason: String },
}

fn process(item: &str) -> Result<()> {
    validate(item)?;
    Ok(())
}
```

### ❌ Avoid

```rust
// Don't use String as error type
fn process() -> Result<(), String> {
    Err("error".to_string())
}

// Don't ignore errors
fn process() {
    let _ = might_fail();  // BAD: Ignores error
}
```
