# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Benchmark environment setup: create tables for each control group."""

from pathlib import Path

import lancedb

def setup_group(
    db_uri: Path,
    group: str,
    data: object,
    cluster_keys: list[str] | None = None,
    target_rows_per_fragment: int | None = None,
    vector_index_config: dict[str, object] | None = None,
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

    if group in ("A", "C"):
        table = db.create_table(table_name, data=data, mode="overwrite")
    else:
        table = db.create_table(
            table_name,
            data=data,
            mode="overwrite",
            cluster_by=cluster_keys,
        )

    if group in ("B", "D", "E") and cluster_keys:
        table.cluster(target_rows_per_fragment=target_rows_per_fragment)

    if group in ("C", "D") and cluster_keys:
        table.create_scalar_index(cluster_keys[0], index_type="BTREE")

    if group == "E" and vector_index_config is not None:
        table.create_index(
            vector_column_name=vector_index_config["column"],
            index_type=vector_index_config["index_type"],
            num_partitions=vector_index_config["num_partitions"],
            num_sub_vectors=vector_index_config["num_sub_vectors"],
        )

    return table


def setup_all_groups(
    db_uri: Path,
    data: object,
    cluster_keys: list[str],
    target_rows_per_fragment: int | None = None,
    vector_index_config: dict[str, object] | None = None,
) -> dict[str, lancedb.table.Table]:
    """Setup all benchmark groups and return a mapping.

    WARNING
    -------
    Group E builds an IVF_PQ vector index, which can take several minutes at
    100K+ rows if vector_index_config is not scale-appropriate. Scalar
    benchmark runners should call ``setup_group`` directly for groups A-D to
    avoid unexpected hangs.
    """
    groups = {}
    for group in ("A", "B", "C", "D", "E"):
        groups[group] = setup_group(
            db_uri,
            group,
            data,
            cluster_keys=cluster_keys,
            target_rows_per_fragment=target_rows_per_fragment,
            vector_index_config=vector_index_config,
        )
    return groups
