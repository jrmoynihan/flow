---
name: rust-testing
description: 'Write Rust tests following project conventions. Use when writing unit tests, integration tests, test modules, or test helpers. Handles test organization, Result-based tests, test fixtures, and common testing patterns.'
---

# Rust Testing

Guidelines for writing Rust tests following this project's conventions.

## When to Use This Skill

- Writing unit tests
- Creating integration tests
- Organizing test modules
- Writing test helpers and fixtures
- Testing error conditions
- Testing with Result types

## Basic Test Structure

### Simple Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        let result = function_to_test();
        assert_eq!(result, expected_value);
    }
}
```

### Test with Result

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_returns_result() -> Result<(), ErrorType> {
        let result = function_that_returns_result()?;
        assert_eq!(result, expected_value);
        Ok(())
    }
}
```

## Common Patterns

### Testing Error Conditions

```rust
#[test]
fn test_error_condition() {
    let result = function_that_can_fail();
    assert!(result.is_err());
    
    // Or check specific error
    match result {
        Err(ErrorType::SpecificError) => {},
        _ => panic!("Expected SpecificError"),
    }
}
```

### Testing with Assertions

```rust
#[test]
fn test_with_assertions() {
    let value = function_to_test();
    
    assert_eq!(value, expected);           // Equality
    assert_ne!(value, unexpected);          // Inequality
    assert!(condition);                     // Boolean
    assert!(value > threshold, "Value {} too small", value);  // With message
}
```

### Testing Collections

```rust
#[test]
fn test_collection() {
    let vec = function_that_returns_vec();
    
    assert_eq!(vec.len(), expected_len);
    assert!(vec.contains(&expected_value));
    assert_eq!(vec[0], first_value);
}
```

### Testing with Setup

```rust
#[test]
fn test_with_setup() {
    // Setup test data
    let test_data = create_test_data();
    
    // Execute
    let result = function_to_test(&test_data);
    
    // Verify
    assert_eq!(result, expected);
}
```

### Parameterized Tests

```rust
#[test]
fn test_multiple_cases() {
    let test_cases = vec![
        (input1, expected1),
        (input2, expected2),
        (input3, expected3),
    ];
    
    for (input, expected) in test_cases {
        let result = function_to_test(input);
        assert_eq!(result, expected, "Failed for input: {:?}", input);
    }
}
```

## Test Organization

### Module-Level Tests

```rust
// In the same file as the code being tested
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_function() {
        // Test public API
    }
    
    #[test]
    fn test_internal_helper() {
        // Test internal helpers if needed
    }
}
```

### Integration Tests

Place in `tests/` directory at crate root:

```rust
// tests/integration_test.rs
use crate_name::*;

#[test]
fn test_integration() {
    // Test multiple components together
}
```

## Test Helpers and Fixtures

### Helper Functions

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_instance() -> TestType {
        // Setup helper
        TestType::new()
    }
    
    #[test]
    fn test_using_helper() {
        let instance = create_test_instance();
        // Use instance in test
    }
}
```

### Test Fixtures

```rust
struct TestFixture {
    data: Vec<f32>,
    config: Config,
}

impl TestFixture {
    fn new() -> Self {
        Self {
            data: vec![1.0, 2.0, 3.0],
            config: Config::default(),
        }
    }
}

#[test]
fn test_with_fixture() {
    let fixture = TestFixture::new();
    let result = function_to_test(&fixture.data, &fixture.config);
    assert_eq!(result, expected);
}
```

## Testing with Random Data

```rust
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn test_with_random_data() {
    let mut rng = StdRng::seed_from_u64(42);
    let test_data: Vec<f32> = (0..100)
        .map(|_| rng.gen_range(0.0..1000.0))
        .collect();
    
    let result = function_to_test(&test_data);
    assert!(result.is_ok());
}
```

## Testing Error Messages

```rust
#[test]
fn test_error_message() {
    let result = function_that_fails();
    
    assert!(result.is_err());
    let error_msg = format!("{}", result.unwrap_err());
    assert!(error_msg.contains("expected text"));
}
```

## Important Rules

1. **Use `#[cfg(test)]`**: Mark test modules with `#[cfg(test)]` to exclude from release builds
2. **Use `use super::*`**: Import parent module items in test modules
3. **Return Result**: Use `-> Result<(), ErrorType>` for tests that can fail
4. **Descriptive names**: Test function names should describe what they test
5. **One assertion per concept**: Group related assertions, but test one concept per test
6. **Use `?` operator**: In Result-returning tests, use `?` for cleaner error handling

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_function_name

# Run tests with output
cargo test -- --nocapture

# Run tests in specific file
cargo test --test integration_test
```

## Examples from Project

See test files in:
- `fcs/src/tests.rs`
- `peacoqc-rs/tests/` directory
- Individual `#[cfg(test)]` modules in source files
