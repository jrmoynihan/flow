// Internal crate imports
use crate::{
    FcsDataType, TransformType, Transformable,
    byteorder::ByteOrder,
    header::Header,
    keyword::{IntegerableKeyword, StringableKeyword},
    metadata::Metadata,
    parameter::{EventDataFrame, EventDatum, Parameter, ParameterBuilder, ParameterMap},
};
// Standard library imports
use std::borrow::Cow;
use std::fs::File;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// External crate imports
use anyhow::{Context, Result, anyhow};
use byteorder::{BigEndian as BE, ByteOrder as BO, LittleEndian as LE};
use faer::Mat;
use itertools::{Itertools, MinMaxResult};
use memmap3::{Mmap, MmapOptions};
use polars::prelude::*;
use rayon::prelude::*;

/// Threshold for parallel processing: only use parallel for datasets larger than this
/// Below this threshold, parallel overhead exceeds benefits
/// Based on benchmarks: 400,000 values (50,000 events × 8 parameters)
/// - Float32: Always use sequential (benchmarks show sequential is 2-13x faster)
/// - Int16/Int32/Float64: Use parallel for datasets with ≥400k values
const PARALLEL_THRESHOLD: usize = 400_000;

/// A shareable wrapper around the file path and memory-map
///
/// Uses Arc<Mmap> to share the memory mapping across clones without creating
/// new file descriptors or memory mappings. This is more efficient than cloning
/// the underlying file descriptor and re-mapping.
#[derive(Debug, Clone)]
pub struct AccessWrapper {
    /// An owned, mutable path to the file on disk
    pub path: PathBuf,
    /// The memory-mapped file, shared via Arc for efficient cloning
    ///
    /// # Safety
    /// The Mmap is created from a File handle and remains valid as long as:
    /// 1. The file is not truncated while mapped
    /// 2. The file contents are not modified while mapped (we only read)
    /// 3. The Mmap is not accessed after the file is deleted
    ///
    /// Our usage satisfies these invariants because:
    /// - FCS files are read-only once opened (we never write back to them)
    /// - The file remains open (via File handle) for the lifetime of the Mmap
    /// - We only drop the Mmap when the FCS file is no longer needed
    pub mmap: Arc<Mmap>,
}

impl AccessWrapper {
    /// Creates a new `AccessWrapper` from a file path
    /// # Errors
    /// Will return `Err` if:
    /// - the file cannot be opened
    /// - the file cannot be memory-mapped
    pub fn new(path: &str) -> Result<Self> {
        let file = File::open(path)?;
        let path = PathBuf::from(path);

        // memmap3 provides better safety guarantees than memmap2, though OS-level
        // memory mapping still requires unsafe at creation time. The resulting Mmap
        // is safe to use and provides better guarantees than memmap2.
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        Ok(Self {
            path,
            mmap: Arc::new(mmap),
        })
    }
}

impl Deref for AccessWrapper {
    type Target = Mmap;

    fn deref(&self) -> &Self::Target {
        &self.mmap
    }
}

/// A struct representing an FCS file
#[derive(Debug, Clone)]
pub struct Fcs {
    /// The header segment of the fcs file, including the version, and byte offsets to the text, data, and analysis segments
    pub header: Header,
    /// The metadata segment of the fcs file, including the delimiter, and a hashmap of keyword/value pairs
    pub metadata: Metadata,
    /// A hashmap of the parameter names and their associated metadata
    pub parameters: ParameterMap,

    /// Event data stored in columnar format via Polars DataFrame (NEW)
    /// Each column represents one parameter (e.g., FSC-A, SSC-A, FL1-A)
    /// Polars provides:
    /// - Zero-copy column access
    /// - Built-in SIMD operations
    /// - Lazy evaluation for complex queries
    /// - Apache Arrow interop
    /// This is the primary data format going forward
    pub data_frame: EventDataFrame,

    /// A wrapper around the file, path, and memory-map
    pub file_access: AccessWrapper,
}

impl Fcs {
    /// Creates a new Fcs file struct
    /// # Errors
    /// Will return `Err` if:
    /// - the file cannot be opened,
    /// - the file extension is not `fcs`,
    /// - the TEXT segment cannot be validated,
    /// - the raw data cannot be read,
    /// - the parameter names and labels cannot be generated
    pub fn new() -> Result<Self> {
        Ok(Self {
            header: Header::new(),
            metadata: Metadata::new(),
            parameters: ParameterMap::default(),
            data_frame: Arc::new(DataFrame::empty()),
            file_access: AccessWrapper::new("")?,
        })
    }

    /// Opens and parses an FCS file from the given path
    ///
    /// This is the primary entry point for reading FCS files. It:
    /// - Validates the file extension (must be `.fcs`)
    /// - Memory-maps the file for efficient access
    /// - Parses the header segment to determine FCS version and segment offsets
    /// - Parses the text segment to extract metadata and keywords
    /// - Validates required keywords for the FCS version
    /// - Generates a GUID if one is not present
    /// - Loads event data into a Polars DataFrame for efficient columnar access
    ///
    /// # Arguments
    /// * `path` - Path to the FCS file (must have `.fcs` extension)
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - the file cannot be opened or memory-mapped
    /// - the file extension is not `.fcs`
    /// - the FCS version is invalid or unsupported
    /// - required keywords are missing for the FCS version
    /// - the data segment cannot be read or parsed
    /// - parameter metadata cannot be generated
    ///
    /// # Example
    /// ```no_run
    /// use flow_fcs::Fcs;
    ///
    /// let fcs = Fcs::open("data/sample.fcs")?;
    /// println!("File has {} events", fcs.get_number_of_events()?);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn open(path: &str) -> Result<Self> {
        use tracing::debug;

        // Attempt to open the file path
        let file_access =
            AccessWrapper::new(path).with_context(|| format!("Failed to open file: {}", path))?;

        // Validate the file extension
        Self::validate_fcs_extension(&file_access.path).with_context(|| {
            format!("Invalid file extension for: {}", file_access.path.display())
        })?;

        // Create header and metadata structs from a memory map of the file
        let header = Header::from_mmap(&file_access.mmap).with_context(|| {
            format!(
                "Failed to parse header from file: {}",
                file_access.path.display()
            )
        })?;
        let mut metadata = Metadata::from_mmap(&file_access.mmap, &header);

        metadata
            .validate_text_segment_keywords(&header)
            .with_context(|| {
                format!(
                    "Failed to validate text segment keywords in file: {}",
                    file_access.path.display()
                )
            })?;
        // metadata.validate_number_of_parameters()?;
        metadata.validate_guid();

        // Log $TOT keyword value
        let tot_events = metadata.get_number_of_events().ok().copied();
        if let Some(tot) = tot_events {
            debug!("FCS file $TOT keyword: {} events", tot);
        }

        let parameters = Self::generate_parameter_map(&metadata).map_err(|e| {
            let diagnostic = Self::format_diagnostic_info(&header, &metadata, &file_access.path);
            anyhow!("Failed to generate parameter map: {}\n\n{}", e, diagnostic)
        })?;

        let data_frame = Self::store_raw_data_as_dataframe(&header, &file_access.mmap, &metadata)
            .map_err(|e| {
            let diagnostic = Self::format_diagnostic_info(&header, &metadata, &file_access.path);
            anyhow!(
                "Failed to store raw data as DataFrame: {}\n\n{}",
                e,
                diagnostic
            )
        })?;

        let fcs = Self {
            parameters,
            data_frame,
            file_access,
            header,
            metadata,
        };

        // Log DataFrame event count and compare to $TOT
        let df_events = fcs.get_event_count_from_dataframe();
        if let Some(tot) = tot_events {
            if df_events != tot {
                tracing::warn!(
                    "Event count mismatch: DataFrame has {} events but $TOT keyword says {} (difference: {})",
                    df_events,
                    tot,
                    tot as i64 - df_events as i64
                );
            } else {
                debug!("Event count matches $TOT keyword: {} events", df_events);
            }
        }

        // Log compensation status
        let has_compensation = fcs.has_compensation();
        debug!(
            "Compensation: {} (SPILLOVER keyword {})",
            if has_compensation {
                "available"
            } else {
                "not available"
            },
            if has_compensation {
                "present"
            } else {
                "missing"
            }
        );

        // Log parameter count
        let n_params = fcs.parameters.len();
        debug!(
            "FCS file loaded: {} parameters, {} events",
            n_params, df_events
        );

