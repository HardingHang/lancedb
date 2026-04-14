# LanceDB Benchmarks

This directory contains performance benchmarks for LanceDB.

## clustering_perf_test

Multi-dimensional clustering performance benchmark suite.

### Quick start

```bash
cd python/benchmarks
python -m clustering_perf_test.runner --scenario scalar --scale small --quick
```

### Full test

```bash
python -m clustering_perf_test.runner --scenario scalar --scale medium --dimensions 2
```

See `clustering_perf_test/DESIGN.md` for the full test design document.
