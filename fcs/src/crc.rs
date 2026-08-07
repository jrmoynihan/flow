//! The FCS 3.2 CRC word (§3.7).
//!
//! Every FCS data set ends with eight ASCII bytes immediately after its last
//! segment. They hold either a 16-bit CRC or eight `0` characters meaning "not
//! computed". Emitting *nothing* - which this crate did until now - is not one
//! of the permitted encodings, so every file we wrote was non-conformant even
//! under the opt-out.
//!
//! # Which CRC-16, exactly
//!
//! The polynomial alone does not pin the algorithm down: `X^16 + X^12 + X^5 + 1`
//! with "each input character interpreted as its bit-reversed image" describes
//! both CRC-16/KERMIT and XMODEM-with-reflected-input, which differ in whether
//! the *output* word is also reflected and disagree on nearly every message.
//!
//! §3.7 settles it with a normative test vector rather than prose:
//!
//! > the result of any compatible CRC calculation of the string
//! > `CatMouse987654321` shall return 49805 (C28D hex)
//!
//! Searching the standard CRC-16 parameter space for poly `0x1021` yields
//! exactly one variant producing that value: init `0x0000`, reflect in and out,
//! no final XOR - the catalog entry **CRC-16/KERMIT**. Both that vector and
//! KERMIT's conventional `check("123456789") == 0x2189` are asserted below, so
//! a future "simplification" toward XMODEM fails immediately.
//!
//! Appendix B points at C# and C++ reference implementations on sourceforge.
//! They are not transcribed here: the test vector is normative, pins behaviour
//! rather than implementation, and is checkable in CI.
//!
//! # The field is decimal
//!
//! The easy mistake is to write the CRC as hex, since the spec quotes it that
//! way. It does not:
//!
//! > a CRC value of 49805 (C28D hex) shall be encoded as `00049805`
//!
//! That is the *decimal* value, left-padded with `0` to eight bytes. A hex field
//! looks perfectly plausible in a dump and is silently unverifiable by every
//! conformant reader.

/// The reflected form of the CCITT polynomial `X^16 + X^12 + X^5 + 1`.
///
/// `0x1021` reflected is `0x8408`. Working in the reflected domain is what lets
/// [`Crc16::update`] consume bytes LSB-first without reversing each one, and
/// makes the final word come out already reflected - so KERMIT needs no
/// post-processing pass at all.
const REFLECTED_POLY: u16 = 0x8408;

/// KERMIT starts from zero. Named because `0x0000` and `0xFFFF` are the two
/// plausible values here and the difference is invisible at a call site.
const INIT: u16 = 0x0000;

/// Width of the on-disk CRC field, in bytes.
pub const FIELD_LEN: usize = 8;

/// The "not computed" encoding: eight ASCII `0` characters.
pub const NOT_COMPUTED: [u8; FIELD_LEN] = *b"00000000";

/// Byte-at-a-time lookup table for [`REFLECTED_POLY`].
///
/// Built at compile time, so the table costs no startup work and no
/// synchronisation. Worth the 512 bytes: the CRC covers the *entire* data set,
/// and a 3M-event x 64-detector spectral file is ~768 MB. At roughly eight
/// operations per byte, the bitwise form would add over a second to every write
/// and every open of a file that size.
const TABLE: [u16; 256] = build_table();

const fn build_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut index = 0usize;
    while index < 256 {
        // `while` rather than `for`: iterators are not available in `const fn`.
        let mut crc = index as u16;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ REFLECTED_POLY
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

/// An in-progress CRC-16/KERMIT computation.
///
/// Incremental on purpose. The CRC spans HEADER + TEXT + DATA, which the writers
/// hold as three separate buffers and stream to disk one after another.
/// Concatenating them just to hash would double peak memory on exactly the large
/// files this matters for.
#[derive(Debug, Clone, Copy)]
pub struct Crc16 {
    state: u16,
}

impl Default for Crc16 {
    fn default() -> Self {
        Self::new()
    }
}

impl Crc16 {
    #[must_use]
    pub const fn new() -> Self {
        Self { state: INIT }
    }

    /// Feeds the next run of bytes.
    ///
    /// Splitting the input at any boundary gives the same result as one call -
    /// there is no final reflection or XOR for a split to interfere with.
    pub fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let index = usize::from((self.state ^ u16::from(byte)) as u8);
            self.state = (self.state >> 8) ^ TABLE[index];
        }
    }

    #[must_use]
    pub const fn finish(self) -> u16 {
        self.state
    }
}

