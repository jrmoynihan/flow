#![allow(deprecated)]
mod helpers;
mod parsing;
#[cfg(test)]
mod tests;
use parsing::*;

use crate::{byteorder::ByteOrder, datatype::FcsDataType};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, hash::Hash, sync::Arc};
use strum_macros::Display;

/// Result of parsing a keyword-value pair from the FCS TEXT segment
///
/// This enum represents the possible types a keyword can be parsed as.
/// The parsing logic attempts to match the keyword name and value format
/// to determine the appropriate type.
#[derive(Debug)]
pub enum KeywordCreationResult {
    /// Successfully parsed as an integer keyword (e.g., `$PAR`, `$TOT`)
    Int(IntegerKeyword),
    /// Successfully parsed as a float keyword (e.g., `$PnG`)
    Float(FloatKeyword),
    /// Successfully parsed as a string keyword (e.g., `$CYT`, `$FIL`, `$GUID`)
    String(StringKeyword),
    /// Successfully parsed as a byte-oriented keyword (e.g., `$BYTEORD`, `$DATATYPE`)
    Byte(ByteKeyword),
    /// Successfully parsed as a mixed-type keyword (e.g., `$SPILLOVER`, `$PnD`, `$PnE`)
    Mixed(MixedKeyword),
    /// Unable to parse the keyword-value pair (fallback to generic string storage)
    UnableToParse,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Display)]
pub enum Keyword {
    Int(IntegerKeyword),
    Float(FloatKeyword),
    String(StringKeyword),
    Byte(ByteKeyword),
    Mixed(MixedKeyword),
}

type LowerBound = f32;
type UpperBound = f32;

#[derive(Clone, Debug, Display, Serialize, Deserialize, PartialEq)]
#[allow(deprecated)]
pub enum MixedKeyword {
    /// Specifies the conversion of arbitrary signal units, recorded as parameter values (uncompensated or compensated)
    /// to some well defined unit. For example, mean equivalent soluble fluorochrome (MESF) or antibody molecules.
    /// * f1 - the number of calibrated units corresponding to a unit signal value of parameter n
    ///
    /// * str - name of the units corresponding to calibration value
    ///
    /// **Example:** If the signal on parameter n has the scale value X then the calibrated value is X * f units
    PnCalibration(f32, String),
    /// Recommends visualization scale for parameter `n`.
    /// * String is either "Linear" or "Logarithmic".
    /// * f1 and f2 parameter values are in "scale" units, not "channel" units, see below for details.
    /// * For linear scaling:
    ///   - f1: Lower bound - the scale value corresponding to the left edge of the display
    ///   - f2: Upper bound - the scale value corresponding to the right edge of the display
    /// * *For logarithmic scaling:*
    ///   - f1: Decades - The number of decades to display.
    ///   - f2: Offset - The scale value corresponding to the left edge of the display
    ///
    /// **Example**: `$P3D (Linear,0,1024)`
    /// - Specifies a linear display range with scale parameter values ranging from 0 to 1024.
    ///
    /// **Example**: `$P2D (Logarithmic,4,0.1)`
    /// - Specifies a logarithmic display ranging from 0.1 to 1000 (scale value), which is *4* decades of display width.
    ///
    /// **Example**: `$P1D (Logarithmic,5,0.01)`
    /// - Specifies a logarithmic display ranging from 0.01 to 1000 (scale value), which is 5 decades of display width.
    ///
    /// **Example**: `$P3B (8) | $P3R (256) | $P3G (4) | $P3E (0,0) | $P3D (Linear,0,32)`:
    /// - This is a linear parameter with channel values going from 0 to 255 (`$P3R`). Taking account the gain (`$PnG`),
    /// the *scale* values go from 0 to 64 (256/4 = 64). The $P3D specifies a linear display from 0 to 32
    /// scale units, which only encompasses the bottom half of the collected data range on this scale.
    ///
    /// **Example**: `$P4B (16) | $P4R (1024) | $P4E (4,1) | $P4D (Linear,0,1000)`
    /// - Specifies a linear display, with channel values going from 0 to 1023 (`$P4R`).
    /// Only the bottom 10th of the scale values shown.
    /// This will restrict the display to channel values between 0 and 768 (the bottom 3 decades),
    /// with channels being distributed exponentially in the linear display.
    ///
    /// **Example**: `$P4B (16) | $P4R (1024) | $P4E (4,1) | $P4D (Logarithmic,3,1)`:
    /// - The display keyword specifies that the data should be shown in logarithmic scaling, with only the bottom 3 decades shown.
    /// This will restrict the display to channel values between 0 and 768 (1024*3/4).
    ///
    PnD(String, LowerBound, UpperBound),

