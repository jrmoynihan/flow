# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.2 (2026-01-18)

<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-ccd3cb1557065cd0c1ba4637c35d937bac39c9f6/>
<csr-id-1b41cd165c4cd315e9759b437e6b4e2a2839af99/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-be95b5180e4ffe4826bcb9a3833295d35a9b7ced/>
<csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/>
<csr-id-005a1cc9bd0bef0c9354d1f16b1fa077828359a3/>

### Chore

 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-ccd3cb1557065cd0c1ba4637c35d937bac39c9f6/> add reference materials and cargo config
   - Add PeacoQC paper PDF for reference
   - Add cargo config for build settings
 - <csr-id-1b41cd165c4cd315e9759b437e6b4e2a2839af99/> remove R reference files and test artifacts
   - Remove R reference implementation files (moved to separate location)
   - Remove test plot images
   - Clean up repository for production use
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Chore

 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.

### Documentation

<csr-id-2721f1f10fe2edd01034e4bd5340dd9cc6fe6b2e/>
<csr-id-76d800d1b9a5b40c8f4628b46074320bd9e51630/>

 - <csr-id-c6e843a730bc3352229c02a60be3b167e9f2d14d/> md formatting on dev notes
 - <csr-id-06a15cc61b34171896102c8de48c275fb811e78d/> consolidate and clean up documentation
   - Create DEV_NOTES.md consolidating technical implementation details
- Update QUICK_START.md with cleaner examples
- Update README.md with improved documentation
- Add .gitignore entry for test artifacts
- Boolean CSV format description and use cases
- Numeric CSV format (R-compatible) description
- JSON metadata format with example structure
- Custom column name examples
- Integration notes for downstream tools (pandas, R, SQL)

### New Features

<csr-id-1164c5de5cd34a0806cf2b89bd87f51e905b8aed/>
<csr-id-9bfc1e2f00f85a894ae962a8a1b7bbe0bb019b10/>

 - <csr-id-d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f/> add QC plot generation functionality
   Add comprehensive QC plot generation for visualizing PeacoQC results.
 - <csr-id-b5ef7a7b3515f707310cd932617bbf32125b0690/> implement FFT-based kernel density estimation
   Replace naive O(n×m) KDE implementation with FFT-based O(n log n) version
   for significant performance improvements.
   
   - Add realfft dependency for efficient FFT operations
- Implement FFT-based convolution for KDE computation
- Update benchmarks to reflect FFT implementation
- Update performance analysis documentation
- 30-87x faster for typical use cases (1k-50k events)
- Better scaling for larger datasets
- No accuracy loss - all tests pass
- export_csv_boolean() and export_csv_boolean_with_name()
- export_csv_numeric() and export_csv_numeric_with_name()
- export_json_metadata()
- Boolean CSV (0/1 values) for general use
- Numeric CSV (2000/6000 values) for R compatibility
- JSON metadata with comprehensive QC metrics

### Other

 - <csr-id-be95b5180e4ffe4826bcb9a3833295d35a9b7ced/> :pushpin: merging cargo.toml

### Refactor

 - <csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/> improve code quality and add features
   - Improve QC algorithm implementations
   - Add plot generation functionality
   - Enhance error handling
   - Update dependencies
   - Improve code organization

