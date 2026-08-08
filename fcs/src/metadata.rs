use super::{
    byteorder::ByteOrder,
    datatype::FcsDataType,
    header::Header,
    keyword::{
        ByteKeyword, FloatKeyword, IntegerKeyword, IntegerableKeyword, Keyword,
        KeywordCreationResult, MixedKeyword, StringKeyword, match_and_parse_keyword,
    },
};
use anyhow::{Result, anyhow};
use memmap3::Mmap;
use regex::bytes::Regex;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;
pub type KeywordMap = FxHashMap<String, Keyword>;

/// Contains keyword-value pairs and delimiter from the TEXT segment of an FCS file
///
/// The TEXT segment contains all metadata about the FCS file, including:
/// - File information (GUID, filename, cytometer type)
/// - Data structure information (number of events, parameters, data type, byte order)
/// - Parameter metadata (names, labels, ranges, transforms)
/// - Optional information (compensation matrices, timestamps, etc.)
///
/// Keywords are stored in a hashmap for fast lookup, with type-safe accessors
/// for different keyword types (integer, float, string, byte, mixed).
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub keywords: KeywordMap,
    pub delimiter: char,
}

impl Metadata {
    #[must_use]
    pub fn new() -> Self {
        Self {
            keywords: FxHashMap::default(),
            delimiter: ' ',
        }
    }
    /// Prints all keywords sorted alphabetically by key name
    ///
    /// This is a debugging utility that displays all keyword-value pairs
    /// in the metadata, sorted for easy reading.
    pub fn print_sorted_by_keyword(&self) {
        // Step 1: Get a Vector from existing text HashMap.
        let mut sorted: Vec<_> = self.keywords.iter().collect();

        // Step 2: sort Vector by key from HashMap.
        // ... This sorts by HashMap keys.
        //     Each tuple is sorted by its first item [.0] (the key).
        sorted.sort_by_key(|a| a.0);

        // Step 3: loop over sorted vector.
        for (key, value) in &sorted {
            println!("{key}: {value}");
        }
    }
    /// Reads the text segment of the fcs file and returns an `Metadata` struct
    ///
    /// Uses memchr for fast delimiter finding (5-10x faster than byte-by-byte iteration)
    #[must_use]
    pub fn from_mmap(mmap: &Mmap, header: &Header) -> Self {
        Self::from_text_segment(mmap, &header.text_offset, header.version)
    }

