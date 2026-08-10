//! Locates the git-tracked Gating-ML compliance corpus relative to this
//! crate's manifest, so tests and benches work on any clone.
//!
//! The corpus lives one directory above the crate root (workspace-level
//! `gates/`), is checked into git, and is the only real-file fixture set the
//! FCS reader is validated against. Hardcoding absolute paths made the
//! equivalence tests machine-local: `Fcs::open` returned `Err` elsewhere and
//! the `.expect()` failed rather than skipped.
//!
//! In *this* repository the corpus is a hard requirement — it is checked in,
//! so its absence means a broken checkout and should fail loudly, which is
//! why this module's own tests assert it is present rather than skipping.
//! The skip affordance ([`is_available`], and [`files`] returning empty) exists
//! for downstream consumers that depend on `flow-fcs` without vendoring the
//! corpus.

use std::path::{Path, PathBuf};

/// Path of the Gating-ML compliance corpus directory.
///
/// `CARGO_MANIFEST_DIR` is `<workspace>/fcs`, and the corpus is a
/// workspace-level directory, hence the `..`.
#[must_use]
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("gates")
        .join("Gating-ML.v1.5.081030.Compliance-tests.081030")
        .join("List-mode Data Files")
}

/// Path of one named corpus file. Does not check that it exists — callers
/// that need a skip-if-missing guard should use [`is_available`].
#[must_use]
pub fn path(file_name: &str) -> PathBuf {
    dir().join(file_name)
}

/// Every `.fcs` file in the corpus, sorted by path.
///
/// Read from the directory rather than hardcoded so the list cannot drift
/// from what is actually checked in. Returns empty if the directory is
/// missing, which lets callers skip rather than panic.
#[must_use]
pub fn files() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("fcs"))
        })
        .collect();
    paths.sort();
    paths
}

/// True if the corpus directory is present. Use to skip corpus-backed tests
/// on a checkout that does not have it.
#[must_use]
pub fn is_available() -> bool {
    dir().is_dir()
}

#[cfg(test)]
mod tests {
    #[test]
    fn corpus_dir_resolves_relative_to_manifest() {
        let dir = super::dir();
        assert!(
            dir.is_dir(),
            "corpus directory must exist relative to CARGO_MANIFEST_DIR, got {}",
            dir.display()
        );
    }

    #[test]
    fn corpus_contains_the_ten_tracked_files() {
        let files = super::files();
        assert_eq!(
            files.len(),
            10,
            "expected the 10 git-tracked corpus files, found {}: {:?}",
            files.len(),
            files
        );
    }

    #[test]
    fn corpus_files_are_sorted_for_determinism() {
        let files = super::files();
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted, "files() must return a deterministic order");
    }

    #[test]
    fn corpus_path_joins_a_named_file() {
        let path = super::path("int-10000_events_random.fcs");
        assert!(path.is_file(), "{} should be a file", path.display());
    }
}