### Test

 - <csr-id-005a1cc9bd0bef0c9354d1f16b1fa077828359a3/> add comprehensive test suite
   - Add regression tests for critical fixes
   - Add algorithm correctness tests
   - Add integration tests with known outputs
   - Add R compatibility tests
   - Add spline comparison tests
   - Add peak detection tests
   - Add test documentation in tests/README.md
   - Add debug utilities for QC development

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 21 commits contributed to the release over the course of 3 calendar days.
 - 3 days passed between releases.
 - 17 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - :pushpin: merging cargo.toml ([`be95b51`](https://github.com/jrmoynihan/flow/commit/be95b5180e4ffe4826bcb9a3833295d35a9b7ced))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Md formatting on dev notes ([`c6e843a`](https://github.com/jrmoynihan/flow/commit/c6e843a730bc3352229c02a60be3b167e9f2d14d))
    - Add reference materials and cargo config ([`ccd3cb1`](https://github.com/jrmoynihan/flow/commit/ccd3cb1557065cd0c1ba4637c35d937bac39c9f6))
    - Improve code quality and add features ([`5bd48e4`](https://github.com/jrmoynihan/flow/commit/5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7))
    - Add comprehensive test suite ([`005a1cc`](https://github.com/jrmoynihan/flow/commit/005a1cc9bd0bef0c9354d1f16b1fa077828359a3))
    - Remove R reference files and test artifacts ([`1b41cd1`](https://github.com/jrmoynihan/flow/commit/1b41cd165c4cd315e9759b437e6b4e2a2839af99))
    - Consolidate and clean up documentation ([`06a15cc`](https://github.com/jrmoynihan/flow/commit/06a15cc61b34171896102c8de48c275fb811e78d))
    - Add QC plot generation functionality ([`d262a61`](https://github.com/jrmoynihan/flow/commit/d262a619dbf3ed9a147a9a2e6b2fa0a729991b1f))
    - Implement FFT-based kernel density estimation ([`b5ef7a7`](https://github.com/jrmoynihan/flow/commit/b5ef7a7b3515f707310cd932617bbf32125b0690))
    - Add export formats documentation ([`2721f1f`](https://github.com/jrmoynihan/flow/commit/2721f1f10fe2edd01034e4bd5340dd9cc6fe6b2e))
    - Add convenience export methods to PeacoQCResult ([`1164c5d`](https://github.com/jrmoynihan/flow/commit/1164c5de5cd34a0806cf2b89bd87f51e905b8aed))
    - Add export module for QC results ([`9bfc1e2`](https://github.com/jrmoynihan/flow/commit/9bfc1e2f00f85a894ae962a8a1b7bbe0bb019b10))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - Update README files for peacoqc-cli and peacoqc-rs to include license information, enhance function documentation, and improve contribution guidelines. ([`76d800d`](https://github.com/jrmoynihan/flow/commit/76d800d1b9a5b40c8f4628b46074320bd9e51630))
</details>

<csr-unknown>
 add export formats documentationAdd comprehensive documentation for export formats:Includes code examples for each export format. Update README files for peacoqc-cli and peacoqc-rs to include license information, enhance function documentation, and improve contribution guidelines.Performance improvements:Benchmarks show ~48x speedup for default bin size (1k events),reducing KDE time from ~1.4ms to ~29µs per bin. add convenience export methods to PeacoQCResultAdd methods to PeacoQCResult for easier export:These methods wrap the export functions and provide a moreergonomic API for users. add export module for QC resultsAdd export functionality to support multiple output formats:<csr-unknown/>

## 0.1.1 (2026-01-14)

<csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/>

### Chore

 - <csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/> :hammer: Add cargo scripts for testing and release management for each crate
   Granular control at the crate level.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.1 ([`947c991`](https://github.com/jrmoynihan/flow/commit/947c991bff21beb7b7d60f1f637279bd86b9ab66))
    - :hammer: Add cargo scripts for testing and release management for each crate ([`a6a4ff7`](https://github.com/jrmoynihan/flow/commit/a6a4ff733ae38acaec36d3327f4952d6fded3c0f))
    - Adjusting changelogs prior to release of peacoqc-rs v0.1.1 ([`a84b627`](https://github.com/jrmoynihan/flow/commit/a84b6271257f16432464aff091fb9c34eadf16f0))
    - Release peacoqc-cli v0.1.0 ([`ee76027`](https://github.com/jrmoynihan/flow/commit/ee760271b139b2a192d7065d08063fe5ecf0ffbf))
</details>

## 0.1.0 (2026-01-14)

<csr-id-9eceaee3506dcde315676f0d99dc893acc7430b7/>
<csr-id-deb1cae21a7b99c937335413c7f0ab6ee339365c/>
<csr-id-56accd5d225e545fe0c79e84922ecc8c21272a7e/>
<csr-id-5ac6927216aefa9779c9185841c9e4b6ee12355a/>
<csr-id-1347675f8a5648b939e368949cd30f5b6ec4b379/>
<csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/>
<csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/>
<csr-id-734654c97306d477fe98eda2ed151a92c1e49050/>

### Chore

 - <csr-id-9eceaee3506dcde315676f0d99dc893acc7430b7/> remove R source files and example files
   - Deleted PeacoQC Helper Functions, Plot Helper Functions, and main R script files as they are no longer in use.
   - Removed associated example FCS file and QC plot image to clean up the repository.
 - <csr-id-deb1cae21a7b99c937335413c7f0ab6ee339365c/> remove test report JSON file
   Remove unused test_report.json file from examples directory.
 - <csr-id-56accd5d225e545fe0c79e84922ecc8c21272a7e/> remove .DS_Store files from git tracking
   - Remove macOS .DS_Store files that were previously tracked
   - These files are already in .gitignore and should not be committed

### Chore

 - <csr-id-734654c97306d477fe98eda2ed151a92c1e49050/> Update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### Chore

 - <csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### Chore

 - <csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Documentation

<csr-id-e20e140346192a329fe65bb1d669036344471a39/>

 - <csr-id-42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f/> add FCS specification PDF and example QC plot
   - Add FCS 3.1 implementation guidance PDF for reference

### New Features

<csr-id-89520c5f677d2ca74c9777765b160554cca49eb5/>
<csr-id-bcf9880c6dcff0414818a09746adf8a315d14444/>

 - <csr-id-e08f165f1b18fdee7d303db125685066f6846ac2/> add QC plotting functionality
   - Reformatted Cargo.toml for improved readability and added new dependencies `plotters` and `image` for plotting.

### Bug Fixes

 - <csr-id-1cb95844e0c987752bf9f12854f03457c26bc408/> implement dynamic grid sizing for QC plots
   - Add calculate_grid_dimensions function that creates square-ish grids

### Refactor

 - <csr-id-5ac6927216aefa9779c9185841c9e4b6ee12355a/> extract CLI functionality to separate crate
   - Remove CLI binary from peacoqc-rs/src/bin/
   - Update examples to reflect library-only usage
   - Update Cargo.toml to remove binary targets

### Test

 - <csr-id-1347675f8a5648b939e368949cd30f5b6ec4b379/> add R compatibility tests
   - Add comprehensive R compatibility test suite
   - Ensure algorithm parity with original R implementation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 20 commits contributed to the release over the course of 7 calendar days.
 - 14 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.0 ([`ae4bc91`](https://github.com/jrmoynihan/flow/commit/ae4bc91414dde199edfdac0965c9df44e9036f2f))
    - Update CHANGELOG for upcoming release ([`734654c`](https://github.com/jrmoynihan/flow/commit/734654c97306d477fe98eda2ed151a92c1e49050))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`3292c46`](https://github.com/jrmoynihan/flow/commit/3292c46b282d226aa48c2a83bc17c50896bb8341))
    - Update CHANGELOG for upcoming release ([`037f74e`](https://github.com/jrmoynihan/flow/commit/037f74e0e364ebfc8d68cf672dca0f758a3f2952))
    - Remove R source files and example files ([`9eceaee`](https://github.com/jrmoynihan/flow/commit/9eceaee3506dcde315676f0d99dc893acc7430b7))
    - Merge pull request #7 from jrmoynihan/feat/cli-plot-generation ([`e0cd286`](https://github.com/jrmoynihan/flow/commit/e0cd286f9faa58d264eb27cc6dc6b57958389f78))
    - Remove test report JSON file ([`deb1cae`](https://github.com/jrmoynihan/flow/commit/deb1cae21a7b99c937335413c7f0ab6ee339365c))
    - Implement dynamic grid sizing for QC plots ([`1cb9584`](https://github.com/jrmoynihan/flow/commit/1cb95844e0c987752bf9f12854f03457c26bc408))
    - Merge pull request #6 from jrmoynihan/flow-gates ([`dcec55b`](https://github.com/jrmoynihan/flow/commit/dcec55bc4f08bb2bd3d6db1bfe4b603a014c3beb))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Add QC plotting functionality ([`e08f165`](https://github.com/jrmoynihan/flow/commit/e08f165f1b18fdee7d303db125685066f6846ac2))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Remove .DS_Store files from git tracking ([`56accd5`](https://github.com/jrmoynihan/flow/commit/56accd5d225e545fe0c79e84922ecc8c21272a7e))
    - Add FCS specification PDF and example QC plot ([`42a6b5d`](https://github.com/jrmoynihan/flow/commit/42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f))
    - Add R compatibility tests ([`1347675`](https://github.com/jrmoynihan/flow/commit/1347675f8a5648b939e368949cd30f5b6ec4b379))
    - Add documentation, R helper functions, and update examples ([`e20e140`](https://github.com/jrmoynihan/flow/commit/e20e140346192a329fe65bb1d669036344471a39))
    - Extract CLI functionality to separate crate ([`5ac6927`](https://github.com/jrmoynihan/flow/commit/5ac6927216aefa9779c9185841c9e4b6ee12355a))
    - Refactor and improve QC algorithms ([`89520c5`](https://github.com/jrmoynihan/flow/commit/89520c5f677d2ca74c9777765b160554cca49eb5))
    - Initialize PeacoQC library for flow cytometry quality control ([`bcf9880`](https://github.com/jrmoynihan/flow/commit/bcf9880c6dcff0414818a09746adf8a315d14444))
</details>

