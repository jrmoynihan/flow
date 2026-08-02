# third_party

Path patches for crates that ship a `.vscode/` directory (creating that name is
blocked in some Cursor agent sandboxes). Workspace `Cargo.toml` patches:

- `pastey` → `third_party/pastey` (0.1.1)
- `bit-vec` → `third_party/bit-vec` (0.9.1)

Extracted with `tar --exclude='*/.vscode'`. Burn / cubeCL come from crates.io
(`0.21` / `0.10`). Large local checkouts of those trees (if present) are gitignored.

