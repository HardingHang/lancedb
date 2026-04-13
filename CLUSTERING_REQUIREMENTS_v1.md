# LanceDB 多维聚簇功能 - 需求澄清文档（终版）

**版本**: v1.0-final  
**日期**: 2026-04-11  
**状态**: 已确认，进入设计阶段

---

## 1. 功能概述

多维聚簇是一种**纯物理数据布局优化**，通过按指定列排序数据，提升范围查询性能。

**特点**:
- 纯物理优化，无 fragment-level 元信息维护
- 类似 ClickHouse `ORDER BY`，仅保证数据有序
- 查询利用现有 min-max statistics 过滤

---

## 2. 功能范围

### 2.1 包含的功能
| 功能 | 说明 |
|------|------|
| 建表时指定聚簇键 | 支持指定多个列作为聚簇键 |
| 查询聚簇配置 | 支持获取当前聚簇配置 |
| Hilbert/Z-Order 排序 | 多维数据按空间填充曲线排序 |
| 可插拔算法 | 提供接口支持更换排序算法 |
| 全局重写 | `OptimizeAction::Cluster { full: true }` 重写全部数据 |
| 自动重建索引 | Cluster 完成后自动重建索引 |
| 查询优化 | 利用现有 min-max statistics 进行 fragment 过滤 |

### 2.2 预留但不实现的功能
| 功能 | 行为 |
|------|------|
| 增量聚簇 | 接口预留，调用时返回 `NotImplemented` 错误 |
| Fragment 级聚簇元信息 | 不维护任何 fragment-level 聚簇状态 |
| 取消聚簇 | 不支持删除或禁用聚簇 |

### 2.3 不包含的功能
| 功能 | 说明 |
|------|------|
| 写时聚簇 | 写入时不自动排序 |
| 聚簇键变更 | 建表后不可变更 |
| DDL 冲突处理 | 不处理聚簇键列被删除/修改类型的情况 |
| 自动聚簇维护 | 不自动检测或修复乱序 |

---

## 3. 聚簇键定义

### 3.1 API

```rust
// 建表时指定聚簇键
db.create_table("my_table", data)
    .cluster_by(&["col1", "col2"])  // 指定聚簇键
    .cluster_algorithm("hilbert")    // 可选，默认 hilbert
    .execute()
    .await?;

// 查询聚簇配置
let config = table.cluster_config().await?;
// 返回: Some(ClusterConfig { keys: ["A", "B"], algorithm: "hilbert" })
// 或: None (未设置聚簇)

// 不支持 set_cluster_keys，聚簇键不可变更
// 不支持取消聚簇
```

### 3.2 约束与校验

**建表时校验**（异常时返回 `InvalidInput` 错误）:

| 校验项 | 规则 | 错误信息 |
|--------|------|---------|
| 重复列名 | 聚簇键列名不可重复 | `"Duplicate clustering key column: {col}"` |
| 空列名 | 列名不可为空 | `"Empty clustering key column name"` |
| 列存在性 | 聚簇键列必须存在于 schema | `"Clustering key column '{col}' not found in schema"` |
| 列类型 | 仅支持数值和时间类型 | `"Type '{type}' is not supported for clustering"` |
| 维度数量 | 1-4 个聚簇键 | `"Clustering keys count {n} exceeds maximum of 4"` |
| NULL 值 | 警告但不报错 | 日志: `"{n} rows with NULL values in clustering keys will be excluded from clustering"` |

### 3.3 数据类型支持

| 类型 | 支持 | 说明 |
|------|------|------|
| 整数 | ✅ | i8, i16, i32, i64, u8, u16, u32, u64 |
| 浮点数 | ✅ | f32, f64 |
| 时间类型 | ✅ | Timestamp, Date |
| 字符串 | ❌ | 当前版本不支持 |
| 布尔 | ❌ | 当前版本不支持 |
| NULL | ⚠️ | 含 NULL 的行不参与排序，放在末尾 |

---

## 4. 聚簇算法

### 4.1 算法选择

| 维度数 | 算法 | 说明 |
|--------|------|------|
| 1维 | 直接排序 | 按该列值升序排序 |
| 2-4维 | Hilbert曲线 | 使用 hilbert_curve crate |

### 4.2 可插拔接口

```rust
pub trait ClusteringAlgorithm: Send + Sync {
    fn name(&self) -> &str;
    
    /// 计算排序键值（用于数据排序）
    fn compute_sort_key(&self, values: &[ScalarValue]) -> Vec<u8>;
}
```

---

## 5. 元数据存储

### 5.1 表级元数据（必须维护）

存储位置：Manifest `schema_metadata`

| 字段 | 类型 | 说明 |
|------|------|------|
| `lancedb.cluster.keys` | JSON array | 聚簇键列名，如 `["A","B"]` |
| `lancedb.cluster.algorithm` | string | 算法名称，如 `"hilbert"` |
| `lancedb.cluster.algorithm_params` | JSON object | 算法参数，如 `{"bits":16}` |

```rust
// 建表时写入 schema_metadata
dataset.schema_metadata["lancedb.cluster.keys"] = "[\"A\",\"B\"]";
dataset.schema_metadata["lancedb.cluster.algorithm"] = "hilbert";
```

