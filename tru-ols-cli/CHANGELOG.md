# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.0 (2026-02-15)

<csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/>
<csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/>
<csr-id-70008ac39d1d08497c2f59e7fde438d0755433d3/>
<csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/>

### New Features

<csr-id-a086f6c1501996fe7eee5d3b1798f7fab924f853/>
<csr-id-85475ba5e55685cbc14b6dcea413b0a7110faf23/>

 - <csr-id-5c6c02a44bcc7abe9a79297d7b33ddbcd15e7fcb/> peak detection, synthetic data, and spectral unmixing
   - Peak detection enabled by default for single-stain control analysis
   - Automated scatter and doublet gating (optional `--auto-gate`), peak-based median for single-stain controls, CLI options `--peak-detection`, `--peak-threshold`, `--peak-bias`
   - Synthetic FCS generation example and Julia comparison utilities

### Documentation

 - <csr-id-292bd202b232c6f780a9cc7170cc1d53b443e05e/> add CLI reference and validation reports
   - Complete argument reference for `tru-ols unmix` (CLI_ARGUMENTS_REFERENCE)

### Refactor

 - <csr-id-70008ac39d1d08497c2f59e7fde438d0755433d3/> update for faer-based fcs and tru-ols APIs
   - faer-ext for ndarray↔faer conversion; `MatRef` for spectral unmixing, spill matrix, and CSV export; ndarray retained for plotting/export

### Chore

 - <csr-id-46bee42d4f28d185b38446c0d950c2579c422f43/> update dependencies and align workspace configurations
   - Workspace-wide dependency updates (e.g. polars 0.53, faer 0.24, ndarray-linalg 0.18); more dependencies use workspace references.
 - <csr-id-c987a225570c2afae480800327d0072ab4b4e4ad/> clean up unused imports and variables
   - Removed unused imports in clustering/gating modules, fixed unreachable code in DBSCAN, dropped unnecessary `mut`; general warning cleanup.
 - <csr-id-089feff624625a5ddf0b1da570e4f60b6fedf09b/> update changelogs prior to release

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release over the course of 21 calendar days.
 - 8 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.1, flow-plots v0.2.1, flow-utils v0.1.0, flow-gates v0.2.1, peacoqc-rs v0.2.0, peacoqc-cli v0.2.0, flow-tru-ols v0.1.0, flow-tru-ols-cli v0.1.0 ([`1e3ae1e`](https://github.com/jrmoynihan/flow/commit/1e3ae1e2a91b53f70120cb96987ba5a8f02dc21e))
    - Update changelogs prior to release ([`089feff`](https://github.com/jrmoynihan/flow/commit/089feff624625a5ddf0b1da570e4f60b6fedf09b))
    - Update dependencies and align workspace configurations ([`46bee42`](https://github.com/jrmoynihan/flow/commit/46bee42d4f28d185b38446c0d950c2579c422f43))
    - Update for faer-based fcs and tru-ols APIs ([`70008ac`](https://github.com/jrmoynihan/flow/commit/70008ac39d1d08497c2f59e7fde438d0755433d3))
    - Add CLI reference and validation reports ([`292bd20`](https://github.com/jrmoynihan/flow/commit/292bd202b232c6f780a9cc7170cc1d53b443e05e))
    - Peak detection, synthetic data, and spectral unmixing ([`5c6c02a`](https://github.com/jrmoynihan/flow/commit/5c6c02a44bcc7abe9a79297d7b33ddbcd15e7fcb))
    - Integrate automated gating (Task 3.1) ([`a086f6c`](https://github.com/jrmoynihan/flow/commit/a086f6c1501996fe7eee5d3b1798f7fab924f853))
    - Integrate peak detection for single-stain controls ([`85475ba`](https://github.com/jrmoynihan/flow/commit/85475ba5e55685cbc14b6dcea413b0a7110faf23))
    - Clean up unused imports and variables ([`c987a22`](https://github.com/jrmoynihan/flow/commit/c987a225570c2afae480800327d0072ab4b4e4ad))
</details>

<csr-unknown>
COMPARISON_WITH_JULIA: Rust vs Julia comparison frameworkPEAK_DETECTION_VALIDATION: peak detection validation reportVALIDATION_REPORT: algorithm validation and fixesTRU-OLS vs AutoSpectral: academic comparisonUNMIXING_RESULTS_PLATE001: Plate_001 analysis resultsSynthetic FCS data generation with known ground truthSpectral unmixing with –controls auto-detectionExamples: generate_synthetic_test_data, compare_with_julia, check_unmixedPeak detection unit testsAdd –auto-gate flag to enable automated preprocessing gatesApply scatter and doublet gates to controls before processingGate results logged (full filtering requires FCS creation API)Add flow-gates dependencyCreate comprehensive testing instructions documentAll compilation errors resolvedAdd flow-utils dependency for KDE peak detectionAdd CLI options: –peak-detection, –peak-threshold, –peak-biasImplement calculate_peak_based_median functionReplace simple median with peak-based median when enabledAdd SingleStainConfig struct for configurationFallback to simple median if peak detection fails<csr-unknown/>