/// One-shot CRC over a contiguous buffer.
#[must_use]
pub fn compute(bytes: &[u8]) -> u16 {
    let mut crc = Crc16::new();
    crc.update(bytes);
    crc.finish()
}

/// What the eight CRC bytes of a data set turned out to hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredCrc {
    /// Eight ASCII `0`s: the writer declined to compute a CRC.
    ///
    /// Also reported for a genuine CRC of zero, which the spec's encoding makes
    /// indistinguishable from the opt-out. That collides for 1 message in 65,536
    /// and is unfixable from our side - reporting "absent" is the reading that
    /// cannot produce a false mismatch warning.
    Absent,
    /// A parsed CRC value.
    Value(u16),
    /// Eight bytes that are neither. Carried rather than discarded so a warning
    /// can quote what was actually on disk.
    Malformed(Box<str>),
    /// The data set ends before its CRC field does.
    ///
    /// Distinct from [`Malformed`](Self::Malformed): the file is truncated (or
    /// was written by this crate before CRC support), not merely garbled. Every
    /// FCS file this workspace produced prior to `flow-crates-x17.3` lands here.
    Missing,
}

impl StoredCrc {
    /// Whether `actual` contradicts what the file claims.
    ///
    /// Only [`Value`](Self::Value) can contradict anything - an absent, garbled,
    /// or truncated field asserts nothing about the bytes, so treating it as a
    /// mismatch would reject files that are merely old rather than corrupt.
    #[must_use]
    pub fn conflicts_with(&self, actual: u16) -> bool {
        matches!(self, Self::Value(stored) if *stored != actual)
    }
}

/// Renders the CRC field: decimal, left-padded to eight bytes.
///
/// `None` yields [`NOT_COMPUTED`]. A `u16` never exceeds five digits, so the
/// field cannot overflow.
#[must_use]
pub fn format_field(crc: Option<u16>) -> [u8; FIELD_LEN] {
    let Some(crc) = crc else {
        return NOT_COMPUTED;
    };
    let mut field = NOT_COMPUTED;
    let rendered = crc.to_string();
    let start = FIELD_LEN - rendered.len();
    field[start..].copy_from_slice(rendered.as_bytes());
    field
}

