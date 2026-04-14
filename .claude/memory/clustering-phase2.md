---
name: LanceDB Clustering Phase 2
description: Phase 2 complete - automatic index rebuild after clustering operation
type: project
originSessionId: f2184b1a-66e3-4a0d-9a56-42f2025f4801
---
# LanceDB 多维聚簇功能 - Phase 2 记忆摘要

**创建时间**: 2026-04-14
**用途**: Phase 2 索引重建开发上下文

---

## 当前状态

- **阶段**: Phase 2 (索引重建) - ✅ 已完成
- **所有任务完成**: 索引捕获、自动重建、失败回滚、集成测试
- **测试统计**: 24个测试 + 1个文档测试全部通过

---

## Phase 2 完成总结

### 1. 索引重建架构 (`table/optimize.rs`)

**执行流程** (`execute_cluster`)：
1. 数据重写**之前**捕获当前所有索引配置 (`table.list_indices().await`)
2. 记录操作前的 dataset 版本号 (`dataset.version().version`)，用于失败回滚
3. 调用 `execute_cluster_direct` 完成数据全局排序和重写 (`WriteMode::Overwrite`)
4. 调用 `rebuild_indices` 逐个删除旧索引并创建新索引
5. 更新 `ClusterStats.indices_rebuilt` 统计

### 2. 索引重建实现 (`rebuild_indices`)

```rust
async fn rebuild_indices(table: &NativeTable, indices: Vec<IndexConfig>) -> Result<usize>
```

**关键设计**:
- 删除旧索引时忽略 "not found" 错误（因为 `WriteMode::Overwrite` 后旧索引可能已从 manifest 中移除）
- 使用 `crate::index::IndexBuilder` 重建索引，保留原始列名和索引名称
- Scalar 索引（BTree/Bitmap/LabelList/FTS）精确重建
- Vector 索引使用 `Index::Auto` 回退重建

### 3. Vector 索引的限制

**根本原因**: Lance 4.0 的 manifest 不保存 vector index 的完整构建参数：
- `num_partitions`
- `num_sub_vectors`
- `num_bits`
- `distance_type` 等

**当前方案**: `Index::Auto` 回退，让 Lance 自动推断最佳参数。

**未来改进**: 若 Lance 后续版本在 `IndexMetadata` 中保存完整参数，可改为精确重建。

### 4. 失败回滚机制

```rust
match rebuild_indices(table, indices).await {
    Ok(rebuilt) => { stats.indices_rebuilt = rebuilt; }
    Err(e) => {
        log::error!("Index rebuild failed...");
        if let Err(rollback_err) = table.dataset.as_time_travel(version_before).await {
            log::error!("Failed to rollback...");
        }
        return Err(e);
    }
}
```

**回滚方式**: 利用 Lance 版本系统，通过 `as_time_travel(version_before)` 切回旧版本。

### 5. 新增测试 (2个)

| 测试 | 描述 | 验证点 |
|------|------|--------|
| `test_cluster_rebuilds_btree_index` | 单 BTree 索引 | Cluster 后索引存在、数据有序、`num_indexed_rows=3` |
| `test_cluster_rebuilds_multiple_indices` | 多个 BTree 索引 | `indices_rebuilt=2`、所有索引覆盖全部行 |

---

## 关键设计决策

### 为什么不在 `execute_cluster_direct` 内做索引重建？

**分层原则**:
- `execute_cluster_direct` 是纯执行层，只负责数据排序和重写
- `execute_cluster`（调用方）负责编排：配置读取 → 数据重写 → 索引重建
- 这样保持了 `cluster` 模块的独立性，避免耦合索引逻辑

### 为什么不使用 `optimize_indices` 而是 `drop + recreate`？

**对比**:
- `optimize_indices`：只对现有索引做增量优化，不处理 row_id 变化后的失效问题
- `drop + recreate`：彻底重建，确保索引基于新的物理布局和 row_ids

**结论**: Cluster 操作改变了数据的物理布局和 row_ids，必须完全重建索引。

---

## 测试统计

| 类型 | 数量 | 新增 |
|-----|------|------|
| 单元测试 | 12个 | 0 |
| 集成测试 | 11个 | +2 |
| 文档测试 | 1个 | 0 |
| **总计** | **24个** | **+2** |

---

## 快速参考

```bash
# 运行聚簇测试
cargo test --quiet --features remote -p lancedb table::cluster

# 当前测试数: 24个全部通过
```

---

## Phase 3 准备

Phase 2 已完成，准备进入 Phase 3:
- **Phase 3 目标**: Hilbert 算法 - 多维聚簇 (2-4维)
- **核心任务**: 集成 `hilbert_curve` crate，实现多维归一化
- **技术点**: `hilbert_curve` crate、简单线性归一化、算法注册表

---

## 相关提交

- `8812d40e`: feat: implement Phase 2 index rebuild for clustering
