# Benchmark and profiling artifacts

## Criterion HTML (time + throughput plots)

From the **workspace root** (`flow-crates/`), run benches **without** `--noplot`, then open in a browser:

| Report | Path (after `cargo bench`) |
|--------|----------------------------|
| All Criterion output | `target/criterion/report/index.html` |
| `ols_method_compare` group | `target/criterion/ols_method_matrix/report/index.html` |

Use `CARGO_TARGET_DIR=target` if your environment points Cargo elsewhere.

## Flame graphs (`flamegraphs/`)

### `samply` on macOS (recommended when `cargo flamegraph` fails)

```bash
cargo install samply
# from workspace root, with target dir containing the release example:
cargo build -p flow-tru-ols --no-default-features --release --example profile_hot_path
samply record -s -n -o tru-ols/benchmark_output/flamegraphs/name.json \
  ./target/release/examples/profile_hot_path MODE --n-events N --iter K
```

Load **`name.json`** at [https://profiler.firefox.com/](https://profiler.firefox.com/).

**Modes:** `tru_ols_unmix` (TRU-OLS), `normal_equations`, `ols_qr`, or `normal_equations_gpu` (build with `--features cubecl`).

### `cargo flamegraph`

May hit **XML collapse** errors after Instruments recording on some macOS versions; see parent doc [PROFILING.md](../docs/PROFILING.md).

### Expected hot stacks

- **`normal_equations`:** large **GEMM** (`observations * mixing_matrix`), **Cholesky**, **parallel `llt.solve`**.
- **`tru_ols_unmix`:** **`unmix`**, **`unmix_event`**, **`solve_linear_system`**, column subset rebuilds.

Large `.trace` files from Instruments are ignored via `.gitignore`. Committed **`*.json`** files here are small **samply** captures for smoke testing the workflow.
