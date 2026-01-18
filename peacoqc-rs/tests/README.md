# Test Suite Overview

This directory contains comprehensive tests for the PeacoQC Rust implementation, organized into several categories:

## Test Files

### 1. `regression_tests.rs` - Critical Regression Tests

**Purpose**: Prevent breakage of critical fixes and behaviors

**Key Tests**:

- `test_feature_matrix_structure_per_cluster` - **CRITICAL**: Verifies feature matrix has one column per cluster (not per channel). If this breaks, IT results will be wrong.
- `test_feature_matrix_cluster_median_defaults` - Verifies bins without peaks use cluster median
- `test_isolation_tree_feature_matrix` - Verifies IT receives correct feature matrix structure
- `test_mad_filters_to_it_passed_bins` - Verifies MAD only processes bins that passed IT
- `test_doublet_removal_scaled_mad` - Verifies scaled MAD is used (matches R's `stats::mad()`)
- `test_peaks_sorted_before_clustering` - Verifies peaks are sorted before clustering
- `test_feature_matrix_multiple_channels_clusters` - Tests complex multi-channel, multi-cluster scenarios
- `test_consecutive_bins_filtering` - Verifies consecutive bin filtering removes short isolated regions

**Why These Matter**: These tests cover the most critical fixes made during the port:

1. Feature matrix structure (one column per cluster) - **CRITICAL FIX**
2. Scaled MAD for doublet removal
3. Peak sorting before clustering
4. Preprocessing order

### 2. `algorithm_correctness_tests.rs` - Algorithm Correctness

**Purpose**: Verify algorithms produce mathematically correct results

**Key Tests**:

- `test_median_calculation` - Verifies median calculation (odd/even length)
- `test_mad_calculation` - Verifies MAD calculation and scaling
- `test_isolation_tree_split_selection` - Verifies IT chooses good splits
- `test_mad_threshold_calculation` - Verifies MAD threshold calculation
- `test_peak_detection_finds_maxima` - Verifies peak detection finds local maxima
- `test_binning_overlap` - Verifies binning creates correct 50% overlap
- `test_consecutive_bins_removes_short_regions` - Verifies consecutive bin filtering logic
- `test_spline_smoothing_reduces_noise` - Verifies spline smoothing reduces noise

**Why These Matter**: These tests verify the algorithms themselves are correct, independent of R comparison.

### 3. `integration_regression_tests.rs` - Integration Tests with Known Outputs

**Purpose**: Verify processing specific files produces expected results

**Key Tests** (require test files, run with `--ignored`):

- `test_flow_file_start_up_regression` - Verifies `flow_file_start_up.fcs` produces expected results (~7-15% removal)
- `test_clean_file_zero_removal` - Verifies `flow_file_low_medium_high_speed.fcs` produces 0% removal (known clean file)
- `test_preprocessing_order_regression` - Verifies preprocessing order: RemoveMargins → RemoveDoublets → Compensate → Transform
- `test_feature_matrix_structure_integration` - Verifies feature matrix structure with real data

**Why These Matter**: These tests catch regressions in the full pipeline with real data.

### 4. `r_compatibility.rs` - R Compatibility Tests

**Purpose**: Verify compatibility with R's PeacoQC implementation

**Key Tests**:

- `test_overlapping_bins_match_r` - Verifies binning matches R's `SplitWithOverlap`
- `test_mad_scale_factor` - Verifies MAD scale factor matches R's `stats::mad` constant
- `test_median_matches_r` - Verifies median calculation matches R
- `test_mad_matches_r` - Verifies MAD calculation matches R
- `test_default_parameters_match_r` - Verifies default parameters match R

**Why These Matter**: These tests ensure the Rust implementation matches R's behavior.

### 5. `test_peak_detection.rs` - Peak Detection Tests

**Purpose**: Test peak detection functionality

### 6. `spline_r_comparison.rs` - Spline Comparison Tests

**Purpose**: Compare spline smoothing with R's `stats::smooth.spline`

## Running Tests

### Run all tests

```bash
cargo test --package peacoqc-rs
```

### Run specific test file

```bash
cargo test --package peacoqc-rs --test regression_tests
cargo test --package peacoqc-rs --test algorithm_correctness_tests
```

### Run integration tests (requires test files)

```bash
cargo test --package peacoqc-rs --test integration_regression_tests -- --ignored
```

### Run with output

```bash
cargo test --package peacoqc-rs --test regression_tests -- --nocapture
```

## Test Coverage

### Critical Paths Covered

1. ✅ Feature matrix structure (one column per cluster)
2. ✅ Preprocessing order
3. ✅ Transformation logic (biexponential vs arcsinh)
4. ✅ MAD scaling (R compatibility)
5. ✅ Peak sorting and clustering
6. ✅ IT algorithm correctness
7. ✅ MAD outlier detection
8. ✅ Consecutive bin filtering
9. ✅ Binning overlap

### Edge Cases Covered

- Empty peak frames
- Empty feature matrices
- Small datasets
- Constant data
- Edge bins in consecutive filtering

## Adding New Tests

When adding new tests:

1. **Regression Tests** (`regression_tests.rs`): Add tests for critical fixes or behaviors that must not break
2. **Algorithm Tests** (`algorithm_correctness_tests.rs`): Add tests for algorithm correctness
3. **Integration Tests** (`integration_regression_tests.rs`): Add tests for full pipeline with real data
4. **R Compatibility Tests** (`r_compatibility.rs`): Add tests for R compatibility

### Test Naming Convention

- `test_<functionality>_<aspect>` - e.g., `test_feature_matrix_structure_per_cluster`
- `test_<functionality>_regression` - e.g., `test_preprocessing_order_regression`

### Test Documentation

Each test should have a doc comment explaining:

- What it tests
- Why it matters (especially for regression tests)
- Any special setup required

## Continuous Integration

These tests should be run:

- Before every commit
- In CI/CD pipeline
- Before releases

The regression tests are especially critical - if any fail, investigate immediately as they indicate a breaking change in critical functionality.
