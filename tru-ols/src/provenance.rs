//! Provenance recorded onto unmixed FCS files.
//!
//! An unmixed `.fcs` is a derived product: its DATA segment is abundances, not
//! detector signal, and the transform that produced it lives nowhere in the
//! file unless we put it there. Without that record you cannot audit, reproduce
//! or even attribute an unmixed file from the file alone - `$SPILLOVER` is
//! deliberately overwritten with an identity matrix (the detector basis is
//! gone, so downstream tools must not re-compensate), which is correct but
//! carries zero information about what happened.
//!
//! [`UnmixProvenance`] is that record. [`UnmixProvenance::write_to`] stamps it
//! onto a file and [`UnmixProvenance::read_from`] recovers it, so the round trip
//! is testable rather than write-only.
//!
//! # Keywords
//!
//! `$TRUOLS_*` are vendor keywords - not in the FCS standard, but `$`-prefixed
//! non-standard keys are explicitly permitted and readers must ignore what they
//! do not recognise. Where the standard already has a home for something we use
//! it: `$UNSTAINEDINFO`, `$ORIGINALITY`, `$LAST_MODIFIER` and `$LAST_MODIFIED`
//! are all FCS 3.2 keywords.

use crate::unmixing::UnmixingStrategy;
use flow_fcs::Fcs;
use flow_fcs::keyword::{Keyword, MixedKeyword, StringKeyword};
use std::sync::Arc;

/// The rectangular mixing matrix the unmixing solved against.
pub const MIXMAT_KEYWORD: &str = "$TRUOLS_MIXMAT";
/// `$GUID` of the stained source file.
pub const RAW_DATASOURCE_GUID_KEYWORD: &str = "$RAW_DATASOURCE_GUID";
/// `$GUID` of the unstained control used to derive cutoffs and background.
pub const UNSTAINED_DATASOURCE_GUID_KEYWORD: &str = "$UNSTAINED_DATASOURCE_GUID";
/// Free text describing how autofluorescence / background was derived (FCS 3.2).
pub const UNSTAINEDINFO_KEYWORD: &str = "$UNSTAINEDINFO";
/// Percentile used by the cutoff calculator, as a fraction (e.g. `0.995`).
pub const CUTOFF_PCT_KEYWORD: &str = "$TRUOLS_CUTOFF_PCT";
/// Which [`UnmixingStrategy`] handled irrelevant abundances.
pub const STRATEGY_KEYWORD: &str = "$TRUOLS_STRATEGY";
/// Zero-based column index of the autofluorescence endmember.
pub const AF_INDEX_KEYWORD: &str = "$TRUOLS_AF_INDEX";
/// Software that performed the unmixing (FCS 3.2 `$LAST_MODIFIER`).
pub const LAST_MODIFIER_KEYWORD: &str = "$LAST_MODIFIER";
/// When the unmixing ran, ISO 8601 UTC (FCS 3.2 `$LAST_MODIFIED`).
pub const LAST_MODIFIED_KEYWORD: &str = "$LAST_MODIFIED";
/// FCS 3.2 provenance flag; always `DataModified` for an unmixed product.
pub const ORIGINALITY_KEYWORD: &str = "$ORIGINALITY";

