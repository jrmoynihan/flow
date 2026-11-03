//! Bounded row-major FCS input adapters.

use std::collections::HashSet;
use std::ops::Range;

use anyhow::{Result, anyhow};
use flow_linalg::{MatrixShape, RowSource};
use thiserror::Error;

use crate::Fcs;

/// Backend failure while reading an FCS row block.
#[derive(Debug, Error)]
#[error("FCS row read failed: {0}")]
pub struct FcsRowReadError(#[from] anyhow::Error);

/// A selected-channel [`RowSource`] over an open FCS file.
#[derive(Debug)]
pub struct FcsRowSource<'a> {
    fcs: &'a Fcs,
    channels: Vec<usize>,
    shape: MatrixShape,
}

impl<'a> FcsRowSource<'a> {
    /// Validate a channel selection and create a fixed-shape source.
    pub fn new(fcs: &'a Fcs, channels: &[usize]) -> Result<Self> {
        let rows = *fcs.get_number_of_events()?;
        let columns = *fcs.get_number_of_parameters()?;
        if channels.is_empty() {
            return Err(anyhow!("at least one channel must be selected"));
        }
        let mut seen = HashSet::with_capacity(channels.len());
        for &channel in channels {
            if channel >= columns {
                return Err(anyhow!(
                    "channel index {channel} is out of range for {columns} channels"
                ));
            }
            if !seen.insert(channel) {
                return Err(anyhow!(
                    "channel index {channel} was selected more than once"
                ));
            }
        }
        let shape = MatrixShape::new(rows, channels.len())
            .map_err(|error| anyhow!("invalid FCS row-source shape: {error}"))?;
        Ok(Self {
            fcs,
            channels: channels.to_vec(),
            shape,
        })
    }

    /// Selected zero-based channel indices, in output order.
    pub fn channels(&self) -> &[usize] {
        &self.channels
    }
}

impl RowSource<f32> for FcsRowSource<'_> {
    type Error = FcsRowReadError;

    fn shape(&self) -> MatrixShape {
        self.shape
    }

    fn read_rows(
        &mut self,
        rows: Range<usize>,
        storage: &mut Vec<f32>,
    ) -> std::result::Result<(), FcsRowReadError> {
        self.fcs
            .read_selected_rows_into(&self.channels, rows, storage)
            .map_err(FcsRowReadError::from)
    }
}
