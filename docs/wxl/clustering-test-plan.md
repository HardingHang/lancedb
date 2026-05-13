# ZOrder / XMCK 聚簇扩展测试计划

## 1. 测试目标

测试需要证明三件事：

1. 新增算法生成的布局符合其算法语义。
2. 新增算法不破坏现有 `direct` / `hilbert` 行为。
3. 公共 API 在 Rust、Python、TypeScript 中行为一致。

## 2. Rust 算法级测试

### 2.1 ZOrder

需要覆盖：

- 2D Morton order 的确定性顺序。
- 3D / 4D key 生成不冲突。
- `bits` 参数生效。
- `dimension * bits > 64` 返回 `InvalidInput`。
- NULL 聚簇值返回空 key，并由执行器放到末尾。
- 整数、浮点、日期、时间戳类型可参与归一化。
- 相同 min/max bounds 下所有坐标归一化为 0。

建议用小网格构造确定性用例：

```text
(0,0), (0,1), (1,0), (1,1)
```

ZOrder 的预期顺序应来自 bit interleaving，而不是 Hilbert 顺序。

### 2.2 XMCK

需要覆盖：

- 按配置生成稳定 block id。
- 按 `ceil((N / (S * P)) ^ (1 / M))` 计算每维块数。
- 按 `delta_i = (max_i - min_i) / block_count_per_dim` 计算每维等宽边界。
- 第 `k` 个区间为 `[min_i + k * delta_i, min_i + (k + 1) * delta_i)`，最后一个区间包含 `max_i`。
- 行被分配到正确多维块。
- 同一块内按用户指定的聚簇键顺序排序；例如指定 `(b, a, c)` 时，块内依次比较 `b`、`a`、`c`，不能改成 schema 顺序或字母顺序。
- NULL 行放在末尾。
- 块参数非法时返回明确错误。
- 小数据下 fragment/page 输出顺序可预测。
- 实现必须走显式分桶、块内排序、按块写出路径，不允许使用复合 sort key 近似。

## 3. Rust 执行级测试

需要新增或扩展 table cluster 测试：

- `cluster_algorithm("zorder")` 后，`cluster_config().algorithm == "zorder"`。
- 2D ZOrder 聚簇后，读取数据验证 sort key 单调递增。
- 3D / 4D ZOrder 聚簇可成功执行。
- 未指定算法时 2D 仍默认 `hilbert`。
- 未配置 clustering 的表执行 cluster 仍返回现有错误。
- 已有索引的表聚簇后仍重建索引。
- 空表和全 NULL 表行为与现有聚簇一致。

XMCK 执行测试应在实现路径确认后补充：

- 多块输出顺序稳定。
- 每个块内排序稳定。
- `target_rows_per_fragment` 与块写出策略交互符合设计。

## 4. 绑定层测试

### 4.1 Python

需要覆盖：

- `create_table(..., cluster_by=["x", "y"], cluster_algorithm="zorder")`。
- `cluster_config()` 返回 `{"algorithm": "zorder", ...}`。
- `table.cluster()` 可执行并返回 `rows_processed`、`fragments_written`、`indices_rebuilt`。
- async 和 sync API 都覆盖。
- 非法算法名抛出 `ValueError` 或现有映射错误类型。

### 4.2 TypeScript

需要覆盖：

- `createTable(..., { clusterBy: ["x", "y"], clusterAlgorithm: "zorder" })`。
- `clusterConfig()` 返回算法名。
- `table.cluster()` 成功。
- 非法算法名 rejected，并包含清晰错误信息。

## 5. Benchmark 设计

当前 benchmark 分成两层：

- 最小阶段性 benchmark：Rust example，主要用于开发中快速验证布局方向。
- 完整 benchmark：基于 `python/benchmarks/clustering_perf_test`，用于正式对比无聚簇、Hilbert、ZOrder、XMCK。

### 5.1 最小阶段性 benchmark

最小 benchmark 的定位是开发期 smoke test，不作为最终性能结论来源。它主要验证：

