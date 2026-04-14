# LanceDB 多维聚簇性能测试设计文档

## 1. 测试目标

量化 LanceDB 多维聚簇（Multi-dimensional Clustering）在不同查询场景下的性能收益，并拆解收益来源：

1. **验证纯聚簇的数据跳过收益**（无索引依赖）
2. **对比聚簇 vs 标量索引**的投资回报率（ROI）
3. **验证聚簇与索引的协同效应**
4. **量化向标融合查询中的聚簇收益**
5. **识别聚簇收益的关键边界条件**（selectivity、数据分布、维度数）

---

## 2. 核心测试原理

LanceDB 多维聚簇通过 Hilbert 曲线 / Direct Sort 对数据进行全局重排，使物理存储顺序与聚簇键的局部性一致。由于 Lance 的存储格式在每个 Fragment 和 Page 级别维护了 **min-max 统计信息**，查询优化器可以在执行过滤时 **跳过不包含目标范围的 Fragment**，从而减少 `fragments_scanned` 和 `bytes_read`。

> **关键前提**：聚簇本身不会修改查询计划中的向量 ANN 路径，因此纯向量查询的收益预期为零；收益主要来自于 **伴随标量过滤的查询**。

---

## 3. 对照组设计（Baseline Framework）

| 组别 | 名称 | 数据状态 | 测试目的 |
|:---|:---|:---|:---|
| **A** | **No Clustering** | 原始数据，默认写入顺序，无聚簇，无相关索引 | 100% 基准线 |
| **B** | **Clustering Only** | 执行 `optimize(cluster_by=...)`, **不建任何标量/向量索引** | 测量纯聚簇收益 |
| **C** | **Index Only** | 在原始无序数据上，建立与聚簇键相关的标量索引 | 对比传统索引路径 |
| **D** | **Clustering + Index** | 先聚簇，再建立相同标量索引 | 协同效应 |
| **E** | **Clustering + Vector Index** | 先聚簇，再建立向量索引（IVF_PQ） | 向量场景收益 |

---

## 4. 数据集设计

### 4.1 数据规模

| 规模级别 | 行数 | 用途 |
|:---|:---|:---|
| **Small** | 100K | 快速验证测试逻辑正确性 |
| **Medium** | 1M | 主要性能测试 |
| **Large** | 10M+ | 验证收益随规模是否保持 |

### 4.2 数据 Schema

```python
schema = {
    "id": pa.int64(),
    "timestamp": pa.int64(),      # 聚簇候选键（1D时间）
    "lat": pa.float32(),          # 聚簇候选键（2D空间）
    "lng": pa.float32(),          # 聚簇候选键（2D空间）
    "category": pa.utf8(),        # 离散维度（3D/4D聚簇候选）
    "price": pa.float32(),        # 数值维度
    "embedding": pa.list_(pa.float32(), 128),  # 向量列
    "text": pa.utf8(),            # 全文搜索列
}
```

### 4.3 数据分布

| 分布类型 | 描述 | 测试目的 |
|:---|:---|:---|
| **Uniform** | 均匀分布 | 理想情况下的最优收益 |
| **Gaussian/Clustered** | 高斯聚集 | 验证真实世界空间/时间数据的表现 |
| **Skewed** | Zipf/幂律分布 | 验证热点区域查询的边界情况 |

---

## 5. 查询场景设计

### 5.1 标量查询场景

| 编号 | 查询类型 | Filter 示例 | 测试组 | 关键变量 |
|:---|:---|:---|:---|:---|
| S1 | 多维范围查询 | `lat BETWEEN 30 AND 35 AND lng BETWEEN 120 AND 125` | A, B, C, D | Selectivity: 0.1%, 1%, 10%, 50% |
| S2 | 1D范围查询 | `timestamp BETWEEN 1700000000 AND 1700100000` | A, B, C, D | 验证1D聚簇与多维聚簇在1D查询上的差异 |
| S3 | 点查询 | `lat = 31.23 AND lng = 121.47` | A, B, C, D | 命中 vs 未命中 |
| S4 | 全表聚合扫描 | `AVG(price)` | A, B | 验证聚簇是否引入负收益 |
| S5 | 混合条件查询 | `lat BETWEEN ... AND category = 'A'` | A, B, C, D | 部分聚簇键命中 |

### 5.2 向标融合查询场景

