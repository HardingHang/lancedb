# SPDX-License-Identifier: Apache-2.0
# SPDX-FileCopyrightText: Copyright The LanceDB Authors

"""Main benchmark runner entrypoint."""

from __future__ import annotations

import argparse
from pathlib import Path

from clustering_perf_test.config import (
    BENCHMARK_DIR,
    DATA_DIR,
    RESULTS_DIR,
    SCALE_CONFIGS,
    WARMUP_RUNS,
    TEST_RUNS,
    SELECTIVITIES,
    CLUSTER_CONFIGS,
)
from clustering_perf_test.data_generator import generate_data
from clustering_perf_test.setup import setup_group
from clustering_perf_test.queries import (
    scalar_range_query,
    scalar_1d_range_query,
    scalar_point_query,
    scalar_full_scan_agg,
    scalar_mixed_condition_query,
    make_random_vector,
)
from clustering_perf_test.metrics import measure_query
from clustering_perf_test.reporter import format_scalar_report, save_json_results, metrics_to_dict


def run_scalar_benchmark(
    scale: str,
    distribution: str,
    dimensions: int,
    quick: bool = False,
) -> None:
    """Run scalar query benchmarks for the specified configuration."""
    config = SCALE_CONFIGS[scale]
    n_rows = config["n_rows"]
    target_rows = config.get("target_rows_per_fragment")
    cluster_keys = CLUSTER_CONFIGS[dimensions]["keys"]

    print(f"[Scalar Benchmark] scale={scale}, rows={n_rows}, dist={distribution}, dims={dimensions}")
    print("Generating data...")
    data = generate_data(n_rows, distribution)

    db_uri = DATA_DIR / f"phase2_{scale}_{distribution}_{dimensions}d"
    print(f"Setting up benchmark groups in {db_uri}...")
    groups = {}
    for group in ("A", "B", "C", "D"):
        groups[group] = setup_group(
            db_uri, group, data, cluster_keys=cluster_keys, target_rows_per_fragment=target_rows
        )

    selectivities = [0.1] if quick else SELECTIVITIES
    lat_vals = data["lat"].to_pylist()
    lng_vals = data["lng"].to_pylist()
    ts_vals = data["timestamp"].to_pylist()

    results: dict = {}
    warm_runs = 1 if quick else WARMUP_RUNS
    test_runs = 3 if quick else TEST_RUNS

    # S1: 2D range query
    for sel in selectivities:
        sel_label = f"{sel*100:.1f}%"
        lat_min, lat_max = _percentile_range(lat_vals, sel)
        lng_min, lng_max = _percentile_range(lng_vals, sel)

        for group_name, table in groups.items():
            if group_name not in ("A", "B", "C", "D"):
                continue

            metrics = measure_query(
                lambda s={}, t=table: scalar_range_query(
                    t, s, lat_min, lat_max, lng_min, lng_max
                ),
                warmup_runs=warm_runs,
                test_runs=test_runs,
            )
            results.setdefault("S1_2D_Range", {}).setdefault(sel_label, {})[group_name] = metrics_to_dict(metrics)

    # S2: 1D range query
    for sel in selectivities:
        sel_label = f"{sel*100:.1f}%"
        ts_min, ts_max = _percentile_range(ts_vals, sel)

        ts_min_int = int(ts_min)
        ts_max_int = int(ts_max)
        for group_name, table in groups.items():
            if group_name not in ("A", "B", "C", "D"):
                continue

            metrics = measure_query(
                lambda s={}, t=table: scalar_1d_range_query(t, s, ts_min_int, ts_max_int),
                warmup_runs=warm_runs,
                test_runs=test_runs,
            )
            results.setdefault("S2_1D_Range", {}).setdefault(sel_label, {})[group_name] = metrics_to_dict(metrics)

    # S3: Point query (use median lat/lng)
    lat_point = float(sorted(lat_vals)[len(lat_vals) // 2])
    lng_point = float(sorted(lng_vals)[len(lng_vals) // 2])
    for group_name, table in groups.items():
        if group_name not in ("A", "B", "C", "D"):
            continue

        metrics = measure_query(
            lambda s={}, t=table: scalar_point_query(t, s, lat_point, lng_point),
            warmup_runs=warm_runs,
            test_runs=test_runs,
        )
        results.setdefault("S3_Point_Query", {}).setdefault("point", {})[group_name] = metrics_to_dict(metrics)

    # S4: Full table scan (A and B only)
    for group_name, table in groups.items():
        if group_name not in ("A", "B"):
            continue

        metrics = measure_query(
            lambda s={}, t=table: scalar_full_scan_agg(t, s),
            warmup_runs=warm_runs,
            test_runs=test_runs,
        )
        results.setdefault("S4_Full_Scan", {}).setdefault("all", {})[group_name] = metrics_to_dict(metrics)

    # S5: Mixed condition with partial clustering key hit
    for sel in selectivities:
        sel_label = f"{sel*100:.1f}%"
        lat_min, lat_max = _percentile_range(lat_vals, sel)
        category = "A"

        for group_name, table in groups.items():
            if group_name not in ("A", "B", "C", "D"):
                continue

            metrics = measure_query(
                lambda s={}, t=table: scalar_mixed_condition_query(
                    t, s, lat_min, lat_max, category
                ),
                warmup_runs=warm_runs,
                test_runs=test_runs,
            )
            results.setdefault("S5_Mixed_Condition", {}).setdefault(sel_label, {})[group_name] = metrics_to_dict(metrics)

    # Save outputs
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    json_path = RESULTS_DIR / f"phase2_scalar_{scale}_{distribution}_{dimensions}d.json"
    report_path = RESULTS_DIR / f"phase2_scalar_{scale}_{distribution}_{dimensions}d.md"
    save_json_results(results, json_path)
    format_scalar_report(results, report_path)
    print(f"Results saved to {json_path} and {report_path}")


def _percentile_range(values: list[float], coverage: float) -> tuple[float, float]:
    """Return a range that covers the given fraction of values."""
    import numpy as np
    sorted_vals = sorted(values)
    n = len(sorted_vals)
    start_idx = 0
    end_idx = max(1, int(n * coverage))
    return float(sorted_vals[start_idx]), float(sorted_vals[end_idx])


def main() -> None:
    parser = argparse.ArgumentParser(description="LanceDB Clustering Performance Benchmark")
    parser.add_argument(
        "--scenario",
        choices=["scalar", "vector", "all"],
        default="scalar",
        help="Benchmark scenario to run",
    )
    parser.add_argument(
        "--scale",
        choices=["small", "medium", "large"],
        default="small",
        help="Data scale",
    )
    parser.add_argument(
        "--distribution",
        choices=["uniform", "gaussian", "skewed"],
        default="uniform",
        help="Data distribution",
    )
    parser.add_argument(
        "--dimensions",
        type=int,
        choices=[1, 2, 3, 4],
        default=2,
        help="Number of clustering dimensions",
    )
    parser.add_argument(
        "--quick",
        action="store_true",
        help="Run a quick test with reduced iterations",
    )
    args = parser.parse_args()

    if args.scenario in ("scalar", "all"):
        run_scalar_benchmark(args.scale, args.distribution, args.dimensions, args.quick)

    print("Benchmark completed.")


if __name__ == "__main__":
    main()
