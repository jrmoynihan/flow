"""
Proof-of-concept: Test peacoqc Python bindings with synthetic flow cytometry data.

This script exercises every major binding function using synthetic polars
DataFrames that mimic typical flow cytometry event data.
"""

import polars as pl
import peacoqc
import math
import random

random.seed(42)


def generate_synthetic_fcs_data(
    n_events: int = 10_000,
    n_bad_start: int = 500,
    n_bad_end: int = 300,
) -> pl.DataFrame:
    """Generate synthetic flow cytometry event data with known quality issues.

    Creates data with:
    - A clean middle segment
    - A degraded beginning (shifted mean = instrument warm-up)
    - A degraded end (higher variance = clog/drift)
    - Some margin events at detector boundaries
    """
    fl1 = []
    fl2 = []
    fsc_a = []
    fsc_h = []

    for i in range(n_events):
        if i < n_bad_start:
            # Warm-up: shifted distribution
            fl1.append(random.gauss(30_000, 8_000))
            fl2.append(random.gauss(25_000, 7_000))
        elif i >= n_events - n_bad_end:
            # Drift/clog: much wider distribution
            fl1.append(random.gauss(50_000, 25_000))
            fl2.append(random.gauss(45_000, 22_000))
        else:
            # Clean signal
            fl1.append(random.gauss(50_000, 10_000))
            fl2.append(random.gauss(45_000, 9_000))

        # Scatter channels for doublet detection
        base_fsc = random.gauss(100_000, 15_000)
        fsc_a.append(base_fsc)
        # Singlets: FSC-H ≈ FSC-A; doublets get a wider ratio
        if random.random() < 0.03:
            # ~3% doublet rate
            fsc_h.append(base_fsc * random.gauss(0.6, 0.1))
        else:
            fsc_h.append(base_fsc * random.gauss(1.0, 0.05))

    # Inject some margin events (at detector boundaries)
    for idx in random.sample(range(n_events), min(50, n_events)):
        fl1[idx] = 0.0  # minimum boundary
    for idx in random.sample(range(n_events), min(30, n_events)):
        fl2[idx] = 262_144.0  # maximum boundary

    return pl.DataFrame({
        "FSC-A": pl.Series(fsc_a, dtype=pl.Float32),
        "FSC-H": pl.Series(fsc_h, dtype=pl.Float32),
        "FL1-A": pl.Series(fl1, dtype=pl.Float32),
        "FL2-A": pl.Series(fl2, dtype=pl.Float32),
    })


def test_run_qc():
    """Test the main peacoqc QC pipeline."""
    print("=" * 60)
    print("TEST: run_qc (main PeacoQC pipeline)")
    print("=" * 60)

    df = generate_synthetic_fcs_data()
    print(f"Input: {df.shape[0]} events, {df.shape[1]} channels")
    print(f"Columns: {df.columns}")

    channel_ranges = {
        "FL1-A": (0.0, 262_144.0),
        "FL2-A": (0.0, 262_144.0),
    }

    result = peacoqc.run_qc(
        df,
        channels=["FL1-A", "FL2-A"],
        channel_ranges=channel_ranges,
        mode="all",
        mad=6.0,
        it_limit=0.6,
        consecutive_bins=5,
    )

    print(f"Result: {result}")
    print(f"  good_cells length: {len(result.good_cells)}")
    print(f"  percentage_removed: {result.percentage_removed:.2f}%")
    print(f"  it_percentage: {result.it_percentage}")
    print(f"  mad_percentage: {result.mad_percentage}")
    print(f"  consecutive_percentage: {result.consecutive_percentage:.2f}%")
    print(f"  n_bins: {result.n_bins}")
    print(f"  events_per_bin: {result.events_per_bin}")

    # Apply mask
    good_mask = pl.Series(result.good_cells)
    clean_df = df.filter(good_mask)
    print(f"  Clean events: {clean_df.shape[0]} / {df.shape[0]}")

    assert len(result.good_cells) == df.shape[0], "Mask length mismatch"
    assert 0.0 <= result.percentage_removed <= 100.0, "Invalid percentage"
    print("  PASSED\n")


def test_run_qc_modes():
    """Test different QC modes."""
    print("=" * 60)
    print("TEST: run_qc with different modes")
    print("=" * 60)

    df = generate_synthetic_fcs_data(n_events=5_000)
    channel_ranges = {
        "FL1-A": (0.0, 262_144.0),
        "FL2-A": (0.0, 262_144.0),
    }

    for mode in ["all", "isolation_tree", "mad", "none"]:
        result = peacoqc.run_qc(
            df,
            channels=["FL1-A", "FL2-A"],
            channel_ranges=channel_ranges,
            mode=mode,
        )
        print(f"  mode='{mode}': removed {result.percentage_removed:.2f}%")
        assert len(result.good_cells) == df.shape[0]

    print("  PASSED\n")


def test_remove_margins():
    """Test margin event removal."""
    print("=" * 60)
    print("TEST: remove_margins")
    print("=" * 60)

    df = generate_synthetic_fcs_data()
    channel_ranges = {
        "FL1-A": (0.0, 262_144.0),
        "FL2-A": (0.0, 262_144.0),
    }

    result = peacoqc.remove_margins(
        df,
        channels=["FL1-A", "FL2-A"],
        channel_ranges=channel_ranges,
    )

    print(f"Result: {result}")
    print(f"  mask length: {len(result.mask)}")
    print(f"  percentage_removed: {result.percentage_removed:.2f}%")
    print(f"  margin_matrix: {result.margin_matrix}")

    assert len(result.mask) == df.shape[0], "Mask length mismatch"
    assert result.percentage_removed >= 0.0, "Negative percentage"

    # Apply mask
    clean_df = df.filter(pl.Series(result.mask))
    print(f"  After margins: {clean_df.shape[0]} / {df.shape[0]}")
    print("  PASSED\n")


