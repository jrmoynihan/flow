//! ISO 8601 UTC timestamps for the FCS 3.2 date keywords.
//!
//! FCS 3.2 deprecates `$DATE` / `$BTIM` / `$ETIM` in favour of `$BEGINDATETIME`
//! and `$ENDDATETIME`, both of which are ISO 8601. `$LAST_MODIFIED` uses the
//! same form.
//!
//! Calendar arithmetic comes from `chrono`, which is already in this crate's
//! graph via `polars` and so costs nothing to compile (`flow-crates-16f`).
//! Only `Utc` and `NaiveDate` are used, never `Local`: an FCS timestamp is
//! either zoneless or carries an explicit offset, so reading the host's
//! timezone would invent information. (The workspace declares chrono with
//! `now` rather than `clock` to say so in the manifest too, but that is intent
//! rather than enforcement - cargo unions features, and polars enables `clock`
//! anyway. Not importing `Local` is the part that holds.)
//!
//! What a date library does *not* help with is the half of this module that
//! matters: the spellings of `$DATE` and `$BTIM` that instruments actually
//! write. [`parse_fcs_time`] stays hand-written because FCS 3.0's
//! colon-separated hundredths are not a `strftime` form.
//!
//! Timestamps this module *generates* are UTC and end in `Z`. Timestamps it
//! *converts* from `$DATE`/`$BTIM` carry no zone at all - see
//! [`iso8601_from_fcs_date_and_time`].

use chrono::{DateTime, NaiveDate, Utc};

/// The wire format for every timestamp this module generates.
const ISO_8601_UTC: &str = "%Y-%m-%dT%H:%M:%SZ";

/// Current wall-clock time as `YYYY-MM-DDThh:mm:ssZ`.
pub fn now_iso8601_utc() -> String {
    Utc::now().format(ISO_8601_UTC).to_string()
}

/// Formats a Unix timestamp as `YYYY-MM-DDThh:mm:ssZ`.
///
/// Split out from [`now_iso8601_utc`] so the calendar arithmetic is testable
/// without a clock.
///
/// Falls back to the epoch for a timestamp outside chrono's representable
/// range (roughly ±262,000 years). That is unreachable from a system clock,
/// and a visibly-wrong `1970-01-01` beats a panic inside a metadata write.
pub fn iso8601_utc_from_unix_seconds(secs: i64) -> String {
    DateTime::from_timestamp(secs, 0)
        .unwrap_or(DateTime::UNIX_EPOCH)
        .format(ISO_8601_UTC)
        .to_string()
}

/// Combines an FCS 2.0-3.1 `$DATE` and `$BTIM`/`$ETIM` into an FCS 3.2
/// `$BEGINDATETIME`/`$ENDDATETIME` value.
///
/// Returns `None` unless both halves parse - a partially-understood timestamp
/// is worse than none, because a reader cannot tell a guessed field from a
/// recorded one.
///
/// **No zone offset is emitted.** FCS 3.1 `$DATE`/`$BTIM` carry no timezone, so
/// appending `Z` would assert UTC about an acquisition clock we know nothing
/// about. 3.2 permits the offset to be omitted, which is the honest encoding.
pub fn iso8601_from_fcs_date_and_time(date: &str, time: &str) -> Option<String> {
    let date = parse_fcs_date(date.trim())?;
    let time = parse_fcs_time(time.trim())?;
    Some(format!("{}T{time}", date.format("%Y-%m-%d")))
}

/// `$DATE` spellings, in the only order that works.
///
/// **`%y` must precede `%Y`.** chrono's `%Y` is greedy about digit count and
/// happily reads `15-DEC-99` as the year 99 AD, so trying it first would
/// silently mis-date every FCS 3.0 file. `%y` is safe to try first because it
/// consumes exactly two digits and then fails on the leftovers of a four-digit
/// year, falling through.
///
/// - `%d-%b-%y` - FCS 2.0/3.0 two-digit year. chrono pivots at 69, so `99` is
///   1999 and `05` is 2005.
/// - `%d-%b-%Y` - the FCS 3.1 mandated form (`01-JAN-2024`).
/// - `%Y-%m-%d` - ISO, which some instruments write regardless of version.
///
/// `%b` matches month names case-insensitively and `%d` tolerates an unpadded
/// day, both of which vendors rely on.
const DATE_FORMATS: [&str; 3] = ["%d-%b-%y", "%d-%b-%Y", "%Y-%m-%d"];

