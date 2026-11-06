# FlowSOM performance notes (`flow-clustering`, feature `som`)

Synthetic Criterion grid. Method: Van Gassen et al., *Cytometry Part A*
87(7):636–645 (2015), doi:[10.1002/cyto.a.22625](https://doi.org/10.1002/cyto.a.22625).

Optional paper CyTOF staging (Weber FR-FCM-ZZPH) is documented in
[`comparison-with-parc-paper.md`](comparison-with-parc-paper.md); not required
for the synthetic numbers below.

## How to reproduce

```bash
cargo bench -p flow-clustering --bench flowsom_throughput
```

Default config: 10×10 SOM, 10 epochs, 10 metaclusters, seed 42.

## Synthetic grid (Apple M5 Max, 2026-11-06)

`rustc` 1.98.1; Criterion medians (`flowsom_e2e`).

| Case | Median | Throughput |
|------|--------|------------|
| 20k × 10 | 238 ms | 83.9k evt/s |
| 20k × 30 | 494 ms | 40.5k evt/s |
| 50k × 20 | 858 ms | 58.3k evt/s |
| 100k × 20 | 1.68 s | 59.5k evt/s |
| 100k × 30 | 2.36 s | 42.3k evt/s |

Throughput falls with feature count (more work per BMU distance) and scales
roughly with event count at fixed *d*.
