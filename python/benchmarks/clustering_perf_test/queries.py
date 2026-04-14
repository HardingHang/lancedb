# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Query definitions for clustering benchmarks.

All scalar queries use the low-level lance scanner with scan_stats_callback
so that physical IO metrics can be captured accurately.
"""

from __future__ import annotations

import lancedb
import numpy as np


def _collect_stats(scanner, stats_out: dict) -> None:
    """Execute a scanner and populate stats_out with scan statistics."""
    scanner.scan_stats_callback = lambda s: stats_out.update({
        "bytes_read": s.bytes_read,
        "fragments_scanned": s.fragments_scanned,
        "rows_scanned": s.rows_scanned,
    })
    scanner.to_table()


def scalar_range_query(
    table: lancedb.table.Table,
    stats_out: dict,
    lat_min: float,
    lat_max: float,
    lng_min: float,
    lng_max: float,
) -> None:
    """S1: Multi-dimensional range query on lat/lng."""
    ds = table.to_lance()
    filter_expr = (
        f"lat >= {lat_min} AND lat <= {lat_max} "
        f"AND lng >= {lng_min} AND lng <= {lng_max}"
    )
    scanner = ds.scanner(filter=filter_expr)
    _collect_stats(scanner, stats_out)


def scalar_1d_range_query(
    table: lancedb.table.Table,
    stats_out: dict,
    ts_min: int,
    ts_max: int,
) -> None:
    """S2: 1D range query on timestamp."""
    ds = table.to_lance()
    filter_expr = f"timestamp >= {ts_min} AND timestamp <= {ts_max}"
    scanner = ds.scanner(filter=filter_expr)
    _collect_stats(scanner, stats_out)


def scalar_point_query(
    table: lancedb.table.Table,
    stats_out: dict,
    lat: float,
    lng: float,
) -> None:
    """S3: Point query on lat/lng."""
    ds = table.to_lance()
    filter_expr = f"lat = {lat} AND lng = {lng}"
    scanner = ds.scanner(filter=filter_expr)
    _collect_stats(scanner, stats_out)


def scalar_full_scan_agg(
    table: lancedb.table.Table,
    stats_out: dict,
) -> None:
    """S4: Full table scan with aggregation.

    Uses a filter-less scanner to measure raw scan cost.
    """
    ds = table.to_lance()
    scanner = ds.scanner()
    _collect_stats(scanner, stats_out)


def scalar_mixed_condition_query(
    table: lancedb.table.Table,
    stats_out: dict,
    lat_min: float,
    lat_max: float,
    category: str,
) -> None:
    """S5: Mixed condition with partial clustering key hit."""
    ds = table.to_lance()
    filter_expr = (
        f"lat >= {lat_min} AND lat <= {lat_max} AND category = '{category}'"
    )
    scanner = ds.scanner(filter=filter_expr)
    _collect_stats(scanner, stats_out)


# ---------------------------------------------------------------------------
# Vector-scalar fusion queries
# ---------------------------------------------------------------------------

def vector_pure_ann(
    table: lancedb.table.Table,
    stats_out: dict,
    query_vec: list[float],
    limit: int = 100,
) -> None:
    """V1: Pure vector ANN search.

    Falls back to the high-level API; physical stats may be limited
    depending on lancedb version. For precise stats we use the underlying
    lance scanner with a nearest neighbour clause when available.
    """
    ds = table.to_lance()
    scanner = ds.scanner(nearest={
        "column": "embedding",
        "q": query_vec,
        "k": limit,
    })
    _collect_stats(scanner, stats_out)


def vector_with_2d_filter(
    table: lancedb.table.Table,
    stats_out: dict,
    query_vec: list[float],
    lat_min: float,
    lat_max: float,
    lng_min: float,
    lng_max: float,
    limit: int = 100,
) -> None:
    """V2: Vector ANN with 2D scalar pre-filter."""
    ds = table.to_lance()
    filter_expr = (
        f"lat >= {lat_min} AND lat <= {lat_max} "
        f"AND lng >= {lng_min} AND lng <= {lng_max}"
    )
    scanner = ds.scanner(
        filter=filter_expr,
        nearest={
            "column": "embedding",
            "q": query_vec,
            "k": limit,
        },
    )
    _collect_stats(scanner, stats_out)


def vector_with_1d_filter(
    table: lancedb.table.Table,
    stats_out: dict,
    query_vec: list[float],
    ts_min: int,
    ts_max: int,
    limit: int = 100,
) -> None:
    """V3: Vector ANN with 1D timestamp pre-filter."""
    ds = table.to_lance()
    filter_expr = f"timestamp >= {ts_min} AND timestamp <= {ts_max}"
    scanner = ds.scanner(
        filter=filter_expr,
        nearest={
            "column": "embedding",
            "q": query_vec,
            "k": limit,
        },
    )
    _collect_stats(scanner, stats_out)


def make_random_vector(dim: int = 128, seed: int | None = None) -> list[float]:
    """Generate a random query vector."""
    rng = np.random.default_rng(seed)
    return rng.random(dim).astype(np.float32).tolist()
