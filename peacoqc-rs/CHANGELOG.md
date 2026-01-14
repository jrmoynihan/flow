# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.1 (2026-01-14)

### Chore

 - <csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/> :hammer: Add cargo scripts for testing and release management for each crate
   Granular control at the crate level.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
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

