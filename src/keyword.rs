use super::{byteorder::ByteOrder, datatype::FcsDataType};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, hash::Hash, sync::Arc};
use strum_macros::Display;

pub enum KeywordCreationResult {
    Int(IntegerKeyword),
    Float(FloatKeyword),
    String(StringKeyword),
    Byte(ByteKeyword),
    Mixed(MixedKeyword),
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

impl Eq for MixedKeyword {}
impl Hash for MixedKeyword {
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
pub enum StringKeyword {
    /// The name of the cytometer used to acquire the data (FCS 1.0+)
    CYT(String),
    /// The name of the file containing the dataset (FCS 1.0+)
    FIL(String),
    /// The globally unique identifier for the dataset (FCS 2.0+)
    GUID(String),

    /// Begin date and time of data acquisition (FCS 3.2+)
    BEGINDATETIME(String),
    /// End date and time of data acquisition (FCS 3.2+)
    ENDDATETIME(String),

    /// Generic sample carrier identifier (FCS 3.2+, replaces $PLATEID)
    CARRIERID(String),
    /// Type of sample carrier (FCS 3.2+, replaces $PLATENAME)
    CARRIERTYPE(String),
    /// Location identifier within carrier (FCS 3.2+, replaces $WELLID)
    LOCATIONID(String),

    /// 'Short name' for parameter `n` (FCS 1.0+)
    PnN(String),
    /// Label name for parameter `n` (FCS 1.0+)
    PnS(String),
    /// Name of the optical filter for parameter `n` (FCS 1.0+)
    PnF(String),
    /// The FCS measurement signal types and evaluation features (e.g., area, height, or width) (FCS 1.0+)
    PnType(String),

    /// Detector name for parameter `n` (FCS 3.2+)
    PnDET(String),
    /// Dye specification for parameter `n` (FCS 3.2+)
    PnTAG(String),
    /// Target molecule or process for parameter `n` (FCS 3.2+)
    PnANALYTE(String),
    /// Evaluation features for parameter `n` (FCS 3.2+)
    PnFEATURE(String),

    /// Acquisition flow rate setting (FCS 3.2+)
    FLOWRATE(String),

    /// Sample volume (FCS 3.1+)
    VOL(String),

    /// Distinguish between original and altered data set (FCS 3.1+)
    ORIGINALITY(String),
    /// Who last modified the data set (FCS 3.1+)
    LastModifier(String),
    /// When the data set was last modified (FCS 3.1+)
    LastModified(String),

    // Deprecated keywords (FCS 2.0-3.1, deprecated in FCS 3.2)
    /// Date of data acquisition (FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $BEGINDATETIME)
    #[deprecated(since = "3.2.0", note = "Use BEGINDATETIME instead")]
    DATE(String),

    /// Begin time of data acquisition (FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $BEGINDATETIME)
    #[deprecated(since = "3.2.0", note = "Use BEGINDATETIME instead")]
    BTIM(String),

    /// End time of data acquisition (FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $ENDDATETIME)
    #[deprecated(since = "3.2.0", note = "Use ENDDATETIME instead")]
    ETIM(String),

    /// Data acquisition mode (FCS 2.0-3.1, deprecated in FCS 3.2, fixed to "L" list mode)
    #[deprecated(since = "3.2.0", note = "Fixed to 'L' list mode in FCS 3.2")]
    MODE(String),

    /// Plate identifier (FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $CARRIERID)
    #[deprecated(since = "3.2.0", note = "Use CARRIERID instead")]
    PLATEID(String),

    /// Platform/plate name (FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $CARRIERTYPE)
    #[deprecated(since = "3.2.0", note = "Use CARRIERTYPE instead")]
    PLATENAME(String),

    /// Well identifier (FCS 2.0-3.1, deprecated in FCS 3.2 in favor of $LOCATIONID)
    #[deprecated(since = "3.2.0", note = "Use LOCATIONID instead")]
    WELLID(String),

    /// Gate definition (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GATE(String),

    /// Gate n optical filter (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnF(String),

    /// Gate n short name (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnN(String),

    /// Gate n population name (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnP(String),

    /// Gate n range (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnR(String),

    /// Gate n label name (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnS(String),

    /// Gate n threshold (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnT(String),

