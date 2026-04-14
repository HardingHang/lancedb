# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Report formatting utilities for benchmark results."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from clustering_perf_test.metrics import QueryMetrics


def format_scalar_report(
    results: dict[str, dict[str, dict[str, Any]]],
    output_path: Path | None = None,
) -> str:
    """Format scalar benchmark results as a Markdown report.

    Parameters
    ----------
    results : dict
        Nested mapping: query_name -> selectivity -> group -> metrics dict.
    output_path : Path | None
        If provided, the report is written to this path.

    Returns
    -------
    str
        Markdown report content.
    """
    lines: list[str] = [
        "# Scalar Query Benchmark Report",
        "",
        "## Results Summary",
        "",
    ]

    for query_name, by_selectivity in results.items():
        lines.append(f"### {query_name}")
        lines.append("")
        lines.append(
            "| Selectivity | Group | Latency P50 (ms) | Latency P95 (ms) | "
            "Bytes Read | Fragments Scanned |"
        )
        lines.append(
            "|:---|:---|---:|---:|---:|---:|"
        )
        for selectivity, by_group in by_selectivity.items():
            for group, metrics in by_group.items():
                lines.append(
                    f"| {selectivity} | {group} | "
                    f"{metrics['latency_p50_ms']:.2f} | "
                    f"{metrics['latency_p95_ms']:.2f} | "
                    f"{metrics['bytes_read_avg']:.0f} | "
                    f"{metrics['fragments_scanned_avg']:.1f} |"
                )
        lines.append("")

    report = "\n".join(lines)
    if output_path:
        output_path.write_text(report, encoding="utf-8")
    return report


def save_json_results(
    results: dict[str, Any],
    output_path: Path,
) -> None:
    """Serialize benchmark results to JSON."""
    output_path.write_text(
        json.dumps(results, indent=2, default=lambda o: o.__dict__),
        encoding="utf-8",
    )


def metrics_to_dict(m: QueryMetrics) -> dict[str, Any]:
    """Convert QueryMetrics to a plain dictionary."""
    return {
        "latency_p50_ms": m.latency_p50_ms,
        "latency_p95_ms": m.latency_p95_ms,
        "latency_p99_ms": m.latency_p99_ms,
        "bytes_read_avg": m.bytes_read_avg,
        "fragments_scanned_avg": m.fragments_scanned_avg,
        "rows_scanned_avg": m.rows_scanned_avg,
    }
