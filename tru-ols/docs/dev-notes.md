# Development Notes for TRU-OLS

## Future Enhancements

### Alternative Methods for Choosing Cutoffs and Relevant Endmembers

The current implementation uses a fixed percentile-based cutoff approach for determining relevant endmembers. Future work may benefit from:

1. **Adaptive Cutoff Selection**
   - Implement methods that automatically determine optimal cutoff values based on data characteristics
   - Consider statistical methods (e.g., elbow method, gap statistic)
   - Explore machine learning approaches for cutoff selection

2. **Alternative Endmember Selection Strategies**:
   - Implement correlation-based selection
   - Add support for user-defined endmember subsets
   - Consider hierarchical clustering approaches for endmember grouping
   - Explore methods that account for spectral overlap and crosstalk

3. **Dynamic Str ategy Selection**:
   - Automatically choose between "zero" and "ucm" strategies based on data characteristics
   - Implement hybrid approaches that combine multiple strategies
   - Add validation metrics to guide strategy selection

4. **Configuration Options**:
   - Allow users to specify custom cutoff methods via configuration files
   - Support for multiple cutoff methods in a single run for comparison
   - Export cutoff selection diagnostics for analysis

These enhancements would improve the flexibility and robustness of the TRU-OLS unmixing algorithm, making it more adaptable to different experimental conditions and data characteristics.

## Mixing Matrix Sources

TRU-OLS supports three methods for obtaining the mixing matrix (spectral signature matrix):

### 1. CSV File (`--mixing-matrix`)

A manually prepared CSV file containing the mixing matrix where:

- Rows = detector channels
- Columns = fluorophores/endmembers
- Values = contribution of each fluorophore to each detector

### 2. SPILL Keyword (`--use-spill`)

For spectral cytometry instruments, the SPILL/SPILLOVER keyword in the FCS file contains the mixing matrix directly. This is the spectral signature matrix that describes how each fluorophore's emission is distributed across all detectors.

**How it works:**

- The SPILL matrix is extracted from the `$SPILLOVER`, `$SPILL`, or `$COMP` keyword
- For spectral cytometry, this matrix IS the mixing matrix (not a compensation matrix)
- The matrix format is: detectors × fluorophores
- Each column represents the spectral signature (reference spectrum) of one fluorophore

### 3. Single-Stain Controls (`--single-stain-controls`)

Creates the mixing matrix by analyzing individual single-stain control files, where each file contains cells stained with only one fluorophore.

**How Single-Stain Controls Determine Reference Spectra:**

The algorithm processes each single-stain control file to extract the spectral signature (reference spectrum) for that fluorophore:

1. **Load Control File**: Each control file contains events stained with a single fluorophore (e.g., "AF488_control.fcs" contains only AF488-stained cells)

2. **Extract Median Fluorescence**: For each detector channel, calculate the median fluorescence intensity across all events in the control file
   - This represents the "typical" signal level for that fluorophore on that detector

3. **Subtract Autofluorescence**: Subtract the autofluorescence baseline (obtained from the unstained control) from each detector's median
   - `corrected_median = control_median - autofluorescence_median`
   - This removes background fluorescence that would otherwise contaminate the spectral signature

4. **Identify Primary Detector**: Find the detector with the highest corrected median signal for this fluorophore
   - This is the detector where the fluorophore emits most strongly
   - Example: AF488's primary detector might be "FL1-A"

5. **Normalize to Primary Detector**: Divide all corrected medians by the primary detector's corrected median
   - `normalized_signal[detector] = corrected_median[detector] / corrected_median[primary]`
   - This creates a normalized spectral signature where the primary detector = 1.0
   - Other detectors show relative spillover/contribution as fractions (e.g., 0.15 means 15% spillover)

6. **Create Mixing Matrix Column**: The normalized values become one column of the mixing matrix
   - Each column = one fluorophore's spectral signature
   - Each row = one detector channel
   - The matrix describes: "When fluorophore X is present, what fraction of its signal appears on each detector?"

**Example:**
For AF488 stained control:
- FL1-A (primary): 1000 → corrected: 990 → normalized: 1.0
- FL2-A: 150 → corrected: 140 → normalized: 0.14 (14% spillover)
- FL3-A: 20 → corrected: 10 → normalized: 0.01 (1% spillover)

This creates the mixing matrix column: `[1.0, 0.14, 0.01]` for AF488.

**Why This Works:**

- Single-stain controls provide "pure" spectral signatures because only one fluorophore is present
- Normalization ensures the mixing matrix is scale-invariant (works regardless of absolute signal levels)
- Autofluorescence subtraction ensures the signatures represent only the fluorophore's contribution
- The resulting matrix directly describes how each fluorophore's emission is distributed across detectors

**File Matching:**
Control files are matched to endmember names by:

- Filename matching (case-insensitive substring match)
- Example: "AF488_control.fcs" matches endmember "AF488"
- Files should be named to include the fluorophore name

This approach is standard in flow cytometry for creating compensation/spillover matrices and works equally well for spectral unmixing matrices.
