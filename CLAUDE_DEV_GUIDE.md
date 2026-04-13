# LanceDB 开发规范指南

本文档规范与 Claude 协作开发 LanceDB 的最佳实践，确保开发过程可追踪、可恢复、高质量。

---

## 1. 开发工作流规范

### 1.1 小步快跑原则

**核心原则**: 每次修改后必须立即验证，通过后再进行下一步。

**正确流程**:
```
修改一个函数/方法 → cargo check → 通过 → 提交暂存 → 下一步
```

**禁止行为**:
- ❌ 一次性修改多个文件后再编译
- ❌ 连续写大量代码后再验证
- ❌ 在编译错误未解决前继续添加新功能

### 1.2 原子化提交（核心规范）

**基本原则**: 每个任务对应一个或一系列原子化提交，每个提交必须是完整的、可验证的。

#### 提交粒度要求

**允许的原子化提交类型**:
1. **功能实现提交**: 一个新功能点的完整实现 + 测试 + 文档
2. **修复提交**: 一个问题的完整修复 + 回归测试
3. **重构提交**: 代码重构（不改变行为）+ 验证现有测试通过

**禁止的提交类型**:
- ❌ 编译不通过的"半成品"提交
- ❌ 测试失败的提交
- ❌ 多任务混杂的大提交
- ❌ 仅提交没有测试看护的代码

#### 提交前验证清单

```markdown
- [ ] `cargo check --features remote` 通过
- [ ] `cargo test --features remote -p lancedb -- <相关测试>` 通过
- [ ] 新功能有对应测试覆盖
- [ ] 提交信息符合规范
```

#### 任务与提交的关系

**单任务单提交**（推荐）:
```
任务: 实现空表聚簇处理
↓
修改代码 → 编译通过 → 测试通过 → 提交 → 推送
```

**单任务多提交**（复杂任务）:
```
任务: 实现流式处理
↓
提交1: 添加流式读取接口 + 测试
提交2: 实现分批排序逻辑 + 测试
提交3: 添加分批写入 + 集成测试
```

**关键规则**: 每个提交都可以独立回滚而不破坏其他功能。

#### 提交信息格式

```
<类型>: <功能描述>

<详细变更列表>

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

**类型说明**:
- `feat`: 新功能
- `fix`: 修复
- `refactor`: 重构
- `test`: 测试（独立提交或随功能提交）
- `docs`: 文档

**示例**:
```
feat: add empty table handling for clustering

- Return empty ClusterStats for tables with 0 rows
- Add test for empty table cluster operation
- Update progress tracking

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
```

---

## 2. 工具使用规范

### 2.1 工具选择决策树

```
需要修改文件内容？
├── 是，简单替换（单行、无特殊字符）
│   └── 使用：sed -i 's/old/new/g' file
│
├── 是，复杂替换（多行、特殊字符、模式匹配）
│   └── 使用：Python 脚本
│
├── 是，结构性修改（添加函数、修改逻辑）
│   └── 使用：Read → Edit/Write
│
└── 否，仅需查看
    └── 使用：cat / grep / head / tail
```

### 2.2 工具使用禁区

**sed 禁用场景**:
- 包含 `&`、`*`、`$`、`\` 等特殊字符的替换
- 跨行替换
- 需要条件判断的替换

**sed 安全使用模板**:
```bash
# 简单字符串替换（无特殊字符）
sed -i 's/old_string/new_string/g' file.rs

# 行级别操作（删除/添加行）
sed -i '/pattern/d' file.rs                    # 删除匹配行
sed -i '5a\new_line' file.rs                   # 第5行后添加
```

### 2.3 Python 脚本模板

**推荐场景**: 复杂替换、批量修改、需要条件判断

```python
#!/usr/bin/env python3
import re

def fix_file():
    with open('path/to/file.rs', 'r') as f:
        content = f.read()
    
    # 修改内容
    content = content.replace('old', 'new')
    
    # 写回
    with open('path/to/file.rs', 'w') as f:
        f.write(content)
    print("Fixed!")

if __name__ == '__main__':
    fix_file()
```

### 2.4 Edit/Write 工具使用规范

**使用场景**:
- 添加新函数/结构体
- 修改多行代码块
- 重构逻辑

**强制流程**:
1. **先 Read**: 必须先用 Read 读取文件内容
2. **精确定位**: 使用足够长的 `old_string` 确保唯一性
3. **验证结果**: 修改后立即检查

**示例**:
```python
# 1. 先读取
content = Read(file_path="...")

# 2. 使用 Edit（old_string 至少5行或足够独特）
Edit(
    file_path="...",
    old_string="""fn old_function() {
    let x = 1;
    let y = 2;
    x + y
}""",
    new_string="""fn new_function() {
    let x = 1;
    let y = 2;
    x * y
}"""
)

