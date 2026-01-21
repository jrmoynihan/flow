# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 (2026-01-21)

<csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/>

### Refactor (BREAKING)

 - <csr-id-fec1c6d2c50730d98771b7cdc101bad5071baf29/> remove GPU acceleration implementation
   - Remove GPU module and all GPU-related code
   - Remove GPU dependencies (burn, cubecl, bytemuck)
   - Remove GPU feature flags from Cargo.toml
   - Update batch functions to use CPU-only implementation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-fcs v0.2.0, safety bump 4 crates ([`cd26a89`](https://github.com/jrmoynihan/flow/commit/cd26a8970fc25dbe70c1cc9ac342b367613bcda6))
    - Remove GPU acceleration implementation ([`fec1c6d`](https://github.com/jrmoynihan/flow/commit/fec1c6d2c50730d98771b7cdc101bad5071baf29))
    - Adjusting changelogs prior to release of flow-fcs v0.1.6 ([`7fb88db`](https://github.com/jrmoynihan/flow/commit/7fb88db9ede05b317a03d367cea18a3b8b73c5a1))
</details>

## 0.1.3 (2026-01-21)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 3 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjusting changelogs prior to release of flow-fcs v0.1.5, flow-plots v0.1.3, flow-gates v0.1.2 ([`0fb3ddf`](https://github.com/jrmoynihan/flow/commit/0fb3ddfaf836bf0fb87f5f14dbe542494706f3af))
    - Adjusting changelogs prior to release of flow-fcs v0.1.5, flow-plots v0.1.3, flow-gates v0.1.2 ([`9c8f44a`](https://github.com/jrmoynihan/flow/commit/9c8f44a6b5908a262825a2daa8b3963fdea99a11))
    - Release flow-fcs v0.1.5, flow-gates v0.1.2 ([`4106abc`](https://github.com/jrmoynihan/flow/commit/4106abc5ae2d35328ec470daf9b0a9a549ebd6ba))
</details>

## 0.1.2 (2026-01-18)

<csr-id-339d07ac60343b172cd5962310abbc7899fdc770/>
<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/>

### Chore

 - <csr-id-339d07ac60343b172cd5962310abbc7899fdc770/> update categories in Cargo.toml files
   - Simplify categories in fcs and plots to remove redundant entries.
   - Change peacoqc-cli category to reflect its command-line utility nature.
   - Add algorithms category to peacoqc-rs for better classification.
 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands
 - <csr-id-a6a4ff733ae38acaec36d3327f4952d6fded3c0f/> :hammer: Add cargo scripts for testing and release management for each crate
   Granular control at the crate level.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 12 commits contributed to the release over the course of 4 calendar days.
 - 4 days passed between releases.
 - 5 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.2, flow-gates v0.1.1 ([`2c36741`](https://github.com/jrmoynihan/flow/commit/2c367411265c8385e88b2653e278bd1e2d1d2198))
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`f08823c`](https://github.com/jrmoynihan/flow/commit/f08823cabcae5223efe4250471dd75ea7fcaa936))
    - Update categories in Cargo.toml files ([`339d07a`](https://github.com/jrmoynihan/flow/commit/339d07ac60343b172cd5962310abbc7899fdc770))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - :hammer: Add cargo scripts for testing and release management for each crate ([`a6a4ff7`](https://github.com/jrmoynihan/flow/commit/a6a4ff733ae38acaec36d3327f4952d6fded3c0f))
</details>

## 0.1.1 (2026-01-14)

<csr-id-8818e480d33513c1bb724432a734b76ac57b95f9/>
<csr-id-f64872e441add42bc9d19280d4411df628ff853e/>
<csr-id-a59079c54a230e816e69cd17e309d9ff66b1bea6/>
<csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/>
<csr-id-14aae61d8d5ccf6b031c3ac9dc310dfb89a383af/>
<csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/>

### Chore

 - <csr-id-8818e480d33513c1bb724432a734b76ac57b95f9/> update plotting backend and bindings
   - Update plotters backend implementation
   - Update TypeScript bindings for pixel data

### Chore

 - <csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/> Update CHANGELOG for upcoming release
   - Documented version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization.

### Chore

 - <csr-id-14aae61d8d5ccf6b031c3ac9dc310dfb89a383af/> Update CHANGELOG for upcoming release
   - documented version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - updated plotting backend and TypeScript bindings for pixel data
   - refactored folder names for better organization

### Chore

 - <csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.
 - <csr-id-a59079c54a230e816e69cd17e309d9ff66b1bea6/> removed unused RawPixelData import

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 11 commits contributed to the release over the course of 5 calendar days.
 - 5 days passed between releases.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.1, flow-gates v0.1.0 ([`b5be6ba`](https://github.com/jrmoynihan/flow/commit/b5be6ba4e2093a8b0e972bd44265fa51b8c6be13))
    - Update CHANGELOG for upcoming release ([`f0f0ab2`](https://github.com/jrmoynihan/flow/commit/f0f0ab21b68eb1a28903957bae137f326b5a082b))
    - Update CHANGELOG for upcoming release ([`14aae61`](https://github.com/jrmoynihan/flow/commit/14aae61d8d5ccf6b031c3ac9dc310dfb89a383af))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`621d3ad`](https://github.com/jrmoynihan/flow/commit/621d3aded59ff51f953c6acdb75027c4541a8b97))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Update plotting backend and bindings ([`8818e48`](https://github.com/jrmoynihan/flow/commit/8818e480d33513c1bb724432a734b76ac57b95f9))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - :truck: Rnamed folders without the `flow-` prefix. ([`f64872e`](https://github.com/jrmoynihan/flow/commit/f64872e441add42bc9d19280d4411df628ff853e))
    - Removed unused RawPixelData import ([`a59079c`](https://github.com/jrmoynihan/flow/commit/a59079c54a230e816e69cd17e309d9ff66b1bea6))
</details>

## v0.1.0 (2026-01-08)

<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-d3588b056c11325475ed470006c2829b7d0c1528/>
<csr-id-27e7e939f80820adc297ed7193ba87f3a4e759bb/>
<csr-id-9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e/>
<csr-id-71b90a5b4f798e27fff5634048ad12a9ff57684a/>
<csr-id-94934619d4cea454e9c38cddcc8f8d6d9ffbe068/>
<csr-id-45efa1279eed93d24d598682e3c2875a5859f05a/>
<csr-id-7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c/>
<csr-id-2638feaae082a369694370c9ba633c4c0ed7f083/>
<csr-id-670c81054b4e1a4455e5050f7888e5f96f1a35cb/>
<csr-id-2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e/>
<csr-id-62ee7640139a377207b7a6b5a5590081d473b0a4/>
<csr-id-a236a374302ae611992d7cabec69f7d732c76f54/>
<csr-id-09d31bc88283911ce2856b59311f83fe2dcf5e52/>
<csr-id-f79650c2ce3161b7cc212e87a02738da9c1647a1/>
<csr-id-8fa97683337b2a912ad4ed0d835d4e066099944a/>

### Chore

 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

### Style

 - <csr-id-a236a374302ae611992d7cabec69f7d732c76f54/> :truck: Renamed density calculation module to clarify from density plot implementation

### Chore

 - <csr-id-09d31bc88283911ce2856b59311f83fe2dcf5e52/> remove outdated keywords from Cargo.toml for flow-plots
 - <csr-id-f79650c2ce3161b7cc212e87a02738da9c1647a1/> update dependencies and enhance documentation
   - Added `approx` dependency to Cargo.lock.
   - Created a comprehensive CHANGELOG.md to document notable changes and adhere to Semantic Versioning.
   - Enhanced README documentation for the flow-plots library, detailing features and usage examples.
 - <csr-id-8fa97683337b2a912ad4ed0d835d4e066099944a/> add CHANGELOG.md for project documentation
   - Created a new CHANGELOG.md file to document all notable changes to the flow-plots project.
   - The changelog follows the Keep a Changelog format and adheres to Semantic Versioning.
   - Included detailed entries for version 0.1.0, summarizing new features, refactors, and enhancements made to the library.

### Chore

 - <csr-id-62ee7640139a377207b7a6b5a5590081d473b0a4/> remove outdated keywords from Cargo.toml for flow-plots

### Refactor

 - <csr-id-45efa1279eed93d24d598682e3c2875a5859f05a/> clean up unused imports in helper and density plot files

### Other

 - <csr-id-7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c/> swap to hybrid flow-fcs dependency entry
   allows use of local path during dev and uses the specified version when publishing
 - <csr-id-2638feaae082a369694370c9ba633c4c0ed7f083/> dependency updates
   - enabled the `preset` feature for colorgrad
   - upgraded to flow-fcs 0.1.1

### Chore

 - <csr-id-670c81054b4e1a4455e5050f7888e5f96f1a35cb/> update dependencies and enhance documentation
   - Added `approx` dependency to Cargo.lock.
   - Created a comprehensive CHANGELOG.md to document notable changes and adhere to Semantic Versioning.
   - Enhanced README documentation for the flow-plots library, detailing features and usage examples.
 - <csr-id-2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e/> update flow-fcs dependency version to 0.1.1 in Cargo.toml

### Chore

 - <csr-id-94934619d4cea454e9c38cddcc8f8d6d9ffbe068/> add CHANGELOG.md for project documentation
   - Created a new CHANGELOG.md file to document all notable changes to the flow-plots project.
   - The changelog follows the Keep a Changelog format and adheres to Semantic Versioning.
   - Included detailed entries for version 0.1.0, summarizing new features, refactors, and enhancements made to the library.

### New Features

<csr-id-d807135b00ee17c86bacfebfee220c94a0f4d6bd/>
<csr-id-94d528cc854e4bad71b2cb34df240be2a9c7109d/>
<csr-id-4154d225125c80e22d560c063c679e4063369c63/>
<csr-id-2505a8f0dd3962b24712946402d753bc19e8daa5/>
<csr-id-ed202c7c543ca8a647b5668e24adba7085e94444/>
<csr-id-220489b4562ffb0afe5cb8cae623380ded34a48d/>
<csr-id-a6a1809d4cbe3da0fb712c77763148fa5f260157/>
<csr-id-54c9c93a9f7a4a8157273c467321278399d2b16c/>
<csr-id-3f2afb485498e75aa4b9c5c2b32e0c046a184011/>
<csr-id-042fa281cdc29de70599cac2286bcebf724e9a65/>
<csr-id-dacce4b785a61fc7082889ccb14fe3e76c4e582a/>
<csr-id-cc5fb636e31f055894a5f36c0472c3122b996016/>

 - <csr-id-c27cf93f445a37e318fabb882968a56775d48a8d/> add BinaryPixelChunk and RawPixelData types for optimized pixel handling and binding to frontend (TS) code
   - Introduced `BinaryPixelChunk` type for efficient data transfer, encapsulating raw RGB pixel data along with metadata for canvas rendering.

### Refactor

 - <csr-id-d3588b056c11325475ed470006c2829b7d0c1528/> reorganize plot types and remove legacy density plotting code
   - Deleted the old `plot_types.rs` file, which contained the previous implementation of density plotting.
   - Introduced a new `mod.rs` file in the `plots` directory to better structure plot-related modules and improve organization.
   - Retained the `PlotType` enum for various plot types, ensuring compatibility with existing implementations while enhancing clarity and maintainability.
 - <csr-id-27e7e939f80820adc297ed7193ba87f3a4e759bb/> update density calculation to use DensityPlotOptions
   - Replaced references to the deprecated `PlotOptions` with the new `DensityPlotOptions` struct for improved configuration of density plots.
   - Corrected y-axis scaling calculation to utilize the new axis range properties.
   - Updated color mapping to use the specified colormap from `DensityPlotOptions`, enhancing flexibility in visualization.
 - <csr-id-9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e/> remove executor module for plot job management
   - Deleted the `executor.rs` file, since it deals with application-specific runtime logic

### Style

 - <csr-id-71b90a5b4f798e27fff5634048ad12a9ff57684a/> :truck: Renamed density calculation module to clarify from density plot implementation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 41 commits contributed to the release over the course of 1 calendar day.
 - 29 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.0 ([`43f1292`](https://github.com/jrmoynihan/flow/commit/43f12921cbb9a04491c401b433e90cc7733d51f9))
    - Release flow-plots v0.1.0 ([`3f63b79`](https://github.com/jrmoynihan/flow/commit/3f63b794fbfeb219acdb1115ad12bb1ce8494b73))
    - Remove outdated keywords from Cargo.toml for flow-plots ([`62ee764`](https://github.com/jrmoynihan/flow/commit/62ee7640139a377207b7a6b5a5590081d473b0a4))
    - Remove outdated keywords from Cargo.toml for flow-plots ([`09d31bc`](https://github.com/jrmoynihan/flow/commit/09d31bc88283911ce2856b59311f83fe2dcf5e52))
    - Release flow-plots v0.1.0 ([`7d7bd39`](https://github.com/jrmoynihan/flow/commit/7d7bd396e4a6571f80c4cfb8a61096f2beee777d))
    - Release flow-plots v0.1.0 ([`e6a02b8`](https://github.com/jrmoynihan/flow/commit/e6a02b89a0e26b18f26d9f9d04a5b11530cca8e4))
    - Update dependencies and enhance documentation ([`670c810`](https://github.com/jrmoynihan/flow/commit/670c81054b4e1a4455e5050f7888e5f96f1a35cb))
    - Update dependencies and enhance documentation ([`f79650c`](https://github.com/jrmoynihan/flow/commit/f79650c2ce3161b7cc212e87a02738da9c1647a1))
    - Merge pull request #3 from jrmoynihan:flow-plots ([`91674e1`](https://github.com/jrmoynihan/flow/commit/91674e13a6dc21b9c1979d63bbaa161f28f9dc2b))
    - Merge pull request #3 from jrmoynihan:flow-plots ([`4ab8f89`](https://github.com/jrmoynihan/flow/commit/4ab8f895f8642b59274726cc7f254187e0b14602))
    - Merge branch 'main' into flow-plots ([`5977fb3`](https://github.com/jrmoynihan/flow/commit/5977fb309ee7e726e5e7cefca902278f155b79f8))
    - Merge branch 'main' into flow-plots ([`d7b6226`](https://github.com/jrmoynihan/flow/commit/d7b62269232f1bc6a8b155fd44d905e0a6233887))
    - Update flow-fcs dependency version to 0.1.1 in Cargo.toml ([`2671217`](https://github.com/jrmoynihan/flow/commit/2671217fb91ff7f8e5ad28fc9eb8bf0d4180063e))
    - Add CHANGELOG.md for project documentation ([`9493461`](https://github.com/jrmoynihan/flow/commit/94934619d4cea454e9c38cddcc8f8d6d9ffbe068))
    - Add CHANGELOG.md for project documentation ([`8fa9768`](https://github.com/jrmoynihan/flow/commit/8fa97683337b2a912ad4ed0d835d4e066099944a))
    - Clean up unused imports in helper and density plot files ([`45efa12`](https://github.com/jrmoynihan/flow/commit/45efa1279eed93d24d598682e3c2875a5859f05a))
    - Swap to hybrid flow-fcs dependency entry ([`7d23a3f`](https://github.com/jrmoynihan/flow/commit/7d23a3ffc9799c4e0faa1dcc3b8d0a46b6cb582c))
    - Dependency updates ([`2638fea`](https://github.com/jrmoynihan/flow/commit/2638feaae082a369694370c9ba633c4c0ed7f083))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`987314d`](https://github.com/jrmoynihan/flow/commit/987314dd1120fb723aad0946d8bfb0e882d39454))
    - Merge pull request #2 from jrmoynihan:flow-fcs ([`46431c0`](https://github.com/jrmoynihan/flow/commit/46431c0431afb4b7fa7de240595ac5726e693242))
    - :truck: Renamed density calculation module to clarify from density plot implementation ([`a236a37`](https://github.com/jrmoynihan/flow/commit/a236a374302ae611992d7cabec69f7d732c76f54))
    - :truck: Renamed density calculation module to clarify from density plot implementation ([`71b90a5`](https://github.com/jrmoynihan/flow/commit/71b90a5b4f798e27fff5634048ad12a9ff57684a))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`3d994a8`](https://github.com/jrmoynihan/flow/commit/3d994a81aa585e6d5263c5f9d1db7d36106698d2))
    - Merge pull request #1 from jrmoynihan:flow-plots ([`708ddca`](https://github.com/jrmoynihan/flow/commit/708ddca0149fe7f5c6627e052207d78f06b55ed6))
    - Add BinaryPixelChunk and RawPixelData types for optimized pixel handling and binding to frontend (TS) code ([`c27cf93`](https://github.com/jrmoynihan/flow/commit/c27cf93f445a37e318fabb882968a56775d48a8d))
    - Add README documentation for flow-plots library ([`d807135`](https://github.com/jrmoynihan/flow/commit/d807135b00ee17c86bacfebfee220c94a0f4d6bd))
    - Integrate colorgrad for enhanced colormap options in density plots ([`94d528c`](https://github.com/jrmoynihan/flow/commit/94d528cc854e4bad71b2cb34df240be2a9c7109d))
    - Add helper function for creating DensityPlotOptions from FCS data ([`4154d22`](https://github.com/jrmoynihan/flow/commit/4154d225125c80e22d560c063c679e4063369c63))
    - Reorganize plot types and remove legacy density plotting code ([`d3588b0`](https://github.com/jrmoynihan/flow/commit/d3588b056c11325475ed470006c2829b7d0c1528))
    - Enhance testing framework for density plots and options ([`2505a8f`](https://github.com/jrmoynihan/flow/commit/2505a8f0dd3962b24712946402d753bc19e8daa5))
    - Add ProgressInfo struct and callback type for rendering progress ([`ed202c7`](https://github.com/jrmoynihan/flow/commit/ed202c7c543ca8a647b5668e24adba7085e94444))
    - Add rendering capabilities for density plots ([`220489b`](https://github.com/jrmoynihan/flow/commit/220489b4562ffb0afe5cb8cae623380ded34a48d))
    - Implement DensityPlot for 2D density visualization ([`a6a1809`](https://github.com/jrmoynihan/flow/commit/a6a1809d4cbe3da0fb712c77763148fa5f260157))
    - Update density calculation to use DensityPlotOptions ([`27e7e93`](https://github.com/jrmoynihan/flow/commit/27e7e939f80820adc297ed7193ba87f3a4e759bb))
    - Add DensityPlotOptions struct for density plot configuration ([`54c9c93`](https://github.com/jrmoynihan/flow/commit/54c9c93a9f7a4a8157273c467321278399d2b16c))
    - Add AxisOptions struct for plot axis configuration ([`3f2afb4`](https://github.com/jrmoynihan/flow/commit/3f2afb485498e75aa4b9c5c2b32e0c046a184011))
    - Introduce Plot trait for customizable plot types ([`042fa28`](https://github.com/jrmoynihan/flow/commit/042fa281cdc29de70599cac2286bcebf724e9a65))
    - Add BasePlotOptions struct for plot configuration ([`dacce4b`](https://github.com/jrmoynihan/flow/commit/dacce4b785a61fc7082889ccb14fe3e76c4e582a))
    - Remove executor module for plot job management ([`9f7e36c`](https://github.com/jrmoynihan/flow/commit/9f7e36c139ebe5d3180d10e276f9dc6c2f98bb4e))
    - Implement density plotting with optimized pixel rendering ([`cc5fb63`](https://github.com/jrmoynihan/flow/commit/cc5fb636e31f055894a5f36c0472c3122b996016))
    - Reorganize workspace into separate crates ([`fd12ce3`](https://github.com/jrmoynihan/flow/commit/fd12ce3ff00c02e75c9ea84848adb58b32c4d66f))
</details>

