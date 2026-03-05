---
name: rust-unsafe
description: 'Work with unsafe Rust code safely, understand safety invariants, and review unsafe blocks. Use when writing unsafe code, reviewing unsafe blocks, understanding safety requirements, or working with FFI. Handles unsafe blocks, safety invariants, unsafe traits, and best practices for unsafe code.'
---

# Unsafe Rust

Guidelines for working with unsafe Rust code safely and understanding safety invariants.

## When to Use This Skill

- Writing unsafe code blocks
- Reviewing unsafe code for safety
- Understanding safety invariants
- Working with raw pointers
- Implementing unsafe traits
- Interfacing with FFI

## Unsafe Operations

### Unsafe Blocks

```rust
unsafe {
    // Unsafe operations here
    let raw_ptr = ptr::null_mut();
    *raw_ptr = 42;  // This is unsafe
}
```

### Unsafe Functions

```rust
// Function that can be called from safe code
// but contains unsafe operations
pub fn safe_wrapper() {
    unsafe {
        // Unsafe implementation
    }
}

// Function that is itself unsafe to call
pub unsafe fn unsafe_function(ptr: *mut i32) {
    *ptr = 42;  // Caller must ensure ptr is valid
}
```

### Unsafe Traits

```rust
unsafe trait UnsafeTrait {
    // Trait that requires unsafe to implement
}

unsafe impl UnsafeTrait for MyType {
    // Implementation must maintain safety invariants
}
```

## Safety Invariants

### Pointer Validity

```rust
unsafe fn dereference_ptr(ptr: *const i32) -> i32 {
    // Safety: Caller must ensure:
    // 1. ptr is not null
    // 2. ptr points to valid memory
    // 3. Memory is not deallocated during use
    *ptr
}

// Safe wrapper
pub fn safe_dereference(ptr: *const i32) -> Option<i32> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        Some(*ptr)
    }
}
```

### Memory Safety

```rust
unsafe fn write_to_ptr(ptr: *mut i32, value: i32) {
    // Safety: Caller must ensure:
    // 1. ptr is not null
    // 2. ptr points to valid, writable memory
    // 3. No other references exist to this memory
    *ptr = value;
}
```

### Thread Safety

```rust
unsafe impl Send for MyType {
    // Safety: MyType must be safe to send between threads
}

unsafe impl Sync for MyType {
    // Safety: MyType must be safe to share between threads
}
```

## Common Unsafe Patterns

### Raw Pointers

```rust
use std::ptr;

unsafe fn manipulate_pointers() {
    let mut value = 42;
    let ptr: *mut i32 = &mut value;
    
    // Safety: ptr points to valid memory (value)
    unsafe {
        *ptr = 100;
    }
    
    assert_eq!(value, 100);
}
```

### Calling Unsafe Functions

```rust
// From C FFI
extern "C" {
    fn c_function(ptr: *mut i32) -> i32;
}

pub fn safe_wrapper(value: &mut i32) -> Result<i32> {
    // Safety: value is a valid mutable reference
    unsafe {
        Ok(c_function(value as *mut i32))
    }
}
```

### Unsafe Traits

```rust
unsafe trait MarkerTrait {
    // Marker trait with safety requirements
}

unsafe impl MarkerTrait for MyType {
    // Safety: MyType meets the requirements of MarkerTrait
}
```

## Safety Documentation

### Documenting Safety Requirements

```rust
/// Performs unsafe operation.
///
/// # Safety
///
/// This function is unsafe because:
/// - `ptr` must be a valid, non-null pointer
/// - `ptr` must point to initialized memory
/// - The memory must not be deallocated during the call
/// - No other references to this memory may exist
pub unsafe fn unsafe_operation(ptr: *mut i32) {
    *ptr = 42;
}
```

### Safety Comments

```rust
unsafe {
    // Safety: We know this pointer is valid because:
    // 1. It comes from a valid reference
    // 2. The reference lifetime ensures validity
    // 3. No other code can mutate during this block
    let value = *raw_ptr;
}
```

## Best Practices

### Minimize Unsafe Surface Area

```rust
// ✅ Good: Small unsafe block, large safe interface
pub fn safe_function(input: &[i32]) -> Vec<i32> {
    let mut result = Vec::with_capacity(input.len());
    unsafe {
        // Small unsafe block for performance
        let ptr = result.as_mut_ptr();
        for (i, &value) in input.iter().enumerate() {
            ptr.add(i).write(value);
        }
        result.set_len(input.len());
    }
    result
}

// ❌ Bad: Large unsafe block
pub unsafe fn unsafe_function() {
    // Lots of unsafe code
    // Harder to verify safety
}
```

### Use Safe Abstractions

```rust
// ✅ Good: Wrap unsafe in safe API
pub struct SafeWrapper {
    inner: *mut CType,
}

impl SafeWrapper {
    pub fn new() -> Self {
        unsafe {
            Self {
                inner: c_create(),
            }
        }
    }
    
    pub fn get(&self) -> i32 {
        unsafe {
            c_get(self.inner)
        }
    }
}

impl Drop for SafeWrapper {
    fn drop(&mut self) {
        unsafe {
            c_destroy(self.inner);
        }
    }
}
```

### Validate Invariants

```rust
pub unsafe fn unsafe_operation(ptr: *mut i32, len: usize) {
    // Validate invariants before unsafe operations
    assert!(!ptr.is_null(), "Pointer must not be null");
    assert!(len > 0, "Length must be greater than 0");
    
    // Now safe to use
    for i in 0..len {
        *ptr.add(i) = 0;
    }
}
```

## Important Rules

1. **Document safety requirements**: Always document what callers must ensure
2. **Minimize unsafe scope**: Keep unsafe blocks as small as possible
3. **Validate invariants**: Check safety conditions before unsafe operations
4. **Use safe abstractions**: Wrap unsafe code in safe APIs
5. **Review carefully**: Unsafe code requires extra scrutiny
6. **Test thoroughly**: Unsafe code needs comprehensive testing

## Common Unsafe Operations

### Dereferencing Raw Pointers

```rust
unsafe {
    let value = *raw_ptr;  // Must ensure ptr is valid
}
```

### Calling Unsafe Functions

```rust
unsafe {
    unsafe_function();  // Must ensure function's safety requirements
}
```

### Accessing Static Mutables

```rust
static mut COUNTER: i32 = 0;

unsafe {
    COUNTER += 1;  // Must ensure no data races
}
```

### Implementing Unsafe Traits

```rust
unsafe impl Send for MyType {
    // Must ensure MyType is safe to send
}
```

## Examples from Project

Look for unsafe code in:
- FFI bindings
- Performance-critical sections
- Low-level memory operations
- GPU operations (if using GPU features)

## Safety Checklist

When writing unsafe code, ensure:
- [ ] All safety requirements are documented
- [ ] Invariants are validated before unsafe operations
- [ ] Unsafe block is as small as possible
- [ ] Code is wrapped in safe API when possible
- [ ] Tests cover edge cases and error conditions
- [ ] Code is reviewed by someone familiar with unsafe Rust
