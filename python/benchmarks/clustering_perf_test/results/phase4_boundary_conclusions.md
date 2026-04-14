# Phase 4 边界条件与多维对比结论

## 1. 测试执行摘要

| 测试项 | 状态 | 备注 |
|:---|:---|:---|
| 1D Uniform Medium | ✅ 完成 | 聚簇键：`timestamp` |
| 2D Uniform Medium | ✅ 完成 | 聚簇键：`lat`, `lng` |
| 3D Uniform Medium | ✅ 完成 | 聚簇键：`lat`, `lng`, `category_id` |
| 4D Uniform Medium | ✅ 完成 | 聚簇键：`lat`, `lng`, `category_id`, `timestamp` |
| 2D Gaussian Medium | ✅ 完成 | 高斯聚集分布 |
| 2D Skewed Medium | ✅ 完成 | Zipf/幂律分布 |
| 2D Uniform Large (10M) | ❌ 失败 | OOM (exit code 137)，内存不足 |

---

## 2. 多维对比：维度越高，2D 查询收益越低（S1）

### 2.1 D 组（聚簇+索引）在 S1（2D Range `lat/lng`）中的表现

| 维度 | 0.1% Speedup | 0.1% IO Reduction | 10% Speedup | 10% IO Reduction |
|:---|---:|---:|---:|---:|
| **1D** (`timestamp`) | 0.95x | 1.00x | 0.91x | 1.37x |
| **2D** (`lat/lng`) | **1.88x** | **79.56x** | **5.53x** | **5.51x** |
| **3D** (+`category_id`) | 1.55x | 30.26x | 4.85x | 6.75x |
| **4D** (+`timestamp`) | 1.17x | 11.63x | 4.01x | 9.59x |

**核心发现**：
1. **1D 聚簇对 2D Range 查询几乎无帮助**：聚簇键是 `timestamp`，与 `lat/lng` 无关，符合预期。
2. **2D 聚簇是最优解**：在 2D Range 查询上达到 80x IO 减少，收益峰值最高。
3. **3D 聚簇仍有显著收益**：0.1% selectivity 下仍有 30x IO 减少，说明加入 `category_id` 后 Hilbert 曲线对 `lat/lng` 的局部性保持尚可。
4. **4D 聚簇收益明显下降**：0.1% selectivity 下仅 11.6x IO 减少。增加第 4 维 `timestamp` 后，Hilbert 曲线在 4D 空间中的逼近误差增大，`lat/lng` 的物理局部性被稀释。

### 2.2 B 组（纯聚簇）在 S1 中的表现

| 维度 | 10% Speedup | 10% IO Reduction |
|:---|---:|---:|
| 1D | 0.95x | 1.37x |
| 2D | **5.61x** | 1.56x |
| 3D | 4.77x | 1.21x |
| 4D | 5.27x | 1.15x |

纯聚簇的 latency speedup 在 2D-4D 之间差异不大（4.8x-5.6x），但 4D 的 IO reduction 已经降到 1.15x，说明**顺序局部性仍是主要收益来源**，但 4D 下的数据跳过能力已显著弱化。

---

## 3. 多维对比：1D 查询的收益与聚簇键命中强相关（S2）

S2 过滤的是 `timestamp`，观察 D 组在 10% selectivity 下的表现：

| 维度 | 10% Speedup | 10% IO Reduction | 原因 |
|:---|---:|---:|:---|
| **1D** | **14.82x** | **2.87x** | 聚簇键完全命中 `timestamp` |
| 2D | 0.95x | 0.98x | `timestamp` 不在聚簇键中 |
| 3D | 1.04x | 0.98x | `timestamp` 不在聚簇键中 |
| **4D** | **8.58x** | **2.01x** | `timestamp` 被纳入 4D 聚簇键 |

**核心发现**：
- 1D Direct Sort 对 1D 查询的收益极为惊人（14.8x 提速），甚至高于 2D Hilbert 对 2D 查询的收益（5.5x）。
- 4D 聚簇重新包含 `timestamp` 后，S2 的收益恢复到了 8.6x，但略低于 1D 的 14.8x。这说明 Hilbert 曲线在 4D 下对尾端维度的局部性保持不如 Direct Sort 在 1D 下的精确。

---

## 4. 分布对比：Gaussian 分布带来极端收益，Skewed 分布收益被稀释

### 4.1 S1（2D Range）中 D 组的表现对比

| 分布 | 0.1% Speedup | 0.1% IO Reduction | 10% Speedup | 10% IO Reduction |
|:---|---:|---:|---:|---:|
| **Uniform** | 1.88x | 79.56x | 5.53x | 5.51x |
| **Gaussian** | **2.24x** | **552.10x** | 5.62x | 5.95x |
| **Skewed** | 1.88x | 2.45x | 1.88x | 2.46x |

### 4.2 Gaussian：聚簇的"甜蜜点"分布

Gaussian 分布下，0.1% selectivity 的 D 组达到了惊人的 **552x IO 减少**（A 组读 32MB，D 组只读 58KB）。

原因：
- 高斯数据高度聚集在均值附近（`lat=0, lng=0`）
- 0.1% 的范围查询恰恰落在这个高密度区域
- 聚簇后，这些高密度行被压缩在极少的连续 fragment 中
- 查询引擎几乎可以跳过 99.8% 的 fragment

这比 Uniform 分布下的 80x 还要高一个数量级，说明**真实世界的空间/时间聚集数据（如城市热点）是聚簇收益的最优场景**。

