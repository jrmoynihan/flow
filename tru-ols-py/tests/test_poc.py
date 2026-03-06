"""
Proof-of-concept: Test tru_ols Python bindings with synthetic spectral data.

This script exercises every major binding function using synthetic mixing
matrices and detector data that mimic typical spectral flow cytometry.
"""

import polars as pl
import tru_ols
import random
import math

random.seed(42)


def make_mixing_matrix(n_detectors: int, n_endmembers: int) -> list[list[float]]:
    """Generate a synthetic mixing matrix (detectors x endmembers).

    Each endmember has a peak in one detector with some spectral spillover.
    """
    matrix = []
    for d in range(n_detectors):
        row = []
        for e in range(n_endmembers):
            if d == e % n_detectors:
                row.append(0.8 + random.uniform(0, 0.15))
            elif abs(d - (e % n_detectors)) == 1:
                row.append(0.05 + random.uniform(0, 0.1))
            else:
                row.append(random.uniform(0, 0.03))
        matrix.append(row)
    return matrix


def make_unstained_control(
    n_events: int, n_detectors: int
) -> list[list[float]]:
    """Generate synthetic unstained control data (events x detectors).

    Low background signal with some noise.
    """
    return [
        [max(0.0, random.gauss(5.0, 2.0)) for _ in range(n_detectors)]
        for _ in range(n_events)
    ]


def make_stained_dataset(
    n_events: int,
    n_detectors: int,
    mixing_matrix: list[list[float]],
    n_endmembers: int,
) -> list[list[float]]:
    """Generate synthetic stained data by mixing known abundances.

    Creates events with known ground-truth abundances, then multiplies
    by the mixing matrix to get detector observations.
    """
    dataset = []
    for _ in range(n_events):
        # Random abundances (some zero = irrelevant endmember)
        abundances = []
        for e in range(n_endmembers):
            if random.random() < 0.3:
                abundances.append(0.0)
            else:
                abundances.append(random.gauss(100.0, 30.0))

        # Matrix multiply: observation = M @ abundances + noise
        observation = []
        for d in range(n_detectors):
            val = sum(
                mixing_matrix[d][e] * abundances[e] for e in range(n_endmembers)
            )
            val += random.gauss(0, 2.0)
            observation.append(val)
        dataset.append(observation)
    return dataset


def test_tru_ols_class():
    """Test the TruOls class creation and basic properties."""
    print("=" * 60)
    print("TEST: TruOls class creation and properties")
    print("=" * 60)

    n_detectors = 4
    n_endmembers = 3
    autofluorescence_idx = 2

    mixing_matrix = make_mixing_matrix(n_detectors, n_endmembers)
    unstained = make_unstained_control(100, n_detectors)

    unmixer = tru_ols.TruOls(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        autofluorescence_idx=autofluorescence_idx,
    )

    print(f"  repr: {unmixer}")
    print(f"  n_detectors: {unmixer.n_detectors}")
    print(f"  n_endmembers: {unmixer.n_endmembers}")

    assert unmixer.n_detectors == n_detectors
    assert unmixer.n_endmembers == n_endmembers

    mm_data = unmixer.mixing_matrix_data
    assert len(mm_data) == n_detectors
    assert len(mm_data[0]) == n_endmembers
    print("  PASSED\n")


def test_unmix_single_event():
    """Test unmixing a single event."""
    print("=" * 60)
    print("TEST: unmix_event (single event)")
    print("=" * 60)

    mixing_matrix = [[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]]
    unstained = [[0.1, 0.2, 0.1], [0.15, 0.18, 0.12], [0.08, 0.22, 0.09]]

    unmixer = tru_ols.TruOls(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        autofluorescence_idx=1,
    )

    observation = [100.0, 50.0, 10.0]
    abundances, relevant_indices, irrelevant = unmixer.unmix_event(observation)

    print(f"  Observation: {observation}")
    print(f"  Abundances: {abundances}")
    print(f"  Relevant indices: {relevant_indices}")
    print(f"  Irrelevant: {irrelevant}")

    assert len(abundances) > 0, "Should have at least one abundance"
    assert len(relevant_indices) > 0, "Should have at least one relevant index"
    print("  PASSED\n")


