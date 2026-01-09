# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.1 (2026-01-08)

### Refactor

 - <csr-id-3691bf612ae11ac243fdcc6e3af927d2d3b3780a/> export Transformable and Formattable traits

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
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

