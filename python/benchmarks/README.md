# LanceDB Benchmarks

This directory contains performance benchmarks for LanceDB.

## clustering_perf_test

Multi-dimensional clustering performance benchmark suite.

### Quick start

```bash
cd python/benchmarks
python -m clustering_perf_test.runner --scenario scalar --scale small --quick
```

### Full scalar benchmark

```bash
python -m clustering_perf_test.runner --scenario scalar --scale medium --dimensions 2
```

### Vector benchmark

```bash
python -m clustering_perf_test.runner --scenario vector --scale medium --dimensions 2
```

### Key documents

| Document | Description |
|:---|:---|
| [`clustering_perf_test/DESIGN.md`](clustering_perf_test/DESIGN.md) | Full test design document |
| [`clustering_perf_test/RESULTS.md`](clustering_perf_test/RESULTS.md) | **Final summary report with all phases** |
| [`clustering_perf_test/results/`](clustering_perf_test/results/) | Raw JSON and Markdown reports per phase |