/// A fit-metric keyword bound to the field it serializes.
type FitKeyword = (&'static str, fn(&UnmixFitProvenance) -> f64);

/// Fit-metric keywords, paired with their accessor so write and read cannot
/// drift apart. Adding a metric here wires up both directions at once.
const FIT_KEYWORDS: [FitKeyword; 5] = [
    ("$TRUOLS_FIT_R2_MEAN", |f| f.r_squared_mean),
    ("$TRUOLS_FIT_R2_MEDIAN", |f| f.r_squared_median),
    ("$TRUOLS_FIT_RESID_MEAN", |f| f.residual_abs_mean),
    ("$TRUOLS_FIT_RESID_MEDIAN", |f| f.residual_abs_median),
    ("$TRUOLS_FIT_RESID_MAX", |f| f.residual_abs_max),
];

/// Goodness-of-fit summary, recorded only when the caller asked for it.
///
/// Mirrors the scalar fields of [`crate::metrics::FitMetrics`] without the
/// per-event vector, which would be as large as the DATA segment.
#[derive(Debug, Clone, PartialEq)]
pub struct UnmixFitProvenance {
    pub r_squared_mean: f64,
    pub r_squared_median: f64,
    pub residual_abs_mean: f64,
    pub residual_abs_median: f64,
    pub residual_abs_max: f64,
}

impl From<&crate::metrics::FitMetrics> for UnmixFitProvenance {
    fn from(fit: &crate::metrics::FitMetrics) -> Self {
        Self {
            r_squared_mean: fit.r_squared_mean,
            r_squared_median: fit.r_squared_median,
            residual_abs_mean: fit.residual_abs_mean,
            residual_abs_median: fit.residual_abs_median,
            residual_abs_max: fit.residual_abs_max,
        }
    }
}

/// Everything needed to explain how an unmixed file was produced.
///
/// The matrix and the two name lists are required - they are what makes the
/// file interpretable. Everything else is optional because the two export paths
/// know different amounts: the trait path
/// ([`crate::fcs_integration::apply_tru_ols_unmixing`]) receives precomputed
/// cutoffs and so cannot know the percentile they came from, whereas
/// [`crate::pipeline::export_unmixed_fcs`] computes them and can.
#[derive(Debug, Clone, PartialEq)]
pub struct UnmixProvenance {
    /// Value of `$UNMIXED`, e.g. `TRU-OLS`.
    pub method: String,
    /// Detector channels consumed, in mixing-matrix row order.
    pub detector_names: Vec<String>,
    /// Endmembers produced, in mixing-matrix column order.
    pub endmember_names: Vec<String>,
    /// Row-major, `detector_names.len() * endmember_names.len()` entries.
    ///
    /// Narrowed to `f32` because that is what TEXT can round-trip; the solve
    /// itself runs in `f64`.
    pub mixing_matrix: Vec<f32>,
    pub af_endmember_index: Option<usize>,
    pub strategy: Option<UnmixingStrategy>,
    pub cutoff_percentile: Option<f64>,
    pub raw_datasource_guid: Option<String>,
    pub unstained_datasource_guid: Option<String>,
    pub unstained_info: Option<String>,
    /// `$LAST_MODIFIER`; defaults to this crate's name and version.
    pub software: String,
    /// `$LAST_MODIFIED`, ISO 8601 UTC.
    pub modified_at: String,
    pub fit: Option<UnmixFitProvenance>,
}

impl UnmixProvenance {
    /// A record carrying just the transform, stamped with the current time.
    ///
    /// Callers fill in whatever else they know. `mixing_matrix` is row-major
    /// with detectors as rows; a length that disagrees with the two name lists
    /// is rejected at [`write_to`](Self::write_to) rather than here, so a
    /// mismatch surfaces as a skipped keyword and a warning instead of an
    /// unwrap deep in an export.
    pub fn new(
        method: impl Into<String>,
        detector_names: Vec<String>,
        endmember_names: Vec<String>,
        mixing_matrix: Vec<f32>,
    ) -> Self {
        Self {
            method: method.into(),
            detector_names,
            endmember_names,
            mixing_matrix,
            af_endmember_index: None,
            strategy: None,
            cutoff_percentile: None,
            raw_datasource_guid: None,
            unstained_datasource_guid: None,
            unstained_info: None,
            software: default_software_tag(),
            modified_at: flow_fcs::datetime::now_iso8601_utc(),
            fit: None,
        }
    }

    /// Builds the record from a `faer` matrix, narrowing to `f32` row-major.
    pub fn from_matrix(
        method: impl Into<String>,
        detector_names: Vec<String>,
        endmember_names: Vec<String>,
        matrix: faer::MatRef<'_, f64>,
    ) -> Self {
        let (n_det, n_em) = (matrix.nrows(), matrix.ncols());
        let mut values = Vec::with_capacity(n_det * n_em);
        for r in 0..n_det {
            for c in 0..n_em {
                values.push(matrix[(r, c)] as f32);
            }
        }
        Self::new(method, detector_names, endmember_names, values)
    }

    /// True when the matrix length agrees with the two declared name lists.
    ///
    /// A matrix of the wrong shape produces silently wrong abundances if a
    /// downstream tool trusts it, so the writer refuses to emit one.
    pub fn is_shape_consistent(&self) -> bool {
        self.mixing_matrix.len() == self.detector_names.len() * self.endmember_names.len()
    }

    /// Writes every populated field onto `fcs`, leaving `$GUID` alone.
    ///
    /// Absent optional fields are *removed* rather than left alone, so writing
    /// a fresh record over a file unmixed earlier cannot leave a stale value
    /// from the previous run.
    ///
    /// Use [`stamp_onto`](Self::stamp_onto) when producing a new derived file.
    /// This method exists for the second pass, where a caller enriches a record
    /// already on the file (the pipeline knows the cutoff percentile and fit
    /// metrics that the trait path cannot) and must not churn the identity that
    /// the first pass minted.
    pub fn write_to(&self, fcs: &mut Fcs) {
        ensure_delimiter_survives_provenance(fcs);
        let keywords = &mut fcs.metadata.keywords;

        if self.is_shape_consistent() {
            keywords.insert(
                MIXMAT_KEYWORD.to_string(),
                Keyword::Mixed(MixedKeyword::MixingMatrix {
                    n_detectors: self.detector_names.len(),
                    n_endmembers: self.endmember_names.len(),
                    detector_names: self.detector_names.clone(),
                    endmember_names: self.endmember_names.clone(),
                    matrix_values: self.mixing_matrix.clone(),
                }),
            );
        } else {
            keywords.remove(MIXMAT_KEYWORD);
            tracing::warn!(
                detectors = self.detector_names.len(),
                endmembers = self.endmember_names.len(),
                values = self.mixing_matrix.len(),
                "mixing matrix shape is inconsistent; omitting {MIXMAT_KEYWORD}"
            );
        }

        set(keywords, UNMIXED_KEYWORD, Some(self.method.trim()));
        set(keywords, LAST_MODIFIER_KEYWORD, Some(&self.software));
        set(keywords, LAST_MODIFIED_KEYWORD, Some(&self.modified_at));
        set(keywords, ORIGINALITY_KEYWORD, Some(ORIGINALITY_DATA_MODIFIED));
        set(
            keywords,
            RAW_DATASOURCE_GUID_KEYWORD,
            self.raw_datasource_guid.as_deref(),
        );
        set(
            keywords,
            UNSTAINED_DATASOURCE_GUID_KEYWORD,
            self.unstained_datasource_guid.as_deref(),
        );
        set(
            keywords,
            UNSTAINEDINFO_KEYWORD,
            self.unstained_info.as_deref(),
        );
        set(
            keywords,
            STRATEGY_KEYWORD,
            self.strategy.map(strategy_to_str),
        );
        set(
            keywords,
            AF_INDEX_KEYWORD,
            self.af_endmember_index.map(|i| i.to_string()),
        );
        set(
            keywords,
            CUTOFF_PCT_KEYWORD,
            self.cutoff_percentile.map(|p| p.to_string()),
        );

        for (key, accessor) in FIT_KEYWORDS {
            set(keywords, key, self.fit.as_ref().map(|f| accessor(f).to_string()));
        }

    }

    /// [`write_to`](Self::write_to) plus a fresh product `$GUID`.
    ///
    /// The identity is minted here rather than by the caller because a derived
    /// file that inherits its source's `$GUID` is actively harmful: two
    /// distinct files then claim one identity, and `$RAW_DATASOURCE_GUID`
    /// points at the file it is written on. Routing both export paths through
    /// this one call is what makes that guarantee hold on both.
    pub fn stamp_onto(&self, fcs: &mut Fcs) {
        self.write_to(fcs);
        fcs.metadata.keywords.remove("GUID");
        fcs.metadata.keywords.remove("$GUID");
        fcs.metadata.validate_guid();
    }

    /// Recovers the record from a file, or `None` if it carries no mixing matrix.
    ///
    /// The matrix is the anchor: a file without one was not written by this
    /// crate (or was written before provenance existed), and reporting partial
    /// provenance for it would be misleading. Every other field is best-effort.
    pub fn read_from(fcs: &Fcs) -> Option<Self> {
        let (detector_names, endmember_names, mixing_matrix) =
            match lookup(fcs, MIXMAT_KEYWORD)? {
                Keyword::Mixed(MixedKeyword::MixingMatrix {
                    detector_names,
                    endmember_names,
                    matrix_values,
                    ..
                }) => (
                    detector_names.clone(),
                    endmember_names.clone(),
                    matrix_values.clone(),
                ),
                // Present but not parsed as a matrix: a hand-edited or truncated
                // value. Treat it as absent rather than guessing at a shape.
                _ => return None,
            };

        Some(Self {
            method: text(fcs, UNMIXED_KEYWORD).unwrap_or_default(),
            detector_names,
            endmember_names,
            mixing_matrix,
            af_endmember_index: text(fcs, AF_INDEX_KEYWORD).and_then(|s| s.parse().ok()),
            strategy: text(fcs, STRATEGY_KEYWORD).and_then(|s| strategy_from_str(&s)),
            cutoff_percentile: text(fcs, CUTOFF_PCT_KEYWORD).and_then(|s| s.parse().ok()),
            raw_datasource_guid: text(fcs, RAW_DATASOURCE_GUID_KEYWORD),
            unstained_datasource_guid: text(fcs, UNSTAINED_DATASOURCE_GUID_KEYWORD),
            unstained_info: text(fcs, UNSTAINEDINFO_KEYWORD),
            software: text(fcs, LAST_MODIFIER_KEYWORD).unwrap_or_default(),
            modified_at: text(fcs, LAST_MODIFIED_KEYWORD).unwrap_or_default(),
            fit: read_fit(fcs),
        })
    }
}

/// `$UNMIXED`, re-exported here so provenance keywords sit in one list.
pub use crate::fcs_integration::UNMIXED_KEYWORD;

/// Value of `$ORIGINALITY` for a file whose DATA segment has been recomputed.
pub const ORIGINALITY_DATA_MODIFIED: &str = "DataModified";

/// `name/version`, deliberately with no space in it - see
/// [`ensure_delimiter_survives_provenance`].
fn default_software_tag() -> String {
    format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

/// TEXT delimiter used when a file's own delimiter would corrupt provenance.
///
/// Form feed, which is what Cytek and most FCS 3.1 writers use and what every
/// writer test in `flow-fcs` already sets.
pub const SAFE_TEXT_DELIMITER: char = '\u{000c}';

/// Moves the file off a space (or NUL) TEXT delimiter before provenance is written.
///
/// The writer does not escape the delimiter inside values (`flow-crates-1xb`),
/// so a value containing it truncates and desynchronizes the entire rest of
/// TEXT on reopen. Provenance is the first thing this crate writes that
/// routinely contains spaces - `$UNSTAINEDINFO` is free text by definition -
/// so a space-delimited source file would lose every keyword after the first
/// provenance value, `$PnN` included.
///
/// This is a containment measure, not the fix: a free-text value can contain a
/// form feed too. It is applied here because provenance is where the exposure
/// starts.
fn ensure_delimiter_survives_provenance(fcs: &mut Fcs) {
    if fcs.metadata.delimiter == ' ' || fcs.metadata.delimiter == '\0' {
        fcs.metadata.delimiter = SAFE_TEXT_DELIMITER;
    }
}

/// Stable TEXT spelling of a strategy.
///
/// Deliberately not `Debug`: the derived form would silently change the file
/// format if a variant were ever renamed.
pub fn strategy_to_str(strategy: UnmixingStrategy) -> String {
    match strategy {
        UnmixingStrategy::Zero => "Zero",
        UnmixingStrategy::UnstainedControlMapping => "UnstainedControlMapping",
    }
    .to_string()
}

/// Inverse of [`strategy_to_str`]; `None` for anything unrecognised.
pub fn strategy_from_str(value: &str) -> Option<UnmixingStrategy> {
    match value.trim() {
        "Zero" => Some(UnmixingStrategy::Zero),
        "UnstainedControlMapping" => Some(UnmixingStrategy::UnstainedControlMapping),
        _ => None,
    }
}

/// Inserts `key` when `value` is `Some` and non-empty, removes it otherwise.
fn set<S: std::hash::BuildHasher>(
    keywords: &mut std::collections::HashMap<String, Keyword, S>,
    key: &str,
    value: Option<impl AsRef<str>>,
) {
    match value.as_ref().map(|v| v.as_ref().trim()) {
        Some(v) if !v.is_empty() => {
            keywords.insert(
                key.to_string(),
                Keyword::String(StringKeyword::Other(Arc::from(v))),
            );
        }
        _ => {
            keywords.remove(key);
            keywords.remove(key.trim_start_matches('$'));
        }
    }
}

/// Looks a keyword up under both its `$`-prefixed and bare spellings.
///
/// The writer `$`-prefixes every key, but metadata assembled in memory - which
/// is what the round-trip tests and any caller using the trait path plus their
/// own writer see - may still hold the bare form.
fn lookup<'a>(fcs: &'a Fcs, key: &str) -> Option<&'a Keyword> {
    let bare = key.trim_start_matches('$');
    fcs.metadata
        .keywords
        .get(key)
        .or_else(|| fcs.metadata.keywords.get(bare))
}

