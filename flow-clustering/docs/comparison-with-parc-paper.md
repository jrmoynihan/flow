# Cytometry paper comparison (PARC + FlowSOM)

Wall-clock harness for the shared CyTOF / flow suite used by
[Weber & Robinson 2016](https://doi.org/10.1002/cyto.a.23030) (FlowRepository
**FR-FCM-ZZPH**) and by [Stassen et al. 2020](https://doi.org/10.1093/bioinformatics/btaa042)
(PARC). FlowSOM algorithm citation:
[Van Gassen et al. 2015](https://doi.org/10.1002/cyto.a.22625).

Primary publishable yardstick for **both** Rust PARC and Rust FlowSOM on these
matrices is Weber Table 3 (FlowSOM competitor times / F1) plus Stassen’s PARC
scalability narrative on Samusik_all—not Van Gassen SPADE Figure 6.

## Stage data (local only)

1. Download FR-FCM-ZZPH from FlowRepository.
2. Extract numeric event × marker matrices (arcsinh / as used in the papers) to
   CSV: one row per event, comma-separated `f64` values, **no header**.
3. Optional ground-truth labels: `{id}.labels.txt` — one integer label per line
   (same order as matrix rows). Unlabeled events may use a sentinel such as `-1`
   (skipped in F1).
4. Point the env var at the staging directory (never commit paths or matrices):

```bash
export FLOW_CLUSTERING_BENCH_DATA=/path/to/staged/fr-fcm-zzph
cargo run -p flow-clustering --release --example compare_cytometry --features parc -- \
  --out /path/to/report_dir
```

Missing `FLOW_CLUSTERING_BENCH_DATA` → the example prints a skip message and
exits 0 (CI-safe).

Manifest of expected shapes and Weber FlowSOM reference seconds:
[`cytometry_bench_manifest.toml`](cytometry_bench_manifest.toml).

## What is timed

- **PARC**: `Parc::fit` with default `ParcConfig` (HNSW k-NN, `knn = 30`).
- **FlowSOM**: `FlowSom::fit` with default `FlowSomConfig` (10×10 SOM,
  10 metaclusters, 10 epochs)—document any overrides in the report header.
- Preprocessing (FCS load, transform, marker select) is **excluded**, matching
  Stassen’s reporting convention.
- Optional `RAYON_NUM_THREADS=1` for a single-thread A/B; note both in the report.

## Interpreting results

- Paper times use different languages (R/Python), BLAS, and machines. Treat
  ratios as order-of-magnitude context, not a regression gate.
- Stassen hardware: 8× 3.6 GHz CPUs, 126 GB RAM. Record this-host notes in the
  generated markdown (see also workspace `docs/dev/PERF_HOST.md`).
- When labels are present, the harness reports Hungarian-matched mean F1
  (Weber/Stassen spirit). Rare-population datasets emphasize the rare class F1
  when a single positive label is identified in the labels file metadata is not
  available—mean F1 across matched populations is still reported.

## Outputs

Under `--out` (default: `./clustering_cytometry_bench_out`):

- `throughput_rust.json` — per-dataset wall times, evt/s, cluster counts, optional F1
- `throughput_report.md` — table vs Weber FlowSOM reference seconds