    /// As [`from_mmap`](Self::from_mmap), but takes the TEXT segment's
    /// **file-absolute** byte range directly, plus the version whose escaping
    /// rules apply.
    ///
    /// [`Header`] carries data-set-relative offsets (§2.4.3), so any data set
    /// past the first in a `$NEXTDATA` chain must resolve those against its own
    /// base before they can index the mmap. `from_mmap` is the `base == 0` case.
    ///
    /// `version` decides whether a doubled delimiter is one escaped literal
    /// (FCS 3.1+) or two boundaries around an empty value (3.0 and earlier).
    /// Data sets reached through `$NEXTDATA` have no HEADER of their own, so
    /// callers pass the file's primary version.
    #[must_use]
    pub fn from_text_segment(
        mmap: &Mmap,
        text_range: &std::ops::RangeInclusive<usize>,
        version: crate::version::Version,
    ) -> Self {
        // Read the first byte of the text segment to determine the delimiter:
        let delimiter = mmap[*text_range.start()];

        // Read the text content, SKIPping the first byte, which is the delimiter (used above)
        let text_slice = &mmap[(*text_range.start() + 1)..=*text_range.end()];

        let mut keywords: KeywordMap = FxHashMap::default();
        let mut fields = crate::text::TextFields::new(
            text_slice,
            delimiter,
            crate::text::Escaping::for_version(version),
        )
        .peekable();

        // TEXT need not end on the delimiter (some writers omit the final
        // one). When it doesn't, `TextFields`' last field is an *unterminated*
        // remainder rather than a real, delimiter-closed field — and if that
        // remainder is invalid UTF-8, it decodes to "". The pre-extraction
        // implementation's trailing-segment handling silently dropped that
        // remainder rather than record a bogus empty key or value; restore
        // that guard so this extraction stays a no-op even on malformed
        // input. A merely-empty field that *is* delimiter-bounded (e.g. two
        // consecutive delimiters) is unaffected: it is always genuine, never
        // a decode failure, so it is recorded exactly as before.
        let unterminated_tail = text_slice.last().is_some_and(|&b| b != delimiter);

        while let Some(key) = fields.next() {
            if fields.peek().is_none() && unterminated_tail && key.is_empty() {
                break;
            }
            let Some(value) = fields.next() else {
                // A keyword with no value at the end of TEXT. Invalid FCS, but
                // observed; drop it as the previous implementation did.
                tracing::debug!(
                    "Warning: Keyword '{}' at end of text segment ({:?}) has no value",
                    key, text_range
                );
                break;
            };
            if fields.peek().is_none() && unterminated_tail && value.is_empty() {
                break;
            }
            if key.is_empty() {
                continue;
            }
            // Preserve key as-is: FCS spec reserves $ for standard keywords only.
            // User-defined keywords (e.g. "Tissue") must not gain a $ prefix.
            let normalized_key = key.to_string();
            match match_and_parse_keyword(&key, &value) {
                KeywordCreationResult::Int(k) => { keywords.insert(normalized_key, Keyword::Int(k)); }
                KeywordCreationResult::Float(k) => { keywords.insert(normalized_key, Keyword::Float(k)); }
                KeywordCreationResult::String(k) => { keywords.insert(normalized_key, Keyword::String(k)); }
                KeywordCreationResult::Byte(k) => { keywords.insert(normalized_key, Keyword::Byte(k)); }
                KeywordCreationResult::Mixed(k) => { keywords.insert(normalized_key, Keyword::Mixed(k)); }
                KeywordCreationResult::UnableToParse => {
                    tracing::debug!("Unable to parse keyword: {} with value: {}", key, value);
                }
            }
        }

        Self {
            keywords,
            delimiter: delimiter as char,
        }
    }

    /// Check that required keys are present in the TEXT segment of the metadata
    /// # Errors
    /// Will return `Err` if:
    /// - any of the required keywords are missing from the keywords hashmap
    /// - the number of parameters can't be obtained from the $PAR keyword in the TEXT section
    /// - any keyword has a Pn[X] value where n is greater than the number of parameters indicated by the $PAR keyword
    pub fn validate_text_segment_keywords(&self, header: &Header) -> Result<()> {
        debug!(version = %header.version, "validate FCS TEXT keywords");
        let missing = self.missing_required_keywords(header);
        if missing.is_empty() {
            return Ok(());
        }
        Err(anyhow!(
            "Invalid FCS {} file: missing {} required keyword(s): {}",
            header.version,
            missing.len(),
            missing.join(", ")
        ))
    }

