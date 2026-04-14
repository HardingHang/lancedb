# Phase 2 Scalar Benchmark Conclusions

## Test Configuration

- **Scale**: Medium (1,000,000 rows)
- **Distribution**: Uniform
- **Clustering Dimensions**: 2D (`lat`, `lng`)
- **Selectivities**: 0.1%, 1%, 10%, 50%
- **Warm-up Runs**: 3
- **Test Runs**: 20
- **Groups**: A (Baseline), B (Clustering Only), C (Index Only), D (Clustering + Index)

---

## Key Findings

### 1. Low-Selectivity 2D Range Queries Show Dramatic Improvements (S1)

| Selectivity | Group | Latency P50 | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|
| 0.1% | A | 6.99 ms | 1.00x | 1.00x |
| 0.1% | B | 7.11 ms | 0.98x | 1.38x |
| 0.1% | C | 12.51 ms | 0.56x | 5.39x |
| **0.1%** | **D** | **3.72 ms** | **1.88x** | **79.56x** |
| 1.0% | A | 10.63 ms | 1.00x | 1.00x |
| 1.0% | B | 7.91 ms | 1.34x | 1.39x |
| 1.0% | C | 14.23 ms | 0.75x | 1.02x |
| **1.0%** | **D** | **5.01 ms** | **2.12x** | **50.24x** |
| 10.0% | A | 48.02 ms | 1.00x | 1.00x |
| 10.0% | B | 8.56 ms | 5.61x | 1.56x |
| 10.0% | C | 70.36 ms | 0.68x | 1.38x |
| **10.0%** | **D** | **8.68 ms** | **5.53x** | **5.51x** |
| 50.0% | A | 103.24 ms | 1.00x | 1.00x |
| 50.0% | B | 29.49 ms | 3.50x | 2.71x |
| 50.0% | C | 200.05 ms | 0.52x | 1.01x |
| **50.0%** | **D** | **41.82 ms** | **2.47x** | **2.97x** |

**Conclusion**: Group D (clustering + scalar index) delivers massive IO reductions at low selectivity (79x at 0.1%, 50x at 1%). As selectivity increases to 50%, the benefit dilutes but still remains strong (3x IO reduction, 2.5x speedup). Group B (clustering only) shows excellent latency speedups at medium-to-high selectivity (5.6x at 10%) but modest IO reduction, suggesting the gain comes from improved data locality and sequential reads rather than aggressive fragment skipping.

**Critical Insight**: Scalar index alone (Group C) is a net negative for 2D range queries at all selectivities—it increases latency by 30–100% compared to baseline. Only when combined with clustering (Group D) does the scalar index become highly effective.

---

### 2. 1D Range on Non-Clustered Key Shows No Benefit (S2)

| Selectivity | Best Speedup | Best IO Reduction |
|:---|---:|---:|
| 0.1% | 1.01x (C) | 1.00x |
| 1.0% | 1.06x (C) | 1.04x |
| 10.0% | 1.00x (C) | 1.00x |
| 50.0% | 1.12x (C) | 1.00x |

**Conclusion**: Since the clustering key is `lat/lng` and the query filters on `timestamp`, clustering provides no benefit. This validates the fundamental assumption: multi-dimensional clustering only accelerates queries that filter on the clustered dimensions.

---

### 3. Point Queries Benefit from Scalar Index, Not Clustering (S3)

| Group | Latency P50 | Speedup |
|:---|---:|---:|
| A | 6.43 ms | 1.00x |
| B | 6.60 ms | 0.98x |
| C | 2.85 ms | 2.26x |
| D | 2.98 ms | 2.16x |

**Conclusion**: Point queries see no benefit from clustering (B vs A). The 2.2x speedup comes entirely from the BTREE scalar index (C and D). `bytes_read` is zero across all groups because Lance can reject non-matching fragments purely from min-max metadata without reading data pages.

---

### 4. Full Table Scan Slightly Faster After Clustering (S4)

| Group | Latency P50 | Speedup | Bytes Read |
|:---|---:|---:|---:|
| A | 84.30 ms | 1.00x | 559,893,022 |
| B | 65.31 ms | 1.29x | 559,971,530 |

**Conclusion**: Even with no filter, clustering improves full-scan performance by 29%. The physical reordering improves sequential read locality, reducing CPU/data movement overhead despite reading essentially the same total bytes.

---

### 5. Mixed Conditions Mirror 2D Range Results (S5)

| Selectivity | Group | Latency P50 | Speedup | IO Reduction |
|:---|:---|---:|---:|---:|
| 0.1% | D | 4.76 ms | 2.52x | 75.39x |
| 1.0% | D | 7.54 ms | 3.51x | 48.98x |
| 10.0% | D | 21.17 ms | 4.53x | 7.15x |
| 50.0% | D | 78.63 ms | 2.38x | 1.14x |

**Conclusion**: Adding `category = 'A'` (a non-clustered column) to a `lat` range filter does not break clustering's benefit. The pattern is nearly identical to S1, confirming that partial clustering key hits are sufficient for significant acceleration.

---

## Overall Conclusions

1. **Clustering + Scalar Index (D) is the winning combination** for low-to-moderate selectivity range queries on clustered dimensions, delivering up to **79x IO reduction** and **5.6x latency improvement**.
2. **Clustering alone (B)** provides strong latency wins at medium-to-high selectivity through improved locality, but does not dramatically reduce bytes read.
3. **Scalar index alone (C)** is counter-productive for 2D range queries in this workload, increasing latency by 30–100%. Its value only emerges when paired with clustering.
4. **Clustering has zero benefit** when the query does not filter on clustered dimensions (S2).
5. **Full scans are not harmed** by clustering; in fact, they become ~29% faster due to better sequential locality.
6. **Low selectivity is where clustering shines**: the 0.1% and 1% selectivity cases show the most dramatic improvements. At 50%, benefits persist but are significantly diluted.
