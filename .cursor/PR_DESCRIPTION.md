# Merge gpu-acceleration into main

Fixes #11. Fixes #12.

## Summary

This PR merges 70 commits from `gpu-acceleration` into `main`, bringing major feature additions across the flow-crates workspace. Despite the branch name, this merge includes the removal of GPU acceleration (replaced with optimized CPU/faer-based implementations) plus substantial new functionality.

## Crates Affected

| Crate            | Current (main) | After Merge | Changes                                                                         |
| ---------------- | -------------- | ----------- | ------------------------------------------------------------------------------- |
| flow-fcs         | 0.2.0          | 0.2.1       | Polars 0.51→0.53 migration, faer matrix ops, metadata improvements, FCS write   |
| flow-plots       | 0.2.0          | 0.2.1       | Spectral plots, signal heatmap                                                  |
| flow-gates       | 0.2.0          | 0.2.1       | Automated gating, hierarchy, doublet detection, clustering-based scatter gating |
| flow-utils       | (new)          | 0.1.0       | KDE, clustering (K-means, GMM, DBSCAN), PCA modules                             |
| peacoqc-rs       | 0.1.x          | 0.2.0       | flow-fcs dependency bump                                                        |
| peacoqc-cli      | 0.1.x          | 0.2.0       | flow-fcs dependency bump                                                        |
| flow-tru-ols     | (new)          | 0.1.0       | Unmixing, preprocessing, FCS integration, plotting                              |
| flow-tru-ols-cli | (new)          | 0.1.0       | Peak detection, synthetic data, spectral unmixing                               |

## Key Changes by Crate

### flow-fcs

- Replace ndarray with faer for matrix operations
- Polars 0.51 → 0.53 migration (DataFrame creation, series replacement)
- Fix `$PnDATATYPE` and `$PnR` keyword parsing per FCS 3.2 spec
- Improved metadata handling and FCS write support

### flow-plots

- Add spectral plots and signal heatmap
- Plotters backend improvements

### flow-gates

- Automated scatter gating module
- Doublet detection with comparison
- Clustering-based gating (K-means, GMM)
- Gate hierarchy support, boolean gates, GatingML enhancements
- Synthetic data generation with Gaussian distributions
- Visualization examples

### flow-utils (new crate)

- 2D KDE for density contours
- Clustering: K-means, GMM, DBSCAN (linfa integration)
- PCA module with SVD
- Peak detection helpers

### flow-tru-ols (new crate)

- Spectral unmixing
- Preprocessing pipeline
- FCS integration
- Plotting support

### flow-tru-ols-cli (new crate)

- Peak detection integration
- Synthetic data generation
- Automated gating (Task 3.1)
- Peak biasing, negative event extraction

## Dependency Updates

- Remove OpenBLAS; add optional blas feature
- Workspace ndarray alignment
- faer/faer-ext for linear algebra

## Post-Merge Release Plan

After this PR is merged, run:

```bash
cargo smart-release flow-fcs flow-plots flow-gates flow-utils peacoqc-rs peacoqc-cli flow-tru-ols flow-tru-ols-cli --update-crates-index --execute
```

Dry-run has been performed; version bumps and dependency updates are as shown above.

## Checklist

- [x] Dry-run completed
- [ ] CI passes
- [ ] Changelogs polished (run `cargo changelog --write <crate>` per crate)
- [ ] READMEs updated with new versions
- [ ] Release executed after merge
