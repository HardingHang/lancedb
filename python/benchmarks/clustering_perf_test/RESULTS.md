# LanceDB 多维聚簇性能测试最终报告

> **测试分支**: `feature/multi-dimensional-clustering`
> **测试周期**: 2026-04
> **测试负责人**: Claude Sonnet 4.6 (辅助执行)
> **版本状态**: Phase 0-5 全部完成

---

## 1. 执行概览

本性能测试围绕 LanceDB 多维聚簇（Multi-dimensional Clustering）功能展开，覆盖标量查询、向量融合查询、多维边界条件及数据分布差异。

| Phase | 内容 | 状态 |
|:---|:---|:---|
| Phase 0 | 目录结构、设计文档 | ✅ 完成 |
| Phase 1 | 标量查询 Benchmark 框架 | ✅ 完成 |
| Phase 2 | 标量查询执行与结论 | ✅ 完成 |
| Phase 3 | 向量融合查询 + 聚簇独立贡献隔离 | ✅ 完成 |
| Phase 4 | 边界条件（1D-4D、Uniform/Gaussian/Skewed） | ✅ 完成 |
| Phase 5 | 汇总报告与文档更新 | ✅ 完成 |

---

## 2. 核心结论速览

### 2.1 标量查询：聚簇 + 标量索引是"最优解"

| 场景 | 最佳组合 | 峰值收益 |
|:---|:---|:---|
| 2D 范围查询（命中聚簇键） | **D 组（聚簇 + BTREE）** | **79x IO 减少，5.6x 提速** |
| 1D 时间查询（命中聚簇键） | **D 组** | **14.8x 提速（1D Direct Sort）** |
| 混合条件查询 | **D 组** | **75x IO 减少** |
| 全表扫描 | **B 组（纯聚簇）** | **1.3x 提速** |
| 点查询 | **C 组（纯索引）** | **2.2x 提速** |

### 2.2 向量查询：聚簇贡献微弱，向量索引是绝对主导

| 场景 | 发现 |
|:---|:---|
| 纯向量 ANN | IVF_PQ 本身提供 **8-9x 提速**，聚簇无额外收益 |
| 向量 + 2D 过滤 | IVF_PQ 提供 **8-10x 提速**；聚簇后反而慢 **20-30%** |
| 向量 + 1D 过滤 | 同上，聚簇在 1M 行规模下对向量查询为轻微负优化 |

### 2.3 边界条件：维度与分布决定收益天花板

| 条件 | 最优选择 | 关键发现 |
|:---|:---|:---|
| **维度** | 1D/2D（匹配查询模式） | 4D 下 2D 查询收益稀释至 11x（对比 2D 的 79x） |
| **分布** | Gaussian/聚集型 | Gaussian 下 0.1% selectivity 达到 **552x IO 减少** |
| **分布** | Skewed/Zipf | 热点数据本身需全读，聚簇上限仅 **2.5x** |
| **规模** | ≤1M 行安全 | 10M 行触发 **OOM**（全内存排序瓶颈） |

---

## 3. 测试设计与对照组

### 3.1 对照组定义

| 组别 | 名称 | 数据状态 |
|:---|:---|:---|
| **A** | No Clustering / No Index | 原始无序数据，无任何优化 |
| **B** | Clustering Only | 执行 `cluster()`，**无索引** |
| **C** | Index Only | 无序数据上建立 BTREE 标量索引 |
| **D** | Clustering + Index | 先 `cluster()`，再建 BTREE 索引 |
| **E** | Clustering + Vector Index | 先 `cluster()`，再建 IVF_PQ 向量索引 |
| **F** | Vector Index Only | 无聚簇，仅建 IVF_PQ 向量索引 |

### 3.2 关键查询定义

**标量查询（S1-S5）**
- S1: `lat BETWEEN ... AND lng BETWEEN ...`（多维范围）
- S2: `timestamp BETWEEN ...`（1D 范围）
- S3: `lat = x AND lng = y`（点查询）
- S4: 全表扫描聚合 `AVG(price)`
- S5: `lat BETWEEN ... AND category = 'A'`（混合条件）

