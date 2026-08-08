//! Per-version FCS conformance rules.
//!
//! One function per rule, each taking `(&Metadata, Version)` and pushing zero
//! or more [`Violation`]s. The uniform shape is deliberate: `Version` is
//! currently a label rather than a behaviour selector (it drives only the
//! version string in the HEADER and the required-keyword list), and the
//! follow-up work to give it real per-version dispatch needs these rules to be
//! movable into trait impls without rewriting them. Keeping them here, keyed
//! by `Version`, makes that refactor mechanical.
//!
//! # Where these run
//!
//! **Write path only.** The reader stays deliberately permissive: real vendor
//! files routinely violate their own declared version, and the `$BEGINDATA`
//! fallback in `file.rs` plus the `StringKeyword::Other` catch-all exist
//! precisely to keep such files openable. Enforcing these rules on read would
//! turn that tolerance into a class of "vendor file won't open" bugs.
//! Strictness belongs on write, where we control the bytes.

use tracing::warn;

use crate::keyword::{Keyword, StringableKeyword};
use crate::{ByteOrder, Version};
use crate::metadata::Metadata;

/// How much a rule violation matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The file is not conformant and a strict reader may reject it.
    Error,
    /// The file is readable and conformant, but uses something the version
    /// deprecates or discourages.
    Warning,
}

/// A single conformance rule violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The keyword at fault, when the rule is about one specific keyword.
    pub keyword: Option<String>,
    pub severity: Severity,
    pub message: String,
}

