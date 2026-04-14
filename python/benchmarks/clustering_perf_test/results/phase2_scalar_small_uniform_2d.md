# Scalar Query Benchmark Report

## Results Summary

### S1_2D_Range

| Selectivity | Group | Latency P50 (ms) | Latency P95 (ms) | Bytes Read | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|---:|---:|
| 10.0% | A | 7.83 | 7.97 | 2804096 | 1.00x | 1.00x |
| 10.0% | B | 3.75 | 3.75 | 2707627 | 2.09x | 1.04x |
| 10.0% | C | 10.89 | 11.43 | 2803564 | 0.72x | 1.00x |
| 10.0% | D | 3.43 | 3.46 | 352068 | 2.28x | 7.96x |

### S2_1D_Range

| Selectivity | Group | Latency P50 (ms) | Latency P95 (ms) | Bytes Read | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|---:|---:|
| 10.0% | A | 19.88 | 20.30 | 2804096 | 1.00x | 1.00x |
| 10.0% | B | 19.62 | 19.85 | 2840960 | 1.01x | 0.99x |
| 10.0% | C | 20.98 | 22.77 | 2804096 | 0.95x | 1.00x |
| 10.0% | D | 19.96 | 20.50 | 2840960 | 1.00x | 0.99x |

### S3_Point_Query

| Selectivity | Group | Latency P50 (ms) | Latency P95 (ms) | Bytes Read | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|---:|---:|
| point | A | 2.16 | 2.30 | 0 | 1.00x | 1.00x |
| point | B | 2.27 | 2.45 | 0 | 0.95x | 1.00x |
| point | C | 2.60 | 3.06 | 0 | 0.83x | 1.00x |
| point | D | 2.34 | 2.35 | 0 | 0.92x | 1.00x |

### S4_Full_Scan

| Selectivity | Group | Latency P50 (ms) | Latency P95 (ms) | Bytes Read | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|---:|---:|
| all | A | 28.98 | 32.01 | 55893022 | 1.00x | 1.00x |
| all | B | 9.40 | 9.47 | 55930210 | 3.08x | 1.00x |

### S5_Mixed_Condition

| Selectivity | Group | Latency P50 (ms) | Latency P95 (ms) | Bytes Read | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|---:|---:|
| 10.0% | A | 11.95 | 11.97 | 2904132 | 1.00x | 1.00x |
| 10.0% | B | 5.39 | 5.44 | 2941319 | 2.22x | 0.99x |
| 10.0% | C | 14.73 | 14.96 | 2903581 | 0.81x | 1.00x |
| 10.0% | D | 5.25 | 5.29 | 367514 | 2.28x | 7.90x |
