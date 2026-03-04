---
name: baseline-ui
description: Validates animation durations, enforces typography scale, checks component accessibility, and prevents layout anti-patterns in Tailwind CSS projects. Use when building UI components, reviewing CSS utilities, or enforcing design consistency.
---

# Baseline UI

Enforces an opinionated UI baseline to prevent AI-generated interface slop. Framework-agnostic; applies to any stack using Tailwind CSS and semantic HTML.

## How to use

- `/baseline-ui`
  Apply these constraints to any UI work in this conversation.

- `/baseline-ui <file>`
  Review the file against all constraints below and output:
  - violations (quote the exact line/snippet)
  - why it matters (1 short sentence)
  - a concrete fix (code-level suggestion)

## Stack

- MUST use Tailwind CSS defaults unless custom values already exist or are explicitly requested
- SHOULD use CSS-based animation (transitions, `@keyframes`) or the project’s chosen animation approach for entrance and micro-animations
- MUST use a class utility (`clsx` + `tailwind-merge`, or the project’s equivalent) for conditional or merged class logic
- When using components from shadcn, bits-ui, or Svelte component libraries, consider using `clsx` (or the project’s class merge utility) to merge utility classes passed into components

## Components & accessibility

- MUST use accessible patterns for anything with keyboard or focus: semantic HTML, ARIA where needed, or the project’s existing component primitives
- MUST use the project’s existing component primitives first
- When using Svelte, prefer **shadcn-svelte** for core components unless told otherwise; if the file(s) you’re working in already use a different component library, ask the user for clarification before introducing another
- NEVER mix different primitive or design systems within the same interaction surface
- MUST add an `aria-label` (or visible text) to icon-only buttons
- NEVER rebuild keyboard or focus behavior by hand unless explicitly requested; prefer native elements or established primitives

## Interaction

- MUST use a proper dialog/modal for destructive or irreversible actions (confirm before proceeding)
- SHOULD use structural skeletons for loading states
- NEVER use `h-screen`, use `h-dvh` (or equivalent) for viewport height
- MUST respect `safe-area-inset` for fixed elements
- MUST show errors next to where the action happens
- NEVER block paste in `input` or `textarea` elements

## Animation

- NEVER add animation unless it is explicitly requested
- MUST animate only compositor-friendly properties (`transform`, `opacity`)
- Prefer individual `translate`, `rotate`, and `scale` properties over the `transform` shorthand; they are composable and more predictably ordered
- NEVER animate layout properties (`width`, `height`, `top`, `left`, `margin`, `padding`)
- SHOULD avoid animating paint properties (`background`, `color`) except for small, local UI (text, icons)
- SHOULD use `ease-out` on entrance
- NEVER exceed `200ms` for interaction feedback
- MUST pause looping animations when off-screen
- SHOULD respect `prefers-reduced-motion`
- NEVER introduce custom easing curves unless explicitly requested
- SHOULD avoid animating large images or full-screen surfaces

## Typography

- MUST use `text-balance` for headings and `text-pretty` for body/paragraphs
- MUST use `tabular-nums` for data
- SHOULD use `truncate` or `line-clamp` for dense UI
- NEVER modify `letter-spacing` (`tracking-*`) unless explicitly requested

## Layout

- MUST use a fixed `z-index` scale (no arbitrary `z-*`)
- SHOULD use `size-*` for square elements instead of `w-*` + `h-*`

## Performance

- NEVER animate large `blur()` or `backdrop-filter` surfaces
- NEVER apply `will-change` outside an active animation
- NEVER use lifecycle or effect hooks for logic that can be expressed as derived state or template logic

## Design

- NEVER use gradients unless explicitly requested
- NEVER use purple or multicolor gradients
- NEVER use glow effects as primary affordances
- SHOULD use Tailwind CSS default shadow scale unless explicitly requested
- MUST give empty states one clear next action
- SHOULD limit accent color usage to one per view
- SHOULD use existing theme or Tailwind CSS color tokens before introducing new ones