### 4.3 Skewed：原始数据本身已聚集，聚簇收益被严重稀释

Skewed 分布（Zipf）下的结果非常特殊：

| Selectivity | A 组 Bytes Read | D 组 Bytes Read | IO Reduction |
|:---|---:|---:|---:|
| 0.1% | 525,911,721 | 213,612,229 | 2.46x |
| 10.0% | 525,350,863 | 213,612,229 | 2.46x |
| 50.0% | 525,863,991 | 213,612,229 | 2.46x |

三个关键异常：
1. **A 组在所有 selectivity 下都读了约 500MB**：说明 Zipf 分布下，即使 0.1% selectivity，原始无序数据也无法通过 metadata 有效跳过 fragment。因为热门值散布在很多 fragment 中。
2. **D 组的 bytes_read 在所有 selectivity 下几乎相同（213MB）**：聚簇后，热点数据被集中，但非热点数据也形成了大片连续区域。无论查询哪个范围，都需要读取热点区 + 部分非热点区。
3. **S3 点查询在 A 组读了 525MB**：这是所有分布中唯一出现"点查询读全表"的情况。因为 Zipf 的热门值集中在几个 category/lat 组合上，这些组合在原始数据中随机分布，导致几乎每个 fragment 都"可能包含"这个热门值，metadata 无法跳过。

**结论**：Skewed 分布下，数据本身的热点聚集特性导致：
- 未聚簇时查询极慢（A 组几乎总是全表扫描）
- 聚簇后有所改善（约 2.3-2.5x），但远不如 Uniform/Gaussian
- 这说明 **Zipf 分布下的聚簇优化存在天然上限**：热点数据无论如何都要读

---

## 5. Large 规模测试失败：10M 行触发 OOM

2D Uniform Large（10,000,000 行）测试在执行 `table.cluster()` 阶段被系统 kill（exit code 137 = SIGKILL），原因是**内存不足**。

这意味着：
1. 当前测试环境的内存（WSL2 默认分配）不足以支撑 10M 行的聚簇操作
2. `cluster()` 操作需要将排序后的数据全部加载到内存中再进行分片写入
3. 在 10M 行 × 128 维向量 + 其他列的情况下，峰值内存需求可能超过 8-16GB

**这是否是 LanceDB 的 bug？**
- 不一定是 bug，但暴露了聚簇操作在内存管理上的限制：当前实现似乎是**全内存排序**，而非外部排序（external sort）。
- 对于生产环境的大数据集，这可能是一个严重的可扩展性瓶颈。

**建议**：
- 在文档中明确标注聚簇操作的内存需求与数据规模的对应关系
- 考虑在 Rust 核心中实现流式/外部排序，以支持亿级数据的聚簇

---

## 6. 多维聚簇不支持 Utf8 类型

3D/4D 测试的最初版本使用 `category`（Utf8）作为聚簇键，直接报错：

```
ValueError: Invalid input, Type 'Utf8' is not supported for clustering
```

修复方案：
- 数据生成器中增加 `category_id`（int32）列
- 3D/4D 聚簇键改为 `category_id`

**产品启示**：
- 当前多维聚簇仅支持数值类型（int, float, timestamp）
- 对于字符串维度的聚簇需求，用户需要手动编码为整数
- 应在 API 文档和错误提示中明确说明此限制

---

## 7. 边界条件总结与推荐

### 7.1 维度选择

| 查询模式 | 推荐维度 | 原因 |
|:---|:---|:---|
| 主要按时间过滤 | 1D (`timestamp`) | Direct Sort 精确高效，1D 查询可达 15x 提速 |
| 主要按空间过滤 | 2D (`lat/lng`) | Hilbert 曲线最优，收益峰值最高 |
| 混合空间+离散类别 | 3D (`lat/lng/category_id`) | 仍有显著收益，但比 2D 下降约 50% |
| 时间+空间+类别都要查 | 4D | 收益进一步稀释，仅当所有维度都频繁过滤时才考虑 |

### 7.2 数据分布

| 分布 | 聚簇收益 | 适用场景 |
|:---|:---|:---|
| **Gaussian** | ⭐⭐⭐⭐⭐ 极高 | 城市热点、用户活动聚集区、时间窗口聚集 |
| **Uniform** | ⭐⭐⭐⭐ 高 | 理想基准，收益稳定可预期 |
| **Skewed/Zipf** | ⭐⭐ 中等 | 热点数据本身就需要读，聚簇优化空间有限 |

### 7.3 规模限制

| 规模 | 可行性 | 备注 |
|:---|:---|:---|
| 100K (Small) | ✅ 完全可行 | 秒级聚簇 |
| 1M (Medium) | ✅ 完全可行 | 分钟级聚簇，收益显著 |
| 10M (Large) | ⚠️ 内存受限 | 当前环境 OOM，需更多内存或流式排序支持 |

---

## 8. 待验证假设（未在本次测试中回答）

1. **Large 规模的实际收益曲线**：10M 行下，低 selectivity 的 IO 减少是否比 1M 行更夸张？fragment 数量增加是否会让跳过效应更显著？
2. **Gaussian 分布在 3D/4D 下的收益**：高斯聚集是否能缓解 4D Hilbert 的局部性稀释？
3. **向量查询在 Large 规模下聚簇是否转正**：当标量过滤成本放大后，E 组是否能跑赢 F 组？
