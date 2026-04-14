---
name: LanceDB Clustering Phase 3
description: Phase 3 complete - Hilbert curve multi-dimensional clustering (2-4D)
type: project
originSessionId: f2184b1a-66e3-4a0d-9a56-42f2025f4801
---

# LanceDB 多维聚簇功能 - Phase 3 记忆摘要

**创建时间**: 2026-04-14
**用途**: Phase 3 Hilbert 多维聚簇开发上下文

---

## 当前状态

- **阶段**: Phase 3 (Hilbert 多维聚簇) - ✅ 已完成
- **所有任务完成**: Hilbert 算法集成、执行层通用化、2-4D 集成测试
- **测试统计**: 34个测试 + 1个文档测试全部通过

---

## Phase 3 完成总结

### 1. Hilbert 曲线算法 (`table/cluster/algorithm.rs`)

**库选择**: 使用 `hilbert_index` crate（支持 2-4 维）
- 设计文档原建议 `hilbert_curve`，但该 crate 仅支持 2D
- `hilbert_index` 提供 `ToHilbertIndex` trait：`[usize; D].to_hilbert_index(level) -> usize`

**`HilbertCurveAlgorithm` 结构**:
- `dimension`: 维度数 (2-4)
- `bits`: 坐标量化位数，默认 16（可通过 `algorithm_params` 配置）
- `bounds`: `RwLock<Vec<(f64, f64)>>`，存储各维度的 min/max

**执行流程**:
1. `prepare`: 扫描所有 batches 计算各维度 min/max
2. `compute_sort_key`:
   - `ScalarValue` -> `f64`
   - 线性归一化到 `[0, 2^bits-1]`
   - 构建 `[usize; D]` 坐标数组
   - 调用 `to_hilbert_index(bits)` 生成 `usize`
   - 转为 big-endian 字节作为字典序排序键

**安全校验**:
- 运行时检查 `dimension * bits <= 64`，防止 `usize` 溢出

### 2. 执行层通用化 (`table/cluster/execute.rs`)

**`sort_batch_by_algorithm`**:
- 基于 `BinaryBuilder` 为每行构建 sort key
- 使用 `ScalarValue::try_from_array` 从 Arrow array 提取标量值
- NULL 行标记为 BinaryArray 的 null，排序后放在末尾
- 调用 `sort_to_indices(nulls_last)` 重排所有列

**`execute_cluster_direct` 重构**:
- 签名改为接受 `algorithm: &dyn ClusteringAlgorithm`
- 数据收集后调用 `algorithm.prepare`
- 统一支持 DirectSort（1D）和 Hilbert（2-4D）

### 3. 算法选择 (`optimize.rs`)

**变更**:
- 移除了 `config.keys.len() != 1` 的 "not yet implemented" 错误
- 自动调用 `get_algorithm(config.keys.len(), Some(&config.algorithm), ...)`
- 1D -> `DirectSortAlgorithm`
- 2-4D -> `HilbertCurveAlgorithm`

### 4. `ClusteringAlgorithm` trait 扩展

在 `mod.rs` 中增加了 `prepare` 方法（默认空实现），允许算法在执行前扫描全量数据：

```rust
pub trait ClusteringAlgorithm: Send + Sync {
    fn name(&self) -> &str;
    fn prepare(&self, _batches: &[RecordBatch], _keys: &[String]) -> Result<()> {
        Ok(())
    }
    fn compute_sort_key(&self, values: &[ScalarValue]) -> Result<Vec<u8>>;
}
```

### 5. 新增测试 (10个)

**单元测试** (algorithm.rs):
| 测试 | 描述 |
|------|------|
| `test_hilbert_curve_algorithm_2d` | 2x2 网格验证 Hilbert 顺序 |
| `test_hilbert_curve_algorithm_3d` | 2x2x2 网格验证 8 个 key 互异 |
| `test_hilbert_sort_key_locality` | 远近点产生不同 key |
| `test_hilbert_algorithm_with_null_values` | NULL 值返回空 key |
| `test_get_algorithm` | 1D->direct, 2-4D->hilbert |

**执行层单元测试** (execute.rs):
| 测试 | 描述 |
|------|------|
| `test_sort_batch_by_algorithm_direct` | 1D direct sort 通过通用接口工作 |
| `test_sort_batch_by_algorithm_hilbert_2d` | 2D Hilbert 通过通用接口排序 |
| `test_sort_batch_by_algorithm_with_nulls` | NULL 行放末尾 |

**集成测试** (mod.rs):
| 测试 | 描述 |
|------|------|
| `test_create_table_with_multidimensional_cluster` | 2D 表创建 + cluster 成功（替换旧报错测试） |
| `test_cluster_2d_hilbert_basic` | 4 点 2D 聚簇，验证 sort key 单调递增 |
| `test_cluster_3d_hilbert_basic` | 8 点 3D 聚簇，验证单调递增 |
| `test_cluster_4d_hilbert_basic` | 16 点 4D 聚簇，验证单调递增 |
| `test_cluster_2d_hilbert_with_index` | 2D 聚簇 + BTree 索引自动重建 |

### 6. 文档更新

- `CLUSTERING_DESIGN_v1.md`: 将 `hilbert_curve` 更正为 `hilbert_index`
- `CLUSTERING_REQUIREMENTS_v1.md`: 同步更新 crate 名称
- `CLUSTERING_PROGRESS.md`: 添加 Phase 3 完成总结和测试统计

---

## 关键设计决策

### 为什么用 `hilbert_index` 替代 `hilbert_curve`?

- `hilbert_curve` 仅支持 2D，而需求要求 2-4 维
- `hilbert_index` 通过 const generics 支持任意维度，API 简洁

### 为什么需要 `prepare` 步骤?

- Hilbert 归一化需要全局 min/max
- DirectSort 不需要，因此 trait 提供默认空实现
- 使用 `RwLock` 避免 trait object 的可变引用问题

### NULL 行处理方式

- 任意聚簇键为 NULL 时，整行 sort key 为空
- `sort_to_indices(nulls_last)` 确保 NULL 行排在末尾
- 这与 1D DirectSort 的行为保持一致

---

## 测试统计

| 类型 | 数量 | 说明 |
|------|------|------|
| 单元测试 | 20个 | algorithm 15 + execute 5 |
| 集成测试 | 14个 | Phase 0-3 累积 |
| 文档测试 | 1个 | `cluster_by` 文档 |
| **总计** | **35个** | **全部通过** |

---

## 快速参考

```bash
# 运行聚簇测试
cargo test --quiet --features remote -p lancedb table::cluster

# 当前测试数: 35个全部通过
```

---

## 下一步

Phase 3 已完成，准备进入 Phase 4:
- **Phase 4 目标**: Python 绑定
- **核心任务**: PyO3 方法绑定、`AsyncTable`/`Table` 扩展、Python 集成测试

---

## 相关提交

- (待添加 Phase 3 提交)