    /// Every keyword the declared version requires that this TEXT segment does
    /// not carry, in the order the version lists them.
    ///
    /// Reported in full rather than short-circuiting on the first miss: a
    /// caller repairing a file wants the whole list, and one round trip
    /// through the error message beats one per missing keyword.
    #[must_use]
    pub fn missing_required_keywords(&self, header: &Header) -> Vec<&'static str> {
        header
            .version
            .get_required_keywords()
            .iter()
            .filter(|keyword| !self.keywords.contains_key(**keyword))
            .copied()
            .collect()
    }

    /// Validates if a GUID is present in the file's metadata, and if not, generates a new one.
    pub fn validate_guid(&mut self) {
        if self.get_string_keyword("GUID").is_err() {
            self.insert_string_keyword("GUID".to_string(), Uuid::new_v4().to_string());
        }
    }

    /// Confirm that no stored keyword has a value greater than the $PAR keyword indicates
    #[allow(unused)]
    fn validate_number_of_parameters(&self) -> Result<()> {
        let n_params = self.get_number_of_parameters()?;
        let n_params_string = n_params.to_string();
        let n_digits = n_params_string.chars().count().to_string();
        let regex_string = r"[PR]\d{1,".to_string() + &n_digits + "}[BENRDFGLOPSTVIW]";
        let param_keywords = Regex::new(&regex_string)?;

        for keyword in self.keywords.keys() {
            if !param_keywords.is_match(keyword.as_bytes()) {
                continue; // Skip to the next iteration if the keyword doesn't match
            }

            // If the keyword starts with a $P, then the value of the next non-terminal characters should be less than or equal to the number of parameters
            if keyword.starts_with("$P") {
                let param_number = keyword
                    .chars()
                    .nth(1)
                    .ok_or_else(|| anyhow!("Keyword '{}' should have a second character after '$P'", keyword))?
                    .to_digit(10)
                    .ok_or_else(|| anyhow!("Keyword '{}' should have a digit as the second character to count parameters", keyword))? as usize;
                if param_number > *n_params {
                    return Err(anyhow!(
                        "Invalid FCS file: {} keyword value exceeds number of parameters",
                        keyword
                    ));
                }
            }
        }

        Ok(())
    }
    /// Generic function to get the unwrapped unsigned integer value associated with a numeric keyword (e.g. $PAR, $TOT, etc.)
    fn get_keyword_value_as_usize(&self, keyword: &str) -> Result<&usize> {
        Ok(self.get_integer_keyword(keyword)?.get_usize())
    }

    /// Return the number of parameters in the file from the $PAR keyword in the metadata TEXT section
    /// # Errors
    /// Will return `Err` if the $PAR keyword is not present in the metadata keywords hashmap
    pub fn get_number_of_parameters(&self) -> Result<&usize> {
        self.get_keyword_value_as_usize("$PAR")
    }

    /// Return the number of events in the file from the $TOT keyword in the metadata TEXT section
    /// # Errors
    /// Will return `Err` if the $TOT keyword is not present in the metadata keywords hashmap
    pub fn get_number_of_events(&self) -> Result<&usize> {
        self.get_keyword_value_as_usize("$TOT")
    }

    /// Return the data type from the $DATATYPE keyword in the metadata TEXT section, unwraps and returns it if it exists.
    /// # Errors
    /// Will return `Err` if the $DATATYPE keyword is not present in the metadata keywords hashmap
    pub fn get_data_type(&self) -> Result<&FcsDataType> {
        let keyword = self.get_byte_keyword("$DATATYPE")?;
        if let ByteKeyword::DATATYPE(data_type) = keyword {
            Ok(data_type)
        } else {
            Err(anyhow!("No $DATATYPE value stored."))
        }
    }

    /// Get the data type for a specific channel/parameter (FCS 3.2+)
    ///
    /// First checks for `$PnDATATYPE` keyword to see if this parameter has a specific data type override.
    /// If not found, falls back to the default `$DATATYPE` keyword.
    ///
    /// # Arguments
    /// * `parameter_number` - 1-based parameter index
    ///
    /// # Errors
    /// Will return `Err` if neither `$PnDATATYPE` nor `$DATATYPE` is present
    pub fn get_data_type_for_channel(&self, parameter_number: usize) -> Result<FcsDataType> {
        // First try to get parameter-specific data type (FCS 3.2+)
        if let Ok(pn_datatype_keyword) =
            self.get_parameter_byte_metadata(parameter_number, "DATATYPE")
        {
            if let ByteKeyword::PnDATATYPE(data_type) = pn_datatype_keyword {
                Ok(*data_type)
            } else {
                // Shouldn't happen, but fall back to default
                Ok(self.get_data_type()?.clone())
            }
        } else {
            // Fall back to default $DATATYPE
            Ok(self.get_data_type()?.clone())
        }
    }

    /// Calculate the total bytes per event (record stride)
    ///
    /// Sums the raw `$PnB` bit widths across all parameters first, then rounds up
    /// to a whole byte *once*. This matters for non-byte-aligned (bit-packed)
    /// layouts: e.g. 8 parameters of `$PnB=10` pack into a `ceil(80/8) = 10`-byte
    /// record, not `sum(ceil(10/8)) = 8 * 2 = 16` bytes — rounding per-parameter
    /// before summing overcounts whenever any `$PnB` isn't a multiple of 8. For
    /// byte-aligned layouts (the common case) both orders agree, since rounding a
    /// multiple of 8 is a no-op.
    ///
    /// # Errors
    /// Will return `Err` if the number of parameters cannot be determined or
    /// if any required `$PnB` keyword is missing
    pub fn calculate_bytes_per_event(&self) -> Result<usize> {
        let number_of_parameters = self.get_number_of_parameters()?;
        let mut total_bits = 0;

        for param_num in 1..=*number_of_parameters {
            total_bits += self.get_bits_per_parameter(param_num)?;
        }

        Ok(total_bits.div_ceil(8))
    }

    /// Get the raw, un-rounded `$PnB` bit width for a specific channel
    ///
    /// # Arguments
    /// * `parameter_number` - 1-based parameter index
    ///
    /// # Errors
    /// Will return `Err` if the `$PnB` keyword is missing for this parameter
    pub fn get_bits_per_parameter(&self, parameter_number: usize) -> Result<usize> {
        let bits = self.get_parameter_numeric_metadata(parameter_number, "B")?;
        if let IntegerKeyword::PnB(bits_value) = bits {
            Ok(*bits_value)
        } else {
            Err(anyhow!(
                "$P{}B keyword found but is not the expected PnB variant",
                parameter_number
            ))
        }
    }

    /// Get bytes per parameter for a specific channel
    ///
    /// Uses `$PnB` (bits per parameter) divided by 8 to get bytes per parameter.
    ///
    /// # Arguments
    /// * `parameter_number` - 1-based parameter index
    ///
    /// # Errors
    /// Will return `Err` if the `$PnB` keyword is missing for this parameter
    pub fn get_bytes_per_parameter(&self, parameter_number: usize) -> Result<usize> {
        let bits = self.get_parameter_numeric_metadata(parameter_number, "B")?;
        if let IntegerKeyword::PnB(bits_value) = bits {
            // Convert bits to bytes (round up if not divisible by 8)
            Ok((bits_value + 7) / 8)
        } else {
            Err(anyhow!(
                "$P{}B keyword found but is not the expected PnB variant",
                parameter_number
            ))
        }
    }

    /// Get the declared range (`$PnR`) for a specific channel
    ///
    /// `$PnR` is the *true* resolution of a parameter's ADC, which can be narrower
    /// than the storage width implied by `$PnB`. Instruments (e.g. Beckman
    /// FC500/Gallios/Navios) commonly store sub-16-bit resolution in a 16-bit field,
    /// leaving the unused high bits as instrument noise rather than zeros. Callers
    /// use this to derive a mask (`range.next_power_of_two() - 1`) for integer
    /// parameters.
    ///
    /// # Arguments
    /// * `parameter_number` - 1-based parameter index
    ///
    /// # Errors
    /// Will return `Err` if the `$PnR` keyword is missing for this parameter
    pub fn get_range_for_channel(&self, parameter_number: usize) -> Result<usize> {
        let range = self.get_parameter_numeric_metadata(parameter_number, "R")?;
        if let IntegerKeyword::PnR(range_value) = range {
            Ok(*range_value)
        } else {
            Err(anyhow!(
                "$P{}R keyword found but is not the expected PnR variant",
                parameter_number
            ))
        }
    }

    /// Return the byte order from the $BYTEORD keyword in the metadata TEXT section, unwraps and returns it if it exists.
    /// # Errors
    /// Will return `Err` if the $BYTEORD keyword is not present in the keywords hashmap
    pub fn get_byte_order(&self) -> Result<&ByteOrder> {
        let keyword = self.get_byte_keyword("$BYTEORD")?;
        if let ByteKeyword::BYTEORD(byte_order) = keyword {
            Ok(byte_order)
        } else {
            Err(anyhow!("No $BYTEORD value stored."))
        }
    }
    /// Returns a keyword that holds numeric data from the keywords hashmap, if it exists
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_integer_keyword(&self, keyword: &str) -> Result<&IntegerKeyword> {
        if let Some(keyword) = self.keywords.get(keyword) {
            match keyword {
                Keyword::Int(integer) => Ok(integer),
                _ => Err(anyhow!("Keyword is not integer variant")),
            }
        } else {
            Err(anyhow!("No {keyword} keyword stored."))
        }
    }

    /// Returns a keyword that holds numeric data from the keywords hashmap, if it exists
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_float_keyword(&self, keyword: &str) -> Result<&FloatKeyword> {
        if let Some(keyword) = self.keywords.get(keyword) {
            match keyword {
                Keyword::Float(float) => Ok(float),
                _ => Err(anyhow!("Keyword is not float variant")),
            }
        } else {
            Err(anyhow!("No {keyword} keyword stored."))
        }
    }

    /// Returns a keyword that holds string data from the keywords hashmap, if it exists
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_string_keyword(&self, keyword: &str) -> Result<&StringKeyword> {
        if let Some(keyword) = self.keywords.get(keyword) {
            match keyword {
                Keyword::String(string) => Ok(string),
                _ => Err(anyhow!("Keyword is not a string variant")),
            }
        } else {
            Err(anyhow!("No {keyword} keyword stored."))
        }
    }

    /// Returns a keyword that holds byte-orientation data from the keywords hashmap, if it exists
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_byte_keyword(&self, keyword: &str) -> Result<&ByteKeyword> {
        if let Some(keyword) = self.keywords.get(keyword) {
            match keyword {
                Keyword::Byte(byte) => Ok(byte),
                _ => Err(anyhow!("Keyword is not a byte variant")),
            }
        } else {
            Err(anyhow!("No {keyword} keyword stored."))
        }
    }

    /// Returns a keyword that holds mixed data from the keywords hashmap, if it exists
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_mixed_keyword(&self, keyword: &str) -> Result<&MixedKeyword> {
        if let Some(keyword) = self.keywords.get(keyword) {
            match keyword {
                Keyword::Mixed(mixed) => Ok(mixed),
                _ => Err(anyhow!("Keyword is not a mixed variant")),
            }
        } else {
            Err(anyhow!("No {keyword} keyword stored."))
        }
    }

    /// General function to get a given parameter's string keyword from the file's metadata (e.g. `$PnN` or `$PnS`)
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_parameter_string_metadata(
        &self,
        parameter_number: usize,
        suffix: &str,
    ) -> Result<&StringKeyword> {
        // Interpolate the parameter number into the keyword:
        let keyword = format!("$P{parameter_number}{suffix}");
        self.get_string_keyword(&keyword)
    }

    /// Generic function to get a given parameter's integer keyword from the file's metadata (e.g. `$PnN`, `$PnS`)
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_parameter_numeric_metadata(
        &self,
        parameter_number: usize,
        suffix: &str,
    ) -> Result<&IntegerKeyword> {
        // Interpolate the parameter number into the keyword:
        let keyword = format!("$P{parameter_number}{suffix}");
        self.get_integer_keyword(&keyword)
    }

    /// Generic function to get a given parameter's byte keyword from the file's metadata (e.g. `$PnDATATYPE`)
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_parameter_byte_metadata(
        &self,
        parameter_number: usize,
        suffix: &str,
    ) -> Result<&ByteKeyword> {
        // Interpolate the parameter number into the keyword:
        let keyword = format!("$P{parameter_number}{suffix}");
        self.get_byte_keyword(&keyword)
    }

    /// Get excitation wavelength(s) for a parameter from `$PnL` keyword
    /// Returns the first wavelength if multiple are present (for co-axial lasers)
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_parameter_excitation_wavelength(
        &self,
        parameter_number: usize,
    ) -> Result<Option<usize>> {
        let keyword = format!("$P{parameter_number}L");

        // Try as integer keyword first (older FCS format)
        if let Ok(int_keyword) = self.get_integer_keyword(&keyword) {
            if let IntegerKeyword::PnL(wavelength) = int_keyword {
                return Ok(Some(*wavelength));
            }
        }

        // Try as mixed keyword (FCS 3.1+ format, can have multiple wavelengths)
        if let Ok(mixed_keyword) = self.get_mixed_keyword(&keyword) {
            if let MixedKeyword::PnL(wavelengths) = mixed_keyword {
                // Return the first wavelength if multiple are present
                return Ok(wavelengths.first().copied());
            }
        }

        Ok(None)
    }

    /// Return the name of the parameter's channel from the `$PnN` keyword in the metadata TEXT section, where `n` is the provided parameter index (1-based)
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_parameter_channel_name(&self, parameter_number: usize) -> Result<&str> {
        if let StringKeyword::PnN(name) =
            self.get_parameter_string_metadata(parameter_number, "N")?
        {
            Ok(name.as_ref())
        } else {
            Err(anyhow!(
                "$P{parameter_number}N keyword not found in metadata TEXT section",
            ))
        }
    }

    /// Return the label name of the parameter from the `$PnS` keyword in the metadata TEXT section, where `n` is the provided parameter number
    /// # Errors
    /// Will return `Err` if the keyword is not present in the keywords hashmap
    pub fn get_parameter_label(&self, parameter_number: usize) -> Result<&str> {
        if let StringKeyword::PnS(label) =
            self.get_parameter_string_metadata(parameter_number, "S")?
        {
            Ok(label.as_ref())
        } else {
            Err(anyhow!(
                "$P{parameter_number}S keyword not found in metadata TEXT section",
            ))
        }
    }

    /// Transform the metadata keywords hashmap into a JSON object via serde
    /// # Errors
    /// Will return `Err` if the metadata keywords hashmap is empty
    pub fn get_metadata_as_json_string(&self) -> Result<String> {
        if self.keywords.is_empty() {
            Err(anyhow!("No metadata keywords stored."))
        } else {
            let json = serde_json::to_string(&self.keywords)?;
            Ok(json)
        }
    }

    /// Insert or update a string keyword in the metadata
    pub fn insert_string_keyword(&mut self, key: String, value: String) {
        let normalized_key = if key.starts_with('$') {
            key
        } else {
            format!("${key}")
        };

        let parsed = match_and_parse_keyword(&normalized_key, value.as_str());
        let string_keyword = match parsed {
            KeywordCreationResult::String(string_keyword) => string_keyword,
            // If parsing fails (or parses to a non-string keyword), fall back to `Other`.
            _ => StringKeyword::Other(Arc::from(value)),
        };

        self.keywords
            .insert(normalized_key, Keyword::String(string_keyword));
    }

    /// Create metadata from a DataFrame and ParameterMap
    ///
    /// This helper function creates all required FCS metadata keywords from scratch,
    /// including file structure keywords ($BYTEORD, $DATATYPE, $MODE, $PAR, $TOT, $NEXTDATA)
    /// and parameter-specific keywords ($PnN, $PnS, $PnB, $PnE, $PnR) for each parameter.
    ///
    /// # Arguments
    /// * `df` - The DataFrame containing event data
    /// * `params` - The ParameterMap containing parameter metadata
    ///
    /// # Returns
    /// A new Metadata struct with all required keywords populated
    pub fn from_dataframe_and_parameters(
        df: &polars::prelude::DataFrame,
        params: &super::parameter::ParameterMap,
    ) -> Result<Self> {
        let mut metadata = Self::new();
        let n_events = df.height();
        let n_params = df.width();

        // Required file structure keywords
        // BYTEORD - use LittleEndian as default (1,2,3,4)
        metadata.keywords.insert(
            "$BYTEORD".to_string(),
            Keyword::Byte(ByteKeyword::BYTEORD(ByteOrder::LittleEndian)),
        );

        // DATATYPE - use F (float32) as default
        metadata.keywords.insert(
            "$DATATYPE".to_string(),
            Keyword::Byte(ByteKeyword::DATATYPE(FcsDataType::F)),
        );

        // MODE
        metadata.insert_string_keyword("$MODE".to_string(), "L".to_string());

        // PAR
        metadata.keywords.insert(
            "$PAR".to_string(),
            Keyword::Int(IntegerKeyword::PAR(n_params)),
        );

        // TOT
        metadata.keywords.insert(
            "$TOT".to_string(),
            Keyword::Int(IntegerKeyword::TOT(n_events)),
        );

        // NEXTDATA
        metadata.insert_string_keyword("$NEXTDATA".to_string(), "0".to_string());

        // Add parameter keywords ($PnN, $PnS, $PnB, $PnE, $PnR)
        // Get column names from DataFrame in order
        let column_names = df.get_column_names();
        for (param_idx, param_name) in column_names.iter().enumerate() {
            let param_num = param_idx + 1;

            // Get parameter from ParameterMap if available
            // Convert PlSmallStr to Arc<str> for ParameterMap lookup
            let param_name_arc: Arc<str> = Arc::from(param_name.as_str());
            if let Some(param) = params.get(&param_name_arc) {
                // $PnN - Parameter name
                metadata.insert_string_keyword(format!("$P{}N", param_num), param_name.to_string());

                // $PnS - Parameter label (short name)
                metadata.insert_string_keyword(
                    format!("$P{}S", param_num),
                    param.label_name.to_string(),
                );

                // $PnB - Bits per parameter (default: 32 for float32)
                metadata.keywords.insert(
                    format!("$P{}B", param_num),
                    Keyword::Int(IntegerKeyword::PnB(32)),
                );

                // $PnE - Amplification (default: 0,0)
                metadata.insert_string_keyword(format!("$P{}E", param_num), "0,0".to_string());

                // $PnR - Range (default: 262144)
                metadata.keywords.insert(
                    format!("$P{}R", param_num),
                    Keyword::Int(IntegerKeyword::PnR(262144)),
                );
            } else {
                // Fallback if parameter not in ParameterMap
                metadata.insert_string_keyword(format!("$P{}N", param_num), param_name.to_string());
                metadata.insert_string_keyword(format!("$P{}S", param_num), param_name.to_string());

                metadata.keywords.insert(
                    format!("$P{}B", param_num),
                    Keyword::Int(IntegerKeyword::PnB(32)),
                );

                metadata.insert_string_keyword(format!("$P{}E", param_num), "0,0".to_string());

                metadata.keywords.insert(
                    format!("$P{}R", param_num),
                    Keyword::Int(IntegerKeyword::PnR(262144)),
                );
            }
        }

        // Generate GUID
        metadata.validate_guid();

        Ok(metadata)
    }
}

