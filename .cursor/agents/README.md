# User-Level Rust Development Subagents

These subagents are available across all Rust projects. They provide specialized capabilities for common development tasks.

## Available Subagents

### test-runner
**Model**: fast

Test automation expert. Proactively runs tests, analyzes failures, fixes issues while preserving test intent, and reports results. Handles `cargo test`, test failures, clippy warnings.

**Use when**: You need tests run, failures fixed, or code verified.

### debugger
**Model**: fast

Debugging specialist for Rust code. Handles stack traces, error messages, reproduction steps, root cause analysis, and minimal fixes.

**Use when**: Encountering errors, test failures, panics, or unexpected behavior.

### verifier
**Model**: fast

Skeptical validator that verifies completed work actually functions. Tests everything, runs relevant tests, checks edge cases, and reports what's verified vs incomplete.

**Use when**: After tasks are marked done, to confirm implementations are functional.

### code-reviewer
**Model**: fast

Code review specialist. Reviews code for common issues, safety problems, performance concerns, and best practices. Checks clippy warnings, ownership issues, error handling.

**Use when**: Reviewing code before commit, checking for issues, or ensuring code quality.

### benchmarker
**Model**: fast

Benchmarking specialist. Creates benchmarks, analyzes performance, identifies bottlenecks. Handles Criterion benchmarks, throughput metrics, performance analysis.

**Use when**: Creating benchmarks, analyzing performance, or optimizing code.

### documenter
**Model**: fast

Documentation specialist. Writes documentation, creates examples, improves code documentation. Handles doc comments, examples, API documentation.

**Use when**: Writing documentation, creating examples, or improving code documentation.

### ui-ux-reviewer
**Model**: fast

UI/UX review and improvement specialist. Audits interfaces for accessibility, usability heuristics, motion performance, layout, and design consistency. Uses baseline-ui, accessibility, motion-performance, laws-of-ux, and usability-heuristics skills.

**Use when**: Auditing screens or flows, improving UX, refining UI components, or checking accessibility and motion before release.

### github-issue-dispatcher
**Model**: default

Reads GitHub issues and delegates their tasks to subagents. Uses github-issues and subagent-delegation skills to parse issues and hand off to test-runner, debugger, code-reviewer, benchmarker, documenter, ui-ux-reviewer, verifier, or cargo-release.

**Use when**: Given a GitHub issue URL, number, or pasted issue content to triage and delegate.

## Usage

These subagents are automatically available to Cursor's agent. The agent can delegate tasks to them when appropriate, or you can explicitly invoke them:

- `/test-runner` - Run tests and fix failures
- `/debugger` - Debug an issue
- `/verifier` - Verify completed work
- `/code-reviewer` - Review code
- `/benchmarker` - Create benchmarks
- `/documenter` - Write documentation
- `/ui-ux-reviewer` - Review and improve UI/UX
- `/github-issue-dispatcher` - Triage a GitHub issue and delegate to subagents

## Project-Specific Subagents

If a project has its own subagents in `.cursor/agents/`, those take precedence over these user-level subagents when names conflict.
