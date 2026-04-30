# `flow-tru-ols` helper scripts

| File | Description |
|------|-------------|
| [`inspect_codegen_julia.jl`](inspect_codegen_julia.jl) | Julia: BLAS config, `@code_*` on a pure loop and on `A \\ b`. See [julia-and-blas-on-macos.md](../docs/julia-and-blas-on-macos.md). |
| [`inspect_codegen_rust.sh`](inspect_codegen_rust.sh) | Rust: `cargo asm` for `solve_linear_system` (faer vs `blas` feature). Needs `cargo install cargo-show-asm`. See [PROFILING.md](../docs/PROFILING.md). |

Run both from the **workspace root** (`flow-crates/`).