def test_unmix_dataset():
    """Test unmixing an entire dataset."""
    print("=" * 60)
    print("TEST: unmix (full dataset)")
    print("=" * 60)

    n_detectors = 5
    n_endmembers = 4
    n_events = 500
    autofluorescence_idx = 3

    mixing_matrix = make_mixing_matrix(n_detectors, n_endmembers)
    unstained = make_unstained_control(200, n_detectors)
    dataset = make_stained_dataset(n_events, n_detectors, mixing_matrix, n_endmembers)

    unmixer = tru_ols.TruOls(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        autofluorescence_idx=autofluorescence_idx,
    )

    result = unmixer.unmix(dataset)

    print(f"  Input: {n_events} events x {n_detectors} detectors")
    print(f"  Output: {len(result)} events x {len(result[0])} endmembers")
    print(f"  First event abundances: {[f'{v:.2f}' for v in result[0]]}")

    assert len(result) == n_events, "Should have same number of events"
    assert len(result[0]) == n_endmembers, "Should have n_endmembers columns"
    print("  PASSED\n")


def test_unmix_convenience_function():
    """Test the module-level unmix() convenience function."""
    print("=" * 60)
    print("TEST: unmix() convenience function")
    print("=" * 60)

    mixing_matrix = [[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]]
    unstained = [[0.1, 0.2, 0.1], [0.15, 0.18, 0.12]]
    dataset = [[100.0, 50.0, 10.0], [200.0, 150.0, 20.0], [50.0, 200.0, 15.0]]

    result = tru_ols.unmix(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        dataset=dataset,
        autofluorescence_idx=1,
    )

    print(f"  Input: {len(dataset)} events")
    print(f"  Output: {len(result)} events x {len(result[0])} endmembers")
    for i, row in enumerate(result):
        print(f"    Event {i}: {[f'{v:.2f}' for v in row]}")

    assert len(result) == 3
    assert len(result[0]) == 2
    print("  PASSED\n")


def test_unmix_dataframe():
    """Test unmixing a polars DataFrame."""
    print("=" * 60)
    print("TEST: unmix_dataframe (polars DataFrame)")
    print("=" * 60)

    n_detectors = 3
    n_endmembers = 2
    n_events = 100

    mixing_matrix = make_mixing_matrix(n_detectors, n_endmembers)
    unstained = make_unstained_control(50, n_detectors)
    dataset = make_stained_dataset(n_events, n_detectors, mixing_matrix, n_endmembers)

    # Build polars DataFrame from the synthetic data
    df = pl.DataFrame({
        "Det_A": pl.Series([row[0] for row in dataset], dtype=pl.Float64),
        "Det_B": pl.Series([row[1] for row in dataset], dtype=pl.Float64),
        "Det_C": pl.Series([row[2] for row in dataset], dtype=pl.Float64),
    })

    print(f"  Input DataFrame: {df.shape}")

    result_df = tru_ols.unmix_dataframe(
        df=df,
        detector_columns=["Det_A", "Det_B", "Det_C"],
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        endmember_names=["Dye1", "Autofluorescence"],
        autofluorescence_idx=1,
    )

    print(f"  Output DataFrame: {result_df.shape}")
    print(f"  Columns: {result_df.columns}")
    print(f"  Head:\n{result_df.head(3)}")

    assert result_df.shape[0] == n_events, "Should have same number of events"
    assert result_df.shape[1] == n_endmembers, "Should have n_endmembers columns"
    assert "Dye1" in result_df.columns
    assert "Autofluorescence" in result_df.columns
    print("  PASSED\n")


def test_strategies():
    """Test both unmixing strategies."""
    print("=" * 60)
    print("TEST: Unmixing strategies (zero vs ucm)")
    print("=" * 60)

    mixing_matrix = [[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]]
    unstained = [
        [random.gauss(5, 2) for _ in range(3)] for _ in range(50)
    ]
    dataset = [
        [random.gauss(100, 20) for _ in range(3)] for _ in range(20)
    ]

    for strategy in ["zero", "ucm"]:
        result = tru_ols.unmix(
            mixing_matrix=mixing_matrix,
            unstained_control=unstained,
            dataset=dataset,
            autofluorescence_idx=1,
            strategy=strategy,
        )
        n_nonzero = sum(1 for row in result for v in row if abs(v) > 1e-10)
        print(f"  strategy='{strategy}': {n_nonzero}/{len(result)*2} non-zero abundances")

    print("  PASSED\n")


