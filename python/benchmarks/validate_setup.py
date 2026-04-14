# Quick validation script to ensure scan_stats_callback works correctly.
# Run this before any full benchmark to catch API drift early.
import time

import lancedb

from clustering_perf_test.data_generator import generate_data
from clustering_perf_test.queries import scalar_range_query

data = generate_data(1_000, "uniform")
db_uri = "/tmp/lance_cluster_validate"
db = lancedb.connect(db_uri)
table_name = "validate"
if table_name in db.table_names():
    db.drop_table(table_name)

table = db.create_table(table_name, data=data, mode="overwrite", cluster_by=["lat", "lng"])
table.cluster(target_rows_per_fragment=1_000)

stats = {}
scalar_range_query(table, stats, 0.0, 1.0, 0.0, 1.0)
assert "bytes_read" in stats, f"bytes_read missing from stats: {stats}"
print(f"Validation passed. stats={stats}")
