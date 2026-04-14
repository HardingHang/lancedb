# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Benchmark configuration constants."""

from pathlib import Path

# Base paths
BENCHMARK_DIR = Path(__file__).parent
RESULTS_DIR = BENCHMARK_DIR / "results"
DATA_DIR = BENCHMARK_DIR / "benchmark_data"

# Scale configurations
SCALE_CONFIGS = {
    "small": {"n_rows": 100_000, "target_rows_per_fragment": 10_000},
    "medium": {"n_rows": 1_000_000, "target_rows_per_fragment": 50_000},
    "large": {"n_rows": 10_000_000, "target_rows_per_fragment": 100_000},
}

# Test execution parameters
WARMUP_RUNS = 3
TEST_RUNS = 20
SELECTIVITIES = [0.001, 0.01, 0.1, 0.5]

# Vector index configuration.
# NOTE: IVF_PQ build can be very slow (minutes at 100K+ rows). Do not include
# group E in scalar-only benchmark paths.
_VECTOR_INDEX_CONFIGS = {
    "small": {
        "column": "embedding",
        "index_type": "IVF_PQ",
        "num_partitions": 32,
        "num_sub_vectors": 8,
    },
    "medium": {
        "column": "embedding",
        "index_type": "IVF_PQ",
        "num_partitions": 128,
        "num_sub_vectors": 16,
    },
    "large": {
        "column": "embedding",
        "index_type": "IVF_PQ",
        "num_partitions": 256,
        "num_sub_vectors": 16,
    },
}


def get_vector_index_config(scale: str) -> dict[str, object]:
    """Return vector index configuration appropriate for the data scale."""
    if scale not in _VECTOR_INDEX_CONFIGS:
        raise ValueError(f"Unknown scale: {scale}")
    return _VECTOR_INDEX_CONFIGS[scale]

# Clustering configurations per dimensionality
CLUSTER_CONFIGS = {
    1: {"keys": ["timestamp"]},
    2: {"keys": ["lat", "lng"]},
    3: {"keys": ["lat", "lng", "category"]},
    4: {"keys": ["lat", "lng", "category", "timestamp"]},
}

# Group names for reports
GROUP_NAMES = {
    "A": "No Clustering",
    "B": "Clustering Only",
    "C": "Index Only",
    "D": "Clustering + Index",
    "E": "Clustering + Vector Index",
}
