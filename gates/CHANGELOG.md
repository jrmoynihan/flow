# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.2 (2026-01-21)

### New Features

 - <csr-id-2b7981fa03249f2052e4078ca6b145371c1a661c/> expand error types for new features
   Add comprehensive error types to support new functionality.
   
   - Add HierarchyCycle error for cycle detection
   - Add InvalidBooleanOperation error for boolean gate validation
   - Add GateNotFound error for missing gate references
   - Add InvalidLink error for gate linking operations
   - Add CannotReparent error for hierarchy operations
   - Add InvalidSubtreeOperation error for subtree operations
   - Add EmptyOperands error for boolean operations
   - Add InvalidBuilderState error for builder validation
   - Add DuplicateGateId error for ID conflicts
   - Add helper constructors for all new error types
 - <csr-id-7018701b741c6910e89c93e21ca4249120a1eb1b/> add gate query builder and filtering helpers
   Add fluent API for querying and filtering gates by various criteria.
   
   - Add GateQuery builder with fluent API
   - Add filter_gates_by_parameters() helper
   - Add filter_gates_by_scope() helper
   - Add filter_hierarchy_by_parameters() helper
   - Support filtering by parameters, scope, and type
   - Improve documentation and examples
 - <csr-id-873cfaee2af2b444fe0cd951ed701fade83febc0/> enhance gate hierarchy with reparenting and cloning
   Add advanced hierarchy manipulation methods for reorganizing gate
   structures.
   
   - Add reparent() to move a gate to a new parent
   - Add reparent_subtree() to move entire subtrees
   - Add clone_subtree() to duplicate subtrees with new IDs
   - Add cycle detection to prevent invalid hierarchies
   - Improve error handling with specific error types
 - <csr-id-b6bf3fcdc9e7466c234ecd30b47db57abc52f643/> add boolean gate support to GatingML import/export
   Add support for serializing and deserializing boolean gates in
   GatingML format.
   
   - Add write_boolean_gate for exporting boolean gates to XML
   - Add parse_boolean_gate_v1_5 and parse_boolean_geometry_v2 for import
   - Support AND, OR, and NOT operations in GatingML
   - Replace anyhow::Result with custom GateError::Result
   - Improve error handling with custom error types
 - <csr-id-d2068182f96d737d1febfca6854ad89d84a6cbfe/> add boolean gate support
   Add support for boolean gates that combine multiple gates using
   logical operations (AND, OR, NOT).
   
   - Add BooleanOperation enum (And, Or, Not)
   - Add Boolean variant to GateGeometry with operation and operands
   - Add GateResolver trait for resolving gate IDs to gate references
   - Implement boolean gate filtering with filter_events_boolean
   - Add filter_by_gate_with_resolver for boolean gate support
   - Update EventIndex to handle boolean gates via resolver
 - <csr-id-e8455560b2f20ff0dda711f866f5eaf71d1d323d/> add gate linking system
   Add GateLinks structure for tracking gate references and reuse.
   This is separate from hierarchy - links represent gate references
   (e.g., in boolean gates), not parent-child relationships.
   
   - Add GateLinks with add_link, remove_link, get_links methods
   - Track which gates reference other gates
   - Support querying link counts and checking if gates are linked