fn text(fcs: &Fcs, key: &str) -> Option<String> {
    let value = lookup(fcs, key)?.value_str()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// All five fit metrics or none - a partial set would invite averaging over a
/// mix of runs.
fn read_fit(fcs: &Fcs) -> Option<UnmixFitProvenance> {
    let mut values = [0.0_f64; FIT_KEYWORDS.len()];
    for (slot, (key, _)) in values.iter_mut().zip(FIT_KEYWORDS) {
        *slot = text(fcs, key)?.parse().ok()?;
    }
    Some(UnmixFitProvenance {
        r_squared_mean: values[0],
        r_squared_median: values[1],
        residual_abs_mean: values[2],
        residual_abs_median: values[3],
        residual_abs_max: values[4],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
        use flow_fcs::{Header, Metadata};
    use flow_fcs::file::AccessWrapper;
    use flow_fcs::parameter::{Parameter, ParameterMap};
    use flow_fcs::transform::TransformType;
    use polars::prelude::{Column, DataFrame};
    use std::sync::Arc;

    /// Values are exact in `f32` and survive an ASCII round trip bit-for-bit,
    /// so a mismatch means the encoding is wrong rather than that the last
    /// decimal digit was rounded.
    const MATRIX: [f32; 6] = [0.9375, 0.125, -0.03125, 0.0625, 0.8125, 0.25];

    fn detectors() -> Vec<String> {
        vec!["B1-A".to_string(), "B2-A".to_string()]
    }

    fn endmembers() -> Vec<String> {
        vec!["FITC".to_string(), "PE".to_string(), "AF".to_string()]
    }

    /// A two-parameter file that `write_fcs_file` will accept, so the same
    /// fixture serves both the in-memory and the on-disk round trip.
    ///
    /// `AccessWrapper::new` opens the path, so the file has to exist before the
    /// `Fcs` can be built - even for the tests that never touch the disk again.
    fn minimal_fcs(path: &std::path::Path) -> Fcs {
        std::fs::write(path, b"").expect("create fixture placeholder");
        let df = DataFrame::new_infer_height(vec![
            Column::new("FITC".into(), vec![1.0_f32, 2.0, 3.0]),
            Column::new("PE".into(), vec![4.0_f32, 5.0, 6.0]),
        ])
        .expect("build fixture frame");

        let mut params = ParameterMap::default();
        params.insert(
            "FITC".into(),
            Parameter::new(&1, "FITC", "FITC", &TransformType::Linear),
        );
        params.insert(
            "PE".into(),
            Parameter::new(&2, "PE", "PE", &TransformType::Linear),
        );

        // Builds `$PAR`, `$TOT` and the full `$Pn*` set, which the reader needs
        // to rebuild a parameter map on reopen.
        let mut metadata = Metadata::from_dataframe_and_parameters(&df, &params)
            .expect("build fixture metadata");
        metadata.insert_string_keyword("$GUID".to_string(), "source-guid-0001".to_string());

        Fcs {
            header: Header::new(),
            metadata,
            parameters: params,
            data_frame: Arc::new(df),
            file_access: AccessWrapper::new(path.to_str().unwrap_or(""))
                .expect("build fixture access wrapper"),
            dataset_start: 0,
        }
    }

    fn fully_populated() -> UnmixProvenance {
        let mut prov = UnmixProvenance::new(
            "TRU-OLS",
            detectors(),
            endmembers(),
            MATRIX.to_vec(),
        );
        prov.af_endmember_index = Some(2);
        prov.strategy = Some(UnmixingStrategy::UnstainedControlMapping);
        prov.cutoff_percentile = Some(0.995);
        prov.raw_datasource_guid = Some("raw-guid-1234".to_string());
        prov.unstained_datasource_guid = Some("unstained-guid-5678".to_string());
        prov.unstained_info = Some("AF endmember from unstained control".to_string());
        prov.fit = Some(UnmixFitProvenance {
            r_squared_mean: 0.98,
            r_squared_median: 0.99,
            residual_abs_mean: 0.01,
            residual_abs_median: 0.008,
            residual_abs_max: 0.5,
        });
        prov
    }

    #[test]
    fn every_field_survives_an_in_memory_round_trip() {
        let dir = std::env::temp_dir();
        let mut fcs = minimal_fcs(&dir.join("prov-mem.fcs"));
        let written = fully_populated();
        written.write_to(&mut fcs);

        let read = UnmixProvenance::read_from(&fcs).expect("provenance should be recoverable");
        assert_eq!(read, written);
    }

    /// The point of the exercise: the record has to survive TEXT serialization,
    /// not just a `HashMap`. An unhandled variant in the writer would land the
    /// matrix in `StringKeyword::Other`, which round-trips the *text* perfectly
    /// while losing the type - so this asserts on the parsed values.
    #[test]
    fn every_field_survives_write_and_reopen() {
        let path = std::env::temp_dir().join("truols-prov-roundtrip.fcs");

        let mut fcs = minimal_fcs(&path);
        let written = fully_populated();
        written.write_to(&mut fcs);
        flow_fcs::write_fcs_file(fcs, &path).expect("write fixture");

        let reopened = Fcs::open(path.to_str().expect("utf-8 temp path"))
            .expect("reopen fixture");
        let read = UnmixProvenance::read_from(&reopened).expect("provenance should survive write");

        assert_eq!(read.detector_names, detectors());
        assert_eq!(read.endmember_names, endmembers());
        assert_eq!(read.mixing_matrix, MATRIX.to_vec());
        assert_eq!(read.method, "TRU-OLS");
        assert_eq!(read.af_endmember_index, Some(2));
        assert_eq!(
            read.strategy,
            Some(UnmixingStrategy::UnstainedControlMapping)
        );
        assert_eq!(read.cutoff_percentile, Some(0.995));
        assert_eq!(read.raw_datasource_guid.as_deref(), Some("raw-guid-1234"));
        assert_eq!(
            read.unstained_datasource_guid.as_deref(),
            Some("unstained-guid-5678")
        );
        assert_eq!(read.fit, written.fit);
        assert_eq!(read.software, written.software);
        assert_eq!(read.modified_at, written.modified_at);
    }

    /// Regression for the failure that surfaced when this module was written:
    /// `$UNSTAINEDINFO` is free text and `$LAST_MODIFIER` carries a version, so
    /// on a space-delimited file the unescaped delimiter truncated the value
    /// and desynchronized every keyword after it - `Fcs::open` then failed with
    /// "No $P1N keyword stored". See `flow-crates-1xb` for the underlying
    /// writer bug this contains.
    #[test]
    fn a_space_delimited_source_still_reopens_after_stamping() {
        let path = std::env::temp_dir().join("truols-prov-space-delim.fcs");
        let mut fcs = minimal_fcs(&path);
        assert_eq!(
            fcs.metadata.delimiter, ' ',
            "fixture must start on the delimiter that triggers the bug"
        );

        let mut prov = fully_populated();
        prov.unstained_info = Some("AF from unstained control, 99.5th percentile".to_string());
        prov.write_to(&mut fcs);
        assert_eq!(fcs.metadata.delimiter, SAFE_TEXT_DELIMITER);

        flow_fcs::write_fcs_file(fcs, &path).expect("write fixture");
        let reopened =
            Fcs::open(path.to_str().expect("utf-8 temp path")).expect("reopen fixture");

        // The parameter keywords are the canary: they sort after `$LAST_MODIFIER`
        // and so were the first casualties of the desync.
        assert!(reopened.metadata.keywords.contains_key("$P1N"));
        let read = UnmixProvenance::read_from(&reopened).expect("provenance recoverable");
        assert_eq!(
            read.unstained_info.as_deref(),
            Some("AF from unstained control, 99.5th percentile")
        );
    }

    /// A derived file that keeps its source's `$GUID` makes two distinct files
    /// claim one identity, and points `$RAW_DATASOURCE_GUID` at the file it is
    /// written on.
    #[test]
    fn stamping_mints_an_identity_distinct_from_the_source() {
        let dir = std::env::temp_dir();
        let mut fcs = minimal_fcs(&dir.join("prov-guid.fcs"));
        let source_guid = fcs
            .metadata
            .keywords
            .get("$GUID")
            .and_then(|k| k.value_str())
            .expect("fixture has a source GUID")
            .to_string();

        fully_populated().stamp_onto(&mut fcs);

        let product_guid = fcs
            .metadata
            .keywords
            .get("$GUID")
            .and_then(|k| k.value_str())
            .expect("product must carry a GUID")
            .to_string();

        assert_ne!(product_guid, source_guid);
        assert!(!product_guid.is_empty());
    }

    /// Enriching a record already on the file must not churn the identity the
    /// first pass minted - `$RAW_DATASOURCE_GUID` on some other file may
    /// already point at it.
    #[test]
    fn enriching_an_existing_record_preserves_the_product_identity() {
        let path = std::env::temp_dir().join("truols-prov-enrich.fcs");
        let mut fcs = minimal_fcs(&path);
        UnmixProvenance::new("TRU-OLS", detectors(), endmembers(), MATRIX.to_vec())
            .stamp_onto(&mut fcs);

        let minted = fcs.metadata.keywords["$GUID"].value_str().unwrap().to_string();

        let mut enriched = UnmixProvenance::read_from(&fcs).expect("recoverable");
        enriched.cutoff_percentile = Some(0.995);
        enriched.write_to(&mut fcs);

        assert_eq!(
            fcs.metadata.keywords["$GUID"].value_str().unwrap(),
            minted.as_str()
        );
        assert_eq!(
            UnmixProvenance::read_from(&fcs).unwrap().cutoff_percentile,
            Some(0.995)
        );
    }

    /// Re-stamping must not leave a scalar from the previous run behind, which
    /// would read back as provenance for a transform that never happened.
    #[test]
    fn absent_optional_fields_clear_a_previous_stamp() {
        let dir = std::env::temp_dir();
        let mut fcs = minimal_fcs(&dir.join("prov-restamp.fcs"));
        fully_populated().write_to(&mut fcs);

        UnmixProvenance::new("OLS", detectors(), endmembers(), MATRIX.to_vec())
            .write_to(&mut fcs);

        let read = UnmixProvenance::read_from(&fcs).expect("still recoverable");
        assert_eq!(read.method, "OLS");
        assert_eq!(read.cutoff_percentile, None);
        assert_eq!(read.strategy, None);
        assert_eq!(read.af_endmember_index, None);
        assert_eq!(read.raw_datasource_guid, None);
        assert_eq!(read.fit, None);
    }

    #[test]
    fn a_file_without_a_mixing_matrix_reports_no_provenance() {
        let dir = std::env::temp_dir();
        let mut fcs = minimal_fcs(&dir.join("prov-none.fcs"));
        assert!(UnmixProvenance::read_from(&fcs).is_none());

        // Even a file carrying some of the scalars is not provenance without
        // the transform itself.
        set(
            &mut fcs.metadata.keywords,
            STRATEGY_KEYWORD,
            Some("Zero"),
        );
        assert!(UnmixProvenance::read_from(&fcs).is_none());
    }

    /// A shape mismatch would produce silently wrong abundances downstream, so
    /// the writer drops the keyword rather than emitting a matrix it cannot
    /// describe.
    #[test]
    fn an_inconsistent_shape_is_refused_rather_than_reshaped() {
        let dir = std::env::temp_dir();
        let mut fcs = minimal_fcs(&dir.join("prov-badshape.fcs"));

        let mut prov = fully_populated();
        prov.mixing_matrix.pop();
        assert!(!prov.is_shape_consistent());
        prov.write_to(&mut fcs);

        assert!(!fcs.metadata.keywords.contains_key(MIXMAT_KEYWORD));
        assert!(UnmixProvenance::read_from(&fcs).is_none());
    }

    /// Partial fit metrics would invite averaging across a mix of runs.
    #[test]
    fn fit_metrics_are_all_or_nothing() {
        let dir = std::env::temp_dir();
        let mut fcs = minimal_fcs(&dir.join("prov-fit.fcs"));
        fully_populated().write_to(&mut fcs);

        fcs.metadata.keywords.remove(FIT_KEYWORDS[3].0);
        let read = UnmixProvenance::read_from(&fcs).expect("still recoverable");
        assert_eq!(read.fit, None);
    }

    /// The TEXT spelling is part of the file format, so it must not track a
    /// variant rename the way a derived `Debug` impl would.
    #[test]
    fn strategy_spellings_are_pinned_and_reversible() {
        for (strategy, spelling) in [
            (UnmixingStrategy::Zero, "Zero"),
            (
                UnmixingStrategy::UnstainedControlMapping,
                "UnstainedControlMapping",
            ),
        ] {
            assert_eq!(strategy_to_str(strategy), spelling);
            assert_eq!(strategy_from_str(spelling), Some(strategy));
        }
        assert_eq!(strategy_from_str("Nonsense"), None);
    }

    #[test]
    fn from_matrix_narrows_row_major_with_detectors_as_rows() {
        let m = faer::mat![[1.0_f64, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let prov = UnmixProvenance::from_matrix("TRU-OLS", detectors(), endmembers(), m.as_ref());
        assert_eq!(prov.mixing_matrix, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(prov.is_shape_consistent());
    }
}
