# Julia REPL basics, BLAS inspection, and macOS performance notes

This page is for developers who rarely use Julia but need to **inspect which BLAS** a Julia session uses, **look at native/LLVM code**, or **profile** hot paths when comparing to `flow-tru-ols` (see also [comparison-with-julia.md](comparison-with-julia.md) and [PROFILING.md](PROFILING.md)).

## Why `LinearAlgebra`, `Pkg`, and `OpenBLAS_jll` errored

Julia’s standard library is **modular**. Names like `LinearAlgebra`, `Pkg`, and artifact packages are **not** in scope until you load them.

- Use `using ModuleName` to import a module’s exported names (typical for interactive work).
- `import ModuleName` is finer-grained (e.g. `import LinearAlgebra: BLAS`).

So in a **fresh** REPL, `LinearAlgebra.BLAS.get_config()` fails until you run `using LinearAlgebra` (or qualify with the full module path after `import`).

Hints like “`LinearAlgebra` is loaded but not imported” mean: the runtime has the code available, but you still need `using LinearAlgebra` (or `import`) to bind the name `LinearAlgebra` in `Main`.

## Minimal REPL session: BLAS and build info

Paste **in order** (Julia 1.7+ assumed for `libblastrampoline` / `BLAS.get_config()`).

```julia
using LinearAlgebra

# Which BLAS interfaces are loaded (libblastrampoline / “LBT” on official builds)
BLAS.get_config()

# Optional: cap BLAS threads for fair A/B vs Rayon or other parallel layers
BLAS.set_num_threads(1)

# Build and platform summary (in the default REPL this is usually available)
versioninfo()
```

`versioninfo()` includes a **BLAS** line when available. If it is not defined in your session, use:

```julia
using InteractiveUtils
versioninfo()
```

### `Pkg.status` and JLL packages

```julia
using Pkg

# Active project’s manifest (from the directory you started Julia in, if any)
Pkg.status()

# Filter to one package if present in the manifest
Pkg.status("OpenBLAS_jll")
```

`OpenBLAS_jll` only resolves if some environment **depends** on it. Official Julia binaries route linear algebra through **libblastrampoline**; the actual shared library may be OpenBLAS, but you discover it via `BLAS.get_config()` and `versioninfo()`, not only via `OpenBLAS_jll`.

### Optional: load the OpenBLAS JLL when it exists in the environment

```julia
using OpenBLAS_jll
OpenBLAS_jll.libopenblas_path
```

If `using OpenBLAS_jll` fails, the current project does not depend on that artifact; use `BLAS.get_config()` instead.

### Optional: `ccall` to OpenBLAS (only if that is your backend)

If `BLAS.get_config()` shows an OpenBLAS library, you can sometimes call its version string (exact symbol name depends on the OpenBLAS build):

```julia
using LinearAlgebra
# Example only — may not exist if the active backend is not OpenBLAS
# blas_lib = BLAS.libblas  # internal; prefer BLAS.get_config() for portability
```

Prefer **`BLAS.get_config()`** and **`versioninfo()`** for portability across Julia builds.

## Scripted codegen dump (`inspect_codegen_julia.jl`)

From the **workspace root**:

```bash
julia tru-ols/scripts/inspect_codegen_julia.jl
julia tru-ols/scripts/inspect_codegen_julia.jl --which
INSPECT_LS_ROWS=20 INSPECT_LS_COLS=12 julia tru-ols/scripts/inspect_codegen_julia.jl 2>&1 | tee julia_codegen.txt
```

The script prints **`BLAS.get_config()`**, then:

1. **Pure-Julia loop** (`simd_demo_sum`) — `@code_typed`, `@code_llvm`, `@code_native`. LLVM may fully unroll small constant-trip-count loops (you may see scalar `fadd`/`fmul` chains instead of explicit NEON vectors); larger or unknown-trip loops are better for spotting vectorization.
2. **Dense least squares** `solve_ls(A, b) = A \\ b` at shape **`INSPECT_LS_ROWS` × `INSPECT_LS_COLS`** (default 10×8). The same macros show a **short thunk that calls into the `\` implementation** — for `Float64` matrices this is typically **OpenBLAS/LAPACK**, not Julia SIMD. Vectorization for that work lives **inside `libopenblas`**.

### Reading the output

- **`bl` / branch to a symbol** whose name includes `dgels`, `gelsd`, `geqrf`, etc.: you are looking at the **wrapper**; SIMD is in the shared library.
- **`fmla`, `dup`, `zip`, vector registers (`v0`, `z`…)**: possible **NEON** (or **SVE** on newer cores) — treat as qualitative hints; mnemonics vary by LLVM/CPU.

### Optional: disassemble OpenBLAS (advanced)

If you need to see instructions **inside** the BLAS shared library (not Julia-generated), use the dylib path from **`BLAS.get_config()`**, then tools such as **`lldb`** (disassemble a known entry) or **`nm` / `objdump`** on that file. Symbol names depend on ILP64/OpenBLAS build (e.g. `dgelsd_64` variants).

## Inspecting generated code (`@code_native`, `@code_llvm`)

Macros like `@code_native` and `@code_llvm` need a **callable** and are easiest with a **named function** you define yourself. Anonymous functions are awkward here.

### Small example (not TRU-OLS specific)

```julia
function demo_sum(x::Float64)
    s = zero(Float64)
    @inbounds for i in 1:16
        s += x * Float64(i)
    end
    s
end