def test_set_cutoff_percentile():
    """Test changing the cutoff percentile."""
    print("=" * 60)
    print("TEST: set_cutoff_percentile")
    print("=" * 60)

    mixing_matrix = [[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]]
    unstained = [
        [random.gauss(5, 2) for _ in range(3)] for _ in range(100)
    ]

    unmixer = tru_ols.TruOls(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        autofluorescence_idx=1,
        cutoff_percentile=0.99,
    )

    # Change percentile
    unmixer.set_cutoff_percentile(0.5)
    print("  Changed cutoff to 50th percentile (more aggressive filtering)")

    unmixer.set_cutoff_percentile(0.999)
    print("  Changed cutoff to 99.9th percentile (less aggressive filtering)")
    print("  PASSED\n")


def test_calculate_cutoffs():
    """Test the standalone cutoff calculation function."""
    print("=" * 60)
    print("TEST: calculate_cutoffs")
    print("=" * 60)

    mixing_matrix = [[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]]
    unstained = [
        [random.gauss(5, 2) for _ in range(3)] for _ in range(100)
    ]

    cutoffs = tru_ols.calculate_cutoffs(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        percentile=0.995,
    )

    print(f"  Cutoffs: {[f'{v:.4f}' for v in cutoffs]}")
    assert len(cutoffs) == 2, "Should have one cutoff per endmember"
    print("  PASSED\n")


def test_calculate_nonspecific_observation():
    """Test the standalone nonspecific observation calculation."""
    print("=" * 60)
    print("TEST: calculate_nonspecific_observation")
    print("=" * 60)

    mixing_matrix = [[0.9, 0.1], [0.1, 0.9], [0.05, 0.05]]
    unstained = [
        [random.gauss(5, 2) for _ in range(3)] for _ in range(100)
    ]

    obs = tru_ols.calculate_nonspecific_observation(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        autofluorescence_idx=1,
    )

    print(f"  Nonspecific observation: {[f'{v:.4f}' for v in obs]}")
    assert len(obs) == 3, "Should have one value per detector"
    print("  PASSED\n")


def test_extract_detector_data():
    """Test extracting detector columns from a DataFrame."""
    print("=" * 60)
    print("TEST: extract_detector_data")
    print("=" * 60)

    df = pl.DataFrame({
        "FL1-A": pl.Series([100.0, 200.0, 300.0], dtype=pl.Float32),
        "FL2-A": pl.Series([50.0, 150.0, 250.0], dtype=pl.Float32),
        "FSC-A": pl.Series([1000.0, 2000.0, 3000.0], dtype=pl.Float32),
    })

    result = tru_ols.extract_detector_data(df, ["FL1-A", "FL2-A"])

    print(f"  Input: {df.shape}")
    print(f"  Extracted: {len(result)} events x {len(result[0])} detectors")
    print(f"  First event: {result[0]}")

    assert len(result) == 3
    assert len(result[0]) == 2
    assert abs(result[0][0] - 100.0) < 1e-3
    assert abs(result[0][1] - 50.0) < 1e-3
    print("  PASSED\n")


def test_error_handling():
    """Test that errors are properly raised as Python exceptions."""
    print("=" * 60)
    print("TEST: Error handling")
    print("=" * 60)

    # Invalid percentile
    try:
        tru_ols.calculate_cutoffs(
            mixing_matrix=[[1.0, 0.1], [0.1, 1.0]],
            unstained_control=[[0.1, 0.2]],
            percentile=2.0,
        )
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Invalid percentile: {e}")

    # Dimension mismatch
    try:
        tru_ols.TruOls(
            mixing_matrix=[[1.0, 0.1], [0.1, 1.0]],
            unstained_control=[[0.1, 0.2, 0.3]],  # 3 cols, but matrix has 2 rows
            autofluorescence_idx=0,
        )
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Dimension mismatch: {e}")

    # Invalid strategy
    try:
        tru_ols.TruOls(
            mixing_matrix=[[1.0, 0.1], [0.1, 1.0]],
            unstained_control=[[0.1, 0.2]],
            autofluorescence_idx=0,
            strategy="invalid_strategy",
        )
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Invalid strategy: {e}")

    # Invalid autofluorescence index
    try:
        tru_ols.TruOls(
            mixing_matrix=[[1.0, 0.1], [0.1, 1.0]],
            unstained_control=[[0.1, 0.2]],
            autofluorescence_idx=99,
        )
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Bad AF index: {e}")

    # Empty matrix
    try:
        tru_ols.unmix(
            mixing_matrix=[],
            unstained_control=[[0.1]],
            dataset=[[1.0]],
            autofluorescence_idx=0,
        )
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Empty matrix: {e}")

    # Non-existent FCS file
    try:
        tru_ols.read_fcs("/nonexistent/path.fcs")
        assert False, "Should have raised ValueError"
    except ValueError as e:
        print(f"  Bad FCS path: {e}")

    print("  PASSED\n")


