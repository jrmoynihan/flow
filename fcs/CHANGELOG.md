# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Chore

 - <csr-id-9c44f94e6b8e0236a47361a7dc7156b90d25f37c/> bump version number in Cargo.toml for flow-fcs

### Documentation

 - <csr-id-42a6b5d7214e1ecc6fbad2c74572f9974c4f6a9f/> add FCS specification PDF and example QC plot
   - Add FCS 3.1 implementation guidance PDF for reference
   - Add example QC plot output image

### New Features

 - <csr-id-4d234b204ade5acd6f1cf1f87c36c5e709fd2d4a/> improve FCS file parsing, keyword handling, and transforms
   - Enhance file parsing with better error handling
   - Improve keyword parsing and validation
   - Add transform functionality improvements
   - Update keyword tests with additional coverage
 - <csr-id-590dfaa8e0c551591ea3b2ff98f893df34f6251c/> enhance benchmarking and data parsing capabilities
   - Introduced new benchmarking scripts for analyzing performance of dataframe parsing.
   - Added support for various data types and improved parsing efficiency in FCS data handling.
   - Implemented conditional parallelization based on dataset size to optimize performance.
   - Created a new `analyze_benchmarks` binary for analyzing benchmark results and extracting insights.
   - Updated `Cargo.toml` to include necessary dependencies for benchmarking and data processing.
 - <csr-id-c92c76434e9a2bf957040821c246eaef261e80f8/> enhance FCS data handling and metadata processing
   - Updated FcsDataType enum to include Copy trait for improved memory efficiency.
   - Refactored get_bytes_per_event method to get_bytes_for_bits, allowing dynamic byte calculation based on parameter bits.
   - Added new methods in Metadata for retrieving data types and calculating total bytes per event, improving accuracy in data handling.
   - Normalized keyword storage in metadata to ensure consistent lookups with $ prefix.
   - Enhanced parameter metadata extraction to support new FCS specifications.

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.
 - <csr-id-661e8e00088c6bee38bc02a8a2830f284cd49ac4/> update test module imports and function signatures
   - Refactored import paths in the polars_tests module to streamline access to parameters and keywords.
   - Updated the create_test_fcs function signature to return a Result with a boxed error type for better error handling.
   - Consolidated related imports.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 15 commits contributed to the release over the course of 5 calendar days.
 - 5 days passed between releases.
 - 7 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
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