impl Violation {
    fn error(keyword: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            keyword: Some(keyword.into()),
            severity: Severity::Error,
            message: message.into(),
        }
    }

    fn warning(keyword: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            keyword: Some(keyword.into()),
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.keyword {
            Some(keyword) => write!(f, "{keyword}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

/// Keywords `write::serialize_metadata` emits unconditionally, whether or not
/// they appear in `Metadata::keywords`.
///
/// These are absent from in-memory metadata that was constructed rather than
/// read from a file, but they *will* be in the TEXT segment. Checking the
/// metadata without accounting for them reports missing keywords that the
/// written file actually has - and since this list happens to be exactly the
/// FCS 3.0/3.1 required set, that would mean every write of a constructed
/// file on the default version emitted a full set of false positives.
pub const WRITER_SUPPLIED_KEYWORDS: [&str; 12] = [
    "$BEGINANALYSIS",
    "$BEGINDATA",
    "$BEGINSTEXT",
    "$BYTEORD",
    "$DATATYPE",
    "$ENDANALYSIS",
    "$ENDDATA",
    "$ENDSTEXT",
    "$MODE",
    "$NEXTDATA",
    "$PAR",
    "$TOT",
];

/// Run every rule that applies to `version` and return all violations found.
///
/// Judges `metadata` exactly as it stands. Use [`check_for_write`] when the
/// question is whether the *file we are about to write* conforms, since the
/// serializer fills in keywords the metadata does not carry.
///
/// Rules are never short-circuited: a caller repairing a file wants the
/// complete list, not the first problem.
#[must_use]
pub fn check(metadata: &Metadata, version: Version) -> Vec<Violation> {
    check_assuming_present(metadata, version, &[])
}

/// As [`check`], but treats [`WRITER_SUPPLIED_KEYWORDS`] as present because
/// the serializer will supply them.
#[must_use]
pub fn check_for_write(metadata: &Metadata, version: Version) -> Vec<Violation> {
    check_assuming_present(metadata, version, &WRITER_SUPPLIED_KEYWORDS)
}

/// Shared body of [`check`] and [`check_for_write`]. `assume_present` names
/// keywords that must not be reported missing even when absent from
/// `metadata`.
#[must_use]
fn check_assuming_present(
    metadata: &Metadata,
    version: Version,
    assume_present: &[&str],
) -> Vec<Violation> {
    let mut violations = Vec::new();
    required_keywords_present(metadata, version, assume_present, &mut violations);
    byteord_is_conformant(metadata, version, &mut violations);
    mode_is_list_mode(metadata, version, &mut violations);
    pnb_is_byte_aligned(metadata, version, &mut violations);
    timestep_present_for_time_parameter(metadata, version, &mut violations);
    deprecated_keywords(metadata, version, &mut violations);
    violations
}

/// Emit each violation through `tracing` at a level matching its severity.
/// Returns true if any violation was an [`Severity::Error`].
pub fn log_violations(violations: &[Violation], version: Version, context: &str) -> bool {
    let mut any_error = false;
    for violation in violations {
        if violation.severity == Severity::Error {
            any_error = true;
        }
        warn!(
            version = %version,
            severity = ?violation.severity,
            "{context}: {violation}"
        );
    }
    any_error
}

// ---------------------------------------------------------------------------
// Keyword access
//
// These rules deliberately read the keyword map directly rather than going
// through `Metadata`'s typed accessors. Metadata read from a file has every
// keyword parsed into its specific variant, but metadata assembled in memory
// via `insert_string_keyword` is entirely `StringKeyword::Other` - so
// `get_number_of_parameters()` and friends return `Err` on perfectly ordinary
// in-memory metadata. A rule gated on those would not report a false
// violation, it would silently skip, which is the worse failure for a
// conformance check: the caller cannot tell "conformant" from "not examined".
// ---------------------------------------------------------------------------

/// A keyword's value as it would be serialized, whatever variant holds it.
///
/// `None` for structured keywords - see [`Keyword::value_str`]. No rule here
/// inspects one; `$SPILLOVER` and `$TRUOLS_MIXMAT` are checked, if at all, by
/// matching their variant.
fn value_of(keyword: &Keyword) -> Option<std::borrow::Cow<'_, str>> {
    keyword.value_str()
}

/// Every `$P<n><suffix>` keyword present, as `(n, value)`.
///
/// Driven off the keys actually present rather than `1..=$PAR`, so it works
/// whether or not `$PAR` parsed into its typed variant.
fn indexed_parameter_values<'a>(
    metadata: &'a Metadata,
    suffix: &str,
) -> impl Iterator<Item = (String, std::borrow::Cow<'a, str>)> {
    metadata.keywords.iter().filter_map(move |(key, keyword)| {
        let digits = key.strip_prefix("$P")?.strip_suffix(suffix)?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some((key.clone(), value_of(keyword)?))
    })
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// Every keyword the declared version marks required must be present.
///
/// Applies to all versions; the required set itself is what varies.
fn required_keywords_present(
    metadata: &Metadata,
    version: Version,
    assume_present: &[&str],
    out: &mut Vec<Violation>,
) {
    for keyword in version.get_required_keywords() {
        if metadata.keywords.contains_key(*keyword) || assume_present.contains(keyword) {
            continue;
        }
        out.push(Violation::error(
            *keyword,
            format!("required by {version} but not present"),
        ));
    }
}

/// `$BYTEORD` must be `1,2,3,4` or `4,3,2,1` in FCS 3.1+.
///
/// Earlier versions permit the mixed orderings (`3,4,1,2` and friends) that
/// 3.1 removed.
///
/// Checks the *value*, not the variant, because the same conformant value can
/// be stored two ways. A file read from disk parses `$BYTEORD` into
/// `ByteKeyword::BYTEORD`, but metadata assembled in memory via
/// `Metadata::insert_string_keyword` bypasses `match_and_parse_keyword`
/// entirely and lands in `StringKeyword::Other` regardless of what the value
/// says. Keying off the variant would flag those as non-conformant when
/// nothing is actually wrong with the file being written.
fn byteord_is_conformant(metadata: &Metadata, version: Version, out: &mut Vec<Violation>) {
    if !matches!(version, Version::V3_1 | Version::V3_2 | Version::V4_0) {
        return;
    }
    let Some(keyword) = metadata.keywords.get("$BYTEORD") else {
        return; // absence is the required_keywords_present rule's business
    };
    let value = match keyword {
        Keyword::Byte(byte) => byte.get_str(),
        Keyword::String(string) => string.get_str(),
        other => {
            out.push(Violation::error(
                "$BYTEORD",
                format!("expected a byte-order value, found {other:?}"),
            ));
            return;
        }
    };
    if ByteOrder::from_keyword_str(value.trim()).is_err() {
        out.push(Violation::error(
            "$BYTEORD",
            format!("{version} allows only 1,2,3,4 or 4,3,2,1, found \"{value}\""),
        ));
    }
}

/// `$MODE` must be `L` (list mode) where it is still meaningful.
///
/// FCS 3.1 deprecated the correlated (`C`) and uncorrelated (`U`) histogram
/// modes; 3.2 deprecates `$MODE` outright but still requires `L` if written.
fn mode_is_list_mode(metadata: &Metadata, version: Version, out: &mut Vec<Violation>) {
    if !matches!(version, Version::V3_1 | Version::V3_2 | Version::V4_0) {
        return;
    }
    let Some(value) = metadata.keywords.get("$MODE").and_then(value_of) else {
        return;
    };
    if value.trim() != "L" {
        out.push(Violation::error(
            "$MODE",
            format!("{version} requires list mode (\"L\"), found \"{value}\""),
        ));
    }
}

/// FCS 3.2 requires every `$PnB` to be a whole number of bytes.
///
/// 3.1 and earlier permit bit-packed widths (10-bit ADC values in a 10-bit
/// field); 3.2 removed that, so any `$PnB` not divisible by 8 makes the file
/// non-conformant even though this library can still read it.
fn pnb_is_byte_aligned(metadata: &Metadata, version: Version, out: &mut Vec<Violation>) {
    if !matches!(version, Version::V3_2 | Version::V4_0) {
        return;
    }
    let mut flagged: Vec<Violation> = indexed_parameter_values(metadata, "B")
        .filter_map(|(key, value)| {
            let bits = value.trim().parse::<usize>().ok()?;
            (bits % 8 != 0).then(|| {
                Violation::error(
                    key,
                    format!("{version} requires a whole number of bytes, found {bits} bits"),
                )
            })
        })
        .collect();
    // Map iteration order is unspecified; sort so violations are reported in
    // parameter order and the output is reproducible.
    flagged.sort_by(|a, b| a.keyword.cmp(&b.keyword));
    out.append(&mut flagged);
}

/// `$TIMESTEP` is required once a parameter is named `TIME`.
///
/// Without it the time channel's units are undefined, so any downstream
/// rate or kinetics calculation is silently meaningless rather than wrong.
fn timestep_present_for_time_parameter(
    metadata: &Metadata,
    version: Version,
    out: &mut Vec<Violation>,
) {
    if !matches!(version, Version::V3_1 | Version::V3_2 | Version::V4_0) {
        return;
    }
    if metadata.keywords.contains_key("$TIMESTEP") {
        return;
    }
    let time_channel = indexed_parameter_values(metadata, "N")
        .filter(|(_, name)| name.trim().eq_ignore_ascii_case("TIME"))
        .min_by(|(a, _), (b, _)| a.cmp(b));

    if let Some((key, name)) = time_channel {
        out.push(Violation::error(
            "$TIMESTEP",
            format!(
                "{key} is \"{name}\" but $TIMESTEP is absent, leaving the time channel's units undefined"
            ),
        ));
    }
}

/// Keywords a version deprecates. Reported as warnings: a deprecated keyword
/// still parses, and stripping it would lose information a 3.1 reader wants.
fn deprecated_keywords(metadata: &Metadata, version: Version, out: &mut Vec<Violation>) {
    /// `(keyword, what to use instead)`
    const DEPRECATED_IN_V3_2: [(&str, &str); 10] = [
        ("$GATING", "gating is out of scope for FCS 3.2"),
        ("$DATE", "$BEGINDATETIME / $ENDDATETIME (ISO 8601)"),
        ("$BTIM", "$BEGINDATETIME (ISO 8601)"),
        ("$ETIM", "$ENDDATETIME (ISO 8601)"),
        ("$PLATEID", "$CARRIERID"),
        ("$PLATENAME", "$CARRIERTYPE"),
        ("$WELLID", "$LOCATIONID"),
        ("$MODE", "list mode is now the only mode"),
        ("$COMP", "$SPILLOVER"),
        ("$UNICODE", "TEXT is UTF-8 in FCS 3.2"),
    ];

    if !matches!(version, Version::V3_2 | Version::V4_0) {
        return;
    }
    for (keyword, replacement) in DEPRECATED_IN_V3_2 {
        if metadata.keywords.contains_key(keyword) {
            out.push(Violation::warning(
                keyword,
                format!("deprecated in {version}; use {replacement}"),
            ));
        }
    }

    // `$PnP` (percent emitted) is the one indexed deprecation, so it has to be
    // found by scanning keys rather than by name.
    let mut indexed: Vec<Violation> = indexed_parameter_values(metadata, "P")
        .map(|(key, _)| Violation::warning(key, format!("deprecated in {version}")))
        .collect();
    indexed.sort_by(|a, b| a.keyword.cmp(&b.keyword));
    out.append(&mut indexed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyword::{
        IntegerKeyword, Keyword, KeywordCreationResult, StringKeyword, match_and_parse_keyword,
    };
    use std::sync::Arc;

    /// Parse a keyword the same way the reader does, so fixtures land in the
    /// same variants a real file would. `metadata.rs` inlines this match at
    /// its two call sites rather than exposing a conversion.
    fn parsed(key: &str, value: &str) -> Keyword {
        match match_and_parse_keyword(key, value) {
            KeywordCreationResult::Int(k) => Keyword::Int(k),
            KeywordCreationResult::Float(k) => Keyword::Float(k),
            KeywordCreationResult::String(k) => Keyword::String(k),
            KeywordCreationResult::Byte(k) => Keyword::Byte(k),
            KeywordCreationResult::Mixed(k) => Keyword::Mixed(k),
            KeywordCreationResult::UnableToParse => {
                Keyword::String(StringKeyword::Other(Arc::from(value)))
            }
        }
    }

    /// A minimal 3.2-conformant TEXT segment. Each test breaks exactly one
    /// thing so a violation can be attributed to the rule under test.
    fn conformant_v3_2() -> Metadata {
        let mut metadata = Metadata::new();
        metadata.delimiter = '\u{000c}';
        for (key, value) in [
            ("$BEGINDATA", "256"),
            ("$BYTEORD", "1,2,3,4"),
            ("$CYT", "Test Cytometer"),
            ("$DATATYPE", "F"),
            ("$ENDDATA", "1055"),
            ("$NEXTDATA", "0"),
            ("$PAR", "2"),
            ("$TOT", "100"),
            ("$P1N", "FSC-A"),
            ("$P2N", "FL1-A"),
        ] {
            metadata.keywords.insert(key.to_string(), parsed(key, value));
        }
        for n in 1..=2 {
            metadata
                .keywords
                .insert(format!("$P{n}B"), Keyword::Int(IntegerKeyword::PnB(32)));
        }
        metadata
    }

    fn errors(violations: &[Violation]) -> Vec<&Violation> {
        violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .collect()
    }

    fn keywords_flagged(violations: &[Violation], severity: Severity) -> Vec<String> {
        violations
            .iter()
            .filter(|v| v.severity == severity)
            .filter_map(|v| v.keyword.clone())
            .collect()
    }

    /// The serializer supplies all 12 of FCS 3.0/3.1's required keywords, so
    /// a file we are about to write on the default version can never be
    /// missing one - even though metadata built in memory carries none of the
    /// offset keywords. `check` sees them as missing; `check_for_write` must
    /// not, or every write on the default version warns about itself.
    #[test]
    fn check_for_write_credits_keywords_the_serializer_supplies() {
        let mut bare = Metadata::new();
        bare.delimiter = '\u{000c}';
        bare.insert_string_keyword("$P1N".to_string(), "FSC-A".to_string());

        let inspecting = keywords_flagged(&check(&bare, Version::V3_1), Severity::Error);
        assert_eq!(
            inspecting.len(),
            12,
            "inspecting bare metadata should report the whole 3.1 required set, got {inspecting:?}"
        );

        assert!(
            check_for_write(&bare, Version::V3_1).is_empty(),
            "the writer supplies all of them: {:?}",
            check_for_write(&bare, Version::V3_1)
        );
    }

    /// `$CYT` is the one 3.2 requirement the serializer does not invent, which
    /// is what makes upgrading a 3.1 file to 3.2 a real check rather than a
    /// formality.
    #[test]
    fn check_for_write_still_catches_missing_cyt_in_3_2() {
        let mut metadata = conformant_v3_2();
        metadata.keywords.remove("$CYT");

        let flagged = keywords_flagged(&check_for_write(&metadata, Version::V3_2), Severity::Error);
        assert_eq!(flagged, vec!["$CYT".to_string()]);
    }

    #[test]
    fn a_conformant_file_produces_no_violations() {
        let violations = check(&conformant_v3_2(), Version::V3_2);
        assert!(
            violations.is_empty(),
            "baseline fixture must be clean, got: {violations:?}"
        );
    }

    #[test]
    fn missing_required_keywords_are_all_reported_not_just_the_first() {
        let mut metadata = conformant_v3_2();
        metadata.keywords.remove("$CYT");
        metadata.keywords.remove("$TOT");
        metadata.keywords.remove("$NEXTDATA");

        let violations = check(&metadata, Version::V3_2);
        let flagged = keywords_flagged(&violations, Severity::Error);
        assert_eq!(
            flagged.len(),
            3,
            "all three misses must be reported, got {flagged:?}"
        );
        for expected in ["$CYT", "$TOT", "$NEXTDATA"] {
            assert!(flagged.iter().any(|k| k == expected), "missing {expected}");
        }
    }

    #[test]
    fn pnb_not_divisible_by_eight_is_an_error_in_3_2_but_not_3_1() {
        let mut metadata = conformant_v3_2();
        // 10-bit ADC packed into a 10-bit field: legal in 3.1, not in 3.2.
        metadata
            .keywords
            .insert("$P2B".to_string(), Keyword::Int(IntegerKeyword::PnB(10)));

        let flagged = keywords_flagged(&check(&metadata, Version::V3_2), Severity::Error);
        assert_eq!(flagged, vec!["$P2B".to_string()]);

        let v3_1 = check(&metadata, Version::V3_1);
        assert!(
            !keywords_flagged(&v3_1, Severity::Error).contains(&"$P2B".to_string()),
            "3.1 permits bit-packed widths"
        );
    }

    #[test]
    fn a_time_parameter_without_timestep_is_an_error() {
        let mut metadata = conformant_v3_2();
        metadata.keywords.insert(
            "$P2N".to_string(),
            Keyword::String(StringKeyword::PnN(Arc::from("TIME"))),
        );

        let flagged = keywords_flagged(&check(&metadata, Version::V3_2), Severity::Error);
        assert!(
            flagged.contains(&"$TIMESTEP".to_string()),
            "expected $TIMESTEP violation, got {flagged:?}"
        );

        metadata.keywords.insert(
            "$TIMESTEP".to_string(),
            Keyword::String(StringKeyword::Other(Arc::from("0.01"))),
        );
        let flagged = keywords_flagged(&check(&metadata, Version::V3_2), Severity::Error);
        assert!(
            !flagged.contains(&"$TIMESTEP".to_string()),
            "supplying $TIMESTEP must clear the violation"
        );
    }

    /// A conformant `$BYTEORD` must pass regardless of which variant it landed
    /// in. Reading a file produces `ByteKeyword::BYTEORD`; building metadata
    /// with `insert_string_keyword` produces `StringKeyword::Other` with the
    /// identical value. An earlier draft of this rule keyed off the variant
    /// and rejected the second form.
    #[test]
    fn byteord_is_judged_by_value_not_by_how_it_was_stored() {
        for value in ["1,2,3,4", "4,3,2,1"] {
            let mut typed = conformant_v3_2();
            typed
                .keywords
                .insert("$BYTEORD".to_string(), parsed("$BYTEORD", value));
            assert!(
                matches!(typed.keywords["$BYTEORD"], Keyword::Byte(_)),
                "fixture precondition: parsed form should be typed"
            );
            assert!(
                check(&typed, Version::V3_2).is_empty(),
                "parsed {value} must be accepted"
            );

            let mut untyped = conformant_v3_2();
            untyped.insert_string_keyword("$BYTEORD".to_string(), value.to_string());
            assert!(
                matches!(untyped.keywords["$BYTEORD"], Keyword::String(_)),
                "fixture precondition: inserted form should be untyped"
            );
            assert!(
                check(&untyped, Version::V3_2).is_empty(),
                "hand-inserted {value} must be accepted too"
            );
        }
    }

    #[test]
    fn a_non_conformant_byteord_value_is_an_error() {
        let mut metadata = conformant_v3_2();
        // Legal in FCS 2.0/3.0, removed in 3.1.
        metadata.insert_string_keyword("$BYTEORD".to_string(), "3,4,1,2".to_string());

        let flagged = keywords_flagged(&check(&metadata, Version::V3_2), Severity::Error);
        assert!(
            flagged.contains(&"$BYTEORD".to_string()),
            "mixed byte order must be rejected in 3.2, got {flagged:?}"
        );
        // 3.0 still permits mixed byte orders. Scoped to $BYTEORD: this 3.2
        // fixture is independently non-conformant under 3.0, which requires
        // $BEGINSTEXT/$ENDSTEXT/$BEGINANALYSIS/$ENDANALYSIS that 3.2 dropped.
        assert!(
            !keywords_flagged(&check(&metadata, Version::V3_0), Severity::Error)
                .contains(&"$BYTEORD".to_string()),
            "3.0 still permits mixed byte orders"
        );
    }

    #[test]
    fn mode_other_than_list_is_an_error() {
        let mut metadata = conformant_v3_2();
        metadata.keywords.insert(
            "$MODE".to_string(),
            Keyword::String(StringKeyword::MODE(Arc::from("C"))),
        );

        let flagged = keywords_flagged(&check(&metadata, Version::V3_2), Severity::Error);
        assert!(
            flagged.contains(&"$MODE".to_string()),
            "correlated histogram mode must be rejected, got {flagged:?}"
        );
    }

    #[test]
    fn deprecated_keywords_warn_rather_than_error() {
        let mut metadata = conformant_v3_2();
        metadata.insert_string_keyword("$PLATEID".to_string(), "PLATE-1".to_string());
        metadata.insert_string_keyword("$DATE".to_string(), "06-AUG-2026".to_string());

        let violations = check(&metadata, Version::V3_2);
        assert!(
            errors(&violations).is_empty(),
            "deprecated keywords must not be errors, got {:?}",
            errors(&violations)
        );
        let warned = keywords_flagged(&violations, Severity::Warning);
        for expected in ["$PLATEID", "$DATE"] {
            assert!(
                warned.iter().any(|k| k == expected),
                "expected a warning for {expected}, got {warned:?}"
            );
        }
    }

    /// `$PnP` is the one *indexed* deprecation, so it is found by scanning keys
    /// rather than by name. The scan used to be driven by `$PAR` through a
    /// typed accessor, which fails on metadata built in memory - so the rule
    /// quietly examined nothing. Pin the scan explicitly.
    #[test]
    fn indexed_deprecated_keywords_are_warned_per_parameter() {
        let mut metadata = conformant_v3_2();
        metadata.insert_string_keyword("$P2P".to_string(), "10".to_string());

        let violations = check(&metadata, Version::V3_2);
        assert!(
            errors(&violations).is_empty(),
            "deprecation must not be an error, got {:?}",
            errors(&violations)
        );
        assert!(
            keywords_flagged(&violations, Severity::Warning)
                .iter()
                .any(|k| k == "$P2P"),
            "expected a warning for $P2P, got {violations:?}"
        );
    }

    #[test]
    fn deprecation_warnings_do_not_fire_for_older_versions() {
        let mut metadata = conformant_v3_2();
        metadata.insert_string_keyword("$PLATEID".to_string(), "PLATE-1".to_string());

        let warned = keywords_flagged(&check(&metadata, Version::V3_1), Severity::Warning);
        assert!(
            warned.is_empty(),
            "$PLATEID is current in 3.1, got {warned:?}"
        );
    }
}
