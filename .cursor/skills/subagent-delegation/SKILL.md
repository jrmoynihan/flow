---
name: subagent-delegation
description: Choose the right subagent for a task and write a clear delegation prompt including issue context and success criteria.
---

# Subagent Delegation

This skill defines when to use which subagent and how to write the delegation prompt so the subagent has full context and does not need to open the issue again.

## Subagent mapping

| Task type / keywords | Subagent |
|----------------------|----------|
| Bugs, errors, panics, reproduction, stack traces | **debugger** |
| Run/fix tests, clippy, CI, test failures | **test-runner** |
| Code review, safety, style, best practices | **code-reviewer** |
| Benchmarks, performance analysis, throughput | **benchmarker** |
| Docs, examples, API comments, readme | **documenter** |
| UI/UX, accessibility, motion, layout, design | **ui-ux-reviewer** |
| Verify completed work, re-run checks, validate | **verifier** |
| Release, versioning, changelog, semver | **cargo-release** |

When the issue spans multiple areas (e.g. fix a bug and add tests), delegate in sequence: e.g. **debugger** then **test-runner**, then optionally **verifier**. State the order in your summary to the user.

## Delegation prompt format

The prompt you pass to the subagent must include:

1. **Issue title and link or number** – So the subagent knows which issue this is.
2. **Issue body (or a short summary)** – What was requested, steps to reproduce, or context.
3. **Repo/workspace path** – The project path so the subagent knows where to work.
4. **Acceptance criteria** – From the issue or inferred (checklists, "done when", requirements).
5. **Explicit task in one sentence** – e.g. "Fix the failing test in `foo.rs` and ensure `cargo test` passes."

Rule: the subagent should be able to do the job without opening the issue again.

## Invoking the subagent

Use the platform's subagent/task invocation tool (e.g. `mcp_task`). Pass:

- **subagent_type** – The type that matches the chosen subagent (e.g. `rust-debugger`, `test-runner`, `code-reviewer`, `benchmarker`, `documenter`, `ui-ux-reviewer`, `verifier`, `cargo-release`). Use the exact subagent type names supported by your environment.
- **prompt** – The full delegation text built from the format above.
- **description** – A short (3–5 word) summary of the task for the tool.

Do not invent tool or parameter names; use the names provided by your environment for the subagent/task tool.
