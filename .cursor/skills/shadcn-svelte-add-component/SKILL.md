---
name: shadcn-svelte-add-component
description: Install shadcn-svelte UI components via CLI. Use when adding a new shadcn-svelte component (e.g. switch, label, checkbox) or when the user asks to add a component from shadcn-svelte.
---

# Adding shadcn-svelte Components

## When to Use

- User asks to add a shadcn-svelte component (e.g. Switch, Label, Dialog).
- You need a UI primitive that shadcn-svelte provides and it is not yet in `src/lib/components/ui/`.

## Installation Command

**Always install components using the CLI** so files are generated in the project’s UI folder with the correct structure and styling.

```bash
bunx shadcn-svelte@latest add <component-name>
```

- **Package manager**: Use `bunx` (this project uses bun).
- **Component name**: Lowercase, e.g. `switch`, `label`, `checkbox`, `dialog`.
- Components are added under `src/lib/components/ui/<component-name>/`.

## Examples

```bash
bunx shadcn-svelte@latest add switch
bunx shadcn-svelte@latest add label
bunx shadcn-svelte@latest add checkbox
```

## After Adding

1. Import from the project path, e.g. `import { Switch } from "$lib/components/ui/switch/index.js";`
2. Prefer the exported name (e.g. `Switch`) from the component’s `index.ts` when using it in Svelte.

## Do Not

- Do not hand-copy component code from the shadcn-svelte docs unless the CLI cannot be run.
- Do not use `npx` or `pnpm dlx` unless the user has specified a different package manager for this project.
