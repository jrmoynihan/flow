"""Type stubs for the peacoqc native module."""

from __future__ import annotations

import polars as pl

class QCResult:
    """Result of the PeacoQC quality control algorithm."""

    @property
    def good_cells(self) -> list[bool]:
        """Boolean list: True = keep, False = remove."""
        ...
    @property
    def percentage_removed(self) -> float:
        """Percentage of events removed."""
        ...
    @property
    def it_percentage(self) -> float | None:
        """Isolation Tree percentage (if used)."""
        ...
    @property
    def mad_percentage(self) -> float | None:
        """MAD percentage (if used)."""
        ...
    @property
    def consecutive_percentage(self) -> float:
        """Consecutive filtering percentage."""
        ...
    @property
    def n_bins(self) -> int:
        """Number of bins used."""
        ...
    @property
    def events_per_bin(self) -> int:
        """Events per bin."""
        ...

class MarginResult:
    """Result of margin removal."""

    @property
    def mask(self) -> list[bool]:
        """Boolean mask: True = keep, False = margin event."""
        ...
    @property
    def percentage_removed(self) -> float:
        """Total percentage of events removed."""
        ...
    @property
    def margin_matrix(self) -> dict[str, tuple[int, int]]:
        """Per-channel removal counts: {channel: (min_removed, max_removed)}."""
        ...

class DoubletResult:
    """Result of doublet removal."""

    @property
    def mask(self) -> list[bool]:
        """Boolean mask: True = keep, False = doublet."""
        ...
    @property
    def percentage_removed(self) -> float:
        """Percentage of events removed."""
        ...
    @property
    def median_ratio(self) -> float:
        """Median ratio used."""
        ...
    @property
    def mad_ratio(self) -> float:
        """MAD of ratios."""
        ...
    @property
    def threshold(self) -> float:
        """Threshold used for doublet detection."""
        ...

class FcsFile:
    """Wrapper for an FCS file loaded from disk."""

    @staticmethod
    def open(path: str) -> FcsFile:
        """Open an FCS file from disk."""
        ...
    @property
    def n_events(self) -> int:
        """Number of events in the file."""
        ...
    @property
    def channel_names(self) -> list[str]:
        """Channel names."""
        ...
    @property
    def fluorescence_channels(self) -> list[str]:
        """Fluorescence channel names (auto-detected, excluding FSC/SSC/Time)."""
        ...
    @property
    def dataframe(self) -> pl.DataFrame:
        """Return the event data as a polars DataFrame."""
        ...
    def has_compensation(self) -> bool:
        """Whether compensation info ($SPILLOVER) exists."""
        ...

def run_qc(
    df: pl.DataFrame,
    channels: list[str],
    channel_ranges: dict[str, tuple[float, float]],
    mode: str = "all",
    mad: float = 6.0,
    it_limit: float = 0.6,
    consecutive_bins: int = 5,
    min_cells: int = 150,
    max_bins: int = 500,
    events_per_bin: int | None = None,
    remove_zeros: bool = False,
    peak_removal: float | None = None,
) -> QCResult:
    """Run the PeacoQC quality control algorithm on a polars DataFrame."""
    ...

def remove_margins(
    df: pl.DataFrame,
    channels: list[str],
    channel_ranges: dict[str, tuple[float, float]],
    remove_min: list[str] | None = None,
    remove_max: list[str] | None = None,
) -> MarginResult:
    """Remove margin events from flow cytometry data."""
    ...

def remove_doublets(
    df: pl.DataFrame,
    channel_ranges: dict[str, tuple[float, float]],
    channel1: str = "FSC-A",
    channel2: str = "FSC-H",
    nmad: float = 4.0,
    b: float = 0.0,
) -> DoubletResult:
    """Remove doublet events based on area/height scatter ratio."""
    ...

def read_fcs(path: str) -> pl.DataFrame:
    """Open an FCS file and return its data as a polars DataFrame."""
    ...

def run_qc_on_fcs(
    path: str,
    channels: list[str] | None = None,
    apply_compensation: bool = True,
    apply_transformation: bool = True,
    mode: str = "all",
    mad: float = 6.0,
    it_limit: float = 0.6,
    consecutive_bins: int = 5,
) -> tuple[QCResult, pl.DataFrame]:
    """Open an FCS file, preprocess, and run PeacoQC in one step."""
    ...

def open_fcs(path: str) -> FcsFile:
    """Open an FCS file as an FcsFile object."""
    ...

def run_qc_on_fcs_obj(
    fcs: FcsFile,
    channels: list[str] | None = None,
    mode: str = "all",
    mad: float = 6.0,
    it_limit: float = 0.6,
    consecutive_bins: int = 5,
) -> QCResult:
    """Run PeacoQC directly on an FcsFile object."""
    ...

def preprocess(
    fcs: FcsFile,
    apply_compensation: bool = True,
    apply_transformation: bool = True,
) -> FcsFile:
    """Preprocess an FCS file (compensation + transformation)."""
    ...

def filter_fcs(fcs: FcsFile, mask: list[bool]) -> FcsFile:
    """Filter an FcsFile by a boolean mask, returning a new FcsFile."""
    ...
