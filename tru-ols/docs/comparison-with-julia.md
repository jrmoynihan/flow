# Comparison with Julia Implementation

## Overview

We've created a comparison framework to validate the Rust TRU-OLS implementation against the original Julia code. This ensures algorithmic correctness and helps identify any implementation differences.

## Comparison Framework

### Created Files

1. **`tru-ols-cli/examples/compare_with_julia.rs`** - Rust example that:
   - Loads FCS files and mixing matrix
   - Runs Rust TRU-OLS (timed: preprocessing, `TruOls::from_preprocessed` build, `unmix` — no duplicate preprocess inside `TruOls::new`)
   - Exports all data and results to CSV format
   - Writes **`throughput_rust.json`** (wall seconds, events/s, CPU best-effort, `rustc -vV`, relevant thread env vars) and **`throughput_report.md`**
   - Generates a Julia script for comparison

2. **`tru-ols-cli/examples/COMPARISON_README.md`** - Detailed usage instructions

After **`julia compare_with_julia.jl`**, the output directory also contains **`throughput_julia.json`**, **`julia_blas_info.txt`** (full BLAS/runtime lines), and the same CSVs as before.

For manual inspection in the Julia REPL (e.g. `BLAS.get_config()`, `@code_native`), see [julia-and-blas-on-macos.md](julia-and-blas-on-macos.md).

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
1. **Different linear algebra stacks**: Default `flow-tru-ols` uses **faer** (pure Rust). Optional **`blas`** feature uses OpenBLAS-backed paths where applicable. Julia typically uses **libblastrampoline** with a BLAS/LAPACK of your install (OpenBLAS, MKL, Apple Accelerate, etc.).
2. **Floating-point rounding**: Different order of operations can cause small differences
3. **Least squares implementation**: Normal equations vs QR decomposition (both valid)

## Throughput and environment

The example records **wall-clock** times and **events/s** so you can compare Rust and Julia on the same exported inputs.

| Artifact | Contents |
| -------- | -------- |
| `throughput_rust.json` | Rust: seconds per phase (`preprocess`, `tru_ols_build_from_preprocessed`, `unmix`), throughput, event counts, `rustc -vV`, best-effort CPU string (macOS/Linux), `RAYON_NUM_THREADS` / `OMP_NUM_THREADS` / BLAS-related env if set |
| `throughput_julia.json` | Julia: `@elapsed` sections (`mean_unmix`, baseline adjust, `TRU_OLS`, optional `create_complete_dataframe`), throughput, `blas_vendor`, pointer to detail file |
| `julia_blas_info.txt` | Lines such as `BLAS.get_config()`, thread getter when available, `Sys.CPU_NAME`, `VERSION` |
| `throughput_report.md` | Short checklist for fair comparisons and embedded Rust JSON summary |

**Recording the machine and BLAS for published numbers**

1. Note **hardware** (CPU model, RAM) and **OS**; `throughput_rust.json` → `environment.cpu` is a hint (e.g. sysctl / `/proc/cpuinfo`).
2. **Rust**: default build is **faer**; if you enable `flow-tru-ols`’s `blas` feature, record that and your OpenBLAS/MKL setup.
3. **Julia**: use **`julia_blas_info.txt`** and `throughput_julia.json` → `blas_vendor`; run `versioninfo()` in the same session if you need more (optional manual note).
4. For apples-to-apples threading, align **`RAYON_NUM_THREADS`**, **`JULIA_NUM_THREADS`**, and BLAS thread variables (**`OPENBLAS_NUM_THREADS`**, **`MKL_NUM_THREADS`**, **`VECLIB_MAXIMUM_THREADS`**, etc.) across both runs on the **same** output directory.

**Mapping phases (approximate)**

- Rust **`preprocess`** ≈ Julia **`mean_unmix`** (both drive cutoffs / abundances used downstream; not identical line-by-line).
- Rust **`unmix`** ≈ Julia **`TRU_OLS`** on adjusted stained data.
- Rust **`preprocess_plus_new_plus_unmix`** ≈ Julia **`pipeline_comparable`** (`mean_unmix` + baseline/adjust + `TRU_OLS`). The Julia script also times **`create_complete_dataframe`** separately (extra work used only to fill the export dataframe).

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

### Step 3: Compare Results and Throughput

Compare the CSV files:
- `rust_cutoffs.csv` vs `julia_cutoffs.csv`
- `rust_nonspecific.csv` vs `julia_nonspecific.csv`
- `rust_unmixed.csv` vs `julia_unmixed.csv`

Compare timing sidecars: **`throughput_rust.json`** vs **`throughput_julia.json`**, and align environment notes using **`throughput_report.md`** and **`julia_blas_info.txt`**.

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