- 新算法是否真正改变了数据布局。
- `Hilbert / ZOrder / XMCK` 的相对构建成本是否合理。
- 参数变化是否能带来可观测的布局差异。

当前最小 benchmark 关注的指标：

- 聚簇耗时
- 聚簇峰值内存
- 相邻行距离 `mean_neighbor`
- 范围运行段数 `range_runs`

当前最小 benchmark 的主要限制：

- 数据规模小，默认 `XMCK` 可能只形成 1 个块。
- 只能说明“实现方向是否正确”，不能替代完整 workload benchmark。

### 5.2 完整 benchmark 范围

完整 benchmark 复用 `python/benchmarks/clustering_perf_test` 的数据生成和查询框架，但当前 `Phase 6` 只覆盖 clustering-only 对比，不混入显式标量索引或向量索引。

当前算法对照组：

- `A`：无聚簇，原始写入 baseline。
- `A-R`：无聚簇 rewrite baseline，按 `target_rows_per_fragment` 批量写入，保持与聚簇组相近的 fragment 粒度。
- `B-H`：Hilbert 聚簇。
- `B-Z`：ZOrder 聚簇。
- `B-X`：XMCK 默认参数。
- `B-XM`：XMCK 中等块参数，`cu_size = 10000`，`cus_per_block = 1`。
- `B-XT`：XMCK 更细块参数，`cu_size = 5000`，`cus_per_block = 1`。

说明：

- 这套对照组**没有显式创建 BTREE / IVF 等用户索引**。
- 正式比较算法收益时优先使用 `A-R` 作为 baseline；`A` 只保留用于观察原始写入状态。
- 查询收益仍然会受到 Lance 自带 fragment / page min-max 统计的影响，因此 benchmark 反映的是“聚簇布局 + 元数据裁剪 + 顺序读局部性”的综合效果。

### 5.3 当前数据集矩阵

已纳入完整 benchmark 的数据集维度如下：

- 规模：
  - `small`
  - `medium`
- 分布：
  - `uniform`
  - `gaussian`
  - `skewed`
- 维度：
  - `2D`
  - `3D`
  - `4D`

说明：

- `small`：`100K` 行数据，用于快速验证 benchmark 逻辑和收益方向。
- `medium`：`1M` 行数据，作为当前正式 benchmark 主规模。
- `uniform`：均匀分布，用于观察算法在无热点条件下的基础布局能力。
- `gaussian`：高斯聚集分布，用于观察聚簇在局部簇明显时的收益。
- `skewed`：热点倾斜分布，用于观察热点场景下的稳定性。
- `2D`：二维聚簇键，例如 `(lat, lng)`，是当前最核心的主场景。
- `3D`：在二维基础上增加一个维度，用于观察收益是否因维度升高而被稀释。
- `4D`：四维聚簇键，用于观察更高维下算法差异和 `1D` 查询受益情况。

### 5.4 当前查询场景

完整 clustering-only benchmark 的查询列应限定在聚簇列内，不再混入非聚簇列。场景定义如下：

- `S1_2D_Range`：二维范围查询
- `S2_1D_Range`：一维 `lat` 范围查询
- `S3_Point_Query`：点查询
- `S4_Full_Scan`：全表聚合扫描
- `S5_Mixed_Condition`：只命中部分聚簇列的混合条件查询

选择率当前使用：

- `0.1%`
- `1%`
- `10%`
- `50%`

说明：

