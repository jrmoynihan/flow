//! One tokenizer for the FCS TEXT segment, shared by every reader.
//!
//! TEXT is `<delim>KEY<delim>VALUE<delim>KEY<delim>VALUE<delim>`. Two
//! independent hand-rolled walks of that structure used to exist — one in
//! `Metadata::from_text_segment` and one in `Fcs::find_begindata_offset` —
//! agreeing only by a doc comment that said so. They now share this unit,
//! because delimiter escaping is precisely the change that would desynchronize
//! them, and only on `$NEXTDATA` chains where nothing would notice.

use crate::version::Version;
use std::borrow::Cow;

/// How a run of consecutive delimiters inside TEXT is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Escaping {
    /// Split on every delimiter; a run of N delimiters yields N-1 empty
    /// fields. FCS 3.0 and earlier permit empty keyword values, and corpus
    /// files use them, so `\\` there is a genuine empty value and no
    /// tokenizer can tell it apart from an escape.
    None,
    /// A run of N delimiters encodes N/2 literal delimiter characters, and
    /// terminates the field iff N is odd. Decodable only because FCS 3.1 and
    /// later forbid empty keyword values.
    Doubled,
}

impl Escaping {
    /// The gate sits at 3.1, not 3.0, deliberately. Being wrong at 3.1 means a
    /// rare FCS3.0 file with an escaped delimiter keeps mis-parsing — the
    /// status quo. Being wrong at 3.0 means a common FCS3.0 file with an empty
    /// value *newly* desynchronizes.
    pub(crate) const fn for_version(version: Version) -> Self {
        match version {
            Version::V1_0 | Version::V2_0 | Version::V3_0 => Self::None,
            Version::V3_1 | Version::V3_2 | Version::V4_0 => Self::Doubled,
        }
    }
}

/// Lazy iterator over the delimiter-separated fields of a TEXT segment body.
///
/// `body` must already exclude TEXT's leading delimiter. It may or may not be
/// delimiter-terminated: `Metadata::from_text_segment` passes a bounded slice
/// that is, and `Fcs::find_begindata_offset` passes the unbounded remainder of
/// the mmap that is not. Laziness is what lets the latter stop at
/// `$BEGINDATA` instead of walking into DATA.
pub(crate) struct TextFields<'a> {
    body: &'a [u8],
    delimiter: u8,
    escaping: Escaping,
    pos: usize,
}

impl<'a> TextFields<'a> {
    pub(crate) const fn new(body: &'a [u8], delimiter: u8, escaping: Escaping) -> Self {
        Self { body, delimiter, escaping, pos: 0 }
    }

    /// Length of the run of `self.delimiter` starting at `start`.
    fn run_length(&self, start: usize) -> usize {
        let mut end = start;
        while end < self.body.len() && self.body[end] == self.delimiter {
            end += 1;
        }
        end - start
    }
}

/// FCS requires TEXT to be ASCII. Invalid UTF-8 degrades to an empty field
/// rather than erroring, matching the pre-existing `unwrap_or_default()`
/// behaviour of both tokenizers this replaced.
fn as_str(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or_default()
}