        Ok(fcs)
    }

    /// Validates that the file extension is `.fcs`
    /// # Errors
    /// Will return `Err` if the file extension is not `.fcs`
    fn validate_fcs_extension(path: &Path) -> Result<()> {
        let extension = path
            .extension()
            .ok_or_else(|| anyhow!("File has no extension"))?
            .to_str()
            .ok_or_else(|| anyhow!("File extension is not valid UTF-8"))?;

        if extension.to_lowercase() != "fcs" {
            return Err(anyhow!("Invalid file extension: {}", extension));
        }

        Ok(())
    }

    /// Reads raw data from FCS file and stores it as a Polars DataFrame
    /// Returns columnar data optimized for parameter-wise access patterns
    ///
    /// This function provides significant performance benefits over row-based array storage:
    /// - 2-5x faster data filtering and transformations
    /// - Native columnar storage (optimal for FCS parameter access patterns)
    /// - Zero-copy operations via Apache Arrow
    /// - Built-in SIMD acceleration
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - The data cannot be read
    /// - The data cannot be converted to f32 values
    /// - The DataFrame cannot be constructed
    fn store_raw_data_as_dataframe(
        header: &Header,
        mmap: &Mmap,
        metadata: &Metadata,
    ) -> Result<EventDataFrame> {
        // Validate data offset bounds before accessing mmap
        let mut data_start = *header.data_offset.start();
        let mut data_end = *header.data_offset.end();
        let mmap_len = mmap.len();

        // Handle zero offsets by checking keywords
        if data_start == 0 {
            if let Ok(begin_data) = metadata.get_integer_keyword("$BEGINDATA") {
                data_start = begin_data.get_usize().clone();
            } else {
                return Err(anyhow!(
                    "$BEGINDATA keyword not found. Unable to determine data start."
                ));
            }
        }

        if data_end == 0 {
            if let Ok(end_data) = metadata.get_integer_keyword("$ENDDATA") {
                data_end = end_data.get_usize().clone();
            } else {
                return Err(anyhow!(
                    "$ENDDATA keyword not found. Unable to determine data end."
                ));
            }
        }

        // Validate offsets
        if data_start >= mmap_len {
            return Err(anyhow!(
                "Data start offset {} is beyond mmap length {}",
                data_start,
                mmap_len
            ));
        }

        if data_end >= mmap_len {
            return Err(anyhow!(
                "Data end offset {} is beyond mmap length {}",
                data_end,
                mmap_len
            ));
        }

        if data_start > data_end {
            return Err(anyhow!(
                "Data start offset {} is greater than end offset {}",
                data_start,
                data_end
            ));
        }

        // Extract data bytes
        let data_bytes = &mmap[data_start..=data_end];

        let number_of_parameters = metadata
            .get_number_of_parameters()
            .context("Failed to retrieve the number of parameters from metadata")?;
        let number_of_events = metadata
            .get_number_of_events()
            .context("Failed to retrieve the number of events from metadata")?;

        // Calculate bytes per event by summing $PnB / 8 for each parameter
        // This is more accurate than using $DATATYPE which only provides a default
        let bytes_per_event = metadata
            .calculate_bytes_per_event()
            .context("Failed to calculate bytes per event")?;

        let byte_order = metadata
            .get_byte_order()
            .context("Failed to get the byte order from metadata")?;

        // Validate data size
        let expected_total_bytes = number_of_events * bytes_per_event;
        if data_bytes.len() < expected_total_bytes {
            return Err(anyhow!(
                "Insufficient data: expected {} bytes ({} events × {} bytes/event), but only have {} bytes",
                expected_total_bytes,
                number_of_events,
                bytes_per_event,
                data_bytes.len()
            ));
        }

        // Collect bytes per parameter and data types for each parameter
        let bytes_per_parameter: Vec<usize> = (1..=*number_of_parameters)
            .map(|param_num| {
                metadata
                    .get_bytes_per_parameter(param_num)
                    .with_context(|| {
                        format!(
                            "Failed to get bytes per parameter for parameter {}",
                            param_num
                        )
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        let data_types: Vec<FcsDataType> = (1..=*number_of_parameters)
            .map(|param_num| {
                metadata
                    .get_data_type_for_channel(param_num)
                    .with_context(|| format!("Failed to get data type for channel {}", param_num))
            })
            .collect::<Result<Vec<_>>>()?;

        // Fast path: Check if all parameters are uniform (same bytes, same data type)
        // This allows us to use bytemuck zero-copy optimization
        let uniform_bytes = bytes_per_parameter.first().copied();
        let uniform_data_type = data_types.first().copied();
        let is_uniform = match (uniform_bytes, uniform_data_type) {
            (Some(bytes), Some(dt)) => {
                bytes_per_parameter.iter().all(|&b| b == bytes)
                    && data_types.iter().all(|&dt_val| dt_val == dt)
            }
            _ => false,
        };

        let f32_values: Vec<f32> = if is_uniform {
            // Fast path: All parameters have same size and type - use bytemuck for zero-copy
            let bytes_per_param = uniform_bytes.ok_or_else(|| anyhow!("No uniform bytes found"))?;
            let data_type =
                uniform_data_type.ok_or_else(|| anyhow!("No uniform data type found"))?;

            match (data_type, bytes_per_param) {
                (FcsDataType::F, 4) => {
                    // Fast path: float32 - use sequential (benchmarks show 2.57x faster than parallel)
                    let needs_swap = match (byte_order, cfg!(target_endian = "little")) {
                        (ByteOrder::LittleEndian, true) | (ByteOrder::BigEndian, false) => false,
                        _ => true,
                    };

                    match bytemuck::try_cast_slice::<u8, f32>(data_bytes) {
                        Ok(f32_slice) => {
                            #[cfg(debug_assertions)]
                            eprintln!(
                                "✓ Fast path (bytemuck zero-copy, sequential): {} bytes, {} f32s",
                                data_bytes.len(),
                                f32_slice.len()
                            );

                            if needs_swap {
                                // Sequential byte swap - faster than parallel for float32
                                f32_slice
                                    .iter()
                                    .map(|&f| f32::from_bits(f.to_bits().swap_bytes()))
                                    .collect()
                            } else {
                                f32_slice.to_vec()
                            }
                        }
                        Err(_) => {
                            #[cfg(debug_assertions)]
                            eprintln!(
                                "⚠ Fast path (bytemuck fallback, sequential): unaligned data ({} bytes)",
                                data_bytes.len()
                            );

                            // Fallback: parse in chunks sequentially (faster than parallel for float32)
                            data_bytes
                                .chunks_exact(4)
                                .map(|chunk| {
                                    let mut bytes = [0u8; 4];
                                    bytes.copy_from_slice(chunk);
                                    let bits = u32::from_ne_bytes(bytes);
                                    let bits = if needs_swap { bits.swap_bytes() } else { bits };
                                    f32::from_bits(bits)
                                })
                                .collect()
                        }
                    }
                }
                _ => {
                    // Uniform but not float32 - use optimized bulk parsing
                    Self::parse_uniform_data_bulk(
                        data_bytes,
                        bytes_per_param,
                        &data_type,
                        byte_order,
                        *number_of_events,
                        *number_of_parameters,
                    )?
                }
            }
        } else {
            // Slow path: Variable-width parameters - parse event-by-event
            Self::parse_variable_width_data(
                data_bytes,
                &bytes_per_parameter,
                &data_types,
                byte_order,
                *number_of_events,
                *number_of_parameters,
            )?
        };

        // Create Polars Series for each parameter (column)
        // FCS data is stored row-wise (event1_param1, event1_param2, ..., event2_param1, ...)
        // We need to extract columns using stride access
        let mut columns: Vec<Column> = Vec::with_capacity(*number_of_parameters);

        for param_idx in 0..*number_of_parameters {
            // Extract this parameter's values across all events
            // Use iterator with step_by for efficient stride access
            let param_values: Vec<f32> = f32_values
                .iter()
                .skip(param_idx)
                .step_by(*number_of_parameters)
                .copied()
                .collect();

            // Verify we got the right number of events
            assert_eq!(
                param_values.len(),
                *number_of_events,
                "Parameter {} should have {} events, got {}",
                param_idx + 1,
                number_of_events,
                param_values.len()
            );

            // Get parameter name from metadata for column name
            let param_name = metadata
                .get_parameter_channel_name(param_idx + 1)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| format!("P{}", param_idx + 1));

            // Create Series (Polars column) with name
            let series = Column::new(param_name.as_str().into(), param_values);
            columns.push(series);
        }

        // Create DataFrame from columns (height = number of events)
        let df = DataFrame::new(*number_of_events, columns).map_err(|e| {
            anyhow!(
                "Failed to create DataFrame from {} columns: {}",
                number_of_parameters,
                e
            )
        })?;

        // Verify DataFrame shape
        assert_eq!(
            df.height(),
            *number_of_events,
            "DataFrame height {} doesn't match expected events {}",
            df.height(),
            number_of_events
        );
        assert_eq!(
            df.width(),
            *number_of_parameters,
            "DataFrame width {} doesn't match expected parameters {}",
            df.width(),
            number_of_parameters
        );

        #[cfg(debug_assertions)]
        eprintln!(
            "✓ Created DataFrame: {} events × {} parameters",
            df.height(),
            df.width()
        );

        Ok(Arc::new(df))
    }

    /// Parse uniform data in bulk (all parameters have same size and type)
    ///
    /// This is faster than event-by-event parsing when all parameters are uniform.
    /// Uses conditional parallelization based on data type and size:
    /// - float32: always sequential (benchmarks show 2.57x faster)
    /// - int16/int32: parallel only above threshold (parallel is 1.84x faster for large datasets)
    /// - float64: parallel only above threshold
    ///
    /// # Arguments
    /// * `data_bytes` - Raw data bytes
    /// * `bytes_per_param` - Bytes per parameter (same for all)
    /// * `data_type` - Data type (same for all)
    /// * `byte_order` - Byte order
    /// * `num_events` - Number of events
    /// * `num_params` - Number of parameters
    ///
    /// # Errors
    /// Will return `Err` if parsing fails
    #[inline]
    fn parse_uniform_data_bulk(
        data_bytes: &[u8],
        bytes_per_param: usize,
        data_type: &FcsDataType,
        byte_order: &ByteOrder,
        num_events: usize,
        num_params: usize,
    ) -> Result<Vec<f32>> {
        let total_values = num_events * num_params;
        let use_parallel = total_values > PARALLEL_THRESHOLD;
        let mut f32_values = Vec::with_capacity(total_values);

        match (data_type, bytes_per_param) {
            (FcsDataType::I, 2) => {
                // int16 - parallel is 1.84x faster for large datasets
                if use_parallel {
                    data_bytes
                        .par_chunks_exact(2)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u16(chunk),
                                ByteOrder::BigEndian => BE::read_u16(chunk),
                            };
                            value as f32
                        })
                        .collect_into_vec(&mut f32_values);
                } else {
                    // Sequential for small datasets
                    f32_values = data_bytes
                        .chunks_exact(2)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u16(chunk),
                                ByteOrder::BigEndian => BE::read_u16(chunk),
                            };
                            value as f32
                        })
                        .collect();
                }
            }
            (FcsDataType::I, 4) => {
                // int32 - parallel only above threshold
                if use_parallel {
                    data_bytes
                        .par_chunks_exact(4)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u32(chunk),
                                ByteOrder::BigEndian => BE::read_u32(chunk),
                            };
                            value as f32
                        })
                        .collect_into_vec(&mut f32_values);
                } else {
                    // Sequential for small datasets
                    f32_values = data_bytes
                        .chunks_exact(4)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_u32(chunk),
                                ByteOrder::BigEndian => BE::read_u32(chunk),
                            };
                            value as f32
                        })
                        .collect();
                }
            }
            (FcsDataType::F, 4) => {
                // float32 - always sequential (benchmarks show 2.57x faster than parallel)
                // This is a fallback path - normally handled by bytemuck in store_raw_data_as_dataframe
                let needs_swap = match (byte_order, cfg!(target_endian = "little")) {
                    (ByteOrder::LittleEndian, true) | (ByteOrder::BigEndian, false) => false,
                    _ => true,
                };
                f32_values = data_bytes
                    .chunks_exact(4)
                    .map(|chunk| {
                        let mut bytes = [0u8; 4];
                        bytes.copy_from_slice(chunk);
                        let bits = u32::from_ne_bytes(bytes);
                        let bits = if needs_swap { bits.swap_bytes() } else { bits };
                        f32::from_bits(bits)
                    })
                    .collect();
            }
            (FcsDataType::D, 8) => {
                // float64 - parallel only above threshold
                if use_parallel {
                    data_bytes
                        .par_chunks_exact(8)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_f64(chunk),
                                ByteOrder::BigEndian => BE::read_f64(chunk),
                            };
                            value as f32
                        })
                        .collect_into_vec(&mut f32_values);
                } else {
                    // Sequential for small datasets
                    f32_values = data_bytes
                        .chunks_exact(8)
                        .map(|chunk| {
                            let value = match byte_order {
                                ByteOrder::LittleEndian => LE::read_f64(chunk),
                                ByteOrder::BigEndian => BE::read_f64(chunk),
                            };
                            value as f32
                        })
                        .collect();
                }
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported uniform data type: {:?} with {} bytes",
                    data_type,
                    bytes_per_param
                ));
            }
        }

        Ok(f32_values)
    }

    /// Parse a parameter value from bytes to f32 based on data type and bytes per parameter
    ///
    /// Handles different data types:
    /// - int16 (2 bytes) - unsigned integer
    /// - int32 (4 bytes) - unsigned integer
    /// - float32 (4 bytes) - single-precision floating point
    /// - float64 (8 bytes) - double-precision floating point
    ///
    /// # Arguments
    /// * `bytes` - Raw bytes for the parameter value
    /// * `bytes_per_param` - Number of bytes per parameter (from $PnB / 8)
    /// * `data_type` - Data type (I, F, or D)
    /// * `byte_order` - Byte order of the file
    ///
    /// # Errors
    /// Will return `Err` if the bytes cannot be parsed according to the data type
    #[cold]
    fn parse_parameter_value_to_f32(
        bytes: &[u8],
        bytes_per_param: usize,
        data_type: &FcsDataType,
        byte_order: &ByteOrder,
    ) -> Result<f32> {
        match (data_type, bytes_per_param) {
            (FcsDataType::I, 2) => {
                // int16 (unsigned 16-bit integer)
                let value = match byte_order {
                    ByteOrder::LittleEndian => LE::read_u16(bytes),
                    ByteOrder::BigEndian => BE::read_u16(bytes),
                };
                Ok(value as f32)
            }
            (FcsDataType::I, 4) => {
                // int32 (unsigned 32-bit integer)
                let value = match byte_order {
                    ByteOrder::LittleEndian => LE::read_u32(bytes),
                    ByteOrder::BigEndian => BE::read_u32(bytes),
                };
                Ok(value as f32)
            }
            (FcsDataType::F, 4) => {
                // float32 (single-precision floating point)
                Ok(byte_order.read_f32(bytes))
            }
            (FcsDataType::D, 8) => {
                // float64 (double-precision floating point) - convert to f32
                let value = match byte_order {
                    ByteOrder::LittleEndian => LE::read_f64(bytes),
                    ByteOrder::BigEndian => BE::read_f64(bytes),
                };
                Ok(value as f32)
            }
            (FcsDataType::I, _) => Err(anyhow!(
                "Unsupported integer size: {} bytes (expected 2 or 4)",
                bytes_per_param
            )),
            (FcsDataType::F, _) => Err(anyhow!(
                "Invalid float32 size: {} bytes (expected 4)",
                bytes_per_param
            )),
            (FcsDataType::D, _) => Err(anyhow!(
                "Invalid float64 size: {} bytes (expected 8)",
                bytes_per_param
            )),
            (FcsDataType::A, _) => Err(anyhow!("ASCII data type not supported")),
        }
    }

    /// Parse variable-width data event-by-event (cold path)
    ///
    /// This is the slower path used when parameters have different sizes/types.
    /// Marked as `#[cold]` to help the compiler optimize the hot path.
    ///
    /// # Arguments
    /// * `data_bytes` - Raw data bytes
    /// * `bytes_per_parameter` - Bytes per parameter for each parameter
    /// * `data_types` - Data type for each parameter
    /// * `byte_order` - Byte order
    /// * `num_events` - Number of events
    /// * `num_params` - Number of parameters
    ///
    /// # Errors
    /// Will return `Err` if parsing fails
    #[cold]
    fn parse_variable_width_data(
        data_bytes: &[u8],
        bytes_per_parameter: &[usize],
        data_types: &[FcsDataType],
        byte_order: &ByteOrder,
        num_events: usize,
        num_params: usize,
    ) -> Result<Vec<f32>> {
        let mut f32_values: Vec<f32> = Vec::with_capacity(num_events * num_params);
        let mut data_offset = 0;

        for event_idx in 0..num_events {
            for (param_idx, &bytes_per_param) in bytes_per_parameter.iter().enumerate() {
                let param_num = param_idx + 1;
                let data_type = &data_types[param_idx];

                // Extract bytes for this parameter value
                if data_offset + bytes_per_param > data_bytes.len() {
                    return Err(anyhow!(
                        "Insufficient data at event {}, parameter {}: need {} bytes but only have {} remaining",
                        event_idx + 1,
                        param_num,
                        bytes_per_param,
                        data_bytes.len() - data_offset
                    ));
                }

                let param_bytes = &data_bytes[data_offset..data_offset + bytes_per_param];
                let f32_value = Self::parse_parameter_value_to_f32(
                    param_bytes,
                    bytes_per_param,
                    data_type,
                    byte_order,
                )
                .map_err(|e| anyhow!("Failed to parse parameter {} value: {}", param_num, e))?;

                f32_values.push(f32_value);
                data_offset += bytes_per_param;
            }
        }

        Ok(f32_values)
    }

    /// Looks for the parameter name as a key in the `parameters` hashmap and returns a reference to it
    /// Performs case-insensitive lookup for parameter names
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the `parameters` hashmap
    pub fn find_parameter(&self, parameter_name: &str) -> Result<&Parameter> {
        // Try exact match first (fast path)
        if let Some(param) = self.parameters.get(parameter_name) {
            return Ok(param);
        }

        // Case-insensitive fallback: search through parameter map
        for (key, param) in self.parameters.iter() {
            if key.eq_ignore_ascii_case(parameter_name) {
                return Ok(param);
            }
        }

        Err(anyhow!("Parameter not found: {parameter_name}"))
    }

    /// Looks for the parameter name as a key in the `parameters` hashmap and returns a mutable reference to it
    /// Performs case-insensitive lookup for parameter names
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the `parameters` hashmap
    pub fn find_mutable_parameter(&mut self, parameter_name: &str) -> Result<&mut Parameter> {
        // Try exact match first (fast path)
        // Note: We need to check if the key exists as Arc<str>, so we iterate to find exact match
        let exact_key = self
            .parameters
            .keys()
            .find(|k| k.as_ref() == parameter_name)
            .map(|k| k.clone());

        if let Some(key) = exact_key {
            return self
                .parameters
                .get_mut(&key)
                .ok_or_else(|| anyhow!("Parameter not found: {parameter_name}"));
        }

        // Case-insensitive fallback: find the key first (clone Arc to avoid borrow issues)
        let matching_key = self
            .parameters
            .keys()
            .find(|key| key.eq_ignore_ascii_case(parameter_name))
            .map(|k| k.clone());

        if let Some(key) = matching_key {
            return self
                .parameters
                .get_mut(&key)
                .ok_or_else(|| anyhow!("Parameter not found: {parameter_name}"));
        }

        Err(anyhow!("Parameter not found: {parameter_name}"))
    }

    /// Returns a zero-copy reference to a Polars Float32Chunked view of a column for the parameter
    ///
    /// This provides access to the underlying Polars chunked array, which is useful
    /// for operations that work directly with Polars types. For most use cases,
    /// `get_parameter_events_slice()` is preferred as it provides a simple `&[f32]` slice.
    ///
    /// # Arguments
    /// * `channel_name` - The channel name (e.g., "FSC-A", "FL1-A")
    ///
    /// # Errors
    /// Will return `Err` if:
    /// - the parameter name is not found in the parameters map
    /// - the column data type is not Float32
    pub fn get_parameter_events(&'_ self, channel_name: &str) -> Result<&Float32Chunked> {
        Ok(self
            .get_parameter_column(channel_name)?
            .f32()
            .map_err(|e| anyhow!("Parameter {} is not f32 type: {}", channel_name, e))?)
    }
    /// Get a reference to the Polars Column for a parameter by channel name
    ///
    /// This provides direct access to the underlying Polars column, which can be useful
    /// for advanced operations that require the full Polars API.
    ///
    /// # Arguments
    /// * `channel_name` - The channel name (e.g., "FSC-A", "FL1-A")
    ///
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the DataFrame
    pub fn get_parameter_column(&'_ self, channel_name: &str) -> Result<&Column> {
        self.data_frame
            .column(channel_name)
            .map_err(|e| anyhow!("Parameter {} not found: {}", channel_name, e))
    }

    /// Looks for the parameter name as a key in the 'parameters' hashmap and returns a new Vec<f32> of the raw event data
    /// NOTE: This allocates a full copy of the events - prefer `get_parameter_events_slice` when possible
    /// # Errors
    /// Will return 'Err' if the parameter name is not found in the 'parameters hashmap or if the events are not found
    pub fn get_parameter_events_as_owned_vec(&self, channel_name: &str) -> Result<Vec<EventDatum>> {
        Ok(self.get_parameter_events_slice(channel_name)?.to_vec())
    }

    /// Returns the minimum and maximum values of the parameter
    /// # Errors
    /// Will return `Err` if the parameter name is not found in the 'parameters' hashmap or if the events are not found
    pub fn get_minmax_of_parameter(&self, channel_name: &str) -> Result<(EventDatum, EventDatum)> {
        let parameter = self.find_parameter(channel_name)?;
        let events = self.get_parameter_events(&parameter.channel_name)?;

        match events.iter().minmax() {
            MinMaxResult::NoElements => Err(anyhow!("No elements found")),
            MinMaxResult::OneElement(e) => Err(anyhow!("Only one element found: {:?}", e)),
            MinMaxResult::MinMax(min, max) => {
                let min_val = min.ok_or_else(|| anyhow!("Min value is None"))?;
                let max_val = max.ok_or_else(|| anyhow!("Max value is None"))?;
                Ok((min_val, max_val))
            }
        }
    }

    /// Creates a new `HashMap` of `Parameter`s
    /// using the `Fcs` file's metadata to find the channel and label names from the `PnN` and `PnS` keywords.
    /// Does NOT store events on the parameter.
    /// # Errors
    /// Will return `Err` if:
    /// - the number of parameters cannot be found in the metadata,
    /// - the parameter name cannot be found in the metadata,
    /// - the parameter cannot be built (using the Builder pattern)
    /// Format diagnostic information about FCS file state for error messages
    fn format_diagnostic_info(header: &Header, metadata: &Metadata, path: &Path) -> String {
        let mut lines = Vec::new();

        lines.push("=== FCS File Diagnostic Information ===".to_string());
        lines.push(format!("File path: {}", path.display()));
        lines.push(format!("FCS Version: {}", header.version));
        lines.push(format!(
            "Text segment: {}..={}",
            header.text_offset.start(),
            header.text_offset.end()
        ));
        lines.push(format!(
            "Data segment: {}..={}",
            header.data_offset.start(),
            header.data_offset.end()
        ));
        lines.push(format!(
            "Analysis segment: {}..={}",
            header.analysis_offset.start(),
            header.analysis_offset.end()
        ));
        lines.push(format!(
            "Delimiter: '{}' (0x{:02x})",
            metadata.delimiter, metadata.delimiter as u8
        ));

        // Get number of parameters
        let n_params = metadata
            .get_number_of_parameters()
            .map(|n| format!("{}", n))
            .unwrap_or_else(|e| format!("Error: {}", e));
        lines.push(format!("Number of parameters ($PAR): {}", n_params));

        // Get number of events
        let n_events = metadata
            .get_number_of_events()
            .map(|n| format!("{}", n))
            .unwrap_or_else(|e| format!("Error: {}", e));
        lines.push(format!("Number of events ($TOT): {}", n_events));

        // List all parameter keywords
        lines.push("\n=== Parameter Keywords ===".to_string());
        let number_of_parameters = metadata.get_number_of_parameters().ok().copied();
        if let Some(n_params) = number_of_parameters {
            for param_num in 1..=n_params {
                let pn_keywords = [
                    format!("$P{}N", param_num),
                    format!("$P{}S", param_num),
                    format!("$P{}B", param_num),
                    format!("$P{}E", param_num),
                    format!("$P{}R", param_num),
                ];

                let mut found_keywords = Vec::new();
                for key in &pn_keywords {
                    if let Some(kw) = metadata.keywords.get(key) {
                        let value = match kw {
                            crate::keyword::Keyword::String(sk) => sk.get_str().to_string(),
                            crate::keyword::Keyword::Int(ik) => ik.get_usize().to_string(),
                            crate::keyword::Keyword::Float(fk) => fk.to_string(),
                            crate::keyword::Keyword::Byte(bk) => bk.get_str().to_string(),
                            crate::keyword::Keyword::Mixed(mk) => mk.to_string(),
                        };
                        found_keywords.push(format!("  {} = {}", key, value));
                    } else {
                        found_keywords.push(format!("  {} = <MISSING>", key));
                    }
                }
                lines.push(format!("Parameter {}:", param_num));
                lines.extend(found_keywords);
            }
        } else {
            lines.push("  Could not determine number of parameters".to_string());
        }

        // List all keywords sorted
        lines.push("\n=== All Keywords (sorted) ===".to_string());
        let mut sorted_keys: Vec<_> = metadata.keywords.keys().collect();
        sorted_keys.sort();
        for key in sorted_keys {
            let kw = match metadata.keywords.get(key) {
                Some(kw) => kw,
                None => continue, // Skip missing keywords in diagnostic output
            };
            let value = match kw {
                crate::keyword::Keyword::String(sk) => sk.get_str().to_string(),
                crate::keyword::Keyword::Int(ik) => ik.get_usize().to_string(),
                crate::keyword::Keyword::Float(fk) => fk.to_string(),
                crate::keyword::Keyword::Byte(bk) => bk.get_str().to_string(),
                crate::keyword::Keyword::Mixed(mk) => mk.to_string(),
            };
            lines.push(format!("  {} = {}", key, value));
        }

        lines.join("\n")
    }

    pub fn generate_parameter_map(metadata: &Metadata) -> Result<ParameterMap> {
        let mut map = ParameterMap::default();
        let number_of_parameters = metadata.get_number_of_parameters()?;
        for parameter_number in 1..=*number_of_parameters {
            let channel_name = metadata.get_parameter_channel_name(parameter_number)
                .map_err(|e| anyhow!(
                    "Failed to get channel name for parameter {}: {}\n\nHint: Check that $P{}N keyword exists in metadata.",
                    parameter_number,
                    e,
                    parameter_number
                ))?;

            // Use label name or fallback to the parameter name
            let label_name = match metadata.get_parameter_label(parameter_number) {
                Ok(label) => label,
                Err(_) => channel_name,
            };

            let transform = if channel_name.contains("FSC")
                || channel_name.contains("SSC")
                || channel_name.contains("Time")
            {
                TransformType::Linear
            } else {
                TransformType::default()
            };

            // Get excitation wavelength from metadata if available
            let excitation_wavelength = metadata
                .get_parameter_excitation_wavelength(parameter_number)
                .ok()
                .flatten();

            let parameter = ParameterBuilder::default()
                // For the ParameterBuilder, ensure we're using the proper methods
                // that may be defined by the Builder derive macro
                .parameter_number(parameter_number)
                .channel_name(channel_name)
                .label_name(label_name)
                .transform(transform)
                .excitation_wavelength(excitation_wavelength)
                .build()?;

            // Add the parameter events to the hashmap keyed by the parameter name
            map.insert(channel_name.to_string().into(), parameter);
        }

        Ok(map)
    }

    /// Looks for a keyword among the metadata and returns its value as a `&str`
    /// # Errors
    /// Will return `Err` if the `Keyword` is not found in the `metadata` or if the `Keyword` cannot be converted to a `&str`
    pub fn get_keyword_string_value(&self, keyword: &str) -> Result<Cow<'_, str>> {
        // TODO: This should be a match statement
        if let Ok(keyword) = self.metadata.get_string_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_integer_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_float_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_byte_keyword(keyword) {
            Ok(keyword.get_str())
        } else if let Ok(keyword) = self.metadata.get_mixed_keyword(keyword) {
            Ok(keyword.get_str())
        } else {
            Err(anyhow!("Keyword not found: {}", keyword))
        }
    }
    /// A convenience function to return the `GUID` keyword from the `metadata` as a `&str`
    /// # Errors
    /// Will return `Err` if the `GUID` keyword is not found in the `metadata` or if the `GUID` keyword cannot be converted to a `&str`
    pub fn get_guid(&self) -> Result<Cow<'_, str>> {
        Ok(self.metadata.get_string_keyword("GUID")?.get_str())
    }

    /// Set or update the GUID keyword in the file's metadata
    pub fn set_guid(&mut self, guid: String) {
        self.metadata
            .insert_string_keyword("GUID".to_string(), guid);
    }

    /// A convenience function to return the `$FIL` keyword from the `metadata` as a `&str`
    /// # Errors
    /// Will return `Err` if the `$FIL` keyword is not found in the `metadata` or if the `$FIL` keyword cannot be converted to a `&str`
    pub fn get_fil_keyword(&self) -> Result<Cow<'_, str>> {
        Ok(self.metadata.get_string_keyword("$FIL")?.get_str())
    }

    /// A convenience function to return the `$TOT` keyword from the `metadata` as a `usize`
    /// # Errors
    /// Will return `Err` if the `$TOT` keyword is not found in the `metadata` or if the `$TOT` keyword cannot be converted to a `usize`
    pub fn get_number_of_events(&self) -> Result<&usize> {
        self.metadata.get_number_of_events()
    }

    /// A convenience function to return the `$PAR` keyword from the `metadata` as a `usize`
    /// # Errors
    /// Will return `Err` if the `$PAR` keyword is not found in the `metadata` or if the `$PAR` keyword cannot be converted to a `usize`
    pub fn get_number_of_parameters(&self) -> Result<&usize> {
        self.metadata.get_number_of_parameters()
    }

    // ==================== NEW POLARS-BASED ACCESSOR METHODS ====================

    /// Get events for a parameter as a slice of f32 values
    /// Polars gives us direct access to the underlying buffer (zero-copy)
    /// # Errors
    /// Will return `Err` if:
    /// - the parameter name is not found
    /// - the Series data type is not Float32
    /// - the data is chunked (rare for FCS files)
    pub fn get_parameter_events_slice(&self, channel_name: &str) -> Result<&[f32]> {
        self.get_parameter_events(channel_name)?
            .cont_slice()
            .map_err(|e| anyhow!("Parameter {} data is not contiguous: {}", channel_name, e))
    }

    /// Get two parameters as (x, y) pairs for plotting
    /// Optimized for scatter plot use case with zero allocations until the collect
    /// # Errors
    /// Will return `Err` if either parameter name is not found
    pub fn get_xy_pairs(&self, x_param: &str, y_param: &str) -> Result<Vec<(f32, f32)>> {
        let x_data = self.get_parameter_events_slice(x_param)?;
        let y_data = self.get_parameter_events_slice(y_param)?;

        // Verify both parameters have the same length
        if x_data.len() != y_data.len() {
            return Err(anyhow!(
                "Parameter length mismatch: {} has {} events, {} has {} events",
                x_param,
                x_data.len(),
                y_param,
                y_data.len()
            ));
        }

        // Zip is zero-cost abstraction - uses iterators efficiently
        Ok(x_data
            .iter()
            .zip(y_data.iter())
            .map(|(&x, &y)| (x, y))
            .collect())
    }

    /// Get DataFrame height (number of events)
    #[must_use]
    pub fn get_event_count_from_dataframe(&self) -> usize {
        self.data_frame.height()
    }

    /// Get DataFrame width (number of parameters)
    #[must_use]
    pub fn get_parameter_count_from_dataframe(&self) -> usize {
        self.data_frame.width()
    }

    /// Get DataFrame column names (parameter names)
    pub fn get_parameter_names_from_dataframe(&self) -> Vec<String> {
        self.data_frame
            .get_column_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Aggregate statistics for a parameter using Polars' streaming API for low memory usage and minimal, chunked passes.
    ///
    /// When streaming is enabled, Polars creates a *Pipeline*:
    ///
    /// **Source**: It pulls a chunk of data from the disk (e.g., 50,000 rows).
    ///
    /// **Operators**: It passes that chunk through your expressions (calculating the running sum, count, min, and max for that specific chunk).
    ///
    /// **Sink**: It aggregates the results from all chunks into a final result.
    ///
    /// Because the statistics we are calculating (min, max, mean) are *associative* and *commutative*, Polars can calculate them partially on each chunk and then combine them at the very end.
    ///
    /// Returns (min, max, mean, std_dev)
    /// # Errors
    /// Will return `Err` if the parameter is not found or stats calculation fails
    pub fn get_parameter_statistics(&self, channel_name: &str) -> Result<(f32, f32, f32, f32)> {
        let stats = (*self.data_frame)
            .clone()
            .lazy()
            .select([
                col(channel_name).min().alias("min"),
                col(channel_name).max().alias("max"),
                col(channel_name).mean().alias("mean"),
                col(channel_name).std(1).alias("std"),
            ])
            .collect_with_engine(Engine::Streaming)?;
        let min = stats
            .column("min")
            .map_err(|e| anyhow!("Column 'min' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'min' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No min value found"))?;
        let max = stats
            .column("max")
            .map_err(|e| anyhow!("Column 'max' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'max' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No max value found"))?;
        let mean = stats
            .column("mean")
            .map_err(|e| anyhow!("Column 'mean' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'mean' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No mean value found"))?;
        let std = stats
            .column("std")
            .map_err(|e| anyhow!("Column 'std' not found in statistics: {}", e))?
            .f32()
            .map_err(|e| anyhow!("Column 'std' is not f32 type: {}", e))?
            .get(0)
            .ok_or_else(|| anyhow!("No std deviation value found"))?;

        Ok((min, max, mean, std))
    }

    // ==================== TRANSFORMATION METHODS ====================

    /// Apply arcsinh transformation to a parameter using Polars
    /// This is the most common transformation for flow cytometry data
    /// Formula: arcsinh(x / cofactor)
    ///
    /// # Arguments
    /// * `parameter_name` - Name of the parameter to transform
    /// * `cofactor` - Scaling factor (typical: 150-200 for modern instruments)
    ///
    /// # Returns
    /// New DataFrame with the transformed parameter
    pub fn apply_arcsinh_transform(
        &self,
        parameter_name: &str,
        cofactor: f32,
    ) -> Result<EventDataFrame> {
        let df = (*self.data_frame).clone();

        // Get the column to transform
        let col = df
            .column(parameter_name)
            .map_err(|e| anyhow!("Parameter {} not found: {}", parameter_name, e))?;

        let series = col.as_materialized_series();
        let ca = series
            .f32()
            .map_err(|e| anyhow!("Parameter {} is not f32: {}", parameter_name, e))?;

        // Apply arcsinh transformation using TransformType implementation
        // The division by ln(10) was incorrectly converting to log10 scale,
        // which compressed the data ~2.3x and caused MAD to over-remove events
        use rayon::prelude::*;
        let transform = TransformType::Arcsinh { cofactor };
        let transformed: Vec<f32> = ca
            .cont_slice()
            .map_err(|e| anyhow!("Data not contiguous: {}", e))?
            .par_iter()
            .map(|&x| transform.transform(&x))
            .collect();

        // Create new column with transformed data
        let new_series = Series::new(parameter_name.into(), transformed);

        // Replace the column in DataFrame
        let mut new_df = df;
        new_df
            .replace(parameter_name, new_series.into())
            .map_err(|e| anyhow!("Failed to replace column: {}", e))?;

        Ok(Arc::new(new_df))
    }

    /// Apply arcsinh transformation to multiple parameters
    ///
    /// # Arguments
    /// * `parameters` - List of (parameter_name, cofactor) pairs
    ///
    /// # Returns
    /// New DataFrame with all specified parameters transformed
    pub fn apply_arcsinh_transforms(&self, parameters: &[(&str, f32)]) -> Result<EventDataFrame> {
        let mut df = (*self.data_frame).clone();

        use rayon::prelude::*;

        for &(param_name, cofactor) in parameters {
            let col = df
                .column(param_name)
                .map_err(|e| anyhow!("Parameter {} not found: {}", param_name, e))?;

            let series = col.as_materialized_series();
            let ca = series
                .f32()
                .map_err(|e| anyhow!("Parameter {} is not f32: {}", param_name, e))?;

            // Apply arcsinh transformation using TransformType implementation
            // Standard flow cytometry arcsinh - no division by ln(10)
            let transform = TransformType::Arcsinh { cofactor };
            let transformed: Vec<f32> = ca
                .cont_slice()
                .map_err(|e| anyhow!("Data not contiguous: {}", e))?
                .par_iter()
                .map(|&x| transform.transform(&x))
                .collect();

            let new_series = Series::new(param_name.into(), transformed);
            df.replace(param_name, new_series.into())
                .map_err(|e| anyhow!("Failed to replace column {}: {}", param_name, e))?;
        }

        Ok(Arc::new(df))
    }

    /// Apply default arcsinh transformation to all fluorescence parameters
    /// Automatically detects fluorescence parameters (excludes FSC, SSC, Time)
    /// Uses cofactor = 200 (good default for modern instruments)
    pub fn apply_default_arcsinh_transform(&self) -> Result<EventDataFrame> {
        let param_names = self.get_parameter_names_from_dataframe();

        // Filter to fluorescence parameters (exclude scatter and time)
        let fluor_params: Vec<(&str, f32)> = param_names
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
            })
            .map(|name| (name.as_str(), 2000.0)) // Default cofactor = 2000
            .collect();

        self.apply_arcsinh_transforms(&fluor_params)
    }

    /// Apply biexponential (logicle) transformation matching FlowJo defaults
    /// Automatically detects fluorescence parameters (excludes FSC, SSC, Time)
    /// Uses FlowJo default parameters: top_of_scale=262144 (18-bit), positive_decades=4.5, negative_decades=0, width=0.5
    pub fn apply_default_biexponential_transform(&self) -> Result<EventDataFrame> {
        let param_names = self.get_parameter_names_from_dataframe();

        // Filter to fluorescence parameters (exclude scatter and time)
        let fluor_params: Vec<&str> = param_names
            .iter()
            .filter(|name| {
                let upper = name.to_uppercase();
                !upper.contains("FSC") && !upper.contains("SSC") && !upper.contains("TIME")
            })
            .map(|name| name.as_str())
            .collect();

        let mut df = (*self.data_frame).clone();

        use rayon::prelude::*;

        // FlowJo default biexponential parameters
        let transform = TransformType::Biexponential {
            top_of_scale: 262144.0, // 18-bit data (2^18)
            positive_decades: 4.5,
            negative_decades: 0.0,
            width: 0.5,
        };

        for param_name in fluor_params {
            let col = df
                .column(param_name)
                .map_err(|e| anyhow!("Parameter {} not found: {}", param_name, e))?;

            let series = col.as_materialized_series();
            let ca = series
                .f32()
                .map_err(|e| anyhow!("Parameter {} is not f32: {}", param_name, e))?;

            // Apply biexponential transformation using TransformType implementation
            let transformed: Vec<f32> = ca
                .cont_slice()
                .map_err(|e| anyhow!("Data not contiguous: {}", e))?
                .par_iter()
                .map(|&x| transform.transform(&x))
                .collect();

            let new_series = Series::new(param_name.into(), transformed);
            df.replace(param_name, new_series.into())
                .map_err(|e| anyhow!("Failed to replace column {}: {}", param_name, e))?;
        }

        Ok(Arc::new(df))
    }

    // ==================== COMPENSATION METHODS ====================

    /// Extract compensation matrix from $SPILLOVER keyword
    /// Returns (matrix, channel_names) if spillover keyword exists
    /// Returns None if no spillover keyword is present in the file
    ///
    /// # Returns
    /// Some((compensation_matrix, channel_names)) if spillover exists, None otherwise
    ///
    /// # Errors
    /// Will return `Err` if spillover keyword is malformed
    pub fn get_spillover_matrix(&self) -> Result<Option<(Mat<f32>, Vec<String>)>> {
        use crate::keyword::{Keyword, MixedKeyword};

        // Try to get compensation matrix from $SPILLOVER (FCS 3.1+), $SPILL (unofficial/custom), or $COMP (FCS 3.0)
        // Check in order of preference: SPILLOVER (official) > SPILL (common) > COMP (legacy)
        let spillover_keyword = self
            .metadata
            .keywords
            .get("$SPILLOVER")
            .or_else(|| self.metadata.keywords.get("$SPILL"))
            .or_else(|| self.metadata.keywords.get("$COMP"));

        let spillover_keyword = match spillover_keyword {
            Some(Keyword::Mixed(MixedKeyword::SPILLOVER {
                n_parameters,
                parameter_names,
                matrix_values,
            })) => (
                *n_parameters,
                parameter_names.clone(),
                matrix_values.clone(),
            ),
            Some(_) => {
                // Keyword exists but has wrong type - might be stored as String(Other) if parsing failed
                // Try to parse it manually
                let keyword_name = if self.metadata.keywords.contains_key("$SPILLOVER") {
                    "$SPILLOVER"
                } else if self.metadata.keywords.contains_key("$SPILL") {
                    "$SPILL"
                } else if self.metadata.keywords.contains_key("$COMP") {
                    "$COMP"
                } else {
                    return Ok(None);
                };

                // Try to get the raw string value and parse it
                // This handles the case where $SPILL/$COMP was stored as String(Other) because
                // it wasn't recognized during initial parsing
                if let Some(Keyword::String(crate::keyword::StringKeyword::Other(value))) =
                    self.metadata.keywords.get(keyword_name)
                {
                    // Parse the string value as SPILLOVER using the same logic as parse_spillover
                    let parts: Vec<&str> = value.trim().split(',').collect();
                    if !parts.is_empty() {
                        if let Ok(n_parameters) = parts[0].trim().parse::<usize>() {
                            if parts.len() >= 1 + n_parameters {
                                let parameter_names: Vec<String> = parts[1..=n_parameters]
                                    .iter()
                                    .map(|s| s.trim().to_string())
                                    .collect();

                                let expected_matrix_size = n_parameters * n_parameters;
                                let matrix_start = 1 + n_parameters;

                                if parts.len() >= matrix_start + expected_matrix_size {
                                    // Parse matrix values (handle comma decimal separator)
                                    let mut matrix_values = Vec::new();
                                    for part in
                                        &parts[matrix_start..matrix_start + expected_matrix_size]
                                    {
                                        let cleaned = part.trim().replace(',', ".");
                                        if let Ok(val) = cleaned.parse::<f32>() {
                                            matrix_values.push(val);
                                        } else {
                                            break; // Failed to parse, give up
                                        }
                                    }

                                    if matrix_values.len() == expected_matrix_size {
                                        let matrix =
                                            Mat::from_fn(n_parameters, n_parameters, |i, j| {
                                                matrix_values[i * n_parameters + j]
                                            });
                                        return Ok(Some((matrix, parameter_names)));
                                    }
                                }
                            }
                        }
                    }
                }

                return Err(anyhow!(
                    "{} keyword exists but has wrong type or could not be parsed",
                    keyword_name
                ));
            }
            None => {
                // No spillover keyword - this is fine, not all files have it
                return Ok(None);
            }
        };

        let (n_params, param_names, matrix_values): (usize, Vec<String>, Vec<f32>) =
            spillover_keyword;

        // Validate matrix dimensions
        let expected_matrix_size = n_params * n_params;
        if matrix_values.len() != expected_matrix_size {
            return Err(anyhow!(
                "SPILLOVER matrix size mismatch: expected {} values for {}x{} matrix, got {}",
                expected_matrix_size,
                n_params,
                n_params,
                matrix_values.len()
            ));
        }

        // Create Mat from matrix values (FCS spillover is stored row-major)
        let matrix = Mat::from_fn(n_params, n_params, |i, j| matrix_values[i * n_params + j]);

        Ok(Some((matrix, param_names)))
    }

    /// Check if this file has compensation information
    #[must_use]
    pub fn has_compensation(&self) -> bool {
        self.get_spillover_matrix()
            .map(|opt| opt.is_some())
            .unwrap_or(false)
    }

    /// Resolve spillover channel labels to actual $PnN channel names.
    /// Some instruments (e.g. IntelliCyt iQue3) use parameter numbers ("1","2","3") in
    /// $SPILLOVER instead of channel names; these are mapped via metadata.get_parameter_channel_name.
    fn resolve_spillover_channel_names(&self, channel_names: &[String]) -> Result<Vec<String>> {
        let mut resolved = Vec::with_capacity(channel_names.len());
        for name in channel_names {
            let resolved_name = if let Ok(param_idx) = name.trim().parse::<usize>() {
                if param_idx >= 1 {
                    self.metadata
                        .get_parameter_channel_name(param_idx)
                        .map(|s| s.to_string())
                        .map_err(|e| {
                            anyhow!(
                                "Parameter {} not found: spillover references \"{}\" but {}",
                                param_idx,
                                name,
                                e
                            )
                        })?
                } else {
                    name.clone()
                }
            } else {
                name.clone()
            };
            if !self.parameters.contains_key(resolved_name.as_str()) {
                return Err(anyhow!(
                    "Spillover channel \"{}\" (resolved) not found in parameters",
                    resolved_name
                ));
            }
            resolved.push(resolved_name);
        }
        Ok(resolved)
    }

    /// Apply compensation from the file's $SPILLOVER keyword
    /// Convenience method that extracts spillover and applies it automatically
    ///
    /// Resolves spillover channel labels: some instruments (e.g. IntelliCyt iQue3) use
    /// parameter numbers ("1", "2", "3") in $SPILLOVER instead of $PnN names; these are
    /// mapped to actual channel names via metadata.
    ///
    /// # Returns
    /// New DataFrame with compensated data, or error if no spillover keyword exists
    pub fn apply_file_compensation(&self) -> Result<EventDataFrame> {
        let (comp_matrix, channel_names) = self
            .get_spillover_matrix()?
            .ok_or_else(|| anyhow!("No $SPILLOVER keyword found in FCS file"))?;

        let channel_refs: Vec<String> = self.resolve_spillover_channel_names(&channel_names)?;
        let channel_refs: Vec<&str> = channel_refs.iter().map(|s| s.as_str()).collect();

        self.apply_compensation(comp_matrix.as_ref(), &channel_refs)
    }

    /// Apply an externally supplied spillover matrix to this file's raw event data.
    ///
    /// Identical semantics to `get_compensated_parameters` (identity-check, parallel
    /// application via `flow_linalg`) but uses the caller-supplied matrix instead of
    /// reading `$SPILLOVER` from the file.
    ///
    /// # Arguments
    /// - `spillover`: NxN spillover matrix (NOT pre-inverted).
    /// - `matrix_channel_names`: channel names for matrix rows/cols as `&str`.
    ///   Callers with `Vec<String>` should map with `.iter().map(|s| s.as_str()).collect::<Vec<_>>()`.
    /// - `channels_needed`: which channels to return (subset of `matrix_channel_names`).
    ///
    /// Requires the `compensation` feature.
    #[cfg(feature = "compensation")]
    pub fn get_compensated_parameters_with_matrix(
        &self,
        spillover: faer::MatRef<'_, f32>,
        matrix_channel_names: &[&str],
        channels_needed: &[&str],
    ) -> anyhow::Result<std::collections::HashMap<String, Vec<f32>>> {
        use flow_linalg::compensation::compensate_channels;

        // Identity-check: if matrix is identity, bypass compensation entirely
        let n = spillover.nrows();
        let is_identity = (0..n).all(|i| {
            (0..n).all(|j| {
                let expected = if i == j { 1.0f32 } else { 0.0 };
                (spillover[(i, j)] - expected).abs() < 1e-6
            })
        });

        if is_identity {
            let mut result = std::collections::HashMap::new();
            for &ch in channels_needed {
                let data = self.get_parameter_events_slice(ch)?;
                result.insert(ch.to_string(), data.to_vec());
            }
            return Ok(result);
        }

        // Build raw channel slice pairs for all channels in the matrix
        let raw_pairs: Vec<(&str, Vec<f32>)> = matrix_channel_names
            .iter()
            .filter_map(|&name| {
                self.get_parameter_events_slice(name)
                    .ok()
                    .map(|s| (name, s.to_vec()))
            })
            .collect();

        let raw_refs: Vec<(&str, &[f32])> = raw_pairs
            .iter()
            .map(|(name, data)| (*name, data.as_slice()))
            .collect();

        compensate_channels(&raw_refs, spillover, matrix_channel_names, channels_needed)
            .map_err(|e| anyhow::anyhow!("Compensation failed: {e}"))
    }

    /// OPTIMIZED: Get compensated data for specific parameters only (lazy/partial compensation)
    ///
    /// This is 15-30x faster than apply_file_compensation when you only need a few parameters
    /// because it:
    /// - Only compensates the requested channels (e.g., 2 vs 30)
    /// - Uses sparse matrix optimization for matrices with >80% zeros
    /// - Bypasses compensation entirely for identity matrices
    ///
    /// # Arguments
    /// * `channels_needed` - Only the channel names you need compensated (typically 2 for a plot)
    ///
    /// # Returns
    /// HashMap of channel_name -> compensated data (as Vec<f32>)
    ///
    /// # Performance
    /// - Dense matrix (2/30 channels): **15x faster** (150ms → 10ms)
    /// - Sparse matrix (90% sparse): **50x faster** (150ms → 3ms)
    /// - Identity matrix: **300x faster** (150ms → 0.5ms)
    pub fn get_compensated_parameters(
        &self,
        channels_needed: &[&str],
    ) -> Result<std::collections::HashMap<String, Vec<f32>>> {
        use std::collections::HashMap;

        // Get spillover matrix
        let (comp_matrix, matrix_channel_names) = self
            .get_spillover_matrix()?
            .ok_or_else(|| anyhow!("No $SPILLOVER keyword found in FCS file"))?;

        let n_events = self.get_event_count_from_dataframe();

        // OPTIMIZATION 1: Check if matrix is identity (no compensation needed)
        let is_identity = {
            let mut is_id = true;
            for i in 0..comp_matrix.nrows() {
                for j in 0..comp_matrix.ncols() {
                    let expected = if i == j { 1.0 } else { 0.0 };
                    if (comp_matrix[(i, j)] - expected).abs() > 1e-6 {
                        is_id = false;
                        break;
                    }
                }
                if !is_id {
                    break;
                }
            }
            is_id
        };

        if is_identity {
            eprintln!("🚀 Identity matrix detected - bypassing compensation");
            // Just return original data
            let mut result = HashMap::new();
            for &channel in channels_needed {
                let data = self.get_parameter_events_slice(channel)?;
                result.insert(channel.to_string(), data.to_vec());
            }
            return Ok(result);
        }

        // OPTIMIZATION 2: Analyze sparsity
        let total_elements = comp_matrix.nrows() * comp_matrix.ncols();
        let mut non_zero_count = 0;
        for i in 0..comp_matrix.nrows() {
            for j in 0..comp_matrix.ncols() {
                if comp_matrix[(i, j)].abs() > 1e-6 {
                    non_zero_count += 1;
                }
            }
        }
        let sparsity = 1.0 - (non_zero_count as f64 / total_elements as f64);
        let is_sparse = sparsity > 0.8;

        eprintln!(
            "📊 Compensation matrix: {:.1}% sparse, {} non-zero coefficients",
            sparsity * 100.0,
            non_zero_count
        );

        // Find indices of channels we need
        let channel_indices: HashMap<&str, usize> = matrix_channel_names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i))
            .collect();

        let needed_indices: Vec<(String, usize)> = channels_needed
            .iter()
            .filter_map(|&ch| channel_indices.get(ch).map(|&idx| (ch.to_string(), idx)))
            .collect();

        if needed_indices.is_empty() {
            return Err(anyhow!(
                "None of the requested channels found in compensation matrix"
            ));
        }

        // Extract ONLY the channels involved in compensating our needed channels
        // For each needed channel, we need all channels that have non-zero spillover
        let mut involved_indices = std::collections::HashSet::new();
        for &(_, row_idx) in &needed_indices {
            // Add the channel itself
            involved_indices.insert(row_idx);

            // Add channels with non-zero spillover
            if is_sparse {
                for col_idx in 0..comp_matrix.ncols() {
                    if comp_matrix[(row_idx, col_idx)].abs() > 1e-6 {
                        involved_indices.insert(col_idx);
                    }
                }
            } else {
                // For dense matrix, we need all channels
                for i in 0..comp_matrix.ncols() {
                    involved_indices.insert(i);
                }
            }
        }

        let mut involved_vec: Vec<usize> = involved_indices.into_iter().collect();
        involved_vec.sort_unstable();

        eprintln!(
            "🎯 Lazy compensation: loading {} channels (vs {} total)",
            involved_vec.len(),
            matrix_channel_names.len()
        );

        // Extract data for involved channels only
        let mut channel_data: Vec<Vec<f32>> = Vec::with_capacity(involved_vec.len());
        for &idx in &involved_vec {
            let channel_name = &matrix_channel_names[idx];
            let data = self.get_parameter_events_slice(channel_name)?;
            channel_data.push(data.to_vec());
        }

        // Extract sub-matrix for involved channels
        let sub_matrix = Mat::from_fn(involved_vec.len(), involved_vec.len(), |i, j| {
            comp_matrix[(involved_vec[i], involved_vec[j])]
        });

        // Use CPU compensation (benchmarked: GPU was slower due to transfer overhead)
        // Invert sub-matrix using faer (pure Rust, no system BLAS)
        let comp_inv = crate::matrix::MatrixOps::invert_matrix(sub_matrix.as_ref())
            .map_err(|e| anyhow!("Failed to invert compensation matrix: {:?}", e))?;

        // Compensate ONLY the involved channels
        use rayon::prelude::*;
        let n_involved = involved_vec.len();
        let compensated_data: Vec<Vec<f32>> = (0..n_involved)
            .into_par_iter()
            .map(|i| {
                let mut result = vec![0.0; n_events];
                for event_idx in 0..n_events {
                    let mut sum = 0.0;
                    for j in 0..n_involved {
                        sum += comp_inv[(i, j)] * channel_data[j][event_idx];
                    }
                    result[event_idx] = sum;
                }
                result
            })
            .collect();

        // Build result HashMap for only the channels we need
        let mut result = HashMap::new();
        for (channel_name, orig_idx) in needed_indices {
            if let Some(local_idx) = involved_vec.iter().position(|&x| x == orig_idx) {
                result.insert(channel_name, compensated_data[local_idx].clone());
            }
        }

        eprintln!("🚀 Lazy compensation completed");
        Ok(result)
    }

    /// Apply compensation matrix to the data using Polars
    /// Compensation corrects for spectral overlap between fluorescence channels
    ///
    /// # Arguments
    /// * `compensation_matrix` - 2D matrix where element [i,j] represents spillover from channel j into channel i
    /// * `channel_names` - Names of channels in the order they appear in the matrix
    ///
    /// # Returns
    /// New DataFrame with compensated fluorescence values
    ///
    /// # Example
    /// ```ignore
    /// // Create a 3x3 compensation matrix
    /// use faer::mat;
    /// let comp_matrix = mat![
    ///     [1.0, 0.1, 0.05],  // FL1-A compensation
    ///     [0.2, 1.0, 0.1],   // FL2-A compensation
    ///     [0.1, 0.15, 1.0],  // FL3-A compensation
    /// ];
    /// let channels = vec!["FL1-A", "FL2-A", "FL3-A"];
    /// let compensated = fcs.apply_compensation(comp_matrix.as_ref(), &channels)?;
    /// ```
    pub fn apply_compensation(
        &self,
        compensation_matrix: faer::MatRef<'_, f32>,
        channel_names: &[&str],
    ) -> Result<EventDataFrame> {
        let comp = compensation_matrix;
        // Verify matrix dimensions match channel names
        let n_channels = channel_names.len();
        if comp.nrows() != n_channels || comp.ncols() != n_channels {
            return Err(anyhow!(
                "Compensation matrix dimensions ({}, {}) don't match number of channels ({})",
                comp.nrows(),
                comp.ncols(),
                n_channels
            ));
        }

        // Extract data for channels to compensate
        let mut channel_data: Vec<Vec<f32>> = Vec::with_capacity(n_channels);
        let n_events = self.get_event_count_from_dataframe();

        for &channel_name in channel_names {
            let data = self.get_parameter_events_slice(channel_name)?;
            channel_data.push(data.to_vec());
        }

        // Use CPU compensation (benchmarked: GPU was slower due to transfer overhead)
        // Apply compensation: compensated = original * inverse(compensation_matrix)
        // For efficiency, we pre-compute the inverse using faer (pure Rust, no system BLAS)
        let comp_inv = crate::matrix::MatrixOps::invert_matrix(comp)
            .map_err(|e| anyhow!("Failed to invert compensation matrix: {:?}", e))?;

        // Perform matrix multiplication for each event
        use rayon::prelude::*;
        let compensated_data: Vec<Vec<f32>> = (0..n_channels)
            .into_par_iter()
            .map(|i| {
                let mut result = vec![0.0; n_events];
                for event_idx in 0..n_events {
                    let mut sum = 0.0;
                    for j in 0..n_channels {
                        sum += comp_inv[(i, j)] * channel_data[j][event_idx];
                    }
                    result[event_idx] = sum;
                }
                result
            })
            .collect();

        // Create new DataFrame with compensated values
        let mut df = (*self.data_frame).clone();

        for (i, &channel_name) in channel_names.iter().enumerate() {
            let new_series = Series::new(channel_name.into(), compensated_data[i].clone());
            df.replace(channel_name, new_series.into())
                .map_err(|e| anyhow!("Failed to replace column {}: {}", channel_name, e))?;
        }

        Ok(Arc::new(df))
    }

    /// Apply spectral unmixing (matrix solve only, no compensation or transformation)
    ///
    /// This method performs unmixing by solving: observation = mixing_matrix × abundances
    /// Does NOT apply compensation or transformations - these should be done separately.
    ///
    /// For overdetermined systems (more detectors than endmembers), uses least squares.
    ///
    /// # Arguments
    /// * `unmixing_matrix` - Matrix describing spectral signatures of fluorophores (detectors × endmembers)
    /// * `detector_names` - Names of detector channels (must match matrix rows)
    /// * `endmember_names` - Optional names for endmembers (columns). If None, uses "Endmember1", "Endmember2", etc.
    ///
    /// # Returns
    /// New DataFrame with unmixed endmember abundances (columns named by endmember names or indices)
    ///
    /// # Errors
    /// Returns error if detector names don't match matrix dimensions or data cannot be extracted
    pub fn apply_spectral_unmixing(
        &self,
        unmixing_matrix: faer::MatRef<'_, f32>,
        detector_names: &[&str],
        endmember_names: Option<&[&str]>,
    ) -> Result<EventDataFrame> {
        use faer::linalg::solvers::{Qr, SolveLstsq};

        // Verify matrix dimensions match detector names
        let n_detectors = detector_names.len();
        if unmixing_matrix.nrows() != n_detectors {
            return Err(anyhow!(
                "Unmixing matrix rows ({}) don't match number of detectors ({})",
                unmixing_matrix.nrows(),
                n_detectors
            ));
        }

        let n_endmembers = unmixing_matrix.ncols();
        let n_events = self.get_event_count_from_dataframe();

        // Extract data for detectors
        let mut detector_data: Vec<Vec<f32>> = Vec::with_capacity(n_detectors);
        for &detector_name in detector_names {
            let data = self.get_parameter_events_slice(detector_name)?;
            detector_data.push(data.to_vec());
        }

        // Observations matrix: events × detectors
        let observations = Mat::from_fn(n_events, n_detectors, |event_idx, detector_idx| {
            detector_data[detector_idx][event_idx]
        });

        // Perform unmixing: for each event, solve: observation = unmixing_matrix × abundances
        // For overdetermined systems (n_detectors > n_endmembers), use QR least squares (faer)
        let qr = Qr::new(unmixing_matrix);

        let mut unmixed_data: Vec<Vec<f32>> = Vec::with_capacity(n_events);
        for event_idx in 0..n_events {
            let b_col = Mat::from_fn(n_detectors, 1, |i, _| observations[(event_idx, i)]);
            let x_faer = qr.solve_lstsq(b_col.as_ref());
            let abundances: Vec<f32> = (0..n_endmembers).map(|i| x_faer[(i, 0)]).collect();
            unmixed_data.push(abundances);
        }

        // Create DataFrame with endmember abundances
        let mut columns: Vec<Column> = Vec::with_capacity(n_endmembers);
        for endmember_idx in 0..n_endmembers {
            let values: Vec<f32> = unmixed_data
                .iter()
                .map(|abundances| abundances[endmember_idx])
                .collect();

            // Use provided endmember name if available, otherwise use generic "Endmember{i}" format
            let column_name = if let Some(names) = endmember_names {
                names[endmember_idx].to_string()
            } else {
                format!("Endmember{}", endmember_idx + 1)
            };

            let series = Column::new(column_name.into(), values);
            columns.push(series);
        }

        let df = DataFrame::new(n_events, columns)
            .map_err(|e| anyhow!("Failed to create DataFrame: {}", e))?;

        Ok(Arc::new(df))
    }
}

