# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Synthetic data generation for clustering benchmarks."""

import numpy as np
import pandas as pd
import pyarrow as pa


def generate_data(n_rows: int, distribution: str = "uniform", seed: int = 42) -> pa.Table:
    """Generate a synthetic dataset for benchmarking.

    Parameters
    ----------
    n_rows : int
        Number of rows to generate.
    distribution : str
        One of "uniform", "gaussian", "skewed".
    seed : int
        Random seed for reproducibility.

    Returns
    -------
    pa.Table
        Arrow table with schema suitable for clustering benchmarks.
    """
    rng = np.random.default_rng(seed)

    if distribution == "uniform":
        lat = rng.uniform(-90.0, 90.0, n_rows)
        lng = rng.uniform(-180.0, 180.0, n_rows)
    elif distribution == "gaussian":
        lat = rng.normal(0.0, 20.0, n_rows)
        lng = rng.normal(0.0, 40.0, n_rows)
    elif distribution == "skewed":
        lat = (rng.zipf(2.0, n_rows) % 180).astype(np.float32) - 90.0
        lng = (rng.zipf(2.0, n_rows) % 360).astype(np.float32) - 180.0
    else:
        raise ValueError(f"Unknown distribution: {distribution}")

    categories = rng.choice(["A", "B", "C", "D"], n_rows)
    category_map = {"A": 0, "B": 1, "C": 2, "D": 3}
    df = pd.DataFrame({
        "id": np.arange(n_rows, dtype=np.int64),
        "lat": lat.astype(np.float32),
        "lng": lng.astype(np.float32),
        "timestamp": rng.integers(0, 1_000_000_000, n_rows).astype(np.int64),
        "category": categories,
        "category_id": np.array([category_map[c] for c in categories], dtype=np.int32),
        "price": rng.exponential(100.0, n_rows).astype(np.float32),
        "embedding": [rng.random(128).astype(np.float32).tolist() for _ in range(n_rows)],
        "text": [f"item_{i}" for i in range(n_rows)],
    })

    return pa.Table.from_pandas(df)


if __name__ == "__main__":
    table = generate_data(1_000, "uniform")
    print(f"Generated {table.num_rows} rows, {len(table.schema.names)} columns")
    print(table.schema)
