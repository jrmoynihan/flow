use anyhow::Result;
use serde::{Serialize, Serializer};
use std::fmt::Display;

/// An enum representing FCS file format versions
///
/// Each version has different required keywords and structural requirements.
/// The library supports FCS versions 1.0 through 4.0, with 3.1 as the default.
#[derive(Debug, Clone, Copy, Default, Hash)]
pub enum Version {
    V1_0,
    V2_0,
    V3_0,
    #[default]
    V3_1,
    V3_2,
    V4_0,
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = match self {
            Self::V1_0 => "FCS1.0",
            Self::V2_0 => "FCS2.0",
            Self::V3_0 => "FCS3.0",
            Self::V3_1 => "FCS3.1",
            Self::V3_2 => "FCS3.2",
            Self::V4_0 => "FCS4.0",
        };
        write!(f, "{version}")
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let version = match self {
            Self::V1_0 => "FCS1.0",
            Self::V2_0 => "FCS2.0",
            Self::V3_0 => "FCS3.0",
            Self::V3_1 => "FCS3.1",
            Self::V3_2 => "FCS3.2",
            Self::V4_0 => "FCS4.0",
        };
        serializer.serialize_str(version)
    }
}

impl Version {
    /// Returns the required *non-parameter* indexed keywords for the 'TEXT' segment in a given FCS version as a static array of strings
    #[must_use]
    pub fn get_required_keywords(&self) -> &[&str] {
        const V1_0: [&str; 0] = [];
        const V2_0: [&str; 5] = [
            "$BYTEORD",  // byte order for data acquisition computer
            "$DATATYPE", // type of data in data segment (ASCII, int, float)
            "$MODE",     // data mode (list mode - preferred, histogram - deprecated)
            "$NEXTDATA", // byte-offset to next data set in the file
            "$PAR",      // number of parameters in an event
        ];
        const V3_0_V3_1: [&str; 12] = [
            "$BEGINANALYSIS", // byte-offset to the beginning of analysis segment
            "$BEGINDATA",     // byte-offset of beginning of data segment
            "$BEGINSTEXT",    // byte-offset to beginning of text segment
            "$BYTEORD",       // byte order for data acquisition computer
            "$DATATYPE",      // type of data in data segment (ASCII, int, float)
            "$ENDANALYSIS",   // byte-offset to end of analysis segment
            "$ENDDATA",       // byte-offset to end of data segment
            "$ENDSTEXT",      // byte-offset to end of text segment
            "$MODE",          // data mode (list mode - preferred, histogram - deprecated)
            "$NEXTDATA",      // byte-offset to next data set in the file
            "$PAR",           // number of parameters in an event
            "$TOT",           // total number of events in the data set
        ];
        const V3_2: [&str; 8] = [
            "$BEGINDATA",
            "$BYTEORD",
            "$CYT",
            "$DATATYPE",
            "$ENDDATA",
            "$NEXTDATA",
            "$PAR",
            "$TOT",
        ];
        const V4_0: [&str; 11] = [
            "$BEGINDATA",
            "$BYTEORD",
            "$DATATYPE",
            "$ENDDATA",
            "$NEXTDATA",
            "$PAR",
            "$DATE",
            "$ETIM",
            "$CYT",
            "$BTIM",
            "$TOT",
        ];

        match self {
            Self::V1_0 => &V1_0,
            Self::V2_0 => &V2_0,
            Self::V3_0 | Self::V3_1 => &V3_0_V3_1,
            Self::V3_2 => &V3_2,
            Self::V4_0 => &V4_0,
        }
    }
}

// Optional non-paramater indexed keywords
// const OPTIONAL_KEYWORDS: [&str; 31] = [
//     "$ABRT",          // events lost due to acquisition electronic coincidence
//     "$BTIM",          // clock time at beginning of data acquisition
//     "$CELLS",         // description of objects measured
//     "$COM",           // comment
//     "$CSMODE",        // cell subset mode, number of subsets an object may belong
//     "$CSVBITS",       // number of bits used to encode cell subset identifier
//     "$CYT",           // cytometer type
//     "$CYTSN",         // cytometer serial number
//     "$DATE",          // date of data acquisition
//     "$ETIM",          // clock time at end of data acquisition
//     "$EXP",           // investigator name initiating experiment
//     "$FIL",           // name of data file containing data set
//     "$GATE",          // number of gating parameters
//     "$GATING",        // region combinations used for gating
//     "$INST",          // institution where data was acquired
//     "$LAST_MODIFIED", // timestamp of last modification
//     "$LAST_MODIFIER", // person performing last modification
//     "$LOST",          // number events lost due to computer busy
//     "$OP",            // name of flow cytometry operator
//     "$ORIGINALITY",   // information whether FCS data set has been modified or not
//     "$PLATEID",       // plate identifier
//     "$PLATENAME",     // plate name
//     "$PROJ",          // project name
//     "$SMNO",          // specimen (i.e., tube) label
//     "$SPILLOVER",     // spillover matrix
//     "$SRC",           // source of specimen (cell type, name, etc.)
//     "$SYS",           // type of computer and OS
//     "$TIMESTEP",      // time step for time parameter
//     "$TR",            // trigger paramter and its threshold
//     "$VOL",           // volume of sample run during data acquisition
//     "$WELLID",        // well identifier
// ];