### 5.2 Fragment 级元数据（不维护）

- ❌ 不记录 fragment 是否已聚簇
- ❌ 不记录 fragment 的 config_version
- ❌ 不记录 fragment 的聚簇状态

查询时所有 fragment 一视同仁，仅利用现有 min-max statistics。

---

## 6. Optimize 接口

### 6.1 扩展 OptimizeAction

```rust
pub enum OptimizeAction {
    // ... 现有变体 ...
    
    /// 聚簇优化
    Cluster {
        /// true: 全局重写（实际实现）
        /// false: 增量重写（预留接口，报错）
        full: bool,
        /// 目标行数 per fragment
        target_rows_per_fragment: Option<usize>,
    },
}
```

### 6.2 行为定义

| full 值 | 行为 |
|---------|------|
| `true` | **全局重写**：读取所有数据，按聚簇键排序，重写所有 fragments，**自动重建索引** |
| `false` | **预留接口**：返回 `Error::NotImplemented("Incremental clustering not supported in this version")` |

### 6.3 使用示例

```rust
// 全局重写（唯一可用模式）
table.optimize(OptimizeAction::Cluster {
    full: true,
    target_rows_per_fragment: Some(100000),
}).await?;

// 增量模式（报错）
table.optimize(OptimizeAction::Cluster {
    full: false,
    target_rows_per_fragment: None,
}).await?;
// 返回: Err(NotImplemented("Incremental clustering not supported"))
```

### 6.4 与 Compact 的关系

- **完全独立**：Cluster 和 Compact 互不影响
- 执行顺序由用户决定：
  ```
  推荐流程: 写入数据 → Compact（可选）→ Cluster（全局重写）
  ```
- Compact 不维护聚簇状态，可能破坏已有排序

### 6.5 与索引的关系

**Cluster 操作自动重建索引**:

```rust
// Cluster 流程:
1. 获取当前所有索引信息
2. 执行数据排序和重写
3. 删除旧索引文件
4. 使用原始索引参数重建所有索引
5. 提交新版本
```

**注意**:
- 索引重建可能耗时较长
- 重建后的索引与原始索引等价（相同的列和参数）
- 如果索引重建失败，整个 Cluster 操作回滚

---

## 7. 查询优化

### 7.1 优化机制

**利用现有 min-max statistics**（Lance 自动维护）:

```
聚簇键: [A, B]
数据按 Hilbert(A, B) 排序存储

查询: WHERE A > 10 AND A < 100

优化过程:
1. Lance 自动检查各 fragment 的 A 列 min/max
2. 跳过 A.max < 10 或 A.min > 100 的 fragments
3. 读取潜在匹配的 fragments，再过滤
```

### 7.2 不做的优化

- ❌ 不使用 Hilbert 值范围过滤
- ❌ 不维护 fragment-level 聚簇标记
- ❌ 不区分"已聚簇"和"未聚簇"的 fragments

### 7.3 效果预期

- 全局 Cluster 后：数据物理有序，min-max 过滤效果最佳
- 继续写入后：新数据无序，min-max 过滤效果下降
- 用户责任：根据需要重新执行 `Cluster { full: true }`

---

## 8. 版本控制

Lance 格式内置**版本控制**，Cluster 操作的行为：

```
版本 1: 初始数据
版本 2: 添加了索引
版本 3: Cluster 操作（数据重写，产生新版本）
版本 4: 新写入数据
```

**checkout 旧版本**:
- 用户可以 `checkout(2)` 回到 Cluster 之前的状态
- 版本 2 的数据是未聚簇状态
- 这是预期行为，符合 Lance 的版本语义

**版本升级**:
- 从旧版本写入新数据时，遵循当前版本的聚簇配置
- 不会自动应用 Cluster 到新数据

---

## 9. 边界情况

| 场景 | 行为 |
|------|------|
| 空表 Cluster | 无操作，返回空 stats |
| 所有行都是 NULL | NULL 行放在末尾，不参与排序 |
| Cluster 中断 | 遵循 Lance 事务机制，不产生不完整数据 |
| 并发 Cluster | 后执行者失败重试 |
| 无聚簇配置的表调用 Cluster | 报错：`"Table does not have clustering configured"` |
| 聚簇键列被外部删除 | 未定义行为（当前版本不处理）|

---

## 10. 用户责任

由于不维护 fragment 级聚簇元信息，以下由用户负责：

1. **知道何时需要重新 Cluster**：数据乱序后自行判断
2. **管理 Cluster 频率**：根据写入频率决定重写周期
3. **不依赖增量 Cluster**：当前版本仅支持全局重写
4. **注意 DDL 影响**：避免删除或修改聚簇键列

---

## 11. 后续扩展（非当前版本）

- [ ] 增量 Cluster（实际实现）
- [ ] 聚簇键变更
- [ ] Fragment 级聚簇状态追踪
- [ ] 自动 Cluster 触发
- [ ] 取消聚簇功能

---

**文档确认**: 以上需求作为设计依据，进入设计阶段。