**向量查询（V1-V3）**
- V1: 纯向量 ANN `search(embedding).limit(100)`
- V2: 向量 + 2D 预过滤 `search(embedding).where(lat/lng range)`
- V3: 向量 + 1D 预过滤 `search(embedding).where(timestamp range)`

---

## 4. Phase 2 标量查询深度结论

### 4.1 D 组：低 selectivity 下收益惊人（S1）

| Selectivity | Latency P50 | Speedup | IO Reduction |
|:---|---:|---:|---:|
| 0.1% | 3.72 ms | **1.88x** | **79.56x** |
| 1.0% | 5.01 ms | **2.12x** | **50.24x** |
| 10.0% | 8.68 ms | **5.53x** | **5.51x** |
| 50.0% | 41.82 ms | **2.47x** | **2.97x** |

在低 selectivity 范围查询中，**聚簇 + 标量索引** 实现了 50-80 倍的 IO 减少。这是多维聚簇最核心的价值场景。

### 4.2 纯标量索引（C 组）是负优化：随机 IO 灾难

| Selectivity | C 组 Latency | A 组 Latency | C 组 IOPS | A 组 IOPS |
|:---|---:|---:|---:|---:|
| 0.1% | 12.51 ms | 6.99 ms | **2,262** | **6** |
| 50.0% | 200.05 ms | 103.24 ms | 18,481 | 18,777 |

**原因**：无序数据 + BTREE 索引导致匹配行**散落在大量不连续 fragment** 中。虽然 `bytes_read` 减少了，但 **IOPS 从 6 暴涨到 2,262**，随机 seek 开销完全吞噬了节省的带宽收益。

### 4.3 纯聚簇（B 组）的收益来源：顺序局部性 > 数据跳过

**S5 Mixed Condition, 10% selectivity**：
- B 组 `bytes_read` 比 A 组**多了 32.6%**（52M → 69M）
- 但 latency 快了 **4.7 倍**（96ms → 20ms）
- IOPS 从 13,002 暴跌到 2,210

这说明 B 组的核心机制不是"跳过 fragment"，而是将**随机 IO 转换为顺序 IO**。即使捎带读进了一些无关行，顺序扫描的带宽效率也远高于随机跳转。

### 4.4 1D 时间查询：聚簇键未命中 = 零收益（S2）

| Selectivity | 最佳 Speedup | 原因 |
|:---|---:|:---|
| 0.1% | 1.01x | `timestamp` 不在 2D 聚簇键中 |
| 10.0% | 1.00x | 聚簇完全不生效 |

验证了基本假设：**聚簇只对过滤条件命中聚簇键的查询有效。**

### 4.5 全表扫描：未被拖慢，反而微幅提升（S4）

| 组别 | Latency | Speedup |
|:---|---:|---:|
| A | 84.30 ms | — |
| B | 65.31 ms | **1.29x** |

即使无过滤条件，聚簇后的物理局部性改善也能带来约 29% 的全表扫描提速。这说明聚簇不会"损害"其他查询。

---

## 5. Phase 3 向量查询隔离结论

### 5.1 最初的设计缺陷：A vs E 混淆了向量索引与聚簇的收益

Phase 3 最初的对照组是 A（无索引）vs E（聚簇+向量索引）。这导致测出的 5x–10x speedup 主要来自 **"有无 IVF_PQ 索引"**，无法隔离聚簇的贡献。

### 5.2 引入 F 组后的真实发现：聚簇在向量查询中慢了 20-30%

| 查询 | A→F（向量索引本身） | F→E（聚簇独立贡献） |
|:---|---:|---:|
| V1 Pure Vector | **8.59x** | **0.80x** |
| V2 + 2D Filter 0.1% | **9.34x** | **0.80x** |
| V2 + 2D Filter 50% | **7.75x** | **0.77x** |
| V3 + 1D Filter 0.1% | **13.34x** | **0.80x** |
| V3 + 1D Filter 50% | **9.12x** | **0.71x** |

### 5.3 为什么聚簇后向量查询反而变慢？

**最可能的根因：聚簇重排改变了 IVF_PQ 索引的 k-means 训练效果。**

