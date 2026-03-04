---
name: rust-async
description: 'Work with async/await, futures, and asynchronous Rust code. Use when writing async functions, working with tokio or async-std, understanding futures, or implementing async traits. Handles async/await syntax, futures, executors, pinning, Send/Sync bounds, and async patterns.'
---

# Async Rust

Guidelines for working with async/await, futures, and asynchronous programming in Rust.

## When to Use This Skill

- Writing async functions
- Working with futures
- Using tokio or async-std
- Understanding async/await syntax
- Implementing async traits
- Working with pinning and Send/Sync bounds

## Basic Async Syntax

### Async Functions

```rust
// Basic async function
async fn fetch_data() -> Result<String, Error> {
    // Async operations
    Ok("data".to_string())
}

// Calling async functions
async fn example() {
    let result = fetch_data().await?;
}
```

### Async Blocks

```rust
// Async block
let future = async {
    let data = fetch_data().await?;
    process(data).await
};
```

## Futures

### What is a Future?

```rust
use std::future::Future;

// Future trait
trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
```

### Creating Futures

```rust
// From async function
async fn create_future() -> i32 {
    42
}

// Manual future implementation
struct MyFuture {
    value: i32,
}

impl Future for MyFuture {
    type Output = i32;
    
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(self.value)
    }
}
```

## Executors

### Tokio

```rust
use tokio;

#[tokio::main]
async fn main() {
    // Async code here
    let result = fetch_data().await;
}

// Or with runtime builder
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // Async code
    });
}
```

### Async-std

```rust
use async_std;

#[async_std::main]
async fn main() {
    // Async code here
}
```

## Common Async Patterns

### Spawning Tasks

```rust
use tokio;

#[tokio::main]
async fn main() {
    // Spawn concurrent tasks
    let handle1 = tokio::spawn(async {
        fetch_data1().await
    });
    
    let handle2 = tokio::spawn(async {
        fetch_data2().await
    });
    
    // Wait for both
    let (result1, result2) = tokio::join!(handle1, handle2);
}
```

### Select

```rust
use tokio::select;

async fn example() {
    select! {
        result = task1() => {
            // Handle task1 result
        },
        result = task2() => {
            // Handle task2 result
        },
    }
}
```

### Timeout

```rust
use tokio::time::{timeout, Duration};

async fn example() {
    match timeout(Duration::from_secs(5), slow_operation()).await {
        Ok(result) => {
            // Operation completed
        },
        Err(_) => {
            // Operation timed out
        },
    }
}
```

## Pinning

### Why Pin?

```rust
use std::pin::Pin;

// Some types must be pinned to be used
async fn example() {
    let future = create_future();
    let pinned = Box::pin(future);
    pinned.await;
}
```

### Pin in Structs

```rust
use std::pin::Pin;

struct MyStruct {
    future: Pin<Box<dyn Future<Output = i32>>>,
}
```

## Send and Sync Bounds

### Send Trait

```rust
// Send: Can be transferred between threads
async fn send_example<T: Send>(value: T) {
    tokio::spawn(async move {
        // value can be moved across threads
        use_value(value);
    });
}
```

### Sync Trait

```rust
// Sync: Can be shared between threads
async fn sync_example<T: Sync>(value: &T) {
    tokio::spawn(async move {
        // value can be shared across threads
        use_shared(value);
    });
}
```

### Common Bounds

```rust
// Future that is Send
fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(future);
}
```

## Async Traits

### Using async-trait

```rust
use async_trait::async_trait;

#[async_trait]
trait AsyncTrait {
    async fn method(&self) -> Result<(), Error>;
}

#[async_trait]
impl AsyncTrait for MyType {
    async fn method(&self) -> Result<(), Error> {
        // Implementation
        Ok(())
    }
}
```

### Without async-trait (Rust 1.75+)

```rust
// Using associated type (Rust 1.75+)
trait AsyncTrait {
    type Output<'a>: Future<Output = Result<(), Error>>
    where
        Self: 'a;
    
    fn method(&self) -> Self::Output<'_>;
}

impl AsyncTrait for MyType {
    type Output<'a> = impl Future<Output = Result<(), Error>> + 'a;
    
    fn method(&self) -> Self::Output<'_> {
        async move {
            Ok(())
        }
    }
}
```

## Error Handling

### Result in Async

```rust
async fn fallible_operation() -> Result<String, Error> {
    let data = fetch_data().await?;
    process(data).await?;
    Ok("success".to_string())
}
```

### Error Propagation

```rust
async fn chain_operations() -> Result<(), Error> {
    let step1 = operation1().await?;
    let step2 = operation2(step1).await?;
    operation3(step2).await?;
    Ok(())
}
```

## Important Rules

1. **Use `.await` to wait**: Futures don't execute until awaited
2. **Understand Send/Sync**: Know when types need these bounds
3. **Use executors**: Futures need an executor to run
4. **Handle errors**: Use `Result` in async functions
5. **Avoid blocking**: Don't block the async runtime
6. **Use pinning when needed**: Some futures must be pinned

## Common Patterns

### ✅ Good

```rust
// Proper async function
async fn fetch_and_process() -> Result<(), Error> {
    let data = fetch_data().await?;
    process(data).await?;
    Ok(())
}

// Proper error handling
async fn example() -> Result<String, Error> {
    let result = fallible_operation().await?;
    Ok(result)
}
```

### ❌ Avoid

```rust
// Don't block in async code
async fn bad_example() {
    std::thread::sleep(Duration::from_secs(1));  // BAD: Blocks
}

// Use tokio::time::sleep instead
async fn good_example() {
    tokio::time::sleep(Duration::from_secs(1)).await;  // GOOD
}

// Don't forget .await
async fn bad_example() {
    let future = fetch_data();  // BAD: Future not awaited
    // Use: let result = fetch_data().await;
}
```

## Examples from Project

Look for async usage in:
- Network operations
- File I/O operations
- Concurrent processing
- Task spawning and coordination

## Tokio vs Async-std

### Tokio

- More features (timers, networking, etc.)
- Better for complex applications
- More ecosystem support

### Async-std

- Simpler API
- More similar to std library
- Good for simpler applications

Choose based on project needs and ecosystem requirements.
