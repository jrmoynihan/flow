# Comparison with Julia Implementation

## Overview

We've created a comparison framework to validate the Rust TRU-OLS implementation against the original Julia code. This ensures algorithmic correctness and helps identify any implementation differences.

## Comparison Framework

### Created Files

1. **`tru-ols-cli/examples/compare_with_julia.rs`** - Rust example that:
   - Loads FCS files and mixing matrix
   - Runs Rust TRU-OLS
   - Exports all data and results to CSV format
   - Generates a Julia script for comparison

2. **`tru-ols-cli/examples/COMPARISON_README.md`** - Detailed usage instructions

## Key Algorithm Comparisons

### ✅ Phase 1: Preprocessing

**Cutoff Calculation**:
- **Julia**: `mean_unmix(mixmat, unstained_dataset, 0.995)` - calculates 99.5th percentile
- **Rust**: `CutoffCalculator::calculate(&mixing_matrix, &unstained_control, 0.995)` - same approach
- **Status**: ✅ Matches

**Nonspecific Observation**:
- **Julia**: `zero_baseline_mat * neg_abunds` where `zero_baseline_mat[:, end] .= 0.0` (AF column zeroed)
- **Rust**: `mixing_matrix.dot(&mean_abundances)` where `mean_abundances[AF_idx] = 0.0` (AF abundance zero)
- **Status**: ✅ Mathematically equivalent

### ✅ Phase 2: TRU-OLS Unmixing

**Iterative Endmember Removal**:
- **Julia**: `mixmat2 \ v` (backslash operator automatically uses least squares)
- **Rust**: `solve_linear_system(&current_matrix, &adjusted_observation)` (explicit least squares for overdetermined)
- **Status**: ✅ Matches (both use least squares for overdetermined systems)

**Threshold Checking**:
- **Julia**: `if unmix[j] < threshvec2[j]` - marks as irrelevant
- **Rust**: `if abundances[local_idx] < self.cutoffs[global_idx]` - same logic
- **Status**: ✅ Matches

**Autofluorescence Preservation**:
- **Julia**: No explicit check (AF is last column, handled by index)
- **Rust**: `if global_idx == self.autofluorescence_idx { continue; }` - explicit check
- **Status**: ✅ Matches (both preserve AF)

### ⚠️ Phase 3: UCM Strategy

**Unstained Control Mapping**:
- **Julia**: `mapDistribution!` function implements percentile matching
- **Rust**: `UnmixingStrategy::UnstainedControlMapping` - implemented but not yet tested
- **Status**: ⚠️ Needs validation

## Numerical Differences Expected

Small differences (< 1e-6) are expected due to:
1. **Different BLAS/LAPACK implementations**: Rust uses OpenBLAS, Julia may use MKL or OpenBLAS
2. **Floating-point rounding**: Different order of operations can cause small differences
3. **Least squares implementation**: Normal equations vs QR decomposition (both valid)

## Running the Comparison

### Step 1: Export Data from Rust

```bash
cd flow-crates/tru-ols-cli

# First, you need a mixing matrix CSV file
# You can create one from single-stain controls using the CLI, or use an existing one

# Then export data
cargo run --example compare_with_julia -- \
    synthetic_test_data/samples/FullyStained_Sample_1.fcs \
    synthetic_test_data/controls/Unstained_Control.fcs \
    <path_to_mixing_matrix.csv> \
    comparison_output/
```

### Step 2: Run Julia Comparison

```bash
cd comparison_output
julia compare_with_julia.jl
```

### Step 3: Compare Results

Compare the CSV files:
- `rust_cutoffs.csv` vs `julia_cutoffs.csv`
- `rust_nonspecific.csv` vs `julia_nonspecific.csv`
- `rust_unmixed.csv` vs `julia_unmixed.csv`

## Validation Status

| Component                     | Status | Notes                              |
| ----------------------------- | ------ | ---------------------------------- |
| Mixing Matrix Structure       | ✅      | Autofluorescence as last column    |
| Cutoff Calculation            | ✅      | 99.5th percentile matching         |
| Nonspecific Observation       | ✅      | Mathematically equivalent          |
| Least Squares Solve           | ✅      | Both handle overdetermined systems |
| Iterative Removal             | ✅      | Same logic                         |
| Autofluorescence Preservation | ✅      | Both preserve AF                   |
| UCM Strategy                  | ⚠️      | Implemented but needs testing      |

## Next Steps

1. **Run full comparison** on synthetic test data
2. **Validate UCM strategy** implementation
3. **Add automated comparison script** that calculates differences and reports
4. **Test edge cases** (all endmembers removed except AF, etc.)

## Notes

- The comparison framework exports data in CSV format for easy inspection
- Both implementations should produce very similar results (within numerical precision)
- Large differences (> 1e-3) would indicate an implementation bug
- The framework can be extended to compare on multiple datasets automatically
