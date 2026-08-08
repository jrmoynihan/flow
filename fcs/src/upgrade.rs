//! Rewriting a file's metadata into a newer FCS version's idiom.
//!
//! FCS 3.2 did not invent new information so much as move it: `$DATE` plus
//! `$BTIM` became the single ISO 8601 `$BEGINDATETIME`, and the plate-centric
//! `$PLATEID`/`$PLATENAME`/`$WELLID` trio generalized into
//! `$CARRIERID`/`$CARRIERTYPE`/`$LOCATIONID`. A file whose header says
//! `FCS3.2` but whose TEXT still speaks 3.1 is declaring a version it doesn't
//! keep, so anything that stamps a newer version has to migrate the keywords
//! at the same time.
//!
//! **Deprecated originals are kept, not replaced.** FCS 3.2 §3.1 only
//! deprecates them; a 3.1-era reader that doesn't know `$BEGINDATETIME` still
//! finds `$DATE`, and a 3.2 reader prefers the new key. Deleting them would
//! discard information to satisfy a validator, which is the wrong trade.
//!
//! This is deliberately *not* a general downgrade path. Going 3.2 → 3.1 means
//! discarding `$PnDATATYPE`, splitting an offset-bearing datetime into fields
//! that can't hold the zone, and re-packing DATA - a different problem, and
//! nothing in this workspace needs it.

use crate::datetime::iso8601_from_fcs_date_and_time;
use crate::metadata::Metadata;
use crate::version::Version;

/// `$DATE` + this time keyword → this FCS 3.2 datetime keyword.
const DATETIME_MIGRATIONS: [(&str, &str); 2] = [
    ("$BTIM", "$BEGINDATETIME"),
    ("$ETIM", "$ENDDATETIME"),
];

/// FCS 3.1 carrier keyword → its FCS 3.2 replacement.
///
/// The mapping is the one recorded on the [`StringKeyword`] variants
/// themselves (`keyword/mod.rs`). `$PLATENAME` → `$CARRIERTYPE` reads oddly -
/// a plate's *name* is not its *type* - but that is the correspondence the
/// standard draws, and inventing a better one would put us out of step with
/// every other reader.
///
/// [`StringKeyword`]: crate::keyword::StringKeyword
const CARRIER_MIGRATIONS: [(&str, &str); 3] = [
    ("$PLATEID", "$CARRIERID"),
    ("$PLATENAME", "$CARRIERTYPE"),
    ("$WELLID", "$LOCATIONID"),
];

/// Rewrites `metadata` into FCS 3.2 idiom and returns what it changed.
///
/// Idempotent: an already-migrated keyword is left alone rather than
/// overwritten, so re-running this on a file that has both spellings will not
/// clobber a `$BEGINDATETIME` the instrument itself wrote (which may carry a
/// zone offset that the `$DATE`/`$BTIM` pair cannot express).
///
/// Does not touch [`Header::version`] - see [`stamp_v3_2`], which does both.
///
/// [`Header::version`]: crate::header::Header::version
pub fn migrate_keywords_to_v3_2(metadata: &mut Metadata) -> Vec<String> {
    let mut migrated = Vec::new();

    let date = read_text(metadata, "$DATE");
    for (time_key, datetime_key) in DATETIME_MIGRATIONS {
        if lookup(metadata, datetime_key).is_some() {
            continue;
        }
        let (Some(date), Some(time)) = (date.as_deref(), read_text(metadata, time_key)) else {
            continue;
        };
        let Some(iso) = iso8601_from_fcs_date_and_time(date, &time) else {
            tracing::debug!(
                "leaving {datetime_key} unset: cannot read $DATE={date:?} {time_key}={time:?} \
                 as a date and time"
            );
            continue;
        };
        metadata.insert_string_keyword(datetime_key.to_string(), iso);
        migrated.push(datetime_key.to_string());
    }

    for (old_key, new_key) in CARRIER_MIGRATIONS {
        if lookup(metadata, new_key).is_some() {
            continue;
        }
        let Some(value) = read_text(metadata, old_key) else {
            continue;
        };
        metadata.insert_string_keyword(new_key.to_string(), value);
        migrated.push(new_key.to_string());
    }

    migrated
}