# 3. 验证
cargo check
```

---

## 3. 代码修改规范

### 3.1 修改前检查清单

```markdown
- [ ] 是否已读取目标文件？
- [ ] 是否理解当前代码结构？
- [ ] 修改是否影响其他模块？
- [ ] 是否需要添加测试？
```

### 3.2 Rust 代码修改最佳实践

**类型系统**:
- 修改 enum/struct 后，立即检查所有 match 分支
- 使用 `cargo check` 而非 `cargo build` 快速验证

**错误处理**:
- 优先使用 `?` 传播错误
- 添加新 Error 变体时，同步更新所有使用处

**trait 实现**:
- 添加 trait 方法时，同时实现所有实现者
- 使用 `cargo check --all-features` 检查 feature gate

### 3.3 编译错误诊断流程

**遇到编译错误时**:

1. **定位错误**: 从第一个错误开始解决（后续错误可能是连锁反应）
2. **理解错误**: 阅读完整错误信息，不只是第一行
3. **检查上下文**: 查看错误位置的上下文代码
4. **修复验证**: 修复后立即 `cargo check`
5. **连锁解决**: 重复直到无错误

**常见 Rust 错误速查**:

| 错误类型 | 常见原因 | 解决方法 |
|---------|---------|---------|
| E0425 | 未找到值/函数 | 检查拼写、导入、作用域 |
| E0308 | 类型不匹配 | 检查类型转换、泛型参数 |
| E0599 | 方法不存在 | 检查 trait 导入、类型正确性 |
| E0061 | 参数数量不对 | 检查函数签名、添加/删除参数 |

---

## 4. 断点续做规范

### 4.1 断点续做检查清单

**恢复工作时必须执行**:

```bash
# 1. 确认分支
git branch

# 2. 查看未提交变更
git status

# 3. 快速编译验证
cargo check --quiet --features remote

# 4. 阅读进度文档
cat CLUSTERING_PROGRESS.md
```

### 4.2 断点续做决策树

```
工作被中断，如何恢复？
├── 有未提交修改
│   ├── 修改完整且编译通过 → 提交
│   ├── 修改不完整但能编译 → git stash，后续恢复
│   └── 修改导致编译错误 → 考虑 git checkout 恢复
│
└── 无未提交修改
    ├── 阅读进度文档
    ├── 运行测试确认状态
    └── 继续下一步任务
```

### 4.3 进度文档维护

**CLUSTERING_PROGRESS.md 更新时机**:
- 完成一个功能点后
- 中断工作前
- 遇到阻塞问题时

**必须记录的信息**:
- 当前阶段和状态
- 已完成的文件和修改
- 待解决的问题
- 下一步计划

### 4.4 原子化提交流程（新增）

**每次提交前必须执行**: [第1.2节](#12-原子化提交核心规范) 中的验证清单

**提交决策树**:
```
准备提交？
├── 编译错误 → 修复后再提交
├── 测试失败 → 修复后再提交
├── 多个任务混在一起 → 拆分后分别提交
└── 单任务、编译通过、测试通过 → 提交并推送
```

---

## 5. 测试规范

### 5.1 测试驱动开发流程

```
编写测试 → 运行测试（应该失败）→ 实现功能 → 运行测试（应该通过）→ 重构
```

### 5.2 测试命令速查

```bash
# 运行所有测试
cargo test --quiet --features remote

# 运行特定模块测试
cargo test --quiet --features remote -- cluster

# 运行特定测试函数
cargo test --quiet --features remote test_cluster_config_validation

# 检查但不运行测试
cargo test --no-run --quiet --features remote
```

### 5.3 测试覆盖率要求

- 新功能必须有单元测试
- 公共 API 必须有集成测试
- 错误处理路径必须测试

---

## 6. 沟通规范

### 6.1 进度汇报模板

**定期汇报内容**:
```markdown
## 当前状态
- 阶段：X/Y
- 主要进展：...
- 阻塞问题：...

## 下一步计划
- 下一步：...
- 预计时间：...
- 需要确认：...
```

### 6.2 问题升级

**何时需要立即汇报**:
- 技术决策需要确认
- 遇到设计冲突
- 进度严重偏离预期
- 发现重大阻塞问题

---

## 7. 快速参考

### 7.1 常用命令

```bash
# 编译检查
cargo check --quiet --features remote
cargo check --quiet --features remote --tests --examples

# 格式化
cargo fmt --all

# 代码检查
cargo clippy --quiet --features remote --tests --examples

# 测试
cargo test --quiet --features remote

# 完整验证（提交前）
cargo fmt --all && cargo check --quiet --features remote && cargo test --quiet --features remote
```

### 7.2 文件导航

```bash
# 查找符号定义
grep -rn "struct SymbolName" rust/lancedb/src/
grep -rn "fn function_name" rust/lancedb/src/

# 查找 trait 实现
grep -rn "impl TraitName" rust/lancedb/src/

# 查找模块
find rust/lancedb/src -name "*.rs" -type f
```

---

## 8. 开发会话启动模板

每次开始新会话时，按以下步骤执行：

```bash
# Step 1: 环境检查
cd /home/hang/clustering/lancedb
git status
git branch

# Step 2: 编译状态验证
cargo check --quiet --features remote 2>&1 | head -20

# Step 3: 阅读进度
cat CLUSTERING_PROGRESS.md

# Step 4: 确认下一步任务
# （根据进度文档确认）
```

---

**文档版本**: v1.0  
**最后更新**: 2026-04-13  
**适用范围**: LanceDB 多维聚簇功能开发