def test_full_pipeline():
    """Test a complete unmixing pipeline: create data, unmix, analyse results."""
    print("=" * 60)
    print("TEST: Full pipeline")
    print("=" * 60)

    n_detectors = 6
    n_endmembers = 4
    n_events = 1000
    n_unstained = 200
    autofluorescence_idx = 3

    mixing_matrix = make_mixing_matrix(n_detectors, n_endmembers)
    unstained = make_unstained_control(n_unstained, n_detectors)
    dataset = make_stained_dataset(n_events, n_detectors, mixing_matrix, n_endmembers)

    # Step 1: Calculate cutoffs
    cutoffs = tru_ols.calculate_cutoffs(mixing_matrix, unstained, 0.995)
    print(f"  Cutoffs: {[f'{v:.2f}' for v in cutoffs]}")

    # Step 2: Calculate nonspecific observation
    ns_obs = tru_ols.calculate_nonspecific_observation(
        mixing_matrix, unstained, autofluorescence_idx
    )
    print(f"  Nonspecific obs: {[f'{v:.2f}' for v in ns_obs]}")

    # Step 3: Create unmixer and configure
    unmixer = tru_ols.TruOls(
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        autofluorescence_idx=autofluorescence_idx,
        cutoff_percentile=0.995,
        strategy="zero",
    )
    print(f"  Unmixer: {unmixer}")

    # Step 4: Unmix dataset
    result = unmixer.unmix(dataset)
    print(f"  Unmixed: {len(result)} events x {len(result[0])} endmembers")

    # Step 5: Analyse results
    for em_idx in range(n_endmembers):
        values = [row[em_idx] for row in result]
        n_zero = sum(1 for v in values if abs(v) < 1e-10)
        mean_val = sum(values) / len(values)
        print(
            f"    Endmember {em_idx}: mean={mean_val:.2f}, "
            f"zeros={n_zero}/{n_events} ({100*n_zero/n_events:.1f}%)"
        )

    # Step 6: Also test via DataFrame route
    detector_names = [f"Det_{i}" for i in range(n_detectors)]
    endmember_names = [f"Dye_{i}" for i in range(n_endmembers)]

    df = pl.DataFrame({
        name: pl.Series([row[i] for row in dataset], dtype=pl.Float64)
        for i, name in enumerate(detector_names)
    })

    result_df = tru_ols.unmix_dataframe(
        df=df,
        detector_columns=detector_names,
        mixing_matrix=mixing_matrix,
        unstained_control=unstained,
        endmember_names=endmember_names,
        autofluorescence_idx=autofluorescence_idx,
    )

    print(f"  DataFrame result: {result_df.shape}")
    print(f"  Columns: {result_df.columns}")
    print(f"  Sample:\n{result_df.head(3)}")

    assert result_df.shape == (n_events, n_endmembers)
    print("  PASSED\n")


if __name__ == "__main__":
    print("\n🧪 TRU-OLS Python Bindings - Proof of Concept\n")

    test_tru_ols_class()
    test_unmix_single_event()
    test_unmix_dataset()
    test_unmix_convenience_function()
    test_unmix_dataframe()
    test_strategies()
    test_set_cutoff_percentile()
    test_calculate_cutoffs()
    test_calculate_nonspecific_observation()
    test_extract_detector_data()
    test_error_handling()
    test_full_pipeline()

    print("=" * 60)
    print("ALL TESTS PASSED")
    print("=" * 60)