/// Declares `metadata` as FCS 3.2 and migrates its keywords to match.
///
/// Callers must set `header.version = Version::V3_2` themselves; this takes
/// only the metadata so it stays usable on a `Metadata` that is not yet
/// attached to an [`Fcs`]. [`Version::V3_2`] is returned as a nudge toward
/// assigning it.
///
/// [`Fcs`]: crate::file::Fcs
#[must_use]
pub fn stamp_v3_2(metadata: &mut Metadata) -> Version {
    let migrated = migrate_keywords_to_v3_2(metadata);
    if !migrated.is_empty() {
        tracing::debug!("migrated to FCS 3.2 keywords: {}", migrated.join(", "));
    }
    Version::V3_2
}

/// Finds a keyword under either spelling.
///
/// The writer `$`-prefixes every key on the way out, but metadata assembled in
/// memory can still hold the bare form, so a lookup that checks only one
/// spelling silently misses half the cases.
fn lookup<'a>(metadata: &'a Metadata, key: &str) -> Option<&'a crate::keyword::Keyword> {
    metadata
        .keywords
        .get(key)
        .or_else(|| metadata.keywords.get(key.strip_prefix('$')?))
}

/// A keyword's value as text, or `None` if absent, structured, or blank.
fn read_text(metadata: &Metadata, key: &str) -> Option<String> {
    let value = lookup(metadata, key)?.value_str()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyword::StringableKeyword;

    fn metadata_with(pairs: &[(&str, &str)]) -> Metadata {
        let mut metadata = Metadata::new();
        for (key, value) in pairs {
            metadata.insert_string_keyword((*key).to_string(), (*value).to_string());
        }
        metadata
    }

    fn text(metadata: &Metadata, key: &str) -> Option<String> {
        Some(lookup(metadata, key)?.value_str()?.to_string())
    }

    #[test]
    fn date_and_times_become_iso_datetimes() {
        let mut metadata = metadata_with(&[
            ("$DATE", "01-JAN-2024"),
            ("$BTIM", "14:30:00"),
            ("$ETIM", "14:45:30"),
        ]);

        let migrated = migrate_keywords_to_v3_2(&mut metadata);

        assert_eq!(
            text(&metadata, "$BEGINDATETIME").as_deref(),
            Some("2024-01-01T14:30:00")
        );
        assert_eq!(
            text(&metadata, "$ENDDATETIME").as_deref(),
            Some("2024-01-01T14:45:30")
        );
        assert_eq!(migrated.len(), 2, "{migrated:?}");
    }

    /// A 3.1 reader has to keep working against the same file.
    #[test]
    fn the_deprecated_originals_are_kept() {
        let mut metadata = metadata_with(&[
            ("$DATE", "01-JAN-2024"),
            ("$BTIM", "14:30:00"),
            ("$PLATEID", "PL-7"),
        ]);

        migrate_keywords_to_v3_2(&mut metadata);

        assert_eq!(text(&metadata, "$DATE").as_deref(), Some("01-JAN-2024"));
        assert_eq!(text(&metadata, "$BTIM").as_deref(), Some("14:30:00"));
        assert_eq!(text(&metadata, "$PLATEID").as_deref(), Some("PL-7"));
    }

    #[test]
    fn plate_keywords_become_carrier_keywords() {
        let mut metadata = metadata_with(&[
            ("$PLATEID", "PL-7"),
            ("$PLATENAME", "Donor Screen A"),
            ("$WELLID", "B04"),
        ]);

        migrate_keywords_to_v3_2(&mut metadata);

        assert_eq!(text(&metadata, "$CARRIERID").as_deref(), Some("PL-7"));
        assert_eq!(
            text(&metadata, "$CARRIERTYPE").as_deref(),
            Some("Donor Screen A")
        );
        assert_eq!(text(&metadata, "$LOCATIONID").as_deref(), Some("B04"));
    }

    /// An instrument-written `$BEGINDATETIME` can carry a zone offset that
    /// `$DATE`/`$BTIM` cannot express, so synthesizing over it loses
    /// information.
    #[test]
    fn an_existing_target_is_never_overwritten() {
        let mut metadata = metadata_with(&[
            ("$DATE", "01-JAN-2024"),
            ("$BTIM", "14:30:00"),
            ("$BEGINDATETIME", "2024-01-01T14:30:00-05:00"),
            ("$PLATEID", "PL-7"),
            ("$CARRIERID", "CARRIER-1"),
        ]);

        let migrated = migrate_keywords_to_v3_2(&mut metadata);

        assert_eq!(
            text(&metadata, "$BEGINDATETIME").as_deref(),
            Some("2024-01-01T14:30:00-05:00")
        );
        assert_eq!(text(&metadata, "$CARRIERID").as_deref(), Some("CARRIER-1"));
        assert!(
            !migrated.iter().any(|k| k == "$BEGINDATETIME" || k == "$CARRIERID"),
            "{migrated:?}"
        );
    }

    #[test]
    fn migrating_twice_changes_nothing_the_second_time() {
        let mut metadata = metadata_with(&[
            ("$DATE", "01-JAN-2024"),
            ("$BTIM", "14:30:00"),
            ("$WELLID", "B04"),
        ]);

        assert_eq!(migrate_keywords_to_v3_2(&mut metadata).len(), 2);
        assert!(migrate_keywords_to_v3_2(&mut metadata).is_empty());
    }

    /// Half a timestamp is not a timestamp. A `$BTIM` with no `$DATE` has to
    /// leave `$BEGINDATETIME` unset rather than invent a day.
    #[test]
    fn an_unusable_source_leaves_the_target_absent() {
        let mut metadata = metadata_with(&[("$BTIM", "14:30:00")]);
        assert!(migrate_keywords_to_v3_2(&mut metadata).is_empty());
        assert!(text(&metadata, "$BEGINDATETIME").is_none());

        let mut metadata = metadata_with(&[("$DATE", "not a date"), ("$BTIM", "14:30:00")]);
        assert!(migrate_keywords_to_v3_2(&mut metadata).is_empty());
        assert!(text(&metadata, "$BEGINDATETIME").is_none());
    }

    /// Some instruments write the key with an empty value rather than omitting
    /// it; `$CARRIERID` = "" is worse than no `$CARRIERID` at all.
    #[test]
    fn blank_values_do_not_propagate() {
        let mut metadata = metadata_with(&[("$PLATEID", "   ")]);
        assert!(migrate_keywords_to_v3_2(&mut metadata).is_empty());
        assert!(text(&metadata, "$CARRIERID").is_none());
    }

    #[test]
    fn stamping_reports_the_version_to_assign() {
        let mut metadata = metadata_with(&[("$DATE", "01-JAN-2024"), ("$BTIM", "14:30:00")]);
        assert!(matches!(stamp_v3_2(&mut metadata), Version::V3_2));
        assert!(text(&metadata, "$BEGINDATETIME").is_some());
    }

    /// `insert_string_keyword` routes through `match_and_parse_keyword`, so
    /// these should land in their typed variants rather than
    /// `StringKeyword::Other`. If they don't, a downstream `match` on the
    /// variant will miss them.
    #[test]
    fn migrated_keywords_land_in_their_typed_variants() {
        use crate::keyword::{Keyword, StringKeyword};

        let mut metadata = metadata_with(&[
            ("$DATE", "01-JAN-2024"),
            ("$BTIM", "14:30:00"),
            ("$PLATEID", "PL-7"),
        ]);
        migrate_keywords_to_v3_2(&mut metadata);

        assert!(matches!(
            lookup(&metadata, "$BEGINDATETIME"),
            Some(Keyword::String(StringKeyword::BEGINDATETIME(_)))
        ));
        let Some(Keyword::String(carrier @ StringKeyword::CARRIERID(_))) =
            lookup(&metadata, "$CARRIERID")
        else {
            panic!("$CARRIERID is not a CARRIERID variant: {:?}", lookup(&metadata, "$CARRIERID"));
        };
        assert_eq!(carrier.get_str().as_ref(), "PL-7");
    }
}
