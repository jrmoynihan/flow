---
name: rust-lifetimes
description: 'Understand and work with Rust lifetimes, lifetime annotations, and lifetime elision. Use when fixing lifetime errors, understanding borrowing rules, working with references, or implementing traits with lifetime parameters. Handles explicit lifetimes, lifetime elision, higher-ranked trait bounds, and common lifetime patterns.'
---

# Rust Lifetimes

Guidelines for understanding and working with Rust lifetimes and lifetime annotations.

## When to Use This Skill

- Fixing lifetime errors
- Understanding borrowing rules
- Working with references
- Implementing traits with lifetime parameters
- Understanding when explicit lifetimes are needed
- Working with higher-ranked trait bounds (HRTB)

## Basic Lifetime Syntax

### Lifetime Parameters

```rust
// Function with explicit lifetime
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() {
        x
    } else {
        y
    }
}
```

### Structs with Lifetimes

```rust
// Struct holding a reference
struct ImportantExcerpt<'a> {
    part: &'a str,
}

impl<'a> ImportantExcerpt<'a> {
    fn new(part: &'a str) -> Self {
        Self { part }
    }
}
```

### Multiple Lifetimes

```rust
// Function with multiple lifetimes
fn longest<'a, 'b>(x: &'a str, y: &'b str) -> &'a str
where
    'b: 'a,  // 'b must outlive 'a
{
    x
}
```

## Lifetime Elision Rules

### Three Elision Rules

1. **Each parameter gets its own lifetime**
2. **If there's exactly one input lifetime, it's assigned to all output lifetimes**
3. **If there's `&self` or `&mut self`, its lifetime is assigned to all output lifetimes**

### Examples

```rust
// Rule 1: Each parameter gets its own lifetime
fn first<'a, 'b>(x: &'a str, y: &'b str) -> &'a str {
    x
}

// Rule 2: Single input lifetime assigned to output
fn identity<'a>(x: &'a str) -> &'a str {
    x
}

// Rule 3: self lifetime assigned to output
impl<'a> ImportantExcerpt<'a> {
    fn get_part(&self) -> &str {  // Elided: &'a str
        self.part
    }
}
```

## Common Lifetime Patterns

### Returning References

```rust
// ✅ Good: Return reference with appropriate lifetime
fn get_first_word<'a>(s: &'a str) -> &'a str {
    s.split_whitespace().next().unwrap_or("")
}

// ❌ Bad: Returning reference to temporary
fn bad_example(s: &str) -> &str {
    let temp = String::from("temp");
    &temp  // ERROR: temp doesn't live long enough
}
```

### Structs Holding References

```rust
// Struct that holds a reference
struct Parser<'a> {
    input: &'a str,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input }
    }
    
    fn parse(&self) -> &'a str {
        self.input
    }
}
```

### Lifetimes in Methods

```rust
impl<'a> ImportantExcerpt<'a> {
    // Method that returns reference tied to self
    fn get_part(&self) -> &'a str {
        self.part
    }
    
    // Method that returns reference tied to parameter
    fn compare<'b>(&self, other: &'b str) -> &'b str {
        other
    }
    
    // Method with multiple lifetimes
    fn combine<'b>(&self, other: &'b str) -> &'a str
    where
        'b: 'a,
    {
        self.part
    }
}
```

## Static Lifetime

### 'static Lifetime

```rust
// 'static means the reference lives for the entire program
let s: &'static str = "I have a static lifetime.";

// String literals have 'static lifetime
fn get_static() -> &'static str {
    "static string"
}
```

### When to Use 'static

```rust
// ✅ Good: For string literals and constants
const CONSTANT: &'static str = "constant";

// ❌ Avoid: Don't force 'static when not needed
fn bad_function() -> &'static str {
    // This is overly restrictive
}
```

## Higher-Ranked Trait Bounds (HRTB)

### For Closures

```rust
// HRTB for closures that can work with any lifetime
fn call_with_ref<F>(f: F)
where
    F: for<'a> Fn(&'a i32),
{
    let value = 42;
    f(&value);
}
```

### For Trait Objects

```rust
// HRTB for trait objects
trait Trait {
    fn method(&self) -> &str;
}

fn use_trait<T: for<'a> Trait>(t: T) {
    let s = t.method();
}
```

## Common Lifetime Errors

### Error: Missing Lifetime Parameter

```rust
// ❌ Error: Missing lifetime parameter
fn longest(x: &str, y: &str) -> &str {
    // Error: expected named lifetime parameter
}

// ✅ Fix: Add lifetime parameter
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

### Error: Borrowed Value Does Not Live Long Enough

```rust
// ❌ Error: Value doesn't live long enough
fn example() -> &str {
    let s = String::from("hello");
    &s  // ERROR: s doesn't live long enough
}

// ✅ Fix: Return owned value or use different approach
fn example() -> String {
    let s = String::from("hello");
    s
}
```

### Error: Lifetime Mismatch

```rust
// ❌ Error: Lifetimes don't match
struct Holder<'a> {
    value: &'a str,
}

fn example<'a, 'b>(h1: &'a Holder<'a>, h2: &'b Holder<'b>) -> &'a str {
    h2.value  // ERROR: 'b doesn't match 'a
}

// ✅ Fix: Use same lifetime or constrain lifetimes
fn example<'a>(h1: &'a Holder<'a>, h2: &'a Holder<'a>) -> &'a str {
    h2.value  // OK: Same lifetime
}
```

## Important Rules

1. **Lifetimes ensure references are valid**: They don't change how long values live
2. **Use elision when possible**: Let Rust infer lifetimes when rules apply
3. **Be explicit when needed**: Add lifetimes when elision doesn't work
4. **Document lifetime relationships**: Use where clauses to clarify relationships
5. **Avoid 'static unless necessary**: Don't force 'static when not needed
6. **Understand the error**: Lifetime errors tell you what's wrong

## Lifetime Patterns

### Returning Iterator

```rust
// Iterator with lifetime
fn words<'a>(s: &'a str) -> impl Iterator<Item = &'a str> {
    s.split_whitespace()
}
```

### Generic Lifetimes

```rust
// Generic function with lifetime
fn process<'a, T>(value: &'a T) -> &'a T
where
    T: 'a,
{
    value
}
```

### Lifetime in Traits

```rust
trait Processor<'a> {
    type Output: 'a;
    fn process(&self, input: &'a str) -> Self::Output;
}
```

## Examples from Project

Look for lifetime usage in:
- Parsers that hold references to input
- Iterators over slices
- Structs that borrow data
- Trait implementations with references

## Common Solutions

### Problem: Need to Return Reference

```rust
// If you need to return a reference, ensure the data lives long enough
fn get_first<'a>(s: &'a str) -> &'a str {
    s.split_whitespace().next().unwrap_or("")
}
```

### Problem: Struct Needs to Hold Reference

```rust
// Add lifetime parameter to struct
struct Holder<'a> {
    value: &'a str,
}
```

### Problem: Multiple References with Different Lifetimes

```rust
// Use multiple lifetime parameters
fn combine<'a, 'b>(x: &'a str, y: &'b str) -> &'a str
where
    'b: 'a,
{
    x
}
```
