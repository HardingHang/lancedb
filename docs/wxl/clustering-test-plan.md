# 聚簇算法 Benchmark 测试设计

## 1. 测试目标

本 benchmark 用于评估不同聚簇算法在典型过滤查询下的收益、成本和稳定性。测试重点是：

- 对比未聚簇重写布局与不同聚簇算法的查询性能差异。
- 评估不同数据分布、聚簇维度和选择率下的收益变化。
- 记录构建成本和物理布局指标，避免只看查询延迟。
- 确认查询条件真实命中数据，避免把空结果误判为裁剪收益。

本测试只覆盖 clustering-only 场景，不显式创建 `BTREE`、`IVF`、全文索引或向量索引。查询收益来自聚簇布局、Lance fragment/page min-max 统计裁剪，以及顺序读局部性。

## 2. 算法对照组

| 组 | 含义 |
|:---|:---|
| `A-R` | 不聚簇重写布局 |
| `B-H` | Hilbert 聚簇 |
| `B-Z` | ZOrder 聚簇 |
| `B-X` | XMCK 默认参数 |
| `B-XM` | XMCK 中等粒度参数 |
| `B-XT` | XMCK 更细粒度参数 |

`A-R` 是本文比较聚簇算法收益的 baseline。它不是新的聚簇算法，而是把未聚簇数据重写成与聚簇组相近的 fragment/data file 数量，用于隔离“文件数量不同”本身带来的影响。

## 3. 数据集矩阵

完整 benchmark 使用 `python/benchmarks/clustering_perf_test` 的数据生成和查询框架。

| 维度 | 聚簇键 |
|:---|:---|
| `2D` | `lat`, `lng` |
| `3D` | `lat`, `lng`, `category_id` |
| `4D` | `lat`, `lng`, `category_id`, `timestamp` |

| 分布 | 生成方式 | 测试含义 |
|:---|:---|:---|
| `uniform` | `lat/lng` 均匀分布 | 无明显热点的基础场景。 |
| `gaussian` | `lat/lng` 中心聚集分布 | 局部聚集场景，观察聚簇稳定性。 |
| `skewed` | `lat/lng` Zipf 热点分布 | 强热点压力场景，观察退化风险。 |

规模口径：

- `small`：`100K` 行，用于 smoke test 和快速验证 benchmark 逻辑。
- `medium`：`1M` 行，作为正式 benchmark 主规模。

正式主矩阵：

| 数据集 | 用途 |
|:---|:---|
| `medium/uniform/2d` | 二维均匀分布主场景 |
| `medium/gaussian/2d` | 二维中心聚集场景 |
| `medium/skewed/2d` | 二维热点压力场景 |
| `medium/uniform/3d` | 三维聚簇收益变化 |
| `medium/uniform/4d` | 四维聚簇收益变化 |

可选补充矩阵为 `gaussian/skewed 3D/4D`，用于补齐分布和维度的交叉结论。

## 4. 查询场景

查询列限定在聚簇列内，不混入非聚簇列。

| 场景 | 含义 |
|:---|:---|
| `S1_2D_Range` | `lat/lng` 二维范围查询 |
| `S2_1D_Range` | `lat` 一维范围查询 |
| `S3_Point_Query` | `lat/lng` 点查询 |
| `S4_Full_Scan` | 全表聚合扫描 |
| `S5_Mixed_Condition` | `lat range + lng point` 混合条件 |

选择率：

- `0.1%`
- `1%`
- `10%`
- `50%`

查询生成规则：

- `S1_2D_Range` 使用 `lat/lng` 二维聚簇列。为了让标签选择率接近整体结果选择率，每个轴使用 `sqrt(selectivity)` 的居中百分位窗口；例如 `S1 10%` 使用每轴约 `31.6%` 的窗口。
- `S2_1D_Range` 使用 `lat` 单列范围查询。范围使用居中百分位窗口，例如 `S2 10%` 使用 `[p45, p55]`，不使用低端尾部 `[min, p10]`。
- `S3_Point_Query` 使用同一条真实存在数据行中的 `lat/lng` 点值，避免连续浮点列分别取 median 后组合出不存在的点。
- `S4_Full_Scan` 不使用选择率，用于观察全表扫描是否退化。
- `S5_Mixed_Condition` 使用 `lat` 范围和 `lng` 点条件；`lng` 点值来自当前 `lat` 范围内的一条真实行，避免空结果查询。

