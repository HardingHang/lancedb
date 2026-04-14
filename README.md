# LanceDB 多维聚簇 (Multi-Dimensional Clustering)

**开发分支**: `feature/multi-dimensional-clustering`  
**基础仓库**: [LanceDB](https://github.com/lancedb/lancedb)  
**状态**: ✅ Phase 0-6 全部完成

---

## 项目简介

本项目为 **LanceDB** 增加了**多维聚簇 (Clustering)** 能力。通过在建表时指定聚簇键 (clustering keys)，用户可以让数据在物理存储上按照指定的列进行排序，从而显著提升范围查询 (range query) 和数据跳过 (data skipping) 的性能。

- **1 维聚簇**: 使用直接排序 (`direct`) 算法
- **2-4 维聚簇**: 使用 **Hilbert 曲线** (`hilbert`) 算法，在多维空间中保持局部性

聚簇操作执行后，系统会自动重写全表数据，并按排序后的顺序重新写入；同时自动重建所有已有索引，确保查询性能不受影响。

---

## 核心特性

| 特性 | 说明 |
|------|------|
| **1-4D 多维聚簇** | 支持 1 维直接排序和 2-4 维 Hilbert 曲线聚簇 |
| **自动索引重建** | Cluster 操作后自动重建原有索引，失败时通过 Lance 版本系统回滚 |
| **流式分块写入** | 支持 `target_rows_per_fragment` 控制分块大小，避免大数据集 OOM |
| **事务原子性** | 利用 Lance 不可变数据结构和版本系统，聚簇失败不会破坏原数据 |
| **多语言绑定** | Rust 核心 + Python (PyO3) + Node.js (napi-rs) 完整 API 暴露 |
| **NULL 值处理** | 聚簇键为 NULL 的行自动排在末尾，数据完整性保持 |

---

## 快速开始

### Rust

```rust
use lancedb::connect;
use lancedb::table::OptimizeAction;

let db = connect("memory://").execute().await?;

// 创建带聚簇配置的表
let table = db
    .create_table("my_table", batch)
    .cluster_by(&["id"])
    .execute()
    .await?;

// 查询聚簇配置
let config = table.cluster_config().await?;
assert_eq!(config.unwrap().algorithm, "direct");

// 执行聚簇优化
let stats = table
    .optimize(OptimizeAction::Cluster {
        full: true,
        target_rows_per_fragment: Some(100_000),
    })
    .await?;

println!("rows_processed: {:?}", stats.cluster.unwrap().rows_processed);
```

### Python

```python
import lancedb

db = lancedb.connect("memory://")

# 创建带聚簇配置的表
table = db.create_table("my_table", data, cluster_by=["id"])

# 查询聚簇配置
print(table.cluster_config())
# => {"keys": ["id"], "algorithm": "direct", "algorithm_params": None}

# 执行聚簇优化
stats = table.cluster(target_rows_per_fragment=100_000)
print(stats["rows_processed"])
```

### Node.js / TypeScript

```typescript
import * as lancedb from "@lancedb/lancedb";

const db = await lancedb.connect("memory://");

// 创建带聚簇配置的表
const table = await db.createTable("my_table", data, { clusterBy: ["id"] });

// 查询聚簇配置
const config = await table.clusterConfig();
console.log(config?.algorithm); // "direct"

// 执行聚簇优化
const stats = await table.cluster({ targetRowsPerFragment: 100_000 });
console.log(stats.rowsProcessed);
```

---

## 支持的聚簇键类型

- **数值类型**: `Int8` ~ `Int64`, `UInt8` ~ `UInt64`, `Float32`, `Float64`
- **时间类型**: `Timestamp` (各精度), `Date32`, `Date64`
- **维度限制**: 1 ~ 4 列
- **NULL 处理**: 允许，NULL 值排在末尾

---

## 架构概览

```
rust/lancedb/src/table/cluster/
├── mod.rs        # ClusterConfig / ClusterStats / ClusteringAlgorithm trait
├── algorithm.rs  # DirectSortAlgorithm + HilbertCurveAlgorithm
└── execute.rs    # execute_cluster_direct + RecordBatchChunkIterator
```

```
rust/lancedb/src/table/optimize.rs
└── execute_cluster()  # 索引捕获 -> 数据重写 -> 自动重建索引 -> 失败回滚
```

**配置持久化**: 聚簇配置以 JSON 格式存储在表 schema 的 metadata 中，键名为 `lancedb.cluster.config`。

---

## 开发阶段

| 阶段 | 状态 | 主要内容 |
|------|------|---------|
| Phase 0 | ✅ 完成 | 1 维聚簇 MVP：配置管理、直接排序、建表 API |
| Phase 1 | ✅ 完成 | 健壮性：边界测试、流式处理、事务原子性 |
| Phase 2 | ✅ 完成 | 索引重建：自动重建 + 失败回滚 |
| Phase 3 | ✅ 完成 | Hilbert 算法：2-4 维聚簇实现 |
| Phase 4 | ✅ 完成 | Python 绑定暴露 |
| Phase 5 | ✅ 完成 | Node.js 绑定暴露 |
| Phase 6 | ✅ 完成 | 文档完善、代码清理、最终验证 |

---

## 测试覆盖

- **Rust 聚簇测试**: 34 个通过 ✅
- **Rust optimize 测试**: 13 个通过 ✅
- **Python 集成测试**: 7 个聚簇测试通过 ✅
- **Node.js 集成测试**: 5 个聚簇测试通过 ✅
- **总计**: 46 个聚簇专用测试全部通过

### 关键测试场景

- 空表聚簇
- 全 NULL 值聚簇
- 大数据集流式分块 (`target_rows_per_fragment`)
- 事务原子性 (`checkout` 回溯验证)
- 单索引 / 多索引自动重建
- 2D / 3D / 4D Hilbert 曲线排序正确性

---

## 已知限制

1. **增量聚簇** (`full: false`) 尚未支持，当前仅支持全局重写 (`full: true`)。
2. **超大数据集**的全外部排序尚未实现；当前会先将所有 batches 收集到内存后再排序，极端大数据集可能受内存限制。
3. **远程表** (`RemoteTable`) 的聚簇功能暂不支持；`cluster_config()` 返回 `None`。

---

## 相关文件

- 进度详情: [`CLUSTERING_PROGRESS.md`](CLUSTERING_PROGRESS.md)
- 核心配置/算法: `rust/lancedb/src/table/cluster/`
- 优化执行: `rust/lancedb/src/table/optimize.rs`
- Python 绑定: `python/src/table.rs`, `python/src/connection.rs`
- Node.js 绑定: `nodejs/src/table.rs`, `nodejs/src/connection.rs`

---

## 许可证

本项目基于 LanceDB 的许可证 ([Apache-2.0](LICENSE))。