#[cfg(test)]
mod from_text_segment_tests {
    use super::Metadata;
    use std::io::Write;

    /// Writes `text_body` (the TEXT segment's bytes, *including* its leading
    /// delimiter byte, matching what `Header::text_offset` would point at)
    /// to a temp file, memory-maps it, and runs it through
    /// `Metadata::from_text_segment`. `from_text_segment` only needs a byte
    /// range into a mapped file — it does not need a real HEADER/DATA/other
    /// segments around it — so this is a faithful, minimal exercise of the
    /// function under test.
    fn parse(text_body: &[u8]) -> Metadata {
        let mut file = tempfile::NamedTempFile::new().expect("temp file");
        file.write_all(text_body).expect("write temp file");
        file.flush().expect("flush temp file");
        // SAFETY: file is exclusively owned by this test and not mutated
        // after the write above; same convention `Fcs::open` and
        // `compress.rs`'s tests use.
        let mmap = unsafe { memmap3::Mmap::map(file.as_file()) }.expect("mmap temp file");
        // These tests pin FCS2.0-era tokenizer behaviour (an empty value
        // between two delimiters must still be recorded), so exercise them
        // under the pre-3.1 (non-doubling) escaping policy.
        Metadata::from_text_segment(&mmap, &(0..=(text_body.len() - 1)), crate::version::Version::V2_0)
    }