所有查询结果需要记录 `rows_returned`，用于区分“聚簇裁剪有效”和“查询本身返回 0 行”。

## 5. 测量指标

核心指标：

- `bytes_read`
- `iops`
- `rows_returned`
- `p50`
- `p95`
- `p99`

构建成本：

- `create_table_s`
- `cluster_s`
- `total_setup_s`

布局指标：

- `fragment_count`
- `data_file_count`
- `fragment_row_counts`

报告排序以 `p50` 为主，同时参考 `bytes_read` 和 `rows_returned`。如果目标是 tail latency SLA，需要单独提高 `p95/p99` 权重。

## 6. 缓存控制

正式报告口径采用 per-group cold-cache：

- 每个算法组开始前单独执行 `drop_caches`。
- 执行机器需要具备 `sudo` 权限。
- 如果执行环境无法清理 OS page cache，报告中必须标注为非严格 cold-cache 结果。

runner 参数：

```bash
--before-group-command 'sync && sudo sh -c "echo 3 > /proc/sys/vm/drop_caches"'
```

## 7. 输出规则

默认输出：

- 最终 `.json` 结果文件。
- 最终 `.md` 自动汇总文件。

调试输出：

- 只有显式传 `--save-checkpoints` 时，才输出每组和每场景的 checkpoint 文件。

人工汇总报告建议使用以下结构：

```markdown
# 聚簇算法基准测试报告

## 1. 测试环境

## 2. 测试范围

## 3. 执行矩阵

## 4. 总体结论

## 5. 主要结果

## 6. 构建成本

## 7. 布局分析

## 8. 结果解释

## 9. 建议

## 10. 局限性
```

报告口径：

- `测试环境`：机器配置、OS、Python/Rust/LanceDB 版本、cache 控制口径。
- `测试范围`：说明 clustering-only、不含显式索引，并列出算法组、数据分布和查询场景。
- `执行矩阵`：列出实际完成的 `scale`、`distribution`、`dimensions`、`run_id`。
- `总体结论`：3-5 条核心结论，说明主场景最佳算法、收益范围和主要风险。
- `主要结果`：按数据集汇总 `p50`、`bytes_read`、`rows_returned`。
- `构建成本`：记录 `create_table_s`、`cluster_s`、`total_setup_s`。
- `布局分析`：记录 fragment/data file 数量和行数分布，用于解释布局差异。
- `结果解释`：解释收益来源、维度升高影响、分布变化影响、full scan 是否退化。
- `建议`：说明默认算法策略和适用场景。
- `局限性`：说明不含显式索引、per-group cold-cache 依赖 `sudo`、结果只代表当前机器和数据生成器。

## 8. 运行命令

从 benchmark 目录执行，避免模块路径不一致：

```bash
cd /home/weixl/workspace/lancedb/python/benchmarks
python validate_setup.py
sudo -v
```

快速 smoke：

```bash
python -m clustering_perf_test.runner --compare-algorithms-quick --scale small --distribution uniform --dimensions 2 --run-id smoke --before-group-command 'sync && sudo sh -c "echo 3 > /proc/sys/vm/drop_caches"'
```

正式主矩阵：

```bash
for spec in uniform:2 gaussian:2 skewed:2 uniform:3 uniform:4; do
  dist=${spec%:*}
  dims=${spec#*:}
  python -m clustering_perf_test.runner --compare-algorithms --scale medium --distribution "$dist" --dimensions "$dims" --run-id cold_${dist}_${dims}d --before-group-command 'sync && sudo sh -c "echo 3 > /proc/sys/vm/drop_caches"'
done
```

可选补充矩阵：

```bash
for spec in gaussian:3 skewed:3 gaussian:4 skewed:4; do
  dist=${spec%:*}
  dims=${spec#*:}
  python -m clustering_perf_test.runner --compare-algorithms --scale medium --distribution "$dist" --dimensions "$dims" --run-id cold_${dist}_${dims}d --before-group-command 'sync && sudo sh -c "echo 3 > /proc/sys/vm/drop_caches"'
done
```

单点调试模板：

```bash
python -m clustering_perf_test.runner --compare-algorithms --scale medium --distribution uniform --dimensions 2 --algorithms zorder,xmck,xmck-medium,xmck-tuned --scenarios S1_2D_Range --run-id debug
```