def test_remove_doublets():
    """Test doublet removal."""
    print("=" * 60)
    print("TEST: remove_doublets")
    print("=" * 60)

    df = generate_synthetic_fcs_data()
    channel_ranges = {
        "FSC-A": (0.0, 262_144.0),
        "FSC-H": (0.0, 262_144.0),
    }

    result = peacoqc.remove_doublets(
        df,
        channel_ranges=channel_ranges,
        channel1="FSC-A",
        channel2="FSC-H",
        nmad=4.0,
    )

    print(f"Result: {result}")
    print(f"  mask length: {len(result.mask)}")
    print(f"  percentage_removed: {result.percentage_removed:.2f}%")
    print(f"  median_ratio: {result.median_ratio:.4f}")
    print(f"  mad_ratio: {result.mad_ratio:.4f}")
    print(f"  threshold: {result.threshold:.4f}")

    assert len(result.mask) == df.shape[0], "Mask length mismatch"
    print("  PASSED\n")


def test_full_pipeline():
    """Test the complete QC pipeline: margins → doublets → PeacoQC."""
    print("=" * 60)
    print("TEST: Full pipeline (margins → doublets → QC)")
    print("=" * 60)

    df = generate_synthetic_fcs_data(n_events=10_000)
    n_initial = df.shape[0]
    print(f"Initial events: {n_initial}")

    channel_ranges = {
        "FSC-A": (0.0, 262_144.0),
        "FSC-H": (0.0, 262_144.0),
        "FL1-A": (0.0, 262_144.0),
        "FL2-A": (0.0, 262_144.0),
    }

    # Step 1: Remove margins
    margin_result = peacoqc.remove_margins(
        df,
        channels=["FL1-A", "FL2-A"],
        channel_ranges=channel_ranges,
    )
    if margin_result.percentage_removed > 0:
        df = df.filter(pl.Series(margin_result.mask))
    print(f"After margins: {df.shape[0]} (removed {margin_result.percentage_removed:.2f}%)")

    # Step 2: Remove doublets
    doublet_result = peacoqc.remove_doublets(
        df,
        channel_ranges=channel_ranges,
        channel1="FSC-A",
        channel2="FSC-H",
    )
    if doublet_result.percentage_removed > 0:
        df = df.filter(pl.Series(doublet_result.mask))
    print(f"After doublets: {df.shape[0]} (removed {doublet_result.percentage_removed:.2f}%)")

    # Step 3: Run PeacoQC
    qc_result = peacoqc.run_qc(
        df,
        channels=["FL1-A", "FL2-A"],
        channel_ranges=channel_ranges,
        mode="all",
    )
    df = df.filter(pl.Series(qc_result.good_cells))
    print(f"After PeacoQC: {df.shape[0]} (removed {qc_result.percentage_removed:.2f}%)")

    total_removed = (1 - df.shape[0] / n_initial) * 100
    print(f"Total pipeline removal: {total_removed:.2f}%")
    print(f"Final clean events: {df.shape[0]} / {n_initial}")
    print("  PASSED\n")


def test_fcsfile_class():
    """Test the FcsFile class (without an actual FCS file, just verify the class exists)."""
    print("=" * 60)
    print("TEST: FcsFile class availability")
    print("=" * 60)

    assert hasattr(peacoqc, "FcsFile"), "FcsFile class not found"
    assert hasattr(peacoqc, "open_fcs"), "open_fcs function not found"
    assert hasattr(peacoqc, "read_fcs"), "read_fcs function not found"
    assert hasattr(peacoqc, "run_qc_on_fcs"), "run_qc_on_fcs function not found"
    assert hasattr(peacoqc, "preprocess"), "preprocess function not found"
    assert hasattr(peacoqc, "filter_fcs"), "filter_fcs function not found"
    print("  All FCS-related exports present")
    print("  PASSED\n")


def test_error_handling():
    """Test that errors are properly raised as Python exceptions."""
    print("=" * 60)
    print("TEST: Error handling")
    print("=" * 60)

    df = generate_synthetic_fcs_data(n_events=100)

    # Invalid mode
    try:
        peacoqc.run_qc(df, channels=["FL1-A"], channel_ranges={}, mode="invalid")
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Invalid mode error: {e}")

    # Non-existent channel
    try:
        peacoqc.run_qc(
            df,
            channels=["NONEXISTENT"],
            channel_ranges={"NONEXISTENT": (0.0, 100.0)},
        )
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Missing channel error: {e}")

    # Non-existent FCS file
    try:
        peacoqc.open_fcs("/nonexistent/path.fcs")
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Bad path error: {e}")

    print("  PASSED\n")


if __name__ == "__main__":
    print("\n🧪 PeacoQC Python Bindings - Proof of Concept\n")

    test_fcsfile_class()
    test_error_handling()
    test_run_qc()
    test_run_qc_modes()
    test_remove_margins()
    test_remove_doublets()
    test_full_pipeline()

    print("=" * 60)
    print("ALL TESTS PASSED")
    print("=" * 60)
