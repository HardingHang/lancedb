---
name: LanceDB Clustering Phase 4
description: Phase 4 complete - Python bindings for clustering with 7 integration tests passing
type: project
originSessionId: f2184b1a-66e3-4a0d-9a56-42f2025f4801
---
Python bindings for the multi-dimensional clustering feature are complete.

**Why:** Phase 4 was the next step after completing the Rust core (Phases 0-3) to make clustering accessible to Python users.

**What was done:**
- Exposed `cluster_by` parameter on `Connection.create_table` / `create_empty_table`
- Exposed `Table.cluster_config()` returning `dict | None`
- Exposed `Table.cluster(target_rows_per_fragment=None)` returning clustering stats dict
- Added PyO3 bindings in `python/src/table.rs` and `python/src/connection.rs`
- Added type stubs in `python/lancedb/_lancedb.pyi`
- Added Python layer methods in `table.py`, `db.py`, and `remote/table.py`
- Added 7 integration tests in `python/tests/test_table.py`
- Fixed dataset cache invalidation bug in `execute_cluster_direct` by using `table.dataset.update(new_dataset)` instead of `reload()` after `WriteMode::Overwrite`

**Test results:**
- Python `test_table.py`: 82/82 passed
- Rust clustering tests: 34/34 passed
- `make check` and `make format` passed

**Critical files changed:**
- `python/src/table.rs`
- `python/src/connection.rs`
- `python/src/lib.rs`
- `python/lancedb/_lancedb.pyi`
- `python/lancedb/table.py`
- `python/lancedb/db.py`
- `python/lancedb/remote/table.py`
- `python/tests/test_table.py`
- `rust/lancedb/src/table/cluster/execute.rs`
