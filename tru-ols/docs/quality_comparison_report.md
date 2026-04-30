# Quality comparison report (sample)

This file is a **reproducible sample** of TRU-OLS vs OLS **quality** metrics from `[run_comparison](../src/benchmark.rs)`. It uses **synthetic** data (fixed RNG seed); replace with real matrices for instrument-specific conclusions.

**Regenerate body** (from workspace root; re-apply this header manually or keep it in a template):

```bash
cargo run -p flow-tru-ols --no-default-features --example quality_comparison_report -- \
  --output tru-ols/docs/quality_comparison_report_body.md
```

API: `[comparison_report_markdown](../src/benchmark.rs)` (crate re-export `flow_tru_ols::comparison_report_markdown`) formats a `[ComparisonReport](../src/metrics.rs)`. The example `**quality_comparison_report**` accepts `**--json**` for the raw `serde` struct.

---

> **Note:** Synthetic mixing + observations (detectors × endmembers = 10×10), **8000** events, RNG seed **42**. For science conclusions, call `run_comparison` on real FCS-derived matrices.

# TRU-OLS vs OLS — synthetic_seed42_n8000

Events: **8000** · Detectors: **10** · Endmembers: **10**

## Summary

- **Robust SD (rSD):** TRU-OLS shows **lower** per-endmember rSD than OLS on **9 / 10** endmembers (tighter abundance spread where lower is better).
- **R² (mean / median):** OLS **1.0000 / 1.0000** vs TRU-OLS **0.9769 / 0.9820** (fit to observations using full **M**·abundances; TRU-OLS may trade some global fit for sparsity).
- **Residuals (|·| mean / max):** OLS **0.0000 / 0.0000** vs TRU-OLS **1.9191 / 32.9397**.
- **TRU-OLS active endmembers / event (median):** **8.00** of 10 (non-zero threshold in `dimensionality_metrics`).

## Per-endmember spread


| Endmember | OLS rSD   | TRU rSD   | OLS CV % | TRU CV % | OLS mean  | TRU mean  |
| --------- | --------- | --------- | -------- | -------- | --------- | --------- |
| EM0       | 41.953230 | 41.569957 | 107.2133 | 110.7843 | 30.946820 | 29.702175 |
| EM1       | 42.160722 | 42.139071 | 101.9817 | 84.4753  | 32.478128 | 34.555941 |
| EM2       | 41.950733 | 41.696443 | 100.1496 | 84.0820  | 32.989032 | 34.812581 |
| EM3       | 38.179283 | 38.094370 | 79.3775  | 74.3842  | 37.796344 | 38.014965 |
| EM4       | 45.642444 | 45.571436 | 85.1924  | 77.7056  | 42.013164 | 42.496088 |
| EM5       | 42.425374 | 42.438229 | 78.1760  | 73.6276  | 42.746259 | 42.935697 |
| EM6       | 44.623248 | 44.231322 | 84.7202  | 76.7847  | 41.589895 | 42.499441 |
| EM7       | 47.543059 | 47.382625 | 95.8961  | 82.9934  | 38.739790 | 40.001456 |
| EM8       | 39.726769 | 39.530091 | 87.4229  | 77.5895  | 36.139452 | 37.078052 |
| EM9       | 44.710868 | 44.375018 | 91.0805  | 80.7788  | 38.374228 | 39.010490 |


## Unmixing spreading error (USE, unstained control)


| Endmember | rSD full panel | rSD single-dye | USE    |
| --------- | -------------- | -------------- | ------ |
| EM0       | 0.080054       | 0.079263       | 1.0100 |
| EM1       | 0.083723       | 0.081490       | 1.0274 |
| EM2       | 0.077651       | 0.074694       | 1.0396 |
| EM3       | 0.075124       | 0.073311       | 1.0247 |
| EM4       | 0.088467       | 0.081234       | 1.0890 |
| EM5       | 0.078384       | 0.077736       | 1.0083 |
| EM6       | 0.087172       | 0.081951       | 1.0637 |
| EM7       | 0.096883       | 0.087017       | 1.1134 |
| EM8       | 0.082815       | 0.078428       | 1.0559 |
| EM9       | 0.087533       | 0.081015       | 1.0805 |


## Goodness-of-fit (full mixing matrix)

| Method | R² mean | R² median | |residual| mean | |residual| median | |residual| max |
|---|---:|---:|---:|---:|---:|
| OLS | 1.000000 | 1.000000 | 0.000000 | 0.000000 | 0.000000 |
| TRU-OLS | 0.976859 | 0.982030 | 1.919111 | 0.519351 | 32.939707 |

## How to read this sample

- **OLS** here achieves **R² ≈ 1** and **zero residuals** because the synthetic observations lie in the column space of the random **M** (consistent linear system). That is an artifact of how the toy data were built, not a guarantee on real data.
- **TRU-OLS** still uses **truncation** and **fewer effective columns per event**, so R² drops slightly and residuals vs the **full** **M** increase—that is the expected **sparsity vs global fit** trade-off.
- **rSD / CV** on **abundance columns** are the main population-spread comparison; this sample shows **lower rSD** for TRU-OLS on most endmembers and **lower CV** on many (variance reduction narrative).
- **USE** uses the **unstained control** only; values **> 1** mean full-panel unmixing inflates spread relative to single-dye unmix on that control.