impl<'a> Iterator for TextFields<'a> {
    type Item = Cow<'a, str>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.body.len() {
            return None;
        }

        match self.escaping {
            Escaping::None => {
                let start = self.pos;
                match memchr::memchr(self.delimiter, &self.body[start..]) {
                    Some(relative) => {
                        let end = start + relative;
                        self.pos = end + 1;
                        Some(Cow::Borrowed(as_str(&self.body[start..end])))
                    }
                    None => {
                        self.pos = self.body.len();
                        Some(Cow::Borrowed(as_str(&self.body[start..])))
                    }
                }
            }
            Escaping::Doubled => {
                let start = self.pos;
                let mut cursor = start;
                // Stays `None` while the field is one contiguous borrow; only
                // an actual un-doubling forces an owned String.
                let mut owned: Option<String> = None;

                loop {
                    let Some(relative) = memchr::memchr(self.delimiter, &self.body[cursor..])
                    else {
                        // Unterminated trailing field.
                        let tail = as_str(&self.body[cursor..]);
                        self.pos = self.body.len();
                        return Some(match owned {
                            Some(mut buffer) => {
                                buffer.push_str(tail);
                                Cow::Owned(buffer)
                            }
                            None => Cow::Borrowed(as_str(&self.body[start..])),
                        });
                    };

                    let run_start = cursor + relative;
                    let run = self.run_length(run_start);
                    let literals = run / 2;
                    let terminates = run % 2 == 1;

                    if literals == 0 && terminates {
                        // The common case: a lone delimiter ends the field.
                        self.pos = run_start + run;
                        return Some(match owned {
                            Some(mut buffer) => {
                                buffer.push_str(as_str(&self.body[cursor..run_start]));
                                Cow::Owned(buffer)
                            }
                            None => Cow::Borrowed(as_str(&self.body[start..run_start])),
                        });
                    }

                    let buffer = owned.get_or_insert_with(|| {
                        String::from(as_str(&self.body[start..cursor]))
                    });
                    buffer.push_str(as_str(&self.body[cursor..run_start]));
                    for _ in 0..literals {
                        buffer.push(self.delimiter as char);
                    }

                    if terminates {
                        self.pos = run_start + run;
                        return Some(Cow::Owned(owned.take().unwrap_or_default()));
                    }
                    cursor = run_start + run;
                }
            }
        }
    }
}

/// Append `text` to `out`, doubling any occurrence of `delimiter` when the
/// version escapes. The key is escaped as well as the value: user-defined
/// keywords are free-form and can contain the delimiter too.
pub(crate) fn escape_into(out: &mut Vec<u8>, text: &str, delimiter: u8, escaping: Escaping) {
    match escaping {
        Escaping::None => out.extend_from_slice(text.as_bytes()),
        Escaping::Doubled => {
            for &byte in text.as_bytes() {
                out.push(byte);
                if byte == delimiter {
                    out.push(byte);
                }
            }
        }
    }
}