- `S1_2D_Range`：使用 `lat/lng` 这类核心二维聚簇列，验证主路径收益。为了让标签选择率更接近整体结果选择率，二维查询每个轴使用 `sqrt(selectivity)` 的居中百分位窗口；例如 `S1 10%` 使用每轴约 `31.6%` 的窗口，而不是每轴 `10%`。
- `S2_1D_Range`：使用 `lat` 单列范围查询，验证多维聚簇下单列过滤还能保留多少收益。范围使用居中百分位窗口，例如 `S2 10%` 使用 `[p45, p55]`，不使用低端尾部 `[min, p10]`。
- `S3_Point_Query`：使用聚簇列上的点条件，观察极低选择率下的裁剪效果。点条件必须来自同一条真实存在的数据行，避免连续浮点列分别取 median 后组合出不存在的 `(lat, lng)`。
- `S4_Full_Scan` 不使用选择率。
- `S5_Mixed_Condition`：只使用聚簇列，但采用不同谓词形状；当前统一为 `lat` 范围 + `lng` 点条件。`lng` 点值必须来自当前 `lat` 范围内的一条真实行，避免空结果查询。
- 完整 `medium` 跑法默认会执行全部 `S1 ~ S5` 场景。
- 为了开发中快速定位问题，runner 也支持只跑单算法、单场景或少量场景组合。

口径说明：

- 早期 benchmark 曾使用 `timestamp` 作为 `S2` 条件、`category` 作为 `S5` 条件，这两者在部分维度下都不是聚簇列。
- 当前测试口径已调整为“查询场景中的过滤列应全部来自聚簇列”。
- 当前范围查询使用居中百分位窗口，避免所有查询都落在数据最小值尾部。固定使用 `[min, pN]` 会把 benchmark 绑定到边界和热点区域，尤其会放大 `skewed` 分布下的低端热点，不适合作为默认主口径。
- 当前结果需要记录 `rows_returned`，用于区分“聚簇裁剪有效”和“查询本身返回 0 行”。
- 因此后续如果继续补 benchmark 或重跑正式结果，应按新口径使用 `lat` 版 `S2` 和 `lat + lng` 版 `S5`。

按当前聚簇键配置，各维度可用场景应收敛为：

- `2D`：`lat / lng` 都是聚簇列，可保留 `S1`、`S2`、`S3`、`S4`、`S5`。
- `3D`：`lat / lng / category_id` 是聚簇列，可保留 `S1`、`S2`、`S3`、`S4`、`S5`。
- `4D`：`lat / lng / category_id / timestamp` 都是聚簇列，可保留 `S1`、`S2`、`S3`、`S4`、`S5`。

这意味着：

- 旧版 `S2_1D_Range(timestamp)` 和 `S5(lat + category)` 都不再符合当前测试口径。
- 后续如果要严格按当前口径重跑正式 benchmark，需要同时更新 `S2` 和 `S5` 的结果。

### 5.5 当前测量指标

当前完整 benchmark 重点记录四类指标：

- 物理层：
  - `bytes_read`
  - `iops`
- 延迟：
  - `p50`
  - `p95`
  - `p99`
- 构建成本：
  - `create_table_s`
  - `cluster_s`
  - `total_setup_s`
- 布局结构：
  - `fragment_count`
  - `data_file_count`
  - `fragment_row_counts`
  - `XMCK` 组的 `xmck_block_row_counts`
- 结果解释辅助项：
  - 算法组
  - 场景
  - 选择率
  - 数据分布
  - 维度数

说明：

- 当前完整 benchmark 已支持输出 `fragment_count`、`data_file_count`、`fragment_row_counts`。
- `XMCK` 组已支持输出块统计摘要，以及 JSON 中的完整 `xmck_block_row_counts`。
- 如果后续需要更严格的布局分析，仍可继续补更细粒度的 fragment/page 级统计。

### 5.6 缓存控制口径

完整 benchmark 的正式报告口径采用 **per-group cold-cache**：

- 每个算法组开始前单独执行 `drop_caches`。
- 需要执行机器具备 `sudo` 权限。
- 如果执行环境无法清理 OS page cache，必须在报告中降级标注为非严格 cold-cache 结果。

当前 compare runner 已支持：

- `--before-group-command 'sync && sudo sh -c "echo 3 > /proc/sys/vm/drop_caches"'`

这样可以在一次 benchmark 运行内，对每个算法组执行前都单独清缓存。

当前已完成的正式 benchmark 中：

- `medium` 分布和维度测试已采用“整轮前执行一次 `drop_caches`”。
- “每组独立 cold-cache” 仍未补齐，因此当前报告不是最严格的 per-group cold-cache 结论。

### 5.7 输出规则

