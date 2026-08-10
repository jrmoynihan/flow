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
///
/// The granularity of that degradation differs between the two policies, and
/// deliberately so — it falls out of where the function is called. Under
/// [`Escaping::None`] (and under [`Escaping::Doubled`]'s no-escape fast path)
/// it is applied to the whole field, so one bad byte empties the field. Under
/// `Doubled`'s escaped path it is applied to each sub-slice *between*
/// delimiter runs, so a field that contains both an escaped delimiter and an
/// invalid byte yields the still-valid sub-slices concatenated rather than "".
/// The same bytes therefore decode to a partial string one way and an empty
/// string the other. The lossy-UTF-8 policy as a whole is tracked separately
/// (bead `flow-crates-gpc`); this note exists so the asymmetry is not
/// mistaken for an accident. Note that `metadata.rs`'s `unterminated_tail`
/// guard keys off exactly this emptiness.
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

/// True if `field` — a token that landed in a **key** position while
/// tokenizing under [`Escaping::Doubled`] — carries the fingerprint of two
/// standard keywords that a non-conformant empty value merged into one.
///
/// # Why this shape, and why only this shape
///
/// FCS 3.1+ forbids empty keyword values precisely because an empty value is
/// spelled as two back-to-back delimiter bytes, which is byte-for-byte
/// identical to one escaped literal delimiter. A writer that emits
/// `<d>$P1S<d><d>$P2S<d>value<d>` anyway produces a run of two that `Doubled`
/// folds into the ongoing field, yielding the single key `$P1S<d>$P2S` and
/// attributing `value` to it: `$P1S` is lost outright and `$P2S` is
/// unreachable. (The damage is local, not a cascading shift — each empty value
/// removes exactly two fields, so key/value parity is preserved for everything
/// after it. It is still silent data loss.)
///
/// There is **no byte-level signature** that separates that from a conformant
/// escaped delimiter: `a<d><d>b` inside a value and `$A<d><d>$B` across an
/// empty value are the same bytes in the same position. The only usable
/// signature is semantic, and this is it: a key that *begins* with `$`,
/// contains a literal delimiter, and whose text after that delimiter also
/// begins with `$`. `$` is reserved for standard keywords, so `$…<d>$…` inside
/// a single key means two standard keywords were welded together.
///
/// Both halves must look standard, not just the second. A conformant *user*
/// keyword can never begin with `$` — the prefix is reserved — so the leading
/// test excludes every legal user keyword by construction, including one whose
/// name happens to contain `<d>$` (`COST<d>$USD`). Without it that key would be
/// flagged and the whole segment silently re-parsed under the wrong policy,
/// which is the same class of silent corruption this guard exists to prevent.
/// A conformant key that genuinely contains the delimiter (`MY KEY` under the
/// default space delimiter, which `escape_into` escapes on write and this
/// tokenizer un-doubles on read) does not match either, and must not.
///
/// The cost of the check is one `Cow` discriminant test on the common path —
/// callers only reach it for keys that actually un-doubled, which a conformant
/// file essentially never has.
///
/// # Known limits
///
/// A non-conformant empty value between two *user* keywords (neither
/// `$`-prefixed) is not detected. The regression this guards is flow-crates'
/// own former output — empty `$PnS` in FCS 3.2 files, all standard keywords —
/// and widening the predicate to "any literal delimiter in a key" would
/// misfire on the legal `MY KEY` case above.
pub(crate) fn looks_like_merged_keywords(field: &str, delimiter: u8) -> bool {
    field.starts_with('$')
        && field
            .split(delimiter as char)
            .skip(1)
            .any(|part| part.starts_with('$'))
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
    fn an_empty_value_merges_two_keys_under_doubled() {
        // The C1 mechanism, pinned. A non-conformant 3.1+ writer emitting an
        // empty $P1S produces this; `Doubled` welds $P1S and $P2S into one
        // key and gives it $P2S's value. Note the parity is *preserved* —
        // two fields vanish, so nothing after this pair shifts.
        assert_eq!(
            fields("$P1S||$P2S|FSC|$TOT|3|", b'|', Escaping::Doubled),
            vec!["$P1S|$P2S", "FSC", "$TOT", "3"]
        );
        // The same bytes under `None` are what the writer meant.
        assert_eq!(
            fields("$P1S||$P2S|FSC|$TOT|3|", b'|', Escaping::None),
            vec!["$P1S", "", "$P2S", "FSC", "$TOT", "3"]
        );
    }

    #[test]
    fn merged_keyword_detection_flags_welded_standard_keywords_only() {
        use super::looks_like_merged_keywords;
        // Welded standard keywords: the C1 fingerprint.
        assert!(looks_like_merged_keywords("$P1S|$P2S", b'|'));
        assert!(looks_like_merged_keywords("$COM|$BEGINDATA", b'|'));
        // A conformant user keyword whose *name* contains the delimiter must
        // not be flagged — falling back for it would corrupt a legal file.
        assert!(!looks_like_merged_keywords("MY KEY", b' '));
        assert!(!looks_like_merged_keywords("$MY KEY", b' '));
        // A user keyword whose name contains `<delimiter>$`. Legal: the `$`
        // prefix is reserved, so a conformant user keyword never starts with
        // one, and the leading-`$` test is what keeps this from being flagged.
        // Flagging it would silently re-parse a valid segment under `None`.
        assert!(!looks_like_merged_keywords("COST $USD", b' '));
        assert!(!looks_like_merged_keywords("COST|$USD", b'|'));
        // No delimiter at all: the overwhelmingly common case.
        assert!(!looks_like_merged_keywords("$TOT", b'|'));
        assert!(!looks_like_merged_keywords("", b'|'));
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
