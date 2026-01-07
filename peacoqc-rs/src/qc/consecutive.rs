use crate::error::Result;

/// Configuration for consecutive bin filtering
#[derive(Debug, Clone)]
pub struct ConsecutiveConfig {
    /// Minimum number of consecutive "good" bins to keep
    pub consecutive_bins: usize,
}

impl Default for ConsecutiveConfig {
    fn default() -> Self {
        Self {
            consecutive_bins: 5,
        }
    }
}

/// Remove isolated "good" bins surrounded by "bad" bins
///
/// If fewer than `consecutive_bins` good bins are located between bad bins,
/// they are marked as bad.
///
/// # Algorithm
/// 1. Find runs of consecutive good/bad bins
/// 2. Mark runs of good bins shorter than threshold as bad
///
/// # Arguments
/// * `outlier_bins` - Input outlier mask (true = outlier/bad, false = good)
/// * `config` - Configuration with consecutive bins threshold
///
/// # Returns
/// * Updated outlier mask
pub fn remove_short_regions(
    outlier_bins: &[bool],
    config: &ConsecutiveConfig,
) -> Result<Vec<bool>> {
    let mut result = outlier_bins.to_vec();
    
    if outlier_bins.is_empty() {
        return Ok(result);
    }
    
    // Find runs of good bins (false values)
    let mut i = 0;
    while i < result.len() {
        if !result[i] {
            // Start of a good run
            let start = i;
            while i < result.len() && !result[i] {
                i += 1;
            }
            let end = i;
            let run_length = end - start;
            
            // If run is too short and not at the edges, mark as bad
            if run_length < config.consecutive_bins && start > 0 && end < result.len() {
                for j in start..end {
                    result[j] = true;
                }
            }
        } else {
            i += 1;
        }
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_remove_short_regions() {
        // Pattern: BBGGGBBBGGBBBBGGGGGB (B=bad, G=good)
        //          TT...TTT..TTTT.....T
        let outlier_bins = vec![
            true, true,           // Bad start
            false, false, false,  // Good run of 3 (should be removed)
            true, true, true,     // Bad run
            false, false,         // Good run of 2 (should be removed)
            true, true, true, true, // Bad run
            false, false, false, false, false, // Good run of 5 (should be kept)
            true,                 // Bad end
        ];
        
        let config = ConsecutiveConfig {
            consecutive_bins: 5,
        };
        
        let result = remove_short_regions(&outlier_bins, &config).unwrap();
        
        // First 3 good bins should now be bad
        assert!(result[2]);
        assert!(result[3]);
        assert!(result[4]);
        
        // Next 2 good bins should now be bad
        assert!(result[8]);
        assert!(result[9]);
        
        // Last 5 good bins should remain good
        assert!(!result[14]);
        assert!(!result[15]);
        assert!(!result[16]);
        assert!(!result[17]);
        assert!(!result[18]);
    }
}
