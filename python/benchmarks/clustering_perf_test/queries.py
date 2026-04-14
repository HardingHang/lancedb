# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Query definitions for clustering benchmarks.

All scalar queries use the low-level lance scanner with scan_stats_callback
so that physical IO metrics can be captured accurately.
"""

from __future__ import annotations

import lancedb
import numpy as np


def _collect_stats(ds, stats_out: dict, **scanner_kwargs) -> None:
    """Execute a scanner and populate stats_out with scan statistics.

    Raises
    ------
    RuntimeError
        If scan_stats_callback is not invoked (e.g. due to API mismatch or
        unsupported fields), so that silent metric loss is caught immediately.
    """
    scanner = ds.scanner(
        scan_stats_callback=lambda s: stats_out.update({
            "bytes_read": s.bytes_read,
            "iops": s.iops,
        }),
        **scanner_kwargs,
    )
    scanner.to_table()
    if not stats_out:
        raise RuntimeError(
            "scan_stats_callback produced no stats. "
            "This usually means the lance API changed or the callback was ignored."
        )


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
    _collect_stats(ds, stats_out, filter=filter_expr)


def scalar_1d_range_query(
    table: lancedb.table.Table,
    stats_out: dict,
    ts_min: int,
    ts_max: int,
) -> None:
    """S2: 1D range query on timestamp."""
    ds = table.to_lance()
    filter_expr = f"timestamp >= {ts_min} AND timestamp <= {ts_max}"
    _collect_stats(ds, stats_out, filter=filter_expr)


def scalar_point_query(
    table: lancedb.table.Table,
    stats_out: dict,
    lat: float,
    lng: float,
) -> None:
    """S3: Point query on lat/lng."""
    ds = table.to_lance()
    filter_expr = f"lat = {lat} AND lng = {lng}"
    _collect_stats(ds, stats_out, filter=filter_expr)


def scalar_full_scan_agg(
    table: lancedb.table.Table,
    stats_out: dict,
) -> None:
    """S4: Full table scan with aggregation.

    Uses a filter-less scanner to measure raw scan cost.
    """
    ds = table.to_lance()
    _collect_stats(ds, stats_out)


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
    _collect_stats(ds, stats_out, filter=filter_expr)


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
    _collect_stats(
        ds,
        stats_out,
        nearest={
            "column": "embedding",
            "q": query_vec,
            "k": limit,
        },
    )


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
    _collect_stats(
        ds,
        stats_out,
        filter=filter_expr,
        nearest={
            "column": "embedding",
            "q": query_vec,
            "k": limit,
        },
    )


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
    _collect_stats(
        ds,
        stats_out,
        filter=filter_expr,
        nearest={
            "column": "embedding",
            "q": query_vec,
            "k": limit,
        },
    )


def make_random_vector(dim: int = 128, seed: int | None = None) -> list[float]:
    """Generate a random query vector."""
    rng = np.random.default_rng(seed)
    return rng.random(dim).astype(np.float32).tolist()
