"""
tru_ols - Python bindings for flow-tru-ols spectral unmixing.

This package provides Python access to the high-performance Rust implementation
of the TRU-OLS (Truncated ReUnmixing Ordinary Least Squares) algorithm for
flow cytometry spectral unmixing.

Usage:
    import tru_ols

    # Define mixing matrix (detectors x endmembers)
    mixing_matrix = [
        [0.9, 0.1, 0.05],
        [0.1, 0.9, 0.05],
        [0.05, 0.05, 0.9],
    ]

    # Unstained control data (events x detectors)
    unstained = [[0.1, 0.2, 0.1], [0.15, 0.18, 0.12]]

    # Create TRU-OLS instance
    unmixer = tru_ols.TruOls(mixing_matrix, unstained, autofluorescence_idx=2)

    # Unmix a dataset (events x detectors)
    dataset = [[100.0, 50.0, 10.0], [200.0, 150.0, 20.0]]
    result = unmixer.unmix(dataset)
"""

from .tru_ols import *  # noqa: F401, F403