@code_llvm debuginfo=:none demo_sum(1.0)
@code_native debuginfo=:none demo_sum(1.0)
```

Use `debuginfo=:source` when you want richer correlation to Julia source (noisier output).

### What to look for on Apple Silicon

Assembly mnemonics vary by LLVM version and CPU. You may see **NEON**-style patterns (e.g. `fmla`, `ldr`/`str` pairs, `dup`, `zip`, or **SVE** on newer cores). There is no single fixed string that always means “vectorized”; treat this as **qualitative** inspection.

### Hooking a real hot path

1. Put the hot call in a **top-level named function** in a `.jl` file.
2. `include("that_file.jl")` in the REPL.
3. Run `@code_native debuginfo=:none your_function(...)` with representative arguments (types matter for specialization).

For TRU-OLS parity work, the meaningful comparison is usually **wall time + quality metrics** ([comparison-with-julia.md](comparison-with-julia.md)), not a single inner instruction, unless you are chasing a specific kernel.

## Profiling Julia code

### `Profile` (stdlib)

```julia
using Profile

function work(n)
    s = 0.0
    for i in 1:n
        s += sin(Float64(i))
    end
    s
end

Profile.clear()
@profile for _ in 1:1000
    work(10_000)
end

Profile.print()
```

Install **ProfileView** in the active environment for a flame-style viewer: `using Pkg; Pkg.add("ProfileView")` then `using ProfileView; ProfileView.view()`.

### macOS Instruments

Attach **Time Profiler** to the `julia` process while exercising your script. Compare time in `libopenblas` (or other BLAS dylibs) vs time in Julia-generated code. This complements Rust-side [PROFILING.md](PROFILING.md) (`samply`, `cargo flamegraph`).

### Thread oversubscription

When BLAS is multithreaded **and** you also use Julia threads or outer loops, set `BLAS.set_num_threads(1)` (and common env vars like `OPENBLAS_NUM_THREADS=1`, `MKL_NUM_THREADS=1` when relevant) for predictable comparisons. The workspace [AGENTS.md](../../AGENTS.md) notes the same idea for Rust (`OMP_NUM_THREADS=1`) when stacking Rayon and BLAS.

## Apple Accelerate vs OpenBLAS (expectations)

### Julia

Official **julialang.org** macOS binaries typically use **OpenBLAS** (via **libblastrampoline**), not **Apple Accelerate** (`vecLib`), for the standard `LinearAlgebra` BLAS/LAPACK route. Switching the whole stack to Accelerate is **not** a one-line REPL change; it depends on how Julia was built and which artifacts are loaded.

Practical comparison:

- Treat `BLAS.get_config()` + `versioninfo()` as ground truth for *your* Julia.
- A fair “Accelerate vs not” study usually requires a **dedicated build or environment** documented elsewhere, not assumptions from a default install.

### Rust `flow-tru-ols`

The optional Cargo feature **`blas`** enables **`ndarray-linalg`** with the **`openblas-system`** feature: it links against **system OpenBLAS** (typically **Homebrew** `openblas` on macOS via `pkg-config`). It does **not** use **Apple Accelerate** unless someone adds a separate backend.

So:

- **“Accelerate vs OpenBLAS”** for this crate is **not** exposed as a flag today.
- **“BLAS (OpenBLAS) vs default faer”** *is* what the `blas` feature toggles (see below).

## Comparing `flow-tru-ols` with and without the `blas` feature (Rust)

Default linear algebra in this crate uses **faer**. With **`--features blas`**, eligible solves use **`ndarray` + `ndarray-linalg`** against **system OpenBLAS**.

### macOS: install OpenBLAS for the `blas` build

Homebrew example:

```bash
brew install openblas
export PKG_CONFIG_PATH="$(brew --prefix openblas)/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
```

Then verify Cargo sees it:

```bash
pkg-config --libs openblas
```

### Build / run examples on `flow-tru-ols` directly

From the workspace root:

```bash
# faer path (no OpenBLAS for ndarray-linalg)
cargo build -p flow-tru-ols --release --no-default-features --features flow-fcs

# OpenBLAS-backed path (requires pkg-config openblas as above)
cargo build -p flow-tru-ols --release --no-default-features --features flow-fcs,blas
```

Use the same pattern for `cargo run --example profile_hot_path`, Criterion benches, or `compare_with_julia`-style workflows **if** you invoke them through a binary that depends on `flow-tru-ols` with the chosen features.

### `tru-ols` CLI (`compare_with_julia`)

The **`tru-ols`** package ([`tru-ols-cli`](../../tru-ols-cli/Cargo.toml)) enables `flow-fcs` and `plotting` on `flow-tru-ols` but **does not** enable `blas` by default. To A/B **faer vs OpenBLAS** with the CLI examples, add **`"blas"`** to the `flow-tru-ols` `features = [...]` list **temporarily**, rebuild, and compare timings; then remove it for the default configuration. Alternatively, keep a small **wrapper** crate or `cargo run -p flow-tru-ols --features ...` example that turns `blas` on without editing the CLI.

### Fair comparison checklist

- Release builds (`--release`).
- Same **`FLOW_TRU_OLS_FORCE_SEQUENTIAL`**, **`RAYON_NUM_THREADS`**, and **`OMP_NUM_THREADS`** / BLAS threads across runs.
- Same panel sizes and **`TruOls::from_preprocessed`** vs **`TruOls::new`** story as in [PROFILING.md](PROFILING.md) so preprocessing is not double-counted.

---

## Quick reference: commands that failed vs working forms


| You typed                         | Fix                                                                                         |
| --------------------------------- | ------------------------------------------------------------------------------------------- |
| `LinearAlgebra.BLAS.get_config()` | `using LinearAlgebra` first, then `BLAS.get_config()`                                       |
| `OpenBLAS_jll`                    | `using OpenBLAS_jll` **if** the package is in the environment; else use `BLAS.get_config()` |
| `Pkg.status()`                    | `using Pkg` first                                                                           |