/// Parses `$DATE` in any of the spellings FCS has used.
///
/// Impossible dates are rejected, not normalized: chrono refuses
/// `31-FEB-2024` rather than rolling it into March, so a malformed instrument
/// date leaves `$BEGINDATETIME` unset instead of writing an ISO string that is
/// syntactically fine and calendrically nonsense.
fn parse_fcs_date(date: &str) -> Option<NaiveDate> {
    DATE_FORMATS
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(date, format).ok())
}

/// Parses `$BTIM`/`$ETIM` into the time half of an ISO 8601 timestamp.
///
/// Accepts `hh:mm:ss`, the 3.1 fractional `hh:mm:ss.cc`, and the FCS 3.0
/// colon-separated `hh:mm:ss:cc` - which is a fourth *field*, not a fourth
/// level of the hierarchy, and is the reason this cannot just be passed
/// through unmodified.
fn parse_fcs_time(time: &str) -> Option<String> {
    let (clock, fraction) = match time.split_once('.') {
        Some((clock, frac)) => (clock, Some(frac)),
        None => (time, None),
    };

    let mut parts = clock.split(':');
    let hour: u32 = parts.next()?.parse().ok()?;
    let minute: u32 = parts.next()?.parse().ok()?;
    let second: u32 = parts.next()?.parse().ok()?;
    // A fourth colon-separated field is FCS 3.0's fractional seconds.
    let fraction = fraction.or_else(|| parts.next());
    if parts.next().is_some() {
        return None;
    }
    // 60 admits a leap second, which instruments do occasionally record.
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let base = format!("{hour:02}:{minute:02}:{second:02}");
    match fraction {
        Some(f) if !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => {
            Some(format!("{base}.{f}"))
        }
        // An unparseable fraction drops to whole seconds rather than failing:
        // the seconds are still trustworthy, and losing them over a garbled
        // hundredths field would discard the whole timestamp.
        _ => Some(base),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_formats_as_midnight() {
        assert_eq!(iso8601_utc_from_unix_seconds(0), "1970-01-01T00:00:00Z");
    }

    /// Spot values cross-checked against `date -u -r <secs>`.
    #[test]
    fn known_instants_round_trip_to_their_calendar_dates() {
        for (secs, expected) in [
            (1_000_000_000, "2001-09-09T01:46:40Z"),
            (1_234_567_890, "2009-02-13T23:31:30Z"),
            (1_700_000_000, "2023-11-14T22:13:20Z"),
            (2_147_483_647, "2038-01-19T03:14:07Z"),
        ] {
            assert_eq!(iso8601_utc_from_unix_seconds(secs), expected);
        }
    }

    /// Kept from the hand-rolled implementation as an acceptance vector: these
    /// are the instants a calendar routine is most likely to get wrong, and
    /// they now also pin that the chrono swap did not shift anything.
    #[test]
    fn leap_days_are_placed_correctly() {
        assert_eq!(
            iso8601_utc_from_unix_seconds(1_709_164_800),
            "2024-02-29T00:00:00Z"
        );
        // 2000 is a leap year (divisible by 400); 1900 was not.
        assert_eq!(
            iso8601_utc_from_unix_seconds(951_782_400),
            "2000-02-29T00:00:00Z"
        );
    }

    /// Negative timestamps have to floor toward the previous day rather than
    /// truncate toward zero. `$LAST_MODIFIED` will never be pre-1970, but this
    /// is the cheapest available check that the seconds-to-date conversion is
    /// not doing something naive.
    #[test]
    fn pre_epoch_instants_floor_instead_of_truncating() {
        assert_eq!(iso8601_utc_from_unix_seconds(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn the_fcs_31_date_and_time_spelling_converts() {
        assert_eq!(
            iso8601_from_fcs_date_and_time("01-JAN-2024", "14:30:00"),
            Some("2024-01-01T14:30:00".into())
        );
    }

    /// No `Z`: `$DATE`/`$BTIM` carry no zone, so claiming UTC would be a
    /// fabrication. Pinned because it is the kind of thing a later "tidy-up"
    /// would silently add.
    #[test]
    fn no_timezone_is_asserted_on_a_zoneless_source() {
        let converted = iso8601_from_fcs_date_and_time("01-JAN-2024", "14:30:00").unwrap();
        assert!(!converted.ends_with('Z'), "{converted}");
        assert!(!converted.contains('+'), "{converted}");
    }

    #[test]
    fn the_date_spellings_vendors_actually_write_all_parse() {
        for (date, expected) in [
            ("01-JAN-2024", "2024-01-01"),
            ("01-Jan-2024", "2024-01-01"), // mixed case
            ("15-dec-1999", "1999-12-15"), // lower case
            ("15-DEC-99", "1999-12-15"),   // FCS 3.0 two-digit, post-pivot
            ("15-DEC-05", "2005-12-15"),   // FCS 3.0 two-digit, pre-pivot
            ("2024-01-01", "2024-01-01"),  // already ISO
        ] {
            assert_eq!(
                iso8601_from_fcs_date_and_time(date, "00:00:00").as_deref(),
                Some(format!("{expected}T00:00:00").as_str()),
                "{date}"
            );
        }
    }

    /// FCS 3.0 wrote hundredths as a fourth colon-separated field; 3.1 uses a
    /// decimal point. Both have to land on the ISO decimal form.
    #[test]
    fn both_fractional_second_spellings_normalize_to_a_decimal_point() {
        assert_eq!(
            iso8601_from_fcs_date_and_time("01-JAN-2024", "14:30:00.25"),
            Some("2024-01-01T14:30:00.25".into())
        );
        assert_eq!(
            iso8601_from_fcs_date_and_time("01-JAN-2024", "14:30:00:25"),
            Some("2024-01-01T14:30:00.25".into())
        );
    }

    #[test]
    fn a_garbled_half_yields_nothing_rather_than_a_guess() {
        assert_eq!(iso8601_from_fcs_date_and_time("", "14:30:00"), None);
        assert_eq!(iso8601_from_fcs_date_and_time("01-JAN-2024", ""), None);
        assert_eq!(iso8601_from_fcs_date_and_time("01-XXX-2024", "14:30:00"), None);
        assert_eq!(iso8601_from_fcs_date_and_time("01-JAN-2024", "25:00:00"), None);
        assert_eq!(iso8601_from_fcs_date_and_time("01-JAN-2024", "14:30"), None);
        assert_eq!(iso8601_from_fcs_date_and_time("2024-13-01", "14:30:00"), None);
    }

    /// The seconds are still good when only the hundredths are junk, so keep
    /// them rather than discarding a usable timestamp.
    /// REGRESSION: the hand-rolled parser range-checked day 1-31 without
    /// checking month length, so a malformed instrument date produced the
    /// syntactically-valid, calendrically-impossible `2024-02-31T00:00:00`.
    #[test]
    fn an_impossible_date_is_rejected_rather_than_emitted() {
        assert_eq!(iso8601_from_fcs_date_and_time("31-FEB-2024", "00:00:00"), None);
        assert_eq!(iso8601_from_fcs_date_and_time("31-APR-2024", "00:00:00"), None);
        // 2023 was not a leap year; 2024 was.
        assert_eq!(iso8601_from_fcs_date_and_time("29-FEB-2023", "00:00:00"), None);
        assert_eq!(
            iso8601_from_fcs_date_and_time("29-FEB-2024", "00:00:00"),
            Some("2024-02-29T00:00:00".into())
        );
    }

    /// `%Y` is greedy about digit count and reads `99` as the year 99 AD, so
    /// [`DATE_FORMATS`] must try `%y` first. Getting the order wrong
    /// mis-dates every FCS 3.0 file by nineteen centuries while still
    /// returning `Some`, which no other test in here would catch.
    #[test]
    fn the_two_digit_year_format_is_tried_before_the_four_digit_one() {
        assert_eq!(DATE_FORMATS[0], "%d-%b-%y", "{DATE_FORMATS:?}");
        assert_eq!(
            iso8601_from_fcs_date_and_time("15-DEC-99", "00:00:00"),
            Some("1999-12-15T00:00:00".into()),
            "a 99 read as year 99 AD would give 0099-12-15"
        );
    }

    #[test]
    fn an_unparseable_fraction_degrades_to_whole_seconds() {
        assert_eq!(
            iso8601_from_fcs_date_and_time("01-JAN-2024", "14:30:00.??"),
            Some("2024-01-01T14:30:00".into())
        );
    }

    #[test]
    fn the_current_time_has_the_right_shape() {
        let now = now_iso8601_utc();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z'), "{now}");
        assert!(now.starts_with("20"), "clock is badly wrong: {now}");
    }
}