/// The TEXT delimiter must be a single byte in ASCII 1-126.
///
/// NUL is excluded because it cannot be distinguished from padding, and
/// anything at or above 127 is either DEL or the lead byte of a multi-byte
/// UTF-8 sequence — neither is a single-byte delimiter, and `memchr` on a
/// truncated lead byte would split mid-character.
///
/// # Errors
/// Returns `Err` naming the rejected delimiter if it falls outside that range.
pub(crate) fn validate_delimiter(delimiter: char) -> anyhow::Result<u8> {
    let code = delimiter as u32;
    if (1..=126).contains(&code) {
        Ok(code as u8)
    } else {
        Err(anyhow::anyhow!(
            "Invalid TEXT delimiter U+{code:04X}: must be a single ASCII byte in 1-126 \
             (NUL and anything at or above DEL are not representable as a delimiter)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Escaping, TextFields};

    fn fields(body: &str, delimiter: u8, escaping: Escaping) -> Vec<String> {
        TextFields::new(body.as_bytes(), delimiter, escaping)
            .map(|field| field.into_owned())
            .collect()
    }

    #[test]
    fn splits_plain_key_value_pairs() {
        assert_eq!(
            fields("$PAR|2|$TOT|3|", b'|', Escaping::None),
            vec!["$PAR", "2", "$TOT", "3"]
        );
    }

    #[test]
    fn no_escaping_treats_a_doubled_delimiter_as_an_empty_field() {
        // FCS2.0 semantics: `real-8-parameters.data.fcs` really does contain
        // `\Comments\\Row\2\`, and `Comments` really does have an empty value.
        assert_eq!(
            fields("Comments||Row|2|", b'|', Escaping::None),
            vec!["Comments", "", "Row", "2"]
        );
    }

    #[test]
    fn doubled_escaping_folds_a_pair_into_one_literal_delimiter() {
        assert_eq!(
            fields("$COM|a||b|$PAR|2|", b'|', Escaping::Doubled),
            vec!["$COM", "a|b", "$PAR", "2"]
        );
    }

    #[test]
    fn doubled_escaping_handles_an_odd_run_as_literal_plus_separator() {
        // 3 delimiters = one literal + a field boundary.
        assert_eq!(
            fields("$COM|a|||$PAR|2|", b'|', Escaping::Doubled),
            vec!["$COM", "a|", "$PAR", "2"]
        );
    }

    #[test]
    fn doubled_escaping_handles_a_four_run_as_two_literals() {
        assert_eq!(
            fields("$COM|a||||b|", b'|', Escaping::Doubled),
            vec!["$COM", "a||b"]
        );
    }

    #[test]
    fn yields_a_trailing_field_with_no_terminating_delimiter() {
        // find_begindata_offset scans an unbounded tail; the last field there
        // is not delimiter-terminated.
        assert_eq!(
            fields("$PAR|2|$TOT|3", b'|', Escaping::None),
            vec!["$PAR", "2", "$TOT", "3"]
        );
    }

    #[test]
    fn empty_body_yields_nothing() {
        assert!(fields("", b'|', Escaping::None).is_empty());
        assert!(fields("", b'|', Escaping::Doubled).is_empty());
    }

    #[test]
    fn escaping_gate_starts_at_v3_1() {
        use crate::version::Version;
        assert_eq!(Escaping::for_version(Version::V1_0), Escaping::None);
        assert_eq!(Escaping::for_version(Version::V2_0), Escaping::None);
        assert_eq!(Escaping::for_version(Version::V3_0), Escaping::None);
        assert_eq!(Escaping::for_version(Version::V3_1), Escaping::Doubled);
        assert_eq!(Escaping::for_version(Version::V3_2), Escaping::Doubled);
        assert_eq!(Escaping::for_version(Version::V4_0), Escaping::Doubled);
    }

    #[test]
    fn corpus_empty_values_survive_under_v2_0_policy() {
        // real-8-parameters.data.fcs is FCS2.0 and contains `\Comments\\Row\2\`.
        // Under Doubled this would read as Comments="|Row" and shift every
        // subsequent field by one, which is exactly the desynchronization the
        // writer bug causes. Version::V2_0 must therefore not un-double.
        use crate::version::Version;
        let escaping = Escaping::for_version(Version::V2_0);
        assert_eq!(
            fields("Comments||Row|2|", b'|', escaping),
            vec!["Comments", "", "Row", "2"]
        );
    }

    #[test]
    fn escape_into_doubles_the_delimiter_only_under_doubled() {
        let mut out = Vec::new();
        super::escape_into(&mut out, "a|b", b'|', Escaping::Doubled);
        assert_eq!(out, b"a||b");

        let mut out = Vec::new();
        super::escape_into(&mut out, "a|b", b'|', Escaping::None);
        assert_eq!(out, b"a|b");
    }

    #[test]
    fn escape_then_tokenize_round_trips() {
        let mut body = Vec::new();
        for (key, value) in [("$COM", "hello world"), ("$PAR", "2")] {
            super::escape_into(&mut body, key, b' ', Escaping::Doubled);
            body.push(b' ');
            super::escape_into(&mut body, value, b' ', Escaping::Doubled);
            body.push(b' ');
        }
        let got: Vec<String> = TextFields::new(&body, b' ', Escaping::Doubled)
            .map(std::borrow::Cow::into_owned)
            .collect();
        assert_eq!(got, vec!["$COM", "hello world", "$PAR", "2"]);
    }

    #[test]
    fn validate_delimiter_accepts_ascii_1_to_126() {
        assert!(super::validate_delimiter('\u{0001}').is_ok());
        assert!(super::validate_delimiter(' ').is_ok());
        assert!(super::validate_delimiter('\u{000c}').is_ok());
        assert!(super::validate_delimiter('~').is_ok());
    }

    #[test]
    fn validate_delimiter_rejects_nul_and_out_of_range() {
        for bad in ['\u{0000}', '\u{007f}', '\u{00e9}'] {
            let err = super::validate_delimiter(bad).unwrap_err().to_string();
            assert!(
                err.contains("delimiter"),
                "error should name the delimiter, got: {err}"
            );
        }
    }
}