    /// (f1, f2) -Amplification type for parameter n. (FCS 1.0+)
    /// * f1 - number of logarithmic decades
    /// * f2 - linear value obtained for a signal with log value = 0
    /// * 0,0 when the parameter is Linear.
    /// * Also 0,0 when floating-point data (`$DATATYPE` = F or `$DATATYPE` = D) is stored.
    /// **Example**: `$P3E (4,1)` - 4 decades with offset of 1
    PnE(f32, f32),

    /// Gate n amplification type.
    ///
    /// *<small>(FCS v2.0-3.1, deprecated)</small>*
    #[deprecated(since = "3.1.0", note = "Use PnE instead")]
    GnE(f32, f32),

    /// Region n width values - vector of width values for region boundaries
    /// **Example**: `$R1W (0.5,1.2,0.8)` - Three width values
    RnW(Vec<f32>),

    /// Spillover matrix for compensation
    /// Format: n, [param_names...], [matrix_values...]
    /// **Example**: `$SPILLOVER/3,FL2-A,FL1-A,FL3-A,1.0,0.03,0.2,0.1,1.0,0.0,0.05,0,1.0`
    SPILLOVER {
        n_parameters: usize,
        parameter_names: Vec<String>,
        matrix_values: Vec<f32>,
    },

    /// Excitation wavelength(s) for parameter n in nanometers (FCS 1.0+, format updated in FCS 3.1)
    /// Can contain single or multiple wavelengths for co-axial lasers
    /// **Example**: `$P3L (488)` - single wavelength
    /// **Example**: `$P4L (488,532,633)` - multiple co-axial lasers
    PnL(Vec<usize>),
}

