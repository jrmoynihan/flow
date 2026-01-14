# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.0 (2026-01-14)

<csr-id-32d70dc9741a8b5867d784f9e0cfa5f17929cb8c/>
<csr-id-94407a5e6cd66bb753c89c0fbb24c4e026056f35/>

### Chore

 - <csr-id-32d70dc9741a8b5867d784f9e0cfa5f17929cb8c/> update dependency paths in Cargo.toml for peacoqc-cli
   - Changed flow-fcs and peacoqc-rs dependencies to use relative paths and specified versions for better clarity and organization.
 - <csr-id-94407a5e6cd66bb753c89c0fbb24c4e026056f35/> update flow-fcs dependency version in Cargo.toml
   - Changed flow-fcs dependency version from 0.1.0 to 0.1.1 to ensure compatibility with recent updates.

### Chore

 - <csr-id-3292c46b282d226aa48c2a83bc17c50896bb8341/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.
   - Added comprehensive documentation and R helper functions for improved usability.

### New Features

<csr-id-2fb16ca7aab98434c34bd7773295fb6d0b17a8ad/>
<csr-id-395b447bc519ac50168a68589732aace860afc8d/>

 - <csr-id-4a17968a01a3fe08707df80d015650cd3abbb722/> add interactive plot generation to CLI
   - Add --plots and --plot-dir CLI options for plot generation
- Implement interactive prompts using dialoguer crate
- Prompt user to confirm plot generation (default: yes)
- Prompt for plot directory with default to input file directory
- Generate QC plots after successful QC processing
- Store FCS data and QC results during processing for plot generation
- Add new peacoqc-cli crate for command-line interface
- Implement parallel file processing with rayon
- Add comprehensive CLI options and flags
- Support single file, multiple files, and directory processing
- Add JSON report generation
- Include verbose output and progress reporting
- Introduced a new command-line tool `peacoqc-cli` for performing quality control on flow cytometry FCS files.
- Implemented argument parsing using `clap` for user input.
- Added functionality for loading FCS files, removing margins and doublets, and running PeacoQC analysis.
- Included options for saving cleaned FCS files and generating JSON reports.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 12 commits contributed to the release over the course of 7 calendar days.
 - 6 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release peacoqc-rs v0.1.0 ([`ae4bc91`](https://github.com/jrmoynihan/flow/commit/ae4bc91414dde199edfdac0965c9df44e9036f2f))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`3292c46`](https://github.com/jrmoynihan/flow/commit/3292c46b282d226aa48c2a83bc17c50896bb8341))
    - Merge pull request #7 from jrmoynihan/feat/cli-plot-generation ([`e0cd286`](https://github.com/jrmoynihan/flow/commit/e0cd286f9faa58d264eb27cc6dc6b57958389f78))
    - Add interactive plot generation to CLI ([`4a17968`](https://github.com/jrmoynihan/flow/commit/4a17968a01a3fe08707df80d015650cd3abbb722))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge pull request #5 from jrmoynihan/peacoqc-rs ([`198f659`](https://github.com/jrmoynihan/flow/commit/198f659aed1a8ad7a362ebcfc615e1983c6a4ade))
    - Implement CLI tool with parallel processing ([`2fb16ca`](https://github.com/jrmoynihan/flow/commit/2fb16ca7aab98434c34bd7773295fb6d0b17a8ad))
    - Update dependency paths in Cargo.toml for peacoqc-cli ([`32d70dc`](https://github.com/jrmoynihan/flow/commit/32d70dc9741a8b5867d784f9e0cfa5f17929cb8c))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - Update flow-fcs dependency version in Cargo.toml ([`94407a5`](https://github.com/jrmoynihan/flow/commit/94407a5e6cd66bb753c89c0fbb24c4e026056f35))
    - Add peacoqc-cli for flow cytometry quality control ([`395b447`](https://github.com/jrmoynihan/flow/commit/395b447bc519ac50168a68589732aace860afc8d))
</details>

<csr-unknown>
 implement CLI tool with parallel processing add peacoqc-cli for flow cytometry quality control<csr-unknown/>

