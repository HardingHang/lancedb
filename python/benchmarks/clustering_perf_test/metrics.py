# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Metrics collection utilities for benchmarks."""

from __future__ import annotations

import time
from dataclasses import dataclass, field
from typing import Callable

import numpy as np


@dataclass
class QueryMetrics:
    """Aggregated metrics for a single query scenario."""

    latency_p50_ms: float = 0.0
    latency_p95_ms: float = 0.0
    latency_p99_ms: float = 0.0
    bytes_read_avg: float = 0.0
    iops_avg: float = 0.0
    raw_latencies_ms: list[float] = field(default_factory=list)
    raw_bytes_read: list[int] = field(default_factory=list)
    raw_iops: list[int] = field(default_factory=list)


def measure_query(
    query_fn: Callable,
    warmup_runs: int = 3,
    test_runs: int = 20,
) -> QueryMetrics:
    """Execute a query function repeatedly and collect metrics.

    Parameters
    ----------
    query_fn : Callable
        A callable that accepts no arguments and performs one query execution.
        It is responsible for populating any scanner stats internally.
    warmup_runs : int
        Number of warm-up executions before measurement.
    test_runs : int
        Number of measured executions.

    Returns
    -------
    QueryMetrics
        Aggregated metrics over the measured runs.
    """
    for _ in range(warmup_runs):
        query_fn()

    latencies: list[float] = []
    bytes_read_list: list[int] = []
    iops_list: list[int] = []

    for _ in range(test_runs):
        stats: dict = {}
        start = time.perf_counter()
        query_fn(stats)
        latencies.append((time.perf_counter() - start) * 1000.0)
        bytes_read_list.append(stats.get("bytes_read", 0))
        iops_list.append(stats.get("iops", 0))

    return QueryMetrics(
        latency_p50_ms=float(np.percentile(latencies, 50)),
        latency_p95_ms=float(np.percentile(latencies, 95)),
        latency_p99_ms=float(np.percentile(latencies, 99)),
        bytes_read_avg=float(np.mean(bytes_read_list)),
        iops_avg=float(np.mean(iops_list)),
        raw_latencies_ms=latencies,
        raw_bytes_read=bytes_read_list,
        raw_iops=iops_list,
    )