当前 compare runner 的默认输出规则是：

- 默认只保留最终 `.json` 和 `.md`
- 只有显式传 `--save-checkpoints` 时，才输出每组和每场景的 checkpoint 文件

这样做的目的是：

- 正常正式跑法只保留最终结果，避免输出目录过于杂乱。
- 排查长跑问题时仍可打开 checkpoint 模式，逐组观察进度。

正式汇总报告使用固定两级标题：

```markdown
# Clustering Algorithm Benchmark Report

## 1. Executive Summary

## 2. Environment

## 3. Benchmark Scope

## 4. Run Matrix

## 5. Headline Results

## 6. Scenario Results

## 7. Setup Cost

## 8. XMCK Block Analysis

## 9. Interpretation

## 10. Recommendation

## 11. Limitations
```

各部分记录口径：

- `Executive Summary`：3-5 条核心结论，说明 `Hilbert`、`ZOrder`、`XMCK` 哪个在主场景更优。
- `Environment`：commit hash、机器配置、OS、Python/Rust/LanceDB 版本、cache 控制口径。
- `Benchmark Scope`：说明当前只做 clustering-only，不包含 `BTREE` / `IVF`，并列出 `A`、`B-H`、`B-Z`、`B-X`、`B-XM`、`B-XT`。
- `Run Matrix`：列出实际完成的 `scale`、`distribution`、`dimensions`、`run_id`。
- `Headline Results`：按数据集汇总最佳算法、相对 `A` 组的 `p50` speedup、`bytes_read` reduction、`p95/p99` 是否回退。
- `Scenario Results`：按 `S1` 到 `S5` 汇总，用表格区分选择率，不再增加三级标题。
- `Setup Cost`：记录 `create_table_s`、`cluster_s`、`total_setup_s`、`fragment_count`、`data_file_count`。
- `XMCK Block Analysis`：记录 `B-X`、`B-XM`、`B-XT` 的 block 数、非空 block 数、行数 `min/p50/max/avg`。
- `Interpretation`：解释收益来源、维度升高影响、分布变化影响、full scan 是否退化。
- `Recommendation`：说明是否建议调整默认算法或推荐某个 `XMCK` 参数档。
- `Limitations`：说明不含显式索引、per-group cold-cache 依赖 `sudo`、结果只代表当前机器和数据生成器。

### 5.8 当前 benchmark 命令口径

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
python -m clustering_perf_test.runner --compare-algorithms --scale medium --distribution uniform --dimensions 2 --algorithms xmck,xmck-medium,xmck-tuned --scenarios S1_2D_Range --run-id debug
```

## 6. 回归命令

Rust core：

```bash
cargo fmt --all
cargo check --quiet --features remote --tests --examples
cargo test --quiet --features remote -p lancedb table::cluster
```

Python：

```bash
pytest python/python/tests/test_table.py -k cluster
```

TypeScript：

```bash
npm test -- table.test.ts -t clustering
```

实际命令可能需要根据仓库当前 workspace 和 package 脚本调整；若命令不可用，需要在执行记录中说明原因。

## 7. 当前执行状态

截至 2026-05-12，测试计划执行状态如下：

- Rust 算法级测试：已完成 `ZOrder` 与 `XMCK` 的核心算法、参数和执行路径测试。
- Rust 执行级测试：已完成 `zorder` / `xmck` 的端到端聚簇回归，以及默认 `hilbert` 行为保护。
- Python / TypeScript 绑定层测试：已完成算法名、参数透传、`cluster_config()` 和错误路径覆盖。
- 完整 benchmark：旧口径结果已生成过 `small / medium` 多组对照，但当前正式口径下需要重跑。
- benchmark runner：已支持单算法、单场景、`run_id`、构建耗时输出和按需 checkpoint。

尚未完成：

- 按正式主矩阵重跑 per-group cold-cache benchmark。
- 如需完整覆盖，补跑 `gaussian/skewed` 的 `3D/4D` 可选补充矩阵。
- 基于两级标题框架产出最终人工汇总报告。
