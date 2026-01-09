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
        let text_start = header.text_offset.start();

        // Read the first byte of the text segment to determine the delimiter:
        let delimiter = mmap[*text_start];

        // Determine the number of bytes to read, excluding the delimiter:
        let text_end = header.text_offset.end();
        let text_slice = &mmap[(*text_start + 1)..*text_end];

        // Extract keyword value pairs using memchr for fast delimiter finding
        let mut keywords: KeywordMap = FxHashMap::default();

        // Find all delimiter positions using SIMD-accelerated search
        // This is 5-10x faster than manual iteration
        let delimiter_positions: Vec<usize> = memchr::memchr_iter(delimiter, text_slice).collect();

        // Parse keyword-value pairs
        // FCS format: |KEY1|VALUE1|KEY2|VALUE2|...
        // delimiter_positions gives us the split points
        let mut prev_pos = 0;
        let mut is_keyword = true;
        let mut current_key = String::new();

        for &pos in &delimiter_positions {
            // Extract the slice between delimiters
            let segment = &text_slice[prev_pos..pos];

            // SAFETY: FCS spec requires TEXT segment to be ASCII/UTF-8
            let text = std::str::from_utf8(segment).unwrap_or_default();

            if is_keyword {
                // This is a keyword
                current_key = text.to_string();
                is_keyword = false;
            } else {
                // This is a value - parse and store the keyword-value pair
                if !current_key.is_empty() {
                    // Normalize key: ensure it has $ prefix (FCS spec requires it)
                    // Store with $ prefix for consistent lookups
                    let normalized_key: String = if current_key.starts_with('$') {
                        current_key.clone()
                    } else {
                        format!("${}", current_key)
                    };

                    match match_and_parse_keyword(&current_key, text) {
                        KeywordCreationResult::Int(int_keyword) => {
                            keywords.insert(normalized_key.clone(), Keyword::Int(int_keyword));
                        }
                        KeywordCreationResult::Float(float_keyword) => {
                            keywords.insert(normalized_key.clone(), Keyword::Float(float_keyword));
                        }
                        KeywordCreationResult::String(string_keyword) => {
                            keywords
                                .insert(normalized_key.clone(), Keyword::String(string_keyword));
                        }
                        KeywordCreationResult::Byte(byte_keyword) => {
                            keywords.insert(normalized_key.clone(), Keyword::Byte(byte_keyword));
                        }
                        KeywordCreationResult::Mixed(mixed_keyword) => {
                            keywords.insert(normalized_key.clone(), Keyword::Mixed(mixed_keyword));
                        }
                        KeywordCreationResult::UnableToParse => {
                            eprintln!(
                                "Unable to parse keyword: {} with value: {}",
                                current_key, text
                            );
                        }
                    }
                }
                current_key.clear();
                is_keyword = true;
            }

            prev_pos = pos + 1;
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
        println!("Validating FCS file...{}", header.version);
        let required_keywords = header.version.get_required_keywords();
        for keyword in required_keywords {
            if !self.keywords.contains_key(*keyword) {
                // println!("Invalid FCS file: Missing keyword: {:#?}", self.keywords);
                return Err(anyhow!("Invalid FCS file: Missing keyword: {}", keyword));
            }
        }

        Ok(())
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
                    .expect("should have a second character in {keyword}")
                    .to_digit(10)
                    .expect("should be able to convert the character to a digit to count the parameters") as usize;
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
            self.get_parameter_numeric_metadata(parameter_number, "DATATYPE")
        {
            if let IntegerKeyword::PnDATATYPE(datatype_code) = pn_datatype_keyword {
                // Map datatype code to enum: 0=I, 1=F, 2=D
                match datatype_code {
                    0 => Ok(FcsDataType::I),
                    1 => Ok(FcsDataType::F),
                    2 => Ok(FcsDataType::D),
                    _ => Err(anyhow!(
                        "Invalid $P{}DATATYPE code: {}",
                        parameter_number,
                        datatype_code
                    )),
                }
            } else {
                // Shouldn't happen, but fall back to default
                Ok(self.get_data_type()?.clone())
            }
        } else {
            // Fall back to default $DATATYPE
            Ok(self.get_data_type()?.clone())
        }
    }

    /// Calculate the total bytes per event by summing bytes per parameter
    ///
    /// Uses `$PnB` (bits per parameter) divided by 8 to get bytes per parameter,
    /// then sums across all parameters. This is more accurate than using `$DATATYPE`
    /// which only provides a default value.
    ///
    /// # Errors
    /// Will return `Err` if the number of parameters cannot be determined or
    /// if any required `$PnB` keyword is missing
    pub fn calculate_bytes_per_event(&self) -> Result<usize> {
        let number_of_parameters = self.get_number_of_parameters()?;
        let mut total_bytes = 0;

        for param_num in 1..=*number_of_parameters {
            // Get $PnB (bits per parameter)
            let bits = self.get_parameter_numeric_metadata(param_num, "B")?;
            if let IntegerKeyword::PnB(bits_value) = bits {
                // Convert bits to bytes (round up if not divisible by 8)
                let bytes = (bits_value + 7) / 8;
                total_bytes += bytes;
            } else {
                return Err(anyhow!(
                    "$P{}B keyword found but is not the expected PnB variant",
                    param_num
                ));
            }
        }

        Ok(total_bytes)
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

    /// Generic function to get a given parameter's integer keyword from the file's metadata (e.g. `$PnN`, `$PnS`, `$PnDATATYPE`)
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
}
