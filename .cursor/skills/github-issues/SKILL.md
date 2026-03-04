---
name: github-issues
description: Read and parse GitHub issues; extract title, body, labels, checklists, and acceptance criteria to support triage and delegation.
---

# GitHub Issues

This skill guides reading and parsing GitHub issues so task type, scope, and acceptance criteria can be extracted for triage and delegation.

## Getting issue content

- **CLI:** Use `gh issue view <number> --repo owner/repo` to fetch an issue. Use `--web` to open the issue in the browser and get the URL.
- **Pasted content:** If the user pastes issue text, treat it as the body. Infer title and labels from the paste if present (e.g. a "Title:" line or label tags).

Ensure you have the full issue content (title, body, labels) before parsing.

## Issue structure

- **Title:** Short summary; often indicates task type (e.g. "Fix panic in parser", "Add docs for X").
- **Body:** Markdown description; may include steps to reproduce, acceptance criteria, task lists.
- **Labels:** Tags like `bug`, `documentation`, `enhancement`, `testing`, `release`, `ui`, `accessibility`.
- **Assignees / milestone:** Optional; use for priority or scope hints.

Emphasize body and labels when determining task type.

## What to extract

1. **Task type** – bug, feature, docs, refactor, release, test, review, UI/UX, etc., from title, body, and labels.
2. **Acceptance criteria** – Look for:
   - Markdown checklists: `- [ ]` or `- [x]`
   - Sections like `## Acceptance criteria`, `## Requirements`, `## Done when`
   - Numbered or bulleted "must" items in the body
3. **Scope** – Files, modules, or areas mentioned (e.g. "in `src/parser.rs`", "QC page", "workspace persistence"). Note "whole project" if no scope is given.
4. **Repo and context** – Repo (owner/repo), and if mentioned: branch, related PR, or issue links.

## Label → candidate subagent mapping

Use labels to suggest which subagent might handle the work (the **subagent-delegation** skill makes the final choice):

| Label / theme      | Candidate subagent(s)     |
|--------------------|---------------------------|
| `bug`, errors      | debugger, test-runner     |
| `documentation`    | documenter                |
| `testing`, `ci`    | test-runner               |
| `release`          | cargo-release             |
| `ui`, `ux`, `accessibility` | ui-ux-reviewer    |
| Performance        | benchmarker               |
| Review / quality   | code-reviewer             |
| Verify / validate  | verifier                  |

Extract these fields and pass them to the delegation step so the right subagent and a clear prompt can be chosen.
