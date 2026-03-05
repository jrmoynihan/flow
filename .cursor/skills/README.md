# User-Level Rust Development Skills

These skills are available across all Rust projects. They provide guidance for common Rust development tasks.

## Available Skills

### rust-docs
Write Rust documentation comments following standard conventions. Handles `///` for items, `//!` for crate/module docs, markdown formatting, code examples, and panic/safety sections.

### criterion-benchmarks
Create Criterion benchmarks with proper group macros and configuration. Handles benchmark groups, throughput metrics, conditional compilation, and proper `black_box` usage.

### rust-rand
Use `rand` crate correctly for generating random numbers. Handles proper range syntax, seeding RNGs, choosing appropriate RNG types, and generating random values.

### rust-black-box
Use `std::hint::black_box` correctly in benchmarks and tests. Handles proper placement and common patterns for benchmarking.

### rust-testing
Write Rust tests following standard conventions. Handles test organization, Result-based tests, test fixtures, and common testing patterns.

### rust-error-handling
Handle Rust errors using Result types, `thiserror`, and error propagation. Handles `thiserror` derive macros, error conversion, and proper error context.

### rust-features
Work with Cargo features and conditional compilation. Handles feature flags, optional dependencies, and `cfg` attributes.

### github-issues
Read and parse GitHub issues; extract title, body, labels, checklists, and acceptance criteria to support triage and delegation.

### subagent-delegation
Choose the right subagent for a task and write a clear delegation prompt including issue context and success criteria.

## Usage

These skills are automatically available to Cursor's agent in all Rust projects. The agent will use them when relevant based on your prompts.

## Project-Specific Skills

If a project has its own skills in `.cursor/skills/` or `skills/`, those take precedence over these user-level skills when names conflict.
