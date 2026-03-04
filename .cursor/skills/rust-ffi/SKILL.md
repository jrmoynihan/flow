---
name: rust-ffi
description: 'Work with Foreign Function Interface (FFI) in Rust, including C interop, binding generation, and safety in FFI. Use when interfacing with C libraries, creating Rust bindings, calling C functions, or ensuring FFI safety. Handles extern blocks, C types, binding generation, and FFI best practices.'
---

# Rust FFI (Foreign Function Interface)

Guidelines for working with FFI in Rust, including C interop and binding generation.

## When to Use This Skill

- Interfacing with C libraries
- Creating Rust bindings for C code
- Calling C functions from Rust
- Exposing Rust functions to C
- Ensuring FFI safety
- Working with C types

## Basic FFI

### Calling C Functions

```rust
// Declare external C function
extern "C" {
    fn c_function(arg: i32) -> i32;
}

// Call C function
unsafe {
    let result = c_function(42);
}
```

### Exposing Rust Functions to C

```rust
// Export Rust function for C
#[no_mangle]
pub extern "C" fn rust_function(arg: i32) -> i32 {
    arg * 2
}
```

## C Types

### Basic C Types

```rust
use std::os::raw::{c_int, c_void, c_char, c_double};

extern "C" {
    fn c_function(
        int_arg: c_int,
        double_arg: c_double,
        char_ptr: *const c_char,
    ) -> c_int;
}
```

### Common Type Mappings

- `int` → `c_int`
- `void*` → `*mut c_void`
- `char*` → `*const c_char` or `*mut c_char`
- `double` → `c_double`
- `float` → `c_float`
- `size_t` → `usize`

## Safety in FFI

### Unsafe Blocks

```rust
// All FFI calls are unsafe
extern "C" {
    fn c_function(ptr: *mut i32);
}

pub fn safe_wrapper(value: &mut i32) {
    unsafe {
        // Safety: value is a valid mutable reference
        c_function(value as *mut i32);
    }
}
```

### Null Pointer Checks

```rust
extern "C" {
    fn c_function(ptr: *const i32) -> i32;
}

pub fn safe_wrapper(ptr: *const i32) -> Option<i32> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        Some(c_function(ptr))
    }
}
```

### Lifetime Management

```rust
// Ensure C doesn't outlive Rust data
pub struct CWrapper<'a> {
    data: &'a [i32],
    c_ptr: *const i32,
}

impl<'a> CWrapper<'a> {
    pub fn new(data: &'a [i32]) -> Self {
        Self {
            data,
            c_ptr: data.as_ptr(),
        }
    }
    
    pub fn call_c_function(&self) -> i32 {
        unsafe {
            c_function(self.c_ptr)
        }
    }
}
```

## Binding Generation

### Using bindgen

```bash
# Install bindgen
cargo install bindgen-cli

# Generate bindings
bindgen wrapper.h -o bindings.rs
```

### bindgen Configuration

```rust
// build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
```

### Using cbindgen

```bash
# Install cbindgen
cargo install cbindgen

# Generate C headers from Rust
cbindgen --config cbindgen.toml --crate my_crate --output my_crate.h
```

## Common Patterns

### String Handling

```rust
use std::ffi::{CString, CStr};
use std::os::raw::c_char;

// Rust string to C string
pub fn rust_to_c(s: &str) -> CString {
    CString::new(s).expect("CString::new failed")
}

// C string to Rust string
pub unsafe fn c_to_rust(s: *const c_char) -> String {
    let c_str = CStr::from_ptr(s);
    c_str.to_str().expect("Invalid UTF-8").to_string()
}
```

### Callbacks

```rust
type Callback = extern "C" fn(i32) -> i32;

extern "C" {
    fn c_function_with_callback(cb: Callback) -> i32;
}

pub fn call_with_callback() {
    extern "C" fn callback(value: i32) -> i32 {
        value * 2
    }
    
    unsafe {
        c_function_with_callback(callback);
    }
}
```

### Structs

```rust
#[repr(C)]
pub struct CStruct {
    pub field1: i32,
    pub field2: f64,
}

extern "C" {
    fn c_function(s: CStruct) -> CStruct;
}
```

## Error Handling

### Converting C Errors

```rust
extern "C" {
    fn c_function() -> c_int;  // Returns 0 on success, non-zero on error
}

pub fn safe_wrapper() -> Result<(), Error> {
    let result = unsafe { c_function() };
    if result == 0 {
        Ok(())
    } else {
        Err(Error::CFunctionFailed(result))
    }
}
```

## Important Rules

1. **All FFI is unsafe**: Wrap in safe APIs when possible
2. **Check null pointers**: Always validate pointers before use
3. **Manage lifetimes**: Ensure C doesn't outlive Rust data
4. **Use proper types**: Use C types, not Rust types
5. **Document safety**: Document what callers must ensure
6. **Test thoroughly**: FFI code needs extensive testing

## Safety Checklist

- [ ] All FFI calls in unsafe blocks
- [ ] Null pointer checks where needed
- [ ] Lifetime management for borrowed data
- [ ] Proper C type usage
- [ ] Error handling for C errors
- [ ] Safe wrappers for unsafe FFI
- [ ] Documentation of safety requirements

## Common Patterns

### ✅ Good

```rust
// Safe wrapper around unsafe FFI
pub fn safe_c_function(value: &mut i32) -> Result<(), Error> {
    if value.is_null() {
        return Err(Error::NullPointer);
    }
    unsafe {
        let result = c_function(value as *mut i32);
        if result != 0 {
            Err(Error::CFunctionFailed(result))
        } else {
            Ok(())
        }
    }
}
```

### ❌ Avoid

```rust
// Unsafe exposed directly
pub unsafe fn unsafe_c_function(ptr: *mut i32) {
    c_function(ptr);  // BAD: No validation, no safety checks
}

// No null checks
pub fn bad_wrapper(ptr: *const i32) -> i32 {
    unsafe {
        c_function(ptr)  // BAD: Could be null
    }
}
```

## Examples from Project

Look for FFI usage in:
- C library bindings
- System calls
- Native library integration
- GPU operations (if using C libraries)