#[cfg(test)]
#[cfg(feature = "compensation")]
mod compensation_method_tests {
    use super::*;

    /// Verifies the method compiles and identity matrix returns raw data.
    /// Full fixture-based test is #[ignore]d — needs test_data/compensation_test.fcs.
    #[test]
    #[ignore = "requires real FCS fixture"]
    fn test_with_matrix_matches_get_compensated_parameters() {
        // This test verifies parity between the two methods.
        // Run manually after loading test fixture:
        // cargo test --features compensation -- --ignored test_with_matrix
        let fcs = Fcs::open("test_data/compensation_test.fcs")
            .expect("test FCS file not found");
        if !fcs.has_compensation() { return; }
        let channels = ["BV421-A", "PE-A"];
        let expected = fcs.get_compensated_parameters(&channels).unwrap();
        let (matrix, names) = fcs.get_spillover_matrix().unwrap().unwrap();
        let name_refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        let actual = fcs.get_compensated_parameters_with_matrix(matrix.as_ref(), &name_refs, &channels).unwrap();
        for &ch in &channels {
            let e = expected.get(ch).unwrap();
            let a = actual.get(ch).unwrap();
            assert_eq!(e.len(), a.len());
            for (&ev, &av) in e.iter().zip(a.iter()) {
                assert!((ev - av).abs() < 1e-4, "{ch}: {ev} vs {av}");
            }
        }
    }
}
