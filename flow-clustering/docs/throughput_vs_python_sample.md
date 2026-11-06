# Sample: Rust vs Python PARC throughput

Checked-in sample from a release `compare_with_python` run.

| Field | Value |
|-------|--------|
| Date | 2026-11-06 |
| Host | Apple M5 Max (arm64) |
| `rustc` | 1.98.1 (48a229cea 2026-09-01) |
| Python | 3.14 venv; `parc` 0.40, `hnswlib` 0.8.0, `leidenalg` 0.12.0 |
| Warmup / reps | 1 / 3 (sides interleaved) |
| Data | Synthetic two-cloud; clustering-only |

| n | d | k | Rust median | Rust evt/s | Python median | Python evt/s | Speedup |
|---|---|---|-------------|------------|---------------|--------------|---------|
| 5k | 20 | 20 | 0.398 s | 12.6k | 1.54 s | 3.2k | **3.9×** |
| 20k | 20 | 30 | 0.812 s | 24.6k | 3.54 s | 5.7k | **4.4×** |
| 50k | 20 | 30 | 1.29 s | 38.6k | 10.1 s | 5.0k | **7.8×** |
| 100k | 20 | 30 | 2.70 s | 37.0k | 19.5 s | 5.1k | **7.2×** |

Rust `ParcConfig::default()` (HNSW) vs PyPI `parc.PARC` with aligned defaults.
Speedup = Python median / Rust median. Cluster counts are not required to match
(different HNSW stacks). Reproduce:
[`comparison-with-python.md`](comparison-with-python.md).