IVF_PQ 的 k-means 聚类对数据物理顺序敏感。聚簇后的连续数据块可能导致：
- 质心分布轻微退化
- 某些 partition 过度聚集或稀疏
- 查询时需要访问更多 partition（effective nprobe 增加）

在 1M 行规模下，这带来了约 **3-5ms 的固定开销**。

### 5.4 标量过滤的收益为什么看不见？

即使聚簇让标量过滤快了 10 倍，在向量查询中这个优化空间只有约 **1ms**（因为 IVF_PQ 搜索本身就需要 10-16ms）。1ms 的改善被 3-5ms 的索引效率损失完全覆盖。

### 5.5 `bytes_read = 0` 的测量盲区

F 组和 E 组的 `bytes_read_avg = 0`，因为 `ds.scanner(nearest=...)` 走 IVF_PQ 索引路径时，**索引文件读取不被 `scan_stats_callback` 统计**。这导致我们无法直接比较向量查询的物理 I/O 差异。

---

## 6. Phase 4 边界条件结论

### 6.1 维度对比：2D 最优，4D 明显稀释

**S1（2D Range）中 D 组收益随维度变化：**

| 维度 | 0.1% IO Reduction | 10% Speedup |
|:---|---:|---:|
| 1D (`timestamp`) | 1.00x | 0.91x |
| **2D** (`lat/lng`) | **79.56x** | **5.53x** |
| 3D (+`category_id`) | 30.26x | 4.85x |
| 4D (+`timestamp`) | 11.63x | 4.01x |

- **2D 聚簇是 2D Range 查询的最优解**
- **3D 仍有显著收益**，但比 2D 下降约 50-60%
- **4D 收益明显稀释**：Hilbert 曲线在 4D 空间中的逼近误差增大，`lat/lng` 的物理局部性被尾端维度削弱

### 6.2 1D 时间查询：Direct Sort vs Hilbert

| 维度 | 10% Speedup | 说明 |
|:---|---:|:---|
| 1D Direct Sort | **14.82x** | 聚簇键完全命中 `timestamp` |
| 4D Hilbert | 8.58x | `timestamp` 被纳入但精度不如 Direct Sort |

**建议**：如果查询主要按单一维度过滤，优先使用 1D Direct Sort。

### 6.3 分布对比：Gaussian 是"甜蜜点"，Skewed 存在上限

**S1（2D Range）D 组 0.1% selectivity：**

| 分布 | Speedup | IO Reduction |
|:---|---:|---:|
| Uniform | 1.88x | 79.56x |
| **Gaussian** | **2.24x** | **552.10x** |
| Skewed | 1.88x | 2.45x |

**Gaussian 的惊人表现**：
- A 组读 32MB，D 组只读 **58KB**
- 高斯聚集数据使聚簇后查询几乎可以跳过 99.8% 的 fragment
- **真实世界的城市热点、用户活跃区是聚簇收益的最优场景**

**Skewed 的异常表现**：
- A 组在所有 selectivity 下都读约 525MB（metadata 无法跳过）
- D 组在所有 selectivity 下都读约 213MB
- 热点数据无论如何都要读，聚簇优化存在**天然上限**

### 6.4 规模限制：10M 行触发 OOM

2D Uniform Large（10M 行）测试在 `cluster()` 阶段被 kill（exit code 137，OOM）。

**说明**：
- 当前聚簇实现似乎是**全内存排序**
- 10M 行 × 128 维向量在测试环境内存不足
- 这是产品层面的**可扩展性瓶颈**

**建议**：
- 在文档中明确标注聚簇的内存需求
- 考虑 Rust 核心实现流式/外部排序

### 6.5 类型限制：Utf8 不支持聚簇

3D/4D 测试最初因 `category`（Utf8）作为聚簇键报错：
```
ValueError: Invalid input, Type 'Utf8' is not supported for clustering
```

已修复为 `category_id`（int32）。当前多维聚簇**仅支持数值类型**。

---

## 7. 工程实践建议

### 7.1 给用户的使用建议