/// Reads the eight CRC bytes sitting at the end of a data set.
///
/// `bytes` is the whole file (or mapping); `field_start` is the offset just past
/// the data set's final segment.
#[must_use]
pub fn parse_field(bytes: &[u8], field_start: usize) -> StoredCrc {
    let Some(field) = bytes.get(field_start..field_start + FIELD_LEN) else {
        return StoredCrc::Missing;
    };
    if field == NOT_COMPUTED {
        return StoredCrc::Absent;
    }
    // Leading spaces are tolerated on read but never written: §3.8 fills unused
    // space with ASCII 32, and vendors have been known to pad the field that way
    // instead of with zeros. Being liberal here costs nothing and rescues files
    // that do carry a usable CRC.
    match std::str::from_utf8(field).map(str::trim) {
        Ok(text) => match text.parse::<u16>() {
            Ok(value) => StoredCrc::Value(value),
            Err(_) => StoredCrc::Malformed(text.into()),
        },
        Err(_) => StoredCrc::Malformed(String::from_utf8_lossy(field).into_owned().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The normative vector from FCS 3.2 §3.7. This is the assertion that
    /// identifies the algorithm; without it the polynomial admits several
    /// mutually-incompatible readings.
    #[test]
    fn the_spec_test_vector_reproduces() {
        assert_eq!(compute(b"CatMouse987654321"), 49805);
        assert_eq!(compute(b"CatMouse987654321"), 0xC28D);
    }

    /// The CRC catalog's conventional check value for CRC-16/KERMIT. Pinned
    /// alongside the FCS vector so the variant is identifiable by name, and so a
    /// drift toward XMODEM (`0x31C3` here) is unmistakable.
    #[test]
    fn the_kermit_catalog_check_value_matches() {
        assert_eq!(compute(b"123456789"), 0x2189);
    }

    /// KERMIT has no final XOR, so the empty message hashes to the init value.
    #[test]
    fn the_empty_message_hashes_to_the_init_value() {
        assert_eq!(compute(b""), INIT);
    }

    /// The writers hash HEADER, TEXT and DATA as three separate buffers rather
    /// than concatenating them, so a split must be invisible.
    #[test]
    fn splitting_the_input_does_not_change_the_result() {
        let whole = b"CatMouse987654321";
        for split in 0..=whole.len() {
            let (head, tail) = whole.split_at(split);
            let mut crc = Crc16::new();
            crc.update(head);
            crc.update(tail);
            assert_eq!(crc.finish(), 0xC28D, "split at {split}");
        }
    }

    /// §3.7: "a CRC value of 49805 (C28D hex) shall be encoded as 00049805".
    /// Decimal, not hex - the single most likely thing to get wrong here, and
    /// invisible without a conformant reader to complain.
    #[test]
    fn the_field_is_decimal_not_hexadecimal() {
        assert_eq!(&format_field(Some(49805)), b"00049805");
        assert_ne!(&format_field(Some(49805)), b"0000c28d");
    }

    #[test]
    fn the_field_is_eight_bytes_at_both_extremes() {
        assert_eq!(&format_field(Some(0)), b"00000000");
        assert_eq!(&format_field(Some(u16::MAX)), b"00065535");
        assert_eq!(&format_field(None), b"00000000");
    }

    #[test]
    fn a_rendered_field_reads_back_as_the_same_value() {
        for value in [1u16, 9, 10, 4095, 49805, u16::MAX] {
            let field = format_field(Some(value));
            assert_eq!(parse_field(&field, 0), StoredCrc::Value(value), "{value}");
        }
    }

    /// The opt-out and a real zero CRC share an encoding; absent is the reading
    /// that cannot produce a spurious corruption warning.
    #[test]
    fn all_zeroes_reads_as_absent_rather_than_a_zero_value() {
        assert_eq!(parse_field(b"00000000", 0), StoredCrc::Absent);
        assert!(!StoredCrc::Absent.conflicts_with(0));
        assert!(!StoredCrc::Absent.conflicts_with(1234));
    }

    #[test]
    fn a_short_tail_is_missing_rather_than_malformed() {
        assert_eq!(parse_field(b"0004980", 0), StoredCrc::Missing);
        assert_eq!(parse_field(b"", 0), StoredCrc::Missing);
        // Offset past the end, which is what a pre-CRC file looks like.
        assert_eq!(parse_field(b"00049805", 1), StoredCrc::Missing);
    }

    #[test]
    fn garbage_is_reported_with_its_contents() {
        assert_eq!(
            parse_field(b"not-a-num", 0),
            StoredCrc::Malformed("not-a-nu".into())
        );
        // Out of u16 range: eight digits parse fine as text but not as a CRC.
        assert_eq!(
            parse_field(b"99999999", 0),
            StoredCrc::Malformed("99999999".into())
        );
    }

    /// Space-padded rather than zero-padded, which §3.8's white-space rule makes
    /// an easy vendor mistake. We never write this, but we can read it.
    #[test]
    fn a_space_padded_field_is_still_readable() {
        assert_eq!(parse_field(b"   49805", 0), StoredCrc::Value(49805));
    }

    /// Only a real value can contradict the bytes; everything else stays quiet.
    #[test]
    fn only_a_parsed_value_can_conflict() {
        assert!(StoredCrc::Value(1).conflicts_with(2));
        assert!(!StoredCrc::Value(1).conflicts_with(1));
        assert!(!StoredCrc::Missing.conflicts_with(7));
        assert!(!StoredCrc::Malformed("zz".into()).conflicts_with(7));
    }

    /// A single flipped bit anywhere has to move the CRC - that is the entire
    /// point of storing one.
    #[test]
    fn a_single_flipped_bit_changes_the_crc() {
        let base = b"FCS3.2    58     311     312   12345".to_vec();
        let expected = compute(&base);
        for index in 0..base.len() {
            for bit in 0..8u8 {
                let mut corrupted = base.clone();
                corrupted[index] ^= 1 << bit;
                assert_ne!(compute(&corrupted), expected, "byte {index} bit {bit}");
            }
        }
    }
}