impl StringableKeyword for MixedKeyword {
    #[allow(deprecated)]
    fn get_str(&self) -> Cow<'_, str> {
        match self {
            Self::PnCalibration(f1, s) => Cow::Owned(format!("PnCalibration({}, {})", f1, s)),
            Self::PnD(s, f1, f2) => Cow::Owned(format!("PnD({}, {}, {})", s, f1, f2)),
            Self::PnE(f1, f2) => Cow::Owned(format!("PnE({}, {})", f1, f2)),
            Self::GnE(f1, f2) => Cow::Owned(format!("GnE({}, {})", f1, f2)),
            Self::PnL(vec) => Cow::Owned(format!(
                "PnL({})",
                vec.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Self::RnW(vec) => Cow::Owned(format!(
                "RnW({})",
                vec.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Self::SPILLOVER {
                n_parameters,
                parameter_names,
                matrix_values,
            } => Cow::Owned(format!(
                "SPILLOVER({}, {}, {})",
                n_parameters,
                parameter_names.join(", "),
                matrix_values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

impl Eq for MixedKeyword {}
impl Hash for MixedKeyword {
    #[allow(deprecated)]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::PnCalibration(f1, s) => {
                f1.to_bits().hash(state);
                s.hash(state);
            }
            Self::PnD(s, f1, f2) => {
                s.hash(state);
                f1.to_bits().hash(state);
                f2.to_bits().hash(state);
            }
            Self::PnE(f1, f2) | Self::GnE(f1, f2) => {
                f1.to_bits().hash(state);
                f2.to_bits().hash(state);
            }
            Self::PnL(vec) => {
                for v in vec {
                    v.hash(state);
                }
            }
            Self::RnW(vec) => {
                for f in vec {
                    f.to_bits().hash(state);
                }
            }
            Self::SPILLOVER {
                n_parameters,
                parameter_names,
                matrix_values,
            } => {
                n_parameters.hash(state);
                parameter_names.hash(state);
                for f in matrix_values {
                    f.to_bits().hash(state);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Display, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IntegerKeyword {
    /// The offset to the beginning of the DATA segment (FCS 1.0+)
    BeginData(usize),
    /// The offset to the end of the DATA segment (FCS 1.0+)
    EndData(usize),
    /// The offset to the beginning of the ANALYSIS segment (FCS 2.0+)
    BeginAnalysis(usize),
    /// The offset to the end of the ANALYSIS segment (FCS 2.0+)
    EndAnalysis(usize),
    /// The offset to the beginning of the TEXT segment (FCS 1.0+)
    BeginText(usize),
    /// The offset to the end of the TEXT segment (FCS 1.0+)
    EndText(usize),
    /// The number of parameters in the dataset (FCS 1.0+)
    PAR(usize),
    /// The number of events in the dataset (FCS 1.0+)
    TOT(usize),
    /// Range for parameter `n` (FCS 1.0+)
    PnR(usize),
    /// Number of bits reserved for parameter `n` (FCS 1.0+)
    PnB(usize),
    /// Voltage range for parameter `n` (FCS 1.0+)
    PnV(usize),
    /// Excitation wavelength for parameter `n` (FCS 1.0+)
    PnL(usize),
    /// The transformation to apply when displaying the data (FCS 1.0+)
    PnDisplay(usize),
    /// Data type for parameter `n` (FCS 3.2+), overriding the default $DATATYPE for a given parameter
    PnDATATYPE(usize),
}

#[derive(Clone, Debug, Display, Serialize, Deserialize, PartialEq)]
pub enum FloatKeyword {
    /// Gain for parameter n
    PnG(f32),
}

impl Eq for FloatKeyword {}
impl Hash for FloatKeyword {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            FloatKeyword::PnG(f) => f.to_bits().hash(state),
        }
    }
}

#[derive(Clone, Debug, Display, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[allow(deprecated)]
pub enum StringKeyword {
    /// The name of the cytometer used to acquire the data (FCS 1.0+)
    CYT(Arc<str>),
    /// The name of the file containing the dataset (FCS 1.0+)
    FIL(Arc<str>),
    /// The globally unique identifier for the dataset (FCS 2.0+)
    GUID(Arc<str>),

    /// Begin date and time of data acquisition (FCS 3.2+)
    BEGINDATETIME(Arc<str>),
    /// End date and time of data acquisition (FCS 3.2+)
    ENDDATETIME(Arc<str>),

    /// Generic sample carrier identifier (FCS 3.2+, replaces $PLATEID)
    CARRIERID(Arc<str>),
    /// Type of sample carrier (FCS 3.2+, replaces $PLATENAME)
    CARRIERTYPE(Arc<str>),
    /// Location identifier within carrier (FCS 3.2+, replaces $WELLID)
    LOCATIONID(Arc<str>),

    /// 'Short name' for parameter `n` (FCS 1.0+)
    PnN(Arc<str>),
    /// Label name for parameter `n` (FCS 1.0+)
    PnS(Arc<str>),
    /// Name of the optical filter for parameter `n` (FCS 1.0+)
    PnF(Arc<str>),
    /// The FCS measurement signal types and evaluation features (e.g., area, height, or width) (FCS 1.0+)
    PnType(Arc<str>),

    /// Detector name for parameter `n` (FCS 3.2+)
    PnDET(Arc<str>),
    /// Dye specification for parameter `n` (FCS 3.2+)
    PnTAG(Arc<str>),
    /// Target molecule or process for parameter `n` (FCS 3.2+)
    PnANALYTE(Arc<str>),
    /// Evaluation features for parameter `n` (FCS 3.2+)
    PnFEATURE(Arc<str>),

    /// Acquisition flow rate setting (FCS 3.2+)
    FLOWRATE(Arc<str>),

    /// Sample volume (FCS 3.1+)
    VOL(Arc<str>),

    /// Distinguish between original and altered data set (FCS 3.1+)
    ORIGINALITY(Arc<str>),
    /// Who last modified the data set (FCS 3.1+)
    LastModifier(Arc<str>),
    /// When the data set was last modified (FCS 3.1+)
    LastModified(Arc<str>),

    /// Date of data acquisition
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $BEGINDATETIME)</small>
    #[deprecated(since = "3.2.0", note = "Use BEGINDATETIME instead")]
    DATE(Arc<str>),

    /// Begin time of data acquisition
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $BEGINDATETIME)</small>
    #[deprecated(since = "3.2.0", note = "Use BEGINDATETIME instead")]
    BTIM(Arc<str>),

    /// End time of data acquisition
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $ENDDATETIME)</small>
    #[deprecated(since = "3.2.0", note = "Use ENDDATETIME instead")]
    ETIM(Arc<str>),

    /// Data acquisition mode
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2, fixed to "L" list mode)</small>
    #[deprecated(since = "3.2.0", note = "Fixed to 'L' list mode in FCS 3.2")]
    MODE(Arc<str>),

    /// Plate identifier
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $CARRIERID)</small>
    #[deprecated(since = "3.2.0", note = "Use CARRIERID instead")]
    PLATEID(Arc<str>),

    /// Platform/plate name
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $CARRIERTYPE)</small>
    #[deprecated(since = "3.2.0", note = "Use CARRIERTYPE instead")]
    PLATENAME(Arc<str>),

    /// Well identifier
    ///
    /// <small>(FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $LOCATIONID)</small>
    #[deprecated(since = "3.2.0", note = "Use LOCATIONID instead")]
    WELLID(Arc<str>),

    /// Gate definition
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GATE(Arc<str>),

    /// Gate n optical filter
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnF(Arc<str>),

    /// Gate n short name
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnN(Arc<str>),

    /// Gate n population name
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnP(Arc<str>),

    /// Gate n range
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnR(Arc<str>),

    /// Gate n label name
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnS(Arc<str>),

    /// Gate n threshold
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnT(Arc<str>),

    /// Gate n voltage range
    ///
    /// <small>(FCS 2.0-3.1, removed in FCS 3.2)</small>
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnV(Arc<str>),

    /// A catch-all for other keywords, to be stored as Arc<str>
    Other(Arc<str>),
}

// Keywords regarding the data-layout, lacking any associated values
#[derive(Clone, Debug, Display, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ByteKeyword {
    /// The byte order (endianness) of the data
    BYTEORD(ByteOrder),
    /// The data type of the FCS file (number of bytes per event)
    DATATYPE(FcsDataType),
}

pub trait StringableKeyword {
    fn get_str(&self) -> Cow<'_, str>;
}
pub trait IntegerableKeyword {
    fn get_usize(&self) -> &usize;
}
#[allow(unused)]
pub trait FloatableKeyword {
    fn get_f32(&self) -> &f32;
}

impl IntegerableKeyword for IntegerKeyword {
    fn get_usize(&self) -> &usize {
        match self {
            Self::TOT(value)
            | Self::BeginData(value)
            | Self::EndData(value)
            | Self::BeginAnalysis(value)
            | Self::EndAnalysis(value)
            | Self::BeginText(value)
            | Self::EndText(value)
            | Self::PnR(value)
            | Self::PnB(value)
            | Self::PnV(value)
            | Self::PnL(value)
            | Self::PnDisplay(value)
            | Self::PnDATATYPE(value)
            | Self::PAR(value) => value,
        }
    }
}

impl FloatableKeyword for FloatKeyword {
    fn get_f32(&self) -> &f32 {
        match self {
            Self::PnG(value) => value,
        }
    }
}

impl StringableKeyword for StringKeyword {
    /// Get a reference to the string value (if it exists) from a StringKeyword variant
    #[allow(deprecated)]
    fn get_str(&self) -> Cow<'_, str> {
        match self {
            Self::CYT(value)
            | Self::FIL(value)
            | Self::GUID(value)
            | Self::BEGINDATETIME(value)
            | Self::ENDDATETIME(value)
            | Self::CARRIERID(value)
            | Self::CARRIERTYPE(value)
            | Self::LOCATIONID(value)
            | Self::PnN(value)
            | Self::PnS(value)
            | Self::PnF(value)
            | Self::PnType(value)
            | Self::PnDET(value)
            | Self::PnTAG(value)
            | Self::PnANALYTE(value)
            | Self::PnFEATURE(value)
            | Self::FLOWRATE(value)
            | Self::VOL(value)
            | Self::ORIGINALITY(value)
            | Self::LastModifier(value)
            | Self::LastModified(value)
            | Self::DATE(value)
            | Self::BTIM(value)
            | Self::ETIM(value)
            | Self::MODE(value)
            | Self::PLATEID(value)
            | Self::PLATENAME(value)
            | Self::WELLID(value)
            | Self::GATE(value)
            | Self::GnF(value)
            | Self::GnN(value)
            | Self::GnP(value)
            | Self::GnR(value)
            | Self::GnS(value)
            | Self::GnT(value)
            | Self::GnV(value)
            | Self::Other(value) => Cow::Borrowed(value.as_ref()),
        }
    }
}

