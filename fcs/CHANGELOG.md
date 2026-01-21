# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 (2026-01-21)

<csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/>
<csr-id-2d2660406806bdb259dbf66fefa3576fa1a611f3/>

### Refactor (BREAKING)

 - <csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/> remove GPU acceleration implementation
   - Remove GPU module and all GPU-related code
   - Remove GPU dependencies (burn, cubecl, bytemuck)
   - Remove GPU feature flags from Cargo.toml
   - Update batch functions to use CPU-only implementation

### Refactor

 - <csr-id-2d2660406806bdb259dbf66fefa3576fa1a611f3/> remove GPU acceleration implementation
   - Remove GPU module and all GPU-related code
   - Remove GPU dependencies (burn, cubecl, cubecl-wgpu)
   - Remove GPU feature flags from Cargo.toml
   - Reorganize matrix operations into dedicated matrix module
   - Update benchmarks to use CPU-only MatrixOps API
   - Add GPU_BENCHMARKING.md documenting benchmark results
   
   Benchmarks showed CPU implementations are 1.2-21× faster for typical
   flow cytometry workloads due to GPU transfer overhead and kernel launch
   costs. See GPU_BENCHMARKING.md for detailed analysis.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.0 ([`f2fc722`](https://github.com/jrmoynihan/flow/commit/f2fc72250da69b63cacdea28f561db60732c0a39))
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Remove GPU acceleration implementation ([`2d26604`](https://github.com/jrmoynihan/flow/commit/2d2660406806bdb259dbf66fefa3576fa1a611f3))
    - Remove GPU acceleration implementation ([`fec1c6d`](https://github.com/jrmoynihan/flow/commit/fec1c6d2c50730d98771b7cdc101bad5071baf29))
    - Release flow-fcs v0.1.6 ([`bd1ebad`](https://github.com/jrmoynihan/flow/commit/bd1ebad7b940f9c46f3e54202730b1f117a1d70b))
    - Release flow-fcs v0.1.6 ([`3343b32`](https://github.com/jrmoynihan/flow/commit/3343b32dbfeda6e2f0e1efa05c1b903bf457d5be))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`37f1d61`](https://github.com/jrmoynihan/flow/commit/37f1d61dcb790b63c2ef0ea148b4fde57a6414b2))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
</details>

## 0.1.6 (2026-01-21)

### Removed

 - Remove GPU acceleration implementations
   - Removed GPU matrix operations module (`gpu/`) after benchmarking showed CPU implementations are 1.2-21× faster for typical flow cytometry workloads
   - GPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)
   - CPU BLAS/LAPACK implementations are highly optimized for these matrix sizes
   - See `GPU_BENCHMARKING.md` for detailed benchmark results and analysis
- GPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)
- CPU BLAS/LAPACK implementations are highly optimized for these matrix sizes
- See `GPU_BENCHMARKING.md` for detailed benchmark results and analysis

### Refactor

 - Reorganize matrix operations into dedicated `matrix` module
   - Moved CPU matrix operations from `gpu/fallback` to new `matrix` module
   - Simplified codebase by removing GPU dependencies (`burn`, `cubecl`)
   - Updated benchmarks to use new `MatrixOps` API

<csr-unknown>
GPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysisGPU transfer overhead and kernel launch costs exceeded benefits for small-to-medium datasets (10K-1M events, 5-30 channels)CPU BLAS/LAPACK implementations are highly optimized for these matrix sizesSee GPU_BENCHMARKING.md for detailed benchmark results and analysis<csr-unknown/>
<csr-unknown/>

## 0.1.5 (2026-01-21)

### New Features

 - <csr-id-da12f8bdda2def063a9469ff921250a1d8a91aef/> expand parameter exports in lib.rs
   - Added EventDataFrame, EventDatum, and LabelName to the exported parameters in lib.rs for enhanced functionality.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release over the course of 1 calendar day.
 - 3 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
    - Expand parameter exports in lib.rs ([`da12f8b`](https://github.com/jrmoynihan/flow/commit/da12f8bdda2def063a9469ff921250a1d8a91aef))
</details>

## 0.1.4 (2026-01-18)

<csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/>

### Refactor

 - <csr-id-6da76b758d02b9da1abcd3052323f81992dc3fdd/> clean up unused imports and improve code readability
   - Removed unused imports from write.rs and peaks.rs.
   - Updated the loop in isolation_tree.rs to ignore unused variables for clarity.
   - Standardized string conversion in plots.rs for consistency.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Clean up unused imports and improve code readability ([`6da76b7`](https://github.com/jrmoynihan/flow/commit/6da76b758d02b9da1abcd3052323f81992dc3fdd))
</details>

## 0.1.3 (2026-01-18)

<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/>
<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>

### Chore

 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
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

### Refactor

 - <csr-id-5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7/> improve code quality and add features
   - Improve QC algorithm implementations
   - Add plot generation functionality
   - Enhance error handling
   - Update dependencies
   - Improve code organization

### Chore

 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.

### New Features

 - <csr-id-31bd355c1457beae0a9852adfc9dd1bdab7a3cf4/> add FCS file writing and modification utilities
   Add comprehensive FCS file writing capabilities to the previously read-only `flow-fcs` crate.
   
   New functions:
   - `write_fcs_file`: Write Fcs struct to disk

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 3 calendar days.
 - 4 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`f08823c`](https://github.com/jrmoynihan/flow/commit/f08823cabcae5223efe4250471dd75ea7fcaa936))
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Improve code quality and add features ([`5bd48e4`](https://github.com/jrmoynihan/flow/commit/5bd48e4049f6afc1539dc0a23d41d0d0f98ee6f7))
    - Add FCS file writing and modification utilities ([`31bd355`](https://github.com/jrmoynihan/flow/commit/31bd355c1457beae0a9852adfc9dd1bdab7a3cf4))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
</details>

## 0.1.2 (2026-01-13)

<csr-id-9c44f94e6b8e0236a47361a7dc7156b90d25f37c/>
<csr-id-f64872e441add42bc9d19280d4411df628ff853e/>
<csr-id-661e8e00088c6bee38bc02a8a2830f284cd49ac4/>
<csr-id-2fc9efdd0a9bfeadd0613dd309d811067acc709f/>
<csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/>
<csr-id-037f74e0e364ebfc8d68cf672dca0f758a3f2952/>
<csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/>

### Chore

 - <csr-id-9c44f94e6b8e0236a47361a7dc7156b90d25f37c/> bump version number in Cargo.toml for flow-fcs

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

### Chore

 - <csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Chore

 - <csr-id-2fc9efdd0a9bfeadd0613dd309d811067acc709f/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, new features, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Added new FCS specification PDF and example QC plot to documentation.
   - Refactored folder names and updated test module imports for better organization and error handling.

### Documentation

 - <csr-id-42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f/> add FCS specification PDF and example QC plot
   - Add FCS 3.1 implementation guidance PDF for reference

### New Features

<csr-id-590dfaa8e0c551591ea3b2ff98f893df34f6251c/>
<csr-id-c92c76434e9a2bf957040821c246eaef261e80f8/>

 - <csr-id-4d234b204ade5acd6f1cf1f87c36c5e709fd2d4a/> improve FCS file parsing, keyword handling, and transforms
   - Enhance file parsing with better error handling

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.
 - <csr-id-661e8e00088c6bee38bc02a8a2830f284cd49ac4/> update test module imports and function signatures
   - Refactored import paths in the polars_tests module to streamline access to parameters and keywords.
   - Updated the create_test_fcs function signature to return a Result with a boxed error type for better error handling.
   - Consolidated related imports.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 20 commits contributed to the release over the course of 5 calendar days.
 - 5 days passed between releases.
 - 11 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`3292c46`](https://github.com/jrmoynihan/flow/commit/3292c46b282d226aa48c2a83bc17c50896bb8341))
    - Update CHANGELOG for upcoming release ([`037f74e`](https://github.com/jrmoynihan/flow/commit/037f74e0e364ebfc8d68cf672dca0f758a3f2952))
    - Update CHANGELOG for upcoming release ([`621d3ad`](https://github.com/jrmoynihan/flow/commit/621d3aded59ff51f953c6acdb75027c4541a8b97))
    - Update CHANGELOG for upcoming release ([`2fc9efd`](https://github.com/jrmoynihan/flow/commit/2fc9efdd0a9bfeadd0613dd309d811067acc709f))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Add FCS specification PDF and example QC plot ([`42a6b5d`](https://github.com/jrmoynihan/flow/commit/42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f))
    - Improve FCS file parsing, keyword handling, and transforms ([`4d234b2`](https://github.com/jrmoynihan/flow/commit/4d234b204ade5acd6f1cf1f87c36c5e709fd2d4a))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - :truck: Rnamed folders without the `flow-` prefix. ([`f64872e`](https://github.com/jrmoynihan/flow/commit/f64872e441add42bc9d19280d4411df628ff853e))
    - Update test module imports and function signatures ([`661e8e0`](https://github.com/jrmoynihan/flow/commit/661e8e00088c6bee38bc02a8a2830f284cd49ac4))
    - Enhance benchmarking and data parsing capabilities ([`590dfaa`](https://github.com/jrmoynihan/flow/commit/590dfaa8e0c551591ea3b2ff98f893df34f6251c))
    - Enhance FCS data handling and metadata processing ([`c92c764`](https://github.com/jrmoynihan/flow/commit/c92c76434e9a2bf957040821c246eaef261e80f8))
    - Merge branch 'main' into flow-plots ([`5977fb3`](https://github.com/jrmoynihan/flow/commit/5977fb309ee7e726e5e7cefca902278f155b79f8))
    - Merge branch 'main' into flow-plots ([`d7b6226`](https://github.com/jrmoynihan/flow/commit/d7b62269232f1bc6a8b155fd44d905e0a6233887))
    - Bump version number in Cargo.toml for flow-fcs ([`9c44f94`](https://github.com/jrmoynihan/flow/commit/9c44f94e6b8e0236a47361a7dc7156b90d25f37c))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`987314d`](https://github.com/jrmoynihan/flow/commit/987314dd1120fb723aad0946d8bfb0e882d39454))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`46431c0`](https://github.com/jrmoynihan/flow/commit/46431c0431afb4b7fa7de240595ac5726e693242))
    - Release flow-fcs v0.1.1 ([`c3413e1`](https://github.com/jrmoynihan/flow/commit/c3413e1a46a64f0a798ea0fe4d08134117a8c1ca))
</details>

## 0.1.1 (2026-01-08)

<csr-id-3691bf612ae11ac243fdcc6e3af927d2d3b3780a/>

### Refactor

 - <csr-id-3691bf612ae11ac243fdcc6e3af927d2d3b3780a/> export Transformable and Formattable traits

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.1 ([`e0e16cc`](https://github.com/jrmoynihan/flow/commit/e0e16ccaa87b5f5d8413a3eb6198257e2d052ac8))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`3d994a8`](https://github.com/jrmoynihan/flow/commit/3d994a81aa585e6d5263c5f9d1db7d36106698d2))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`708ddca`](https://github.com/jrmoynihan/flow/commit/708ddca0149fe7f5c6627e052207d78f06b55ed6))
    - Export Transformable and Formattable traits ([`3691bf6`](https://github.com/jrmoynihan/flow/commit/3691bf612ae11ac243fdcc6e3af927d2d3b3780a))
</details>

## 0.1.0 (2026-01-07)

<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-d0455271e8573fa035dab1cf9af4448b5e67373b/>
<csr-id-ae41dccd0a40e182ad251439e6191bf6f2db0aa2/>
<csr-id-ea0456e94b12e17eaea070b942e52287423e88e0/>
<csr-id-4d8fc2267ad20d7fc1ddbdea5e69549b978c1eda/>
<csr-id-eb923b039da61abb83b35f527c096aecbf84739e/>
<csr-id-9c184b0cce3e4d8a662b02ac544ea3659cde68f3/>
<csr-id-48e26f4253ec16f5d49ffbbf1b7bb34c595e2c17/>
<csr-id-7b5c00622d44ad9bd5791c7fe2f6e4aaaa57b277/>
<csr-id-ea242306bd6e5c9211c58fb15971c8277ad7abdd/>
<csr-id-9a522b748fbf62fbb2d3638dd0627c40f400acaa/>
<csr-id-d194503be414fe7b7214f65d0f6c06010a884e69/>

### Chore

 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

### Chore

 - <csr-id-d194503be414fe7b7214f65d0f6c06010a884e69/> change category tag for crates.io

### Refactor

 - <csr-id-ae41dccd0a40e182ad251439e6191bf6f2db0aa2/> update deprecated keyword documentation and parsing
   - Added `#[allow(deprecated)]` attributes to suppress warnings for deprecated keywords in `keyword/mod.rs` and `parsing.rs`.
   - Enhanced documentation for deprecated keywords to improve clarity and maintainability.
   - Ensured consistent handling of deprecated keywords in the parsing functions.
 - <csr-id-ea0456e94b12e17eaea070b942e52287423e88e0/> remove unused match arm in MixedKeyword implementation
   - Eliminated the unused match arm in the StringableKeyword implementation for MixedKeyword to enhance code clarity and maintainability.
 - <csr-id-4d8fc2267ad20d7fc1ddbdea5e69549b978c1eda/> clean up imports and remove unused code in flow-fcs
   - Removed unused imports from file.rs, header.rs, and keyword/mod.rs to enhance code clarity and maintainability.
   - Consolidated import statements for better organization and readability.
   - Added `#[allow(deprecated)]` attributes to certain enum implementations in keyword/mod.rs to suppress warnings for deprecated features.
 - <csr-id-eb923b039da61abb83b35f527c096aecbf84739e/> remove ColumnStore struct and related methods from file.rs
   - Deleted the ColumnStore struct and its associated methods, which were previously used for managing column-oriented data storage for FCS files.
   - This change simplifies the codebase by removing unused functionality, streamlining the file handling process.
 - <csr-id-9c184b0cce3e4d8a662b02ac544ea3659cde68f3/> add unused attribute to traits and functions for clarity
   - Added `#[allow(unused)]` attribute to the `validate_number_of_parameters` function in `metadata.rs` to suppress warnings for unused code.
   - Introduced `#[allow(unused)]` to the `Transformable` and `Formattable` traits in `transform.rs` to indicate potential future use.
   - Added `#[allow(unused)]` to the `FloatableKeyword` trait in `keyword/mod.rs` to clarify its intended future implementation.
 - <csr-id-48e26f4253ec16f5d49ffbbf1b7bb34c595e2c17/> rename tests to reflect suffix extraction changes
   - Updated test function names to align with the new `extract_parameter_suffix` function.
   - Simplified tests by removing unnecessary assertions related to parameter numbers.
   - Ensured consistency in testing invalid inputs for suffix extraction.
 - <csr-id-7b5c00622d44ad9bd5791c7fe2f6e4aaaa57b277/> simplify parameter keyword handling in flow-fcs
   - Renamed `extract_parameter_parts` to `extract_parameter_suffix` to focus on suffix extraction.
   - Consolidated logic for checking parameter keywords into a single function using known prefixes.
   - Updated documentation to reflect changes in parameter keyword handling and improved clarity.
   - Enhanced error handling in parsing functions to return `UnableToParse` for invalid inputs.
 - <csr-id-ea242306bd6e5c9211c58fb15971c8277ad7abdd/> remove unnecessary cloning of channel and label names in FCS builder

### Chore

 - <csr-id-9a522b748fbf62fbb2d3638dd0627c40f400acaa/> update dependencies to use memmap3 and add lazy_static
   - Replaced `memmap2` with `memmap3` in Cargo.toml and flow-fcs/Cargo.toml for improved safety.
   - Added `lazy_static` as a dependency in Cargo.lock.
   - Updated file.rs to utilize `memmap3` with enhanced safety guarantees.

### Documentation

 - <csr-id-3014b0af9cac746cf8728a33d4bf7fd0a1124ec0/> added root readme ad updated flow-fcs readme
 - <csr-id-e63e03c98834a3280be7d2f3f32fb4fe93272d53/> :memo: Added a changelog
   Used cargo smart-release to generate a changelog
 - <csr-id-8c420b9f03ce918f7c7e710f622073c66ed0bc64/> :memo: Update changelog

### Chore

 - <csr-id-d0455271e8573fa035dab1cf9af4448b5e67373b/> add script metadata for automated release and changelog generation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 18 commits contributed to the release.
 - 15 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.1.0 ([`18ab133`](https://github.com/jrmoynihan/flow/commit/18ab1338cacc10f8856409097bca33ce1914f248))
    - Change category tag for crates.io ([`d194503`](https://github.com/jrmoynihan/flow/commit/d194503be414fe7b7214f65d0f6c06010a884e69))
    - :memo: Update changelog ([`8c420b9`](https://github.com/jrmoynihan/flow/commit/8c420b9f03ce918f7c7e710f622073c66ed0bc64))
    - Update deprecated keyword documentation and parsing ([`ae41dcc`](https://github.com/jrmoynihan/flow/commit/ae41dccd0a40e182ad251439e6191bf6f2db0aa2))
    - Remove unused match arm in MixedKeyword implementation ([`ea0456e`](https://github.com/jrmoynihan/flow/commit/ea0456e94b12e17eaea070b942e52287423e88e0))
    - Clean up imports and remove unused code in flow-fcs ([`4d8fc22`](https://github.com/jrmoynihan/flow/commit/4d8fc2267ad20d7fc1ddbdea5e69549b978c1eda))
    - Remove ColumnStore struct and related methods from file.rs ([`eb923b0`](https://github.com/jrmoynihan/flow/commit/eb923b039da61abb83b35f527c096aecbf84739e))
    - Update dependencies to use memmap3 and add lazy_static ([`9a522b7`](https://github.com/jrmoynihan/flow/commit/9a522b748fbf62fbb2d3638dd0627c40f400acaa))
    - Add unused attribute to traits and functions for clarity ([`9c184b0`](https://github.com/jrmoynihan/flow/commit/9c184b0cce3e4d8a662b02ac544ea3659cde68f3))
    - Rename tests to reflect suffix extraction changes ([`48e26f4`](https://github.com/jrmoynihan/flow/commit/48e26f4253ec16f5d49ffbbf1b7bb34c595e2c17))
    - Simplify parameter keyword handling in flow-fcs ([`7b5c006`](https://github.com/jrmoynihan/flow/commit/7b5c00622d44ad9bd5791c7fe2f6e4aaaa57b277))
    - Remove unnecessary cloning of channel and label names in FCS builder ([`ea24230`](https://github.com/jrmoynihan/flow/commit/ea242306bd6e5c9211c58fb15971c8277ad7abdd))
    - Reduce keywords to satisfy crates.io ([`343ec47`](https://github.com/jrmoynihan/flow/commit/343ec47bd3bc81aa0c35e068db8af7d71d9bf71b))
    - Update CHANGELOG.md to reflect recent changes, including added documentation for root and flow-fcs readme, automated release script metadata, and a generated changelog. Consolidated commit statistics to show contributions from multiple commits. ([`1879470`](https://github.com/jrmoynihan/flow/commit/1879470acab8a43fcdde844938a6bb67688a4666))
    - Add script metadata for automated release and changelog generation ([`d045527`](https://github.com/jrmoynihan/flow/commit/d0455271e8573fa035dab1cf9af4448b5e67373b))
    - Added root readme ad updated flow-fcs readme ([`3014b0a`](https://github.com/jrmoynihan/flow/commit/3014b0af9cac746cf8728a33d4bf7fd0a1124ec0))
    - :memo: Added a changelog ([`e63e03c`](https://github.com/jrmoynihan/flow/commit/e63e03c98834a3280be7d2f3f32fb4fe93272d53))
    - Reorganize workspace into separate crates ([`fd12ce3`](https://github.com/jrmoynihan/flow/commit/fd12ce3ff00c02e75c9ea84848adb58b32c4d66f))
</details>

