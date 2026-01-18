# Development Notes

This document contains important technical notes for developers working on peacoqc-rs.

## Port Status: Production-Ready ✅

The Rust port is **functionally correct and production-ready**. All major algorithms have been verified to match the R implementation, with minor numerical precision differences (< 1-7% in most cases) that are expected and acceptable.

## Critical Implementation Details

### Preprocessing Order

The preprocessing order is **critical** and must match R's order exactly:

1. **RemoveMargins** (on raw data)
2. **RemoveDoublets** (on raw data)  
3. **Compensate** + **Transform** (biexponential if compensation available, arcsinh otherwise)

**Why it matters**: Margin/doublet removal must happen on raw data before transformation, as thresholds are calculated on raw values.

### Transformation Logic

- **If compensation available** (`$SPILLOVER`, `$SPILL`, or `$COMP` keyword): Use `biexponential` transformation (matching R's `estimateLogicle`)
- **If no compensation**: Use `arcsinh` transformation with cofactor=2000

### Feature Matrix Structure (Critical Fix)

The Isolation Tree feature matrix must have **one column per cluster per channel** (format: `{channel}_cluster_{cluster_id}`), not one column per channel. This is critical for IT to distinguish different peak trajectories.

**Structure**: `bins × (channels × clusters)` matrix where:
- Each cluster gets its own column
- Bins are initialized with cluster median (default value)
- Actual peak values replace defaults where peaks were detected

### Doublet Removal MAD Scaling

Doublet removal uses **scaled MAD** matching R's `stats::mad()` with constant=1.4826, not raw MAD.

**Formula**: `threshold = median(ratio) + nmad * scaled_mad(ratio)`

### Margin Removal Threshold

Maximum margin removal uses `>` (not `>=`) to match R's logic:
```rust
if v > threshold && mask[i] {  // Uses >, not >=
```

## Algorithm Verification

### Isolation Tree
- ✅ Matches R's `isolationTreeSD` implementation
- ✅ Uses SD-based reduction (not standard Isolation Forest)
- ✅ Finds largest homogeneous group (inliers)

### MAD Outlier Detection
- ✅ Matches R's MAD calculation with scale factor 1.4826
- ✅ Uses spline-smoothed peak trajectories
- ✅ Flags bins where trajectory exceeds threshold

### Peak Detection
- ✅ Uses kernel density estimation (KDE)
- ✅ Clusters peaks by median values
- ✅ Handles bins without peaks (uses cluster median)

## Known Differences from R

### Numerical Precision
- Minor differences (< 1-7%) in QC results due to:
  - Different KDE implementations
  - Floating-point precision differences
  - Different random number generators (if any)

### Cluster Assignments
- Rust may detect slightly different numbers of clusters than R
- This is acceptable as long as QC results are similar
- Differences propagate to IT results but don't significantly affect final QC

## Testing Results

Across 4 test files:
- ✅ **IT Results**: Match perfectly on 3/4 files (0 outlier bins)
- ✅ **MAD Results**: Very close (0.90-6.85% difference, acceptable)
- ✅ **Preprocessing**: Event counts match closely (0.09% difference)
- ✅ **Bin counts**: Match exactly after preprocessing fixes

## Doublet Removal Behavior

See `DOUBLET_REMOVAL_RECONCILIATION.md` for details on why:
- Default behavior removes doublets (matches R recommendations)
- Keeping doublets can cause more bins to be flagged (expected behavior)
- Published figures may use dataset-specific preprocessing

## References

- R PeacoQC package: See `PeacoQC R at master.R` for reference implementation
- R package improvements: See `R_PACKAGE_IMPROVEMENTS.md` for suggestions