### Refactor

 - <csr-id-e670a9216137c9a2cedde38f3e21894f280fe516/> update module structure after GPU removal
   - Remove gpu module from lib.rs
   - Update all GPU references to use batch_filtering module
   - Simplify conditional compilation by removing GPU feature flags
 - <csr-id-a0b4bcdd64294de3a0e40795c6db838cbcb18ac0/> remove GPU implementation, use CPU-only batch filtering
   - Remove all GPU code (backend, filter, kernels)
   - Create new batch_filtering module with optimized CPU implementation
   - Remove GPU dependencies (burn, cubecl) from Cargo.toml
   - Update types.rs and filtering/mod.rs to use batch_filtering directly
   - Add GPU_PERFORMANCE_FINDINGS.md documenting why GPU was removed
   - GPU was 2-10x slower than CPU at all batch sizes due to overhead
 - <csr-id-4bbcfad61b695c86b6b07173486e5580d8b9eeae/> update library exports and documentation
   Update public API exports to include new features and improve
   documentation.
   
   - Export GateLinks, GateQuery, and new filtering functions
   - Export BooleanOperation and GateBuilder
   - Export gate geometry traits (GateBounds, GateCenter, etc.)
   - Export GatingML import/export functions
   - Add ParameterSet type alias
   - Update documentation examples to be compilable
   - Fix example code formatting

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release.
 - 3 days passed between releases.
 - 9 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Update module structure after GPU removal ([`e670a92`](https://github.com/jrmoynihan/flow/commit/e670a9216137c9a2cedde38f3e21894f280fe516))
    - Remove GPU implementation, use CPU-only batch filtering ([`a0b4bcd`](https://github.com/jrmoynihan/flow/commit/a0b4bcdd64294de3a0e40795c6db838cbcb18ac0))
    - Merge pull request #9 from jrmoynihan/flow-gates ([`d6e993e`](https://github.com/jrmoynihan/flow/commit/d6e993ea8eb206c676aa0a95d01fc8cfaec882c9))
    - Update library exports and documentation ([`4bbcfad`](https://github.com/jrmoynihan/flow/commit/4bbcfad61b695c86b6b07173486e5580d8b9eeae))
    - Expand error types for new features ([`2b7981f`](https://github.com/jrmoynihan/flow/commit/2b7981fa03249f2052e4078ca6b145371c1a661c))
    - Add gate query builder and filtering helpers ([`7018701`](https://github.com/jrmoynihan/flow/commit/7018701b741c6910e89c93e21ca4249120a1eb1b))
    - Enhance gate hierarchy with reparenting and cloning ([`873cfae`](https://github.com/jrmoynihan/flow/commit/873cfaee2af2b444fe0cd951ed701fade83febc0))
    - Add boolean gate support to GatingML import/export ([`b6bf3fc`](https://github.com/jrmoynihan/flow/commit/b6bf3fcdc9e7466c234ecd30b47db57abc52f643))
    - Add boolean gate support ([`d206818`](https://github.com/jrmoynihan/flow/commit/d2068182f96d737d1febfca6854ad89d84a6cbfe))
    - Add gate linking system ([`e845556`](https://github.com/jrmoynihan/flow/commit/e8455560b2f20ff0dda711f866f5eaf71d1d323d))
</details>

## 0.1.1 (2026-01-18)

<csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/>
<csr-id-8d232b2838f65aa621a81031183d4c954d787543/>
<csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/>
<csr-id-fbbef211ba3c7f4dffa75ea7d56f65e249e72384/>

### Chore

 - <csr-id-d3aa6cdc5a806703131a3ffac63506142f052da9/> update Cargo.toml scripts and dependency versions
   - Standardize version formatting for flow-fcs dependencies across multiple Cargo.toml files.
   - Update dry-release, publish, and changelog scripts to include specific package names for clarity.
 - <csr-id-8d232b2838f65aa621a81031183d4c954d787543/> update publish command in Cargo.toml files to include --update-crates-index
 - <csr-id-4649c7af16150d05880ddab4e732e9dee374d01b/> update Cargo.toml files for consistency and improvements
   - Standardize formatting in Cargo.toml files across multiple crates
   - Update repository URLs to reflect new structure
   - Enhance keywords and categories for better discoverability
   - Ensure consistent dependency declarations and script commands

### Other

 - <csr-id-fbbef211ba3c7f4dffa75ea7d56f65e249e72384/> :arrow_up: bump quick-xml version

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 10 commits contributed to the release over the course of 4 calendar days.
 - 4 days passed between releases.
 - 4 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-plots v0.1.2, flow-gates v0.1.1 ([`2c36741`](https://github.com/jrmoynihan/flow/commit/2c367411265c8385e88b2653e278bd1e2d1d2198))
    - Release flow-fcs v0.1.4, peacoqc-rs v0.1.2 ([`140a59a`](https://github.com/jrmoynihan/flow/commit/140a59af3c1ca751672e66c9cc69708f45ac8453))
    - Release flow-fcs v0.1.3, peacoqc-rs v0.1.2 ([`607fcae`](https://github.com/jrmoynihan/flow/commit/607fcae78304d51ce8d156e82e5dba48a1b6dbfa))
    - Update Cargo.toml scripts and dependency versions ([`d3aa6cd`](https://github.com/jrmoynihan/flow/commit/d3aa6cdc5a806703131a3ffac63506142f052da9))
    - Release flow-fcs v0.1.3 ([`e79b57f`](https://github.com/jrmoynihan/flow/commit/e79b57f8fd7613fbdcc682863fef44178f14bed8))
    - Update publish command in Cargo.toml files to include --update-crates-index ([`8d232b2`](https://github.com/jrmoynihan/flow/commit/8d232b2838f65aa621a81031183d4c954d787543))
    - Merge pull request #8 from jrmoynihan/peacoqc-rs ([`fbeaab2`](https://github.com/jrmoynihan/flow/commit/fbeaab262dc1a72832dba3d6c4708bf95c941929))
    - Merge branch 'main' into peacoqc-rs ([`c52af3c`](https://github.com/jrmoynihan/flow/commit/c52af3c09ae547a7e1ce2c62e9999590314e8f97))
    - Update Cargo.toml files for consistency and improvements ([`4649c7a`](https://github.com/jrmoynihan/flow/commit/4649c7af16150d05880ddab4e732e9dee374d01b))
    - :arrow_up: bump quick-xml version ([`fbbef21`](https://github.com/jrmoynihan/flow/commit/fbbef211ba3c7f4dffa75ea7d56f65e249e72384))
</details>

## 0.1.0 (2026-01-14)

<csr-id-5f63c2c2f02f2abaa1862153743e1923c71d8d86/>
<csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/>
<csr-id-f64872e441add42bc9d19280d4411df628ff853e/>
<csr-id-d14cd7b41828c45396709071065c98d9bda5c967/>
<csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/>
<csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/>

### Chore

 - <csr-id-5f63c2c2f02f2abaa1862153743e1923c71d8d86/> add GatingML 2.0 Specification PDF for reference
 - <csr-id-fd12ce3ff00c02e75c9ea84848adb58b32c4d66f/> reorganize workspace into separate crates

### Chore

 - <csr-id-f0f0ab21b68eb1a28903957bae137f326b5a082b/> Update CHANGELOG for upcoming release
   - Documented version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization.

### Chore

 - <csr-id-621d3aded59ff51f953c6acdb75027c4541a8b97/> update CHANGELOG for upcoming release
   - Documented unreleased changes including version bump, enhancements in FCS file parsing, benchmarking capabilities, and metadata processing improvements.
   - Updated plotting backend and TypeScript bindings for pixel data.
   - Refactored folder names for better organization and removed unused imports.

### New Features

 - <csr-id-7a1233b4426b5c7b5849666b28b75a3bee19e8c7/> introduce flow-gates library for flow cytometry data analysis
   - Added core functionality for creating and managing gates, including Polygon, Rectangle, and Ellipse geometries.

### Refactor

 - <csr-id-f64872e441add42bc9d19280d4411df628ff853e/> :truck: Rnamed folders without the `flow-` prefix.
   Just shorter to type paths.  We'll keep the crates named with the `flow-` prefix when we publish.

### Test

 - <csr-id-d14cd7b41828c45396709071065c98d9bda5c967/> :white_check_mark: Add GatingML compliance test files
   Added readme, test text, fcs, and xml files to parse and validate

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 13 commits contributed to the release over the course of 7 calendar days.
 - 7 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release flow-gates v0.1.0 ([`869b4c2`](https://github.com/jrmoynihan/flow/commit/869b4c2f123ef2ebbf5a464b4453a71f35a6ad06))
    - Remove extra keywords ([`fbf2fa6`](https://github.com/jrmoynihan/flow/commit/fbf2fa66dbee6a2d6c188a8b9a7f933ca3d2929b))
    - Release flow-plots v0.1.1, flow-gates v0.1.0 ([`b5be6ba`](https://github.com/jrmoynihan/flow/commit/b5be6ba4e2093a8b0e972bd44265fa51b8c6be13))
    - Update CHANGELOG for upcoming release ([`f0f0ab2`](https://github.com/jrmoynihan/flow/commit/f0f0ab21b68eb1a28903957bae137f326b5a082b))
    - Release flow-fcs v0.1.2 ([`57f4eb7`](https://github.com/jrmoynihan/flow/commit/57f4eb7de85c2b41ef886db446f63d753c5faf05))
    - Update CHANGELOG for upcoming release ([`621d3ad`](https://github.com/jrmoynihan/flow/commit/621d3aded59ff51f953c6acdb75027c4541a8b97))
    - Merge branch 'main' into flow-gates ([`4d40ba1`](https://github.com/jrmoynihan/flow/commit/4d40ba1bfa95f9df97a3dbfcc3c22c9bf701a5dd))
    - Merge branch 'flow-gates' into main ([`c2f2d13`](https://github.com/jrmoynihan/flow/commit/c2f2d13a61854f93687cdfd2f6a1b4b12e0d9810))
    - :truck: Rnamed folders without the `flow-` prefix. ([`f64872e`](https://github.com/jrmoynihan/flow/commit/f64872e441add42bc9d19280d4411df628ff853e))
    - Introduce flow-gates library for flow cytometry data analysis ([`7a1233b`](https://github.com/jrmoynihan/flow/commit/7a1233b4426b5c7b5849666b28b75a3bee19e8c7))
    - Add GatingML 2.0 Specification PDF for reference ([`5f63c2c`](https://github.com/jrmoynihan/flow/commit/5f63c2c2f02f2abaa1862153743e1923c71d8d86))
    - :white_check_mark: Add GatingML compliance test files ([`d14cd7b`](https://github.com/jrmoynihan/flow/commit/d14cd7b41828c45396709071065c98d9bda5c967))
    - Reorganize workspace into separate crates ([`fd12ce3`](https://github.com/jrmoynihan/flow/commit/fd12ce3ff00c02e75c9ea84848adb58b32c4d66f))
</details>

