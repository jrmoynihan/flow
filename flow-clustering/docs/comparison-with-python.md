# Rust vs Python PARC (same host)

Dual-process harness: `examples/compare_with_python.rs` +
`examples/compare_with_python.py` (ShobiStassen [`parc`](https://github.com/ShobiStassen/PARC)
on PyPI).

## Fairness checklist

1. **Clustering-only:** identical synthetic two-cloud CSV; Python **load is
   outside** the timer; Rust times `Parc::fit` on an in-memory `Array2`.
2. Defaults aligned: `knn` as in the case grid, `dist_std_local=3`,
   `jac_std_global=median`, `too_big_factor=0.4`, `small_pop=10`,
   `n_iter_leiden=5`, `random_seed=42`, HNSW `ef_construction=150`.
3. **Interleave** Rust ↔ Python per case/rep (limits session drift).
4. Warmup ≥1 and reps ≥3 for publishable rows; use `--release`.
5. Record `rustc`, Python path, `parc` / `hnswlib` / `leidenalg` versions, host.
6. Cluster counts may diverge (Rust `flow-knn` HNSW ≠ Python `hnswlib`). This
   harness is for **throughput**, not label agreement.

## Setup

```bash
python3 -m venv .venv-parc
source .venv-parc/bin/activate
pip install parc numpy hnswlib leidenalg python-igraph
export PARC_PYTHON="$(pwd)/.venv-parc/bin/python"
```

## Commands

```bash
# Smoke
cargo run -p flow-clustering --release --example compare_with_python --features parc -- \
  --python "$PARC_PYTHON" --smoke --out /tmp/parc-py-smoke --warmup 0 --reps 1

# Publishable synthetic grid (5k–100k × 20)
cargo run -p flow-clustering --release --example compare_with_python --features parc -- \
  --python "$PARC_PYTHON" --out ./clustering_parc_py_compare --warmup 1 --reps 3
```

Without `PARC_PYTHON` / `--python`, the example exits 0 (CI-safe skip).

Artifacts under `--out`: `throughput_merged.json`, `throughput_report.md`,
per-case `matrix.csv` and `py_rep*.json`.

## Sample results

Checked-in sample: [`throughput_vs_python_sample.md`](throughput_vs_python_sample.md).
Refresh README Performance from a new local run—do not invent Nx claims.
