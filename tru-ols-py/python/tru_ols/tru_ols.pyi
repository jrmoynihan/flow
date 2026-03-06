"""Type stubs for the tru_ols native module."""

from __future__ import annotations

import polars as pl

class TruOls:
    """TRU-OLS spectral unmixing algorithm."""

    def __init__(
        self,
        mixing_matrix: list[list[float]],
        unstained_control: list[list[float]],
        autofluorescence_idx: int,
        cutoff_percentile: float = 0.995,
        strategy: str = "zero",
    ) -> None: ...
    def set_cutoff_percentile(self, percentile: float) -> None:
        """Recalculate cutoffs from unstained control at the given percentile."""
        ...
    def set_strategy(self, strategy: str) -> None:
        """Set unmixing strategy: 'zero' or 'ucm'."""
        ...
    def unmix(self, dataset: list[list[float]]) -> list[list[float]]:
        """Unmix an entire dataset (events x detectors) -> (events x endmembers)."""
        ...
    def unmix_event(
        self, observation: list[float]
    ) -> tuple[list[float], list[int], list[tuple[int, float]]]:
        """Unmix a single event. Returns (abundances, relevant_indices, irrelevant_pairs)."""
        ...
    @property
    def n_detectors(self) -> int:
        """Number of detectors (mixing matrix rows)."""
        ...
    @property
    def n_endmembers(self) -> int:
        """Number of endmembers (mixing matrix columns)."""
        ...
    @property
    def mixing_matrix_data(self) -> list[list[float]]:
        """The mixing matrix as nested list."""
        ...

class UnmixingResult:
    """Result of TRU-OLS unmixing applied to FCS data."""

    @property
    def dataframe(self) -> pl.DataFrame:
        """Unmixed data as a polars DataFrame."""
        ...
    @property
    def column_names(self) -> list[str]:
        """Column names in the result."""
        ...
    @property
    def n_events(self) -> int:
        """Number of events."""
        ...

def unmix(
    mixing_matrix: list[list[float]],
    unstained_control: list[list[float]],
    dataset: list[list[float]],
    autofluorescence_idx: int,
    cutoff_percentile: float = 0.995,
    strategy: str = "zero",
) -> list[list[float]]:
    """Convenience: create TruOls and unmix in one call."""
    ...

def unmix_dataframe(
    df: pl.DataFrame,
    detector_columns: list[str],
    mixing_matrix: list[list[float]],
    unstained_control: list[list[float]],
    endmember_names: list[str],
    autofluorescence_idx: int,
    cutoff_percentile: float = 0.995,
    strategy: str = "zero",
) -> pl.DataFrame:
    """Unmix detector columns from a polars DataFrame, returning endmember DataFrame."""
    ...

def unmix_fcs(
    stained_path: str,
    unstained_path: str,
    mixing_matrix: list[list[float]],
    detector_names: list[str],
    endmember_names: list[str],
    autofluorescence_name: str,
    cutoff_percentile: float = 0.995,
    strategy: str = "zero",
) -> UnmixingResult:
    """Open FCS files and run TRU-OLS unmixing."""
    ...

def read_fcs(path: str) -> pl.DataFrame:
    """Read an FCS file as a polars DataFrame."""
    ...

def extract_detector_data(
    df: pl.DataFrame, detector_columns: list[str]
) -> list[list[float]]:
    """Extract detector columns from a DataFrame into a nested list matrix."""
    ...

def calculate_cutoffs(
    mixing_matrix: list[list[float]],
    unstained_control: list[list[float]],
    percentile: float = 0.995,
) -> list[float]:
    """Calculate cutoff thresholds from unstained control data."""
    ...

def calculate_nonspecific_observation(
    mixing_matrix: list[list[float]],
    unstained_control: list[list[float]],
    autofluorescence_idx: int,
) -> list[float]:
    """Calculate the nonspecific (background) observation vector."""
    ...