impl StringableKeyword for ByteKeyword {
    /// Get a reference to the string value (if it exists) from a ByteKeyword variant
    fn get_str(&self) -> Cow<'_, str> {
        match self {
            Self::DATATYPE(data_type) => Cow::Borrowed(data_type.to_keyword_str()),
            Self::BYTEORD(byte_order) => Cow::Borrowed(byte_order.to_keyword_str()),
        }
    }
}

impl StringableKeyword for IntegerKeyword {
    fn get_str(&self) -> Cow<'_, str> {
        match self {
            Self::BeginData(value)
            | Self::EndData(value)
            | Self::BeginAnalysis(value)
            | Self::EndAnalysis(value)
            | Self::BeginText(value)
            | Self::EndText(value)
            | Self::PAR(value)
            | Self::TOT(value)
            | Self::PnR(value)
            | Self::PnB(value)
            | Self::PnV(value)
            | Self::PnL(value)
            | Self::PnDATATYPE(value)
            | Self::PnDisplay(value) => Cow::Owned(value.to_string()),
        }
    }
}

impl StringableKeyword for FloatKeyword {
    fn get_str(&self) -> Cow<'_, str> {
        match self {
            Self::PnG(value) => Cow::Owned(value.to_string()),
        }
    }
}

/// Main parsing entry point for FCS keywords
///
/// Dispatches to appropriate parsing functions based on keyword name pattern.
/// Attempts to match the keyword against known patterns (fixed keywords, parameter keywords,
/// gate keywords, region keywords) and parse the value accordingly.
///
/// # Arguments
/// * `key` - The keyword name (with or without `$` prefix)
/// * `value` - The keyword value as a string
///
/// # Returns
/// A `KeywordCreationResult` indicating the parsed type, or `UnableToParse` if no pattern matches
///
/// # Example
/// ```ignore
/// let result = match_and_parse_keyword("$PAR", "10");
/// // Returns KeywordCreationResult::Int(IntegerKeyword::PAR(10))
/// ```
pub fn match_and_parse_keyword(key: &str, value: &str) -> KeywordCreationResult {
    // Keywords without $ prefix should be treated as Other, not parsed
    // Exception: GUID keyword doesn't always have $ prefix in some FCS files
    let dollarless_key = if let Some(key) = key.strip_prefix('$') {
        key
    } else if key == "GUID" {
        // GUID is a special case - it can appear without $ prefix
        "GUID"
    } else {
        // No $ prefix - treat as unknown keyword
        return KeywordCreationResult::String(StringKeyword::Other(Arc::from(value.trim())));
    };

    parse_fixed_keywords(dollarless_key, value)
        .or_else(|| parse_parameter_keywords(dollarless_key, value))
        .or_else(|| parse_gate_keywords(dollarless_key, value))
        .or_else(|| parse_region_keywords(dollarless_key, value))
        .unwrap_or_else(|| {
            KeywordCreationResult::String(StringKeyword::Other(Arc::from(value.trim())))
        })
}

impl From<&StringKeyword> for Arc<str> {
    fn from(keyword: &StringKeyword) -> Self {
        keyword.get_str().into()
    }
}

// Extract the variant's value and convert it to a string
impl From<&IntegerKeyword> for String {
    fn from(keyword: &IntegerKeyword) -> Self {
        keyword.get_usize().to_string()
    }
}