    #[test]
    fn a_dangling_key_terminated_by_a_delimiter_is_dropped() {
        // Accepted, harmless divergence from the pre-extraction implementation:
        // that version silently dropped a keyword-with-no-value here (its
        // trailing-segment guard never ran, because nothing followed the
        // dangling key's own terminating delimiter). The rewritten loop
        // reaches its "no value" branch instead and additionally logs a
        // `tracing::debug!` — observationally identical (nothing is
        // returned either way) but worth pinning here so a future change
        // cannot start inserting "BADKEY" without a test noticing.
        let metadata = parse(b"|$PAR|2|BADKEY|");

        assert_eq!(metadata.keywords.len(), 1, "{:?}", metadata.keywords);
        assert!(metadata.keywords.contains_key("$PAR"));
        assert!(!metadata.keywords.contains_key("BADKEY"));
    }

    #[test]
    fn an_unterminated_invalid_utf8_tail_value_is_dropped_not_recorded_as_empty() {
        // TEXT need not end on the delimiter. If the last, unterminated
        // field is invalid UTF-8 (a Latin-1 byte in `$COM`/`$PnS` is a real
        // occurrence in the wild), `TextFields` decodes it to "" — and
        // without the restored guard, that "" was getting inserted as
        // `$COM`'s value, which the pre-extraction implementation never did
        // (its trailing-segment guard checked decoded emptiness and skipped
        // the whole pair). 0xFF is never valid UTF-8 in any position.
        let mut body = b"|$PAR|2|$COM|".to_vec();
        body.push(0xFF);
        let metadata = parse(&body);

        assert_eq!(metadata.keywords.len(), 1, "{:?}", metadata.keywords);
        assert!(metadata.keywords.contains_key("$PAR"));
        assert!(
            !metadata.keywords.contains_key("$COM"),
            "an invalid-UTF-8 unterminated tail must not be recorded as an empty value: {:?}",
            metadata.keywords
        );
    }