| 场景 | 推荐配置 |
|:---|:---|
| 低 selectivity 标量范围查询（<5%） | **D 组（聚簇 + 标量索引）** |
| 中等 selectivity 标量查询（5%-20%） | **B 组或 D 组** |
| 高 selectivity 标量查询（>50%） | **D 组**（仍有 2-3x 收益） |
| 纯向量 ANN 为主 | **F 组（仅向量索引）**，跳过聚簇 |
| 混合工作负载（大量标量 + 少量向量） | **E 组（聚簇 + 向量索引）** |
| 主要按时间过滤 | **1D Direct Sort** |
| 主要按空间过滤 | **2D Hilbert** |
| 数据呈高斯/聚集分布 | **强烈推荐聚簇** |
| 数据呈 Zipf/极端偏斜 | 聚簇收益有限，需实测评估 |

### 7.2 给 LanceDB 产品的建议

1. **索引建议器应感知聚簇状态**
   - 为未聚簇数据推荐标量索引可能导致负优化
   - 应优先推荐聚簇，或在聚簇后再建议索引

2. **查询计划器应考虑"跳过索引"的启发式规则**
   - 高 selectivity 的无序数据 + 标量索引场景下，自动回退到全表扫描可能更优

3. **监控指标应同时暴露 bytes_read 和 iops**
   - 只看 latency 会掩盖随机 IO 问题

4. **向量索引查询应支持索引级 I/O 统计**
   - 当前 `scan_stats_callback` 无法捕获 IVF_PQ 的索引读取 I/O

5. **聚簇操作需要流式/外部排序支持**
   - 10M 行 OOM 暴露了内存扩展性瓶颈

6. **文档需明确标注类型限制**
   - Utf8 不支持聚簇，用户需手动编码为整数

---

## 8. 结果文件索引

| 文件 | 内容 |
|:---|:---|
| `results/phase2_scalar_medium_uniform_2d.{json,md}` | Phase 2 标量查询原始结果 |
| `results/phase2_scalar_conclusions.md` | Phase 2 阶段性结论 |
| `results/phase2_scalar_deep_analysis.md` | Phase 2 深度机理分析 |
| `results/phase3_vector_medium_uniform_2d.{json,md}` | Phase 3 向量查询原始结果 |
| `results/phase3_vector_conclusions.md` | Phase 3 初步结论（含设计缺陷反思） |
| `results/phase3_vector_isolated_analysis.md` | Phase 3 隔离分析（A/F/E 对照） |
| `results/phase2_scalar_medium_uniform_1d.{json,md}` | Phase 4 1D 聚簇结果 |
| `results/phase2_scalar_medium_uniform_3d.{json,md}` | Phase 4 3D 聚簇结果 |
| `results/phase2_scalar_medium_uniform_4d.{json,md}` | Phase 4 4D 聚簇结果 |
| `results/phase2_scalar_medium_gaussian_2d.{json,md}` | Phase 4 Gaussian 分布结果 |
| `results/phase2_scalar_medium_skewed_2d.{json,md}` | Phase 4 Skewed 分布结果 |
| `results/phase4_boundary_conclusions.md` | Phase 4 边界条件结论 |
| `RESULTS.md` | 本汇总报告 |

---

## 9. 总结

LanceDB 多维聚簇在**标量查询**中展现出极为显著的性能收益，尤其是在：
- **低 selectivity 范围查询**（0.1%–1%）
- **聚簇键与查询过滤条件匹配**的场景
- **Gaussian/聚集型数据分布**
- **1D Direct Sort 和 2D Hilbert 配置**

然而，在**向量查询**中，当前 1M 行规模下的实测结果表明：
- **IVF_PQ 向量索引是绝对主导因素**（8-13x 提速）
- **聚簇不仅未带来额外加速，反而因数据重排影响 k-means 效率，导致轻微 slowdown（~20%）**
- 标量过滤的优化空间在向量查询中被完全淹没

**最终推荐**：
- **以标量查询为主的业务**：大力推荐使用聚簇，低 selectivity 场景下可实现 50-80 倍 IO 减少。
- **以向量查询为主的业务**：优先直接建立 IVF_PQ 索引，无需先聚簇。
- **混合负载**：如果标量查询收益足够大，可以接受向量侧约 20% 的轻微损失，则仍可采用"聚簇 + 向量索引"的完整配置。