    /// Gate n voltage range (FCS 2.0-3.1, removed in FCS 3.2)
    #[deprecated(since = "3.2.0", note = "Gate definitions deprecated")]
    GnV(String),

    /// A catch-all for other keywords, to be stored as Strings
    Other(String),
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
            | Self::Other(value) => Cow::Borrowed(value),
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

// Helper function to parse comma-separated decimal numbers
fn parse_float_with_comma_decimal(value: &str) -> Option<f32> {
    // First try standard decimal format
    if let Ok(val) = value.trim().parse::<f32>() {
        return Some(val);
    }

    // If that fails, try comma as decimal separator
    let normalized = value.trim().replace(',', ".");
    normalized.parse::<f32>().ok()
}

// Helper function to parse comma-separated tuple of 2 floats
fn parse_float_tuple(value: &str) -> Option<(f32, f32)> {
    let parts: Vec<&str> = value.trim().split(',').collect();
    if parts.len() == 2 {
        let f1 = parse_float_with_comma_decimal(parts[0])?;
        let f2 = parse_float_with_comma_decimal(parts[1])?;
        Some((f1, f2))
    } else {
        None
    }
}

// Helper function to parse comma-separated vector of floats
fn parse_float_vector(value: &str) -> Option<Vec<f32>> {
    value
        .trim()
        .split(',')
        .map(parse_float_with_comma_decimal)
        .collect()
}

// Helper function to parse $PnD format: (Linear|Logarithmic,f1,f2)
fn parse_pnd(value: &str) -> Option<MixedKeyword> {
    let parts: Vec<&str> = value.trim().split(',').collect();
    if parts.len() == 3 {
        let scale_type = parts[0].trim().to_string();
        let f1 = parse_float_with_comma_decimal(parts[1])?;
        let f2 = parse_float_with_comma_decimal(parts[2])?;
        Some(MixedKeyword::PnD(scale_type, f1, f2))
    } else {
        println!("Invalid $PnD format: {:?}", value);
        None
    }
}

// Helper function to parse $SPILLOVER format
fn parse_spillover(value: &str) -> Option<MixedKeyword> {
    let parts: Vec<&str> = value.trim().split(',').collect();
    if parts.is_empty() {
        return None;
    }

    let n_parameters = parts[0].trim().parse::<usize>().ok()?;

    if parts.len() < 1 + n_parameters {
        return None; // Not enough parts for parameter names
    }

    let parameter_names: Vec<String> = parts[1..=n_parameters]
        .iter()
        .map(|s| s.trim().to_string())
        .collect();

    let expected_matrix_size = n_parameters * n_parameters;
    let matrix_start = 1 + n_parameters;

    if parts.len() < matrix_start + expected_matrix_size {
        return None; // Not enough parts for full matrix
    }

    let matrix_values: Option<Vec<f32>> = parts[matrix_start..matrix_start + expected_matrix_size]
        .iter()
        .map(|s| parse_float_with_comma_decimal(s))
        .collect();

    matrix_values.map(|matrix_values| MixedKeyword::SPILLOVER {
        n_parameters,
        parameter_names,
        matrix_values,
    })
}

// Helper function to extract parameter number from P-prefixed keywords
fn extract_parameter_number(key: &str) -> Option<usize> {
    key.strip_prefix("P").and_then(|rest| {
        let param_str: String = rest.chars().take_while(|c| c.is_numeric()).collect();
        param_str.parse::<usize>().ok()
    })
}

// Helper function to check if a keyword is actually a parameter keyword (P followed by digits)
fn is_parameter_keyword(key: &str) -> bool {
    key.strip_prefix("P")
        .map(|rest| rest.chars().next().map_or(false, |c| c.is_numeric()))
        .unwrap_or(false)
}

// Store the value of a keyword in the appropriate keyword enum variant and return it.
pub fn match_and_parse_keyword(key: &str, value: &str) -> KeywordCreationResult {
    if let Some(dollarless_key) = key.strip_prefix('$') {
        match dollarless_key {
            "FIL" => KeywordCreationResult::String(StringKeyword::FIL(value.to_string())),
            "GUID" => KeywordCreationResult::String(StringKeyword::GUID(value.to_string())),
            "BYTEORD" => ByteOrder::from_keyword_str(value)
                .map_or(KeywordCreationResult::UnableToParse, |byte_order| {
                    KeywordCreationResult::Byte(ByteKeyword::BYTEORD(byte_order))
                }),
            "DATATYPE" => FcsDataType::from_keyword_str(value)
                .map_or(KeywordCreationResult::UnableToParse, |data_type| {
                    KeywordCreationResult::Byte(ByteKeyword::DATATYPE(data_type))
                }),
            "PAR" => value.parse::<usize>().map_or(
                KeywordCreationResult::UnableToParse,
                |number_of_parameters| {
                    KeywordCreationResult::Int(IntegerKeyword::PAR(number_of_parameters))
                },
            ),
            "TOT" => value.trim().parse::<usize>().map_or(
                KeywordCreationResult::UnableToParse,
                |number_of_events| {
                    KeywordCreationResult::Int(IntegerKeyword::TOT(number_of_events))
                },
            ),
            "BEGINDATA" => value.trim().parse::<usize>().map_or(
                KeywordCreationResult::UnableToParse,
                |begin_data_offset| {
                    KeywordCreationResult::Int(IntegerKeyword::BeginData(begin_data_offset))
                },
            ),
            "ENDDATA" => value.trim().parse::<usize>().map_or(
                KeywordCreationResult::UnableToParse,
                |end_data_offset| {
                    KeywordCreationResult::Int(IntegerKeyword::EndData(end_data_offset))
                },
            ),
            // Modern FCS 3.2 keywords
            "BEGINDATETIME" => {
                KeywordCreationResult::String(StringKeyword::BEGINDATETIME(value.to_string()))
            }
            "ENDDATETIME" => {
                KeywordCreationResult::String(StringKeyword::ENDDATETIME(value.to_string()))
            }
            "CARRIERID" => {
                KeywordCreationResult::String(StringKeyword::CARRIERID(value.to_string()))
            }
            "CARRIERTYPE" => {
                KeywordCreationResult::String(StringKeyword::CARRIERTYPE(value.to_string()))
            }
            "LOCATIONID" => {
                KeywordCreationResult::String(StringKeyword::LOCATIONID(value.to_string()))
            }
            "FLOWRATE" => KeywordCreationResult::String(StringKeyword::FLOWRATE(value.to_string())),
            "SPILLOVER" => parse_spillover(value).map_or(
                KeywordCreationResult::UnableToParse,
                KeywordCreationResult::Mixed,
            ),
            "VOL" => KeywordCreationResult::String(StringKeyword::VOL(value.to_string())),
            "ORIGINALITY" => {
                KeywordCreationResult::String(StringKeyword::ORIGINALITY(value.to_string()))
            }
            "LAST_MODIFIER" => {
                KeywordCreationResult::String(StringKeyword::LastModifier(value.to_string()))
            }
            "LAST_MODIFIED" => {
                KeywordCreationResult::String(StringKeyword::LastModified(value.to_string()))
            }

            // Deprecated keywords
            "DATE" => KeywordCreationResult::String(StringKeyword::DATE(value.to_string())),
            "BTIM" => KeywordCreationResult::String(StringKeyword::BTIM(value.to_string())),
            "ETIM" => KeywordCreationResult::String(StringKeyword::ETIM(value.to_string())),
            "MODE" => KeywordCreationResult::String(StringKeyword::MODE(value.to_string())),
            "PLATEID" => KeywordCreationResult::String(StringKeyword::PLATEID(value.to_string())),
            "PLATENAME" => {
                KeywordCreationResult::String(StringKeyword::PLATENAME(value.to_string()))
            }
            "WELLID" => KeywordCreationResult::String(StringKeyword::WELLID(value.to_string())),
            "GATE" => KeywordCreationResult::String(StringKeyword::GATE(value.to_string())),

            _ => {
                // Check if the keyword is a parameter keyword (P followed by digits)
                if is_parameter_keyword(dollarless_key) {
                    let p_trimmed_key = dollarless_key.strip_prefix("P").unwrap();

                    // Trim the numeric characters following the "$P" prefix to get the suffix
                    let number_trimmed_key =
                        p_trimmed_key.trim_start_matches(|c: char| c.is_numeric());

                    match number_trimmed_key {
                        "G" => parse_float_with_comma_decimal(value)
                            .map_or(KeywordCreationResult::UnableToParse, |gain| {
                                KeywordCreationResult::Float(FloatKeyword::PnG(gain))
                            }),
                        "E" => {
                            // Try parsing as tuple (f32, f32) first
                            if let Some((f1, f2)) = parse_float_tuple(value) {
                                KeywordCreationResult::Mixed(MixedKeyword::PnE(f1, f2))
                            } else {
                                // Fallback: try single float for backwards compatibility
                                parse_float_with_comma_decimal(value).map_or(
                                    KeywordCreationResult::UnableToParse,
                                    |single_val| {
                                        KeywordCreationResult::Mixed(MixedKeyword::PnE(
                                            single_val, 0.0,
                                        ))
                                    },
                                )
                            }
                        }
                        "R" => value
                            .trim()
                            .parse::<usize>()
                            .map_or(KeywordCreationResult::UnableToParse, |range| {
                                KeywordCreationResult::Int(IntegerKeyword::PnR(range))
                            }),
                        "B" => value
                            .trim()
                            .parse::<usize>()
                            .map_or(KeywordCreationResult::UnableToParse, |bits| {
                                KeywordCreationResult::Int(IntegerKeyword::PnB(bits))
                            }),
                        "V" => value.trim().parse::<usize>().map_or(
                            KeywordCreationResult::UnableToParse,
                            |voltage_range| {
                                KeywordCreationResult::Int(IntegerKeyword::PnV(voltage_range))
                            },
                        ),
                        "L" => {
                            // Parse as comma-separated list of wavelengths
                            // Handle FCS format with parentheses: (488) or (488,532,633)
                            let cleaned_value = value
                                .trim()
                                .strip_prefix('(')
                                .and_then(|s| s.strip_suffix(')'))
                                .unwrap_or(value.trim());

                            let wavelengths: Option<Vec<usize>> = cleaned_value
                                .split(',')
                                .map(|s| s.trim().parse::<usize>().ok())
                                .collect();

                            wavelengths.map_or(KeywordCreationResult::UnableToParse, |wl| {
                                KeywordCreationResult::Mixed(MixedKeyword::PnL(wl))
                            })
                        }
                        "Display" => value.trim().parse::<usize>().map_or(
                            KeywordCreationResult::UnableToParse,
                            |display_scale| {
                                KeywordCreationResult::Int(IntegerKeyword::PnDisplay(display_scale))
                            },
                        ),
                        "N" => KeywordCreationResult::String(StringKeyword::PnN(value.to_string())),
                        "S" => KeywordCreationResult::String(StringKeyword::PnS(value.to_string())),
                        "F" => KeywordCreationResult::String(StringKeyword::PnF(value.to_string())),
                        "Type" | "TYPE" => {
                            KeywordCreationResult::String(StringKeyword::PnType(value.to_string()))
                        }
                        "DET" => {
                            KeywordCreationResult::String(StringKeyword::PnDET(value.to_string()))
                        }
                        "TAG" => {
                            KeywordCreationResult::String(StringKeyword::PnTAG(value.to_string()))
                        }
                        "ANALYTE" => KeywordCreationResult::String(StringKeyword::PnANALYTE(
                            value.to_string(),
                        )),
                        "FEATURE" => KeywordCreationResult::String(StringKeyword::PnFEATURE(
                            value.to_string(),
                        )),
                        "D" => parse_pnd(value).map_or(
                            KeywordCreationResult::UnableToParse,
                            KeywordCreationResult::Mixed,
                        ),
                        _ => {
                            eprintln!(
                                "Unknown parameter keyword suffix: '{}' for key: '{}' with value: '{}'",
                                number_trimmed_key, key, value
                            );
                            KeywordCreationResult::UnableToParse
                        }
                    }
                } else {
                    // Check for deprecated gate keywords (Gn*)
                    if let Some(gn_key) = dollarless_key.strip_prefix("G") {
                        if gn_key.chars().next().map_or(false, |c| c.is_numeric()) {
                            let suffix = gn_key.trim_start_matches(|c: char| c.is_numeric());
                            match suffix {
                                "E" => {
                                    if let Some((f1, f2)) = parse_float_tuple(value) {
                                        KeywordCreationResult::Mixed(MixedKeyword::GnE(f1, f2))
                                    } else {
                                        KeywordCreationResult::Mixed(MixedKeyword::GnE(0.0, 0.0))
                                    }
                                }
                                "F" => KeywordCreationResult::String(StringKeyword::GnF(
                                    value.to_string(),
                                )),
                                "N" => KeywordCreationResult::String(StringKeyword::GnN(
                                    value.to_string(),
                                )),
                                "P" => KeywordCreationResult::String(StringKeyword::GnP(
                                    value.to_string(),
                                )),
                                "R" => KeywordCreationResult::String(StringKeyword::GnR(
                                    value.to_string(),
                                )),
                                "S" => KeywordCreationResult::String(StringKeyword::GnS(
                                    value.to_string(),
                                )),
                                "T" => KeywordCreationResult::String(StringKeyword::GnT(
                                    value.to_string(),
                                )),
                                "V" => KeywordCreationResult::String(StringKeyword::GnV(
                                    value.to_string(),
                                )),
                                _ => KeywordCreationResult::String(StringKeyword::Other(
                                    value.to_string(),
                                )),
                            }
                        } else {
                            KeywordCreationResult::String(StringKeyword::Other(value.to_string()))
                        }
                    } else if let Some(rn_key) = dollarless_key.strip_prefix("R") {
                        if rn_key.chars().next().map_or(false, |c| c.is_numeric()) {
                            let suffix = rn_key.trim_start_matches(|c: char| c.is_numeric());
                            match suffix {
                                "W" => parse_float_vector(value)
                                    .map_or(KeywordCreationResult::UnableToParse, |vec| {
                                        KeywordCreationResult::Mixed(MixedKeyword::RnW(vec))
                                    }),
                                _ => KeywordCreationResult::String(StringKeyword::Other(
                                    value.to_string(),
                                )),
                            }
                        } else {
                            KeywordCreationResult::String(StringKeyword::Other(value.to_string()))
                        }
                    } else {
                        KeywordCreationResult::String(StringKeyword::Other(value.to_string()))
                    }
                }
            }
        }
    } else {
        KeywordCreationResult::String(StringKeyword::Other(value.to_string()))
    }
}

impl From<&StringKeyword> for Arc<str> {
    fn from(keyword: &StringKeyword) -> Self {
        keyword.get_str().into()
    }
}

// // Extract the variant's value and convert it to a string
impl From<&IntegerKeyword> for String {
    fn from(keyword: &IntegerKeyword) -> Self {
        keyword.get_usize().to_string()
    }
}

// impl Keyword {

//     #[must_use]
//     pub fn get_string_value(&self) -> Option<&str> {
//         match self {
//             Self::PnN(value)
//             | Self::PnS(value)
//             | Self::PnF(value)
//             | Self::FIL(Some(value))
//             | Self::GUID(Some(value))
//             | Self::Other(value) => Some(value.as_str()),
//             _ => None,
//         }
//     }

//     #[must_use]
//     pub const fn get_numeric_value(&self) -> Option<usize> {
//         match self {
//             Self::TOT(Some(value))
//             | Self::PnR(value)
//             | Self::PnB(value)
//             | Self::PnE(value)
//             | Self::PnV(value)
//             | Self::PnG(value)
//             | Self::PnL(value)
//             | Self::PnDisplay(value)
//             | Self::PAR(value) => Some(*value),
//             _ => None,
//         }
//     }

//     #[must_use]
//     pub fn match_and_parse(key: &str, value: &str) -> KeywordCreationResult {
//         match key {
//             "$PAR" => value
//                 .parse::<usize>()
//                 .map_or(KeywordCreationResult::UnableToParse, |number_of_parameters| {
//                     KeywordCreationResult::Success(Self::PAR(number_of_parameters))
//                 }),
//             "$TOT" => value
//                 .parse::<usize>()
//                 .map_or(KeywordCreationResult::UnableToParse, |number_of_events| {
//                     KeywordCreationResult::Success(Self::TOT(Some(number_of_events)))
//                 }),
//             "$DATATYPE" => FcsDataType::from_keyword_str(value)
//                 .map_or(KeywordCreationResult::UnableToParse, |data_type| {
//                     KeywordCreationResult::Success(Self::DATATYPE(data_type))
//                 }),
//             "$BYTEORD" => ByteOrder::from_keyword_str(value)
//                 .map_or(KeywordCreationResult::UnableToParse, |byte_order| {
//                     KeywordCreationResult::Success(Self::BYTEORD(byte_order))
//                 }),
//             "$FIL" => KeywordCreationResult::Success(Self::FIL(Some(value.to_string()))),
//             "GUID" => KeywordCreationResult::Success(Self::GUID(Some(value.to_string()))),
//             // Handle the cases of $PnN and $PnS keywords
//             _ => {
//                 // Check if the keyword is a $PnN, $PnS, $PnR, $PnB, $PnE, $PnV, $PnG, or $PnL keyword
//                 if key.starts_with("$P") {
//                     let end_matches = &['N', 'S', 'R', 'B', 'E', 'V', 'G', 'L', 'F'];
//                     // Get the parameter number from the keyword
//                     key.trim_start_matches("$P")
//                         .trim_end_matches(end_matches)
//                         .parse::<usize>()
//                         .map_or_else(
//                             |_| {
//                                 println!(
//                                     "Should be able to parse parameter number from keyword: {key}"
//                                 );
//                                 KeywordCreationResult::Success(Self::Other(value.to_string()))
//                             },
//                             |parameter_number| match key.chars().last() {
//                                 Some('N') => KeywordCreationResult::Success(Self::PnN(value.to_string())),
//                                 Some('S') => KeywordCreationResult::Success(Self::PnS(value.to_string())),
//                                 Some('R') => KeywordCreationResult::Success(Self::PnR(parameter_number)),
//                                 Some('B') => KeywordCreationResult::Success(Self::PnB(parameter_number)),
//                                 Some('E') => KeywordCreationResult::Success(Self::PnE(parameter_number)),
//                                 Some('V') => KeywordCreationResult::Success(Self::PnV(parameter_number)),
//                                 Some('G') => KeywordCreationResult::Success(Self::PnG(parameter_number)),
//                                 Some('L') => KeywordCreationResult::Success(Self::PnL(parameter_number)),
//                                 Some('F') => KeywordCreationResult::Success(Self::PnF(value.to_string())),
//                                 _ => {
//                                     // If the end matches "Display" then return that keyword in a success
//                                     if key.ends_with("Display") {
//                                         KeywordCreationResult::Success(Self::PnDisplay(parameter_number))
//                                     } else if key.ends_with("Type") {
//                                         KeywordCreationResult::Success(Self::PnType(value.to_string()))
//                                     }
//                                     else {
//                                         println!("Should be able to parse parameter number from keyword: {key}");
//                                         KeywordCreationResult::Success(Self::Other(value.to_string()))
//                                     }
//                                 }
//                             },
//                         )
//                 } else {
//                     // Place any other keyword in a catch-all enum variant, used to retrieve string values
//                     println!("Should be able to parse parameter keyword enum from: {key}");
//                     KeywordCreationResult::Success(Self::Other(value.to_string()))
//                 }
//             }
//         }
//     }
// }

// // Implement ability to convert Keyword Into Arc<str>
// impl From<&Keyword> for Arc<str> {
//     fn from(keyword: &Keyword) -> Self {
//         keyword.to_string().into()
//     }
// }
// // Extract the variant's value and convert it to a string
// impl From<&Keyword> for String {
//     fn from(keyword: &Keyword) -> Self {
//         match keyword {
//             Keyword::TOT(Some(value))
//             | Keyword::PAR(value)
//             | Keyword::PnB(value)
//             | Keyword::PnE(value)
//             | Keyword::PnV(value)
//             | Keyword::PnG(value)
//             | Keyword::PnL(value)
//             | Keyword::PnR(value)
//             | Keyword::PnDisplay(value) => value.to_string(),
//             Keyword::DATATYPE(value) => value.to_string(),
//             Keyword::BYTEORD(value) => value.to_string(),
//             Keyword::FIL(Some(value))
//             | Keyword::GUID(Some(value))
//             | Keyword::PnS(value)
//             | Keyword::PnN(value)
//             | Keyword::PnF(value)
//             | Keyword::Other(value) => value.to_string(),
//             _ => Self::new(),
//         }
//     }
// }
