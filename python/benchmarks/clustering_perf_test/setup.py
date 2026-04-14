# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Benchmark environment setup: create tables for each control group."""

from pathlib import Path

import lancedb

from clustering_perf_test.config import VECTOR_INDEX_CONFIG
from clustering_perf_test.data_generator import generate_data


def setup_group(
    db_uri: Path,
    group: str,
    data: object,
    cluster_keys: list[str] | None = None,
    target_rows_per_fragment: int | None = None,
) -> lancedb.table.Table:
    """Initialize a benchmark group table.

    Groups:
      A - No clustering, no index.
      B - Clustering only.
      C - Scalar index only (on first cluster key).
      D - Clustering + scalar index.
      E - Clustering + vector index.
    """
    table_name = f"group_{group.lower()}"
    db = lancedb.connect(str(db_uri))

    # Clean up existing table
    if table_name in db.table_names():
        db.drop_table(table_name)

    table = db.create_table(table_name, data=data, mode="overwrite")

    if group in ("B", "D", "E") and cluster_keys:
        table.optimize(
            cluster_by=cluster_keys,
            action="cluster",
        )

    if group in ("C", "D") and cluster_keys:
        # Create a scalar index on the first cluster key for simplicity.
        table.create_index(cluster_keys[0], index_type="BTREE")

    if group == "E":
        table.create_index(
            VECTOR_INDEX_CONFIG["column"],
            index_type=VECTOR_INDEX_CONFIG["index_type"],
            num_partitions=VECTOR_INDEX_CONFIG["num_partitions"],
            num_sub_vectors=VECTOR_INDEX_CONFIG["num_sub_vectors"],
        )

    return table


def setup_all_groups(
    db_uri: Path,
    data: object,
    cluster_keys: list[str],
    target_rows_per_fragment: int | None = None,
) -> dict[str, lancedb.table.Table]:
    """Setup all benchmark groups and return a mapping."""
    groups = {}
    for group in ("A", "B", "C", "D", "E"):
        groups[group] = setup_group(
            db_uri,
            group,
            data,
            cluster_keys=cluster_keys,
            target_rows_per_fragment=target_rows_per_fragment,
        )
    return groups
