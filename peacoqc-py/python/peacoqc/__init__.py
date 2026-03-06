"""
peacoqc - Python bindings for peacoqc-rs flow cytometry quality control.

This package provides Python access to the high-performance Rust implementation
of the PeacoQC algorithm for automated quality control of flow cytometry data.

Usage:
    import polars as pl
    import peacoqc

    # Load your data as a polars DataFrame
    df = pl.read_csv("events.csv")

    # Run PeacoQC quality control
    result = peacoqc.run_qc(
        df,
        channels=["FL1-A", "FL2-A"],
        channel_ranges={"FL1-A": (0.0, 262144.0), "FL2-A": (0.0, 262144.0)},
    )
    print(f"Removed {result.percentage_removed:.2f}% of events")

    # Apply the mask to filter good cells
    good_mask = result.good_cells
    clean_df = df.filter(pl.Series(good_mask))
"""

from .peacoqc import *  # noqa: F401, F403