| 编号 | 查询类型 | 查询示例 | 测试组 | 关键变量 |
|:---|:---|:---|:---|:---|
| V1 | 纯向量 ANN | `search(embedding).limit(100)` | A, E | 验证纯向量查询无收益（对照） |
| V2 | 向量 + 多维预过滤 | `search(embedding).where("lat BETWEEN ... AND lng BETWEEN ...").limit(100)` | A, E | Selectivity: 0.1%, 1%, 10% |
| V3 | 向量 + 1D预过滤 | `search(embedding).where("timestamp BETWEEN ...").limit(100)` | A, E | 时间范围过滤 |
| V4 | 混合搜索 + 标量过滤 | `search(embedding).full_text_search("keyword").where("lat BETWEEN ...").limit(100)` | A, E | 验证混合搜索场景 |
| V5 | 暴力向量搜索（无索引） | `search(embedding).limit(100)`（无向量索引） | A, B | 无索引时聚簇对标量过滤的收益 |

---

## 6. 测量指标

### 6.1 物理层指标（通过 `scan_stats_callback` 采集）

| 指标 | 含义 | 聚簇收益判断依据 |
|:---|:---|:---|
| `bytes_read` | 实际读取的字节数 | **最直接指标**，聚簇有效时应显著下降 |
| `fragments_scanned` | 扫描的 Fragment 数量 | 反映数据跳过效率 |
| `rows_scanned` | 扫描的行数 | 与过滤条件的匹配度相关 |
| `iops` | IO 操作次数 | 间接反映随机/顺序读差异 |

### 6.2 端到端指标

| 指标 | 采集方式 | 说明 |
|:---|:---|:---|
| **Latency (P50/P95/P99)** | `time.perf_counter()` | 每个查询执行多次取平均 |
| **Throughput (QPS)** | 并发客户端压测 | 验证高并发下的收益是否保持 |
| **聚簇耗时** | `optimize()` 执行时间 | 数据组织成本 |
| **索引构建耗时** | `create_index()` 执行时间 | 可选，用于 TCO 计算 |
| **存储膨胀率** | 聚簇后大小 / 原始大小 | Lance Overwrite 模式预期无膨胀 |

---

## 7. 关键实现细节

### 7.1 `scan_stats_callback` 采集路径

由于 `lancedb.Table.search()` 高层 API 可能不支持直接绑定 `scan_stats_callback`，需要通过 `table.to_lance()` 获取底层 `lance.Dataset`：

```python
scanner = table.to_lance().scanner(filter="lat >= 30 AND lat <= 35")
stats = {}
scanner.scan_stats_callback = lambda s: stats.update({
    "bytes_read": s.bytes_read,
    "fragments_scanned": s.fragments_scanned,
})
scanner.to_table()
```

### 7.2 缓存控制

为避免 OS page cache 污染结果，每组测试前建议执行：

```bash
sync && echo 3 | sudo tee /proc/sys/vm/drop_caches
```

或每组使用独立的表目录进行进程隔离。

### 7.3 测试独立性

每组对照测试使用独立的表目录：
- `./benchmark_data/phase2/group_a/`
- `./benchmark_data/phase2/group_b/`
- 以此类推

---

## 8. 执行阶段

本性能测试按以下 6 个 Phase 推进：

1. **Phase 0**：文档落地 + 目录结构创建
2. **Phase 1**：标量查询 Benchmark 框架实现
3. **Phase 2**：标量查询测试执行与结论
4. **Phase 3**：向标融合查询测试
5. **Phase 4**：边界条件与多维对比
6. **Phase 5**：汇总报告与最终提交

每阶段完成后将单独提交并推送，经确认后再进入下一阶段。

---

## 9. 预期核心结论

| 结论项 | 预期结果 |
|:---|:---|
| 纯聚簇在低 selectivity 标量查询上收益显著 | B 组 Speedup > 5× |
| 高 selectivity 下收益稀释 | B 组 Speedup 趋近 1× |
| 聚簇 vs 索引存在 trade-off | B 无索引存储/维护成本，C 有索引优化但需额外开销 |
| 聚簇+索引有协同效应 | D 组优于单独的 B 或 C |
| 纯向量查询聚簇无收益 | E 组 vs A 组 Speedup ≈ 1× |
| 向量+标量过滤场景聚簇收益明显 | E 组在 V2/V3 上 Speedup > 2× |