    #[test]
    fn a_genuinely_empty_value_between_delimiters_is_still_recorded() {
        // Contrast case: this is delimiter-bounded, not an unterminated
        // tail, so the restored guard must not touch it. FCS2.0/3.0 files
        // use this to mean "empty value" (see `text.rs`'s
        // `no_escaping_treats_a_doubled_delimiter_as_an_empty_field`), and
        // that must keep working even though it is also the last field.
        let metadata = parse(b"|$PAR|2|$COM||");

        assert_eq!(metadata.keywords.len(), 2, "{:?}", metadata.keywords);
        assert!(metadata.keywords.contains_key("$COM"));
    }
}

#[cfg(test)]
mod delimiter_escaping_read_tests {
    use crate::file::Fcs;

    /// Every corpus file must parse to the same keyword count it did before
    /// escaping existed. Two of the ten (`fcs2_int16_13367ev_8par_GvHD.fcs`
    /// and `real-8-parameters.data.fcs`) carry FCS2.0 empty values and are the
    /// load-bearing cases: under an unconditional un-double they would lose
    /// keywords as fields shifted.
    #[test]
    fn every_corpus_file_parses_without_field_shift() {
        if !crate::corpus::is_available() {
            eprintln!("compliance corpus missing, skipping");
            return;
        }
        for path in crate::corpus::files() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let fcs = Fcs::open(path.to_str().expect("utf-8 path"))
                .unwrap_or_else(|e| panic!("open {name}: {e}"));

            // A shifted tokenizer loses $PAR/$TOT or reads them as garbage,
            // so these two accessors are a sharp shift detector.
            let par = *fcs.metadata.get_number_of_parameters()
                .unwrap_or_else(|e| panic!("{name}: $PAR: {e}"));
            let tot = *fcs.metadata.get_number_of_events()
                .unwrap_or_else(|e| panic!("{name}: $TOT: {e}"));
            assert!(par > 0, "{name}: $PAR must be positive");
            assert!(tot > 0, "{name}: $TOT must be positive");
            assert_eq!(
                fcs.data_frame.width(), par,
                "{name}: decoded column count must match $PAR"
            );
        }
    }
}
