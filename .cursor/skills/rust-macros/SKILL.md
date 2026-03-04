---
name: rust-macros
description: 'Write and understand Rust macros, including declarative macros (macro_rules!) and procedural macros. Use when writing macros, understanding macro expansion, creating derive macros, or working with macro hygiene. Handles macro_rules!, procedural macros, derive macros, and macro best practices.'
---

# Rust Macros

Guidelines for writing and understanding Rust macros.

## When to Use This Skill

- Writing declarative macros (`macro_rules!`)
- Creating procedural macros
- Writing derive macros
- Understanding macro expansion
- Working with macro hygiene
- Debugging macro issues

## Declarative Macros (macro_rules!)

### Basic Macro

```rust
// Simple macro
macro_rules! say_hello {
    () => {
        println!("Hello!");
    };
}

// Usage
say_hello!();
```

### Macro with Parameters

```rust
// Macro with parameters
macro_rules! create_function {
    ($func_name:ident) => {
        fn $func_name() {
            println!("Function {} called", stringify!($func_name));
        }
    };
}

// Usage
create_function!(my_function);
my_function();  // Prints: "Function my_function called"
```

### Macro Patterns

```rust
// Match different patterns
macro_rules! test {
    // Match no arguments
    () => {
        println!("No arguments");
    };
    // Match one argument
    ($x:expr) => {
        println!("Expression: {}", $x);
    };
    // Match two arguments
    ($x:expr, $y:expr) => {
        println!("Expressions: {}, {}", $x, $y);
    };
}
```

## Fragment Specifiers

### Common Specifiers

- `ident`: Identifier (variable name, function name)
- `expr`: Expression
- `ty`: Type
- `pat`: Pattern
- `stmt`: Statement
- `block`: Block of code
- `item`: Item (function, struct, etc.)
- `meta`: Meta item (attribute)
- `tt`: Token tree

### Examples

```rust
// Using different specifiers
macro_rules! example {
    // Match identifier
    ($name:ident) => {
        let $name = 42;
    };
    // Match expression
    ($expr:expr) => {
        println!("{}", $expr);
    };
    // Match type
    ($ty:ty) => {
        let value: $ty = Default::default();
    };
}
```

## Repetition

### Repetition Patterns

```rust
// Match zero or more
macro_rules! vec {
    ($($x:expr),*) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($x);
            )*
            temp_vec
        }
    };
}

// Usage
let v = vec![1, 2, 3];
```

### Repetition Modifiers

- `*`: Zero or more
- `+`: One or more
- `?`: Zero or one

### Examples

```rust
// Zero or more
macro_rules! example {
    ($($x:expr),*) => {
        // Handle zero or more expressions
    };
}

// One or more
macro_rules! example {
    ($($x:expr),+) => {
        // Handle one or more expressions
    };
}

// With separator
macro_rules! example {
    ($($x:expr),+ $(,)?) => {
        // Handle with optional trailing comma
    };
}
```

## Procedural Macros

### Function-like Macros

```rust
// In Cargo.toml
[lib]
proc-macro = true

// In lib.rs
use proc_macro::TokenStream;

#[proc_macro]
pub fn my_macro(input: TokenStream) -> TokenStream {
    // Process input and return TokenStream
    input
}
```

### Attribute Macros

```rust
#[proc_macro_attribute]
pub fn my_attribute(attr: TokenStream, item: TokenStream) -> TokenStream {
    // attr: attribute arguments
    // item: the item the attribute is on
    item
}

// Usage
#[my_attribute]
fn my_function() {
    // ...
}
```

### Derive Macros

```rust
#[proc_macro_derive(MyDerive)]
pub fn my_derive(input: TokenStream) -> TokenStream {
    // Generate code for the derive
    TokenStream::new()
}

// Usage
#[derive(MyDerive)]
struct MyStruct {
    field: i32,
}
```

## Macro Hygiene

### Hygiene Basics

```rust
// Macros have hygienic identifiers
macro_rules! example {
    () => {
        let x = 42;
    };
}

fn test() {
    let x = 10;
    example!();  // Doesn't conflict with outer x
    println!("{}", x);  // Still 10
}
```

### Breaking Hygiene

```rust
// Use $crate for crate-relative paths
macro_rules! example {
    () => {
        $crate::some_function();
    };
}
```

## Common Macro Patterns

### Debug Macro

```rust
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            println!($($arg)*);
        }
    };
}
```

### Assert with Message

```rust
macro_rules! assert_eq_with_msg {
    ($left:expr, $right:expr, $msg:expr) => {
        if $left != $right {
            panic!("{}: left = {:?}, right = {:?}", $msg, $left, $right);
        }
    };
}
```

### Builder Pattern

```rust
macro_rules! builder {
    ($name:ident { $($field:ident: $ty:ty),* }) => {
        struct $name {
            $($field: $ty),*
        }
        
        impl $name {
            fn new() -> Self {
                Self {
                    $($field: Default::default()),*
                }
            }
        }
    };
}
```

## Important Rules

1. **Use macros sparingly**: Prefer functions when possible
2. **Document macro behavior**: Macros can be hard to understand
3. **Test macros thoroughly**: Macro expansion can be tricky
4. **Use hygiene correctly**: Understand identifier scoping
5. **Consider procedural macros**: For complex cases, use proc macros
6. **Follow naming conventions**: Use SCREAMING_SNAKE_CASE for macros

## Debugging Macros

### Using cargo expand

```bash
# Install cargo-expand
cargo install cargo-expand

# Expand macros
cargo expand
```

### Using trace_macros!

```rust
#![feature(trace_macros)]

trace_macros!(true);
my_macro!();
trace_macros!(false);
```

## Examples from Project

Look for macro usage in:
- Test helpers
- Builder patterns
- Code generation
- Derive macros

## Common Patterns

### ✅ Good

```rust
// Clear, well-documented macro
/// Creates a vector with the given elements.
macro_rules! vec {
    ($($x:expr),*) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($x);
            )*
            temp_vec
        }
    };
}
```

### ❌ Avoid

```rust
// Overly complex macro
macro_rules! bad {
    // Too many patterns, hard to understand
    ($a:expr) => { /* ... */ };
    ($a:expr, $b:expr) => { /* ... */ };
    ($a:expr, $b:expr, $c:expr) => { /* ... */ };
    // ... many more patterns
}

// Prefer function when possible
fn good(a: i32, b: i32, c: i32) {
    // Clearer and easier to understand
}
```
