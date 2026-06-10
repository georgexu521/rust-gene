# Opencode 对比分析与追赶设计
Status: Active

> 基于 2026-06-09 对 opencode-dev 源码（TypeScript）与 priority-agent（Rust）的系统对比
> 聚焦三个 priority-agent 可向 opencode 学习的关键领域
> 2026-06-09 Liz review 修订：校准 priority-agent 现状，并把 auto-apply /
> always-allow 方案改为 shadow-first、fail-closed、test-gated 路线。

---

## 目录

0. [修订原则](#0-修订原则)
1. [递进式编辑匹配](#1-递进式编辑匹配)
2. [Tree-Sitter Shell 命令解析](#2-tree-sitter-shell-命令解析)
3. [工具输出智能截断](#3-工具输出智能截断)
4. [总体实施路线](#4-总体实施路线)

---

## 0. 修订原则

opencode 的实现可以借鉴，但不能直接覆盖 priority-agent 的现有硬边界：

- 不弱化 `stale_read`、预写竞态检查、checkpoint、permission review、validation proof。
- 对模糊编辑恢复，第一版优先 diagnostic / shadow，不直接扩大 auto-apply。
- 对 shell AST 和 arity 权限，第一版只产出 observation 和 rule suggestion，不改变审批结果。
- 对长输出提示，区分 `tool-output://` 存储和 bash artifact 文件路径；不要假设当前 route 一定暴露 Task/agent tool。
- 所有新增智能都必须有回退路径、结构化 evidence、针对性测试，再进入默认生产路径。

---

## 1. 递进式编辑匹配

### 1.1 opencode 的实现

**核心文件**: `packages/opencode/src/tool/edit.ts` (711 行)

opencode 的编辑工具在匹配 `old_string` 时，使用 **9 层递进式 fallback** 策略，从最严格到最宽松逐级尝试：

| # | Replacer | 算法 | 宽容度 |
|---|----------|------|--------|
| 1 | `SimpleReplacer` | 精确匹配原始 old_string | 最严格 |
| 2 | `LineTrimmedReplacer` | 逐行 trim 后比较，但返回原始精确位置 | ↑ |
| 3 | `BlockAnchorReplacer` | 锚定首尾行（精确），中间用 Levenshtein 模糊匹配 | ↑ |
| 4 | `WhitespaceNormalizedReplacer` | 连续空白压缩为单空格后匹配 | ↑ |
| 5 | `IndentationFlexibleReplacer` | 剥离公共前导缩进后匹配 | ↑ |
| 6 | `EscapeNormalizedReplacer` | 处理 `\n`/`\t` 等转义序列 | ↑ |
| 7 | `TrimmedBoundaryReplacer` | 整段 trim 后匹配 | ↑ |
| 8 | `ContextAwareReplacer` | 锚定首尾行 + 中间行 50% 精确匹配 | ↑ |
| 9 | `MultiOccurrenceReplacer` | 枚举所有精确出现位置 | 最宽松 |

**关键设计决策**:
- 每种 Replacer 是生成器函数，yield 候选匹配位置（不直接做替换）
- 顶层的 `replace()` 函数对每个候选做 `indexOf` 验证唯一性
- 如果是 `replaceAll` 模式，第一个成功的候选直接全局替换
- 如果不是 `replaceAll`，必须唯一匹配才替换
- 所有 9 层都失败时，抛出详细错误信息告诉 LLM 如何修复

**错误报告格式**:
```
Could not find oldString in the file. It must match exactly, including whitespace, indentation, and line endings.
```
或
```
Found multiple matches for oldString. Provide more surrounding context to make the match unique.
```

### 1.2 priority-agent 的现状

**核心文件**: `src/tools/file_tool/edit_tool.rs` (949 行) + `edit_match.rs` (567 行)

**现有 fallback 策略**（`generate_edit_candidates` 中 4 种）:

| 策略 | 对应 opencode | 自动应用？ |
|------|:---:|:---:|
| `line-trimmed` | Replacer #2 | 是（多行 + 唯一匹配时） |
| `indent-normalized` | Replacer #5 | 是（单候选时） |
| `block-anchor` | Replacer #3 | 否（仅报告候选） |
| `whitespace-normalized` | Replacer #4 | 否（仅报告候选） |

**但我们的 fallback 有重要差异**:
1. 只有 `line-trimmed` 和 `indent-normalized` 支持 **auto-apply**，且只在不产生歧义时
2. `block-anchor` 和 `whitespace-normalized` 只作为 diagnostic 报告给 LLM（`EditCandidateOutcome::Candidates`），需要 LLM 重新思考后再发编辑指令
3. **缺少 escape-normalized**（opencode #6）：模型经常输出 `\n` 字面量而不是真正换行
4. **缺少 trimmed-boundary**（opencode #7）：老字符串两端多余空白
5. **缺少 MultiOccurrenceReplacer**（opencode #9）：我们直接报错而不是列出所有匹配
6. **缺少 ContextAwareReplacer**（opencode #8）：3+ 行编辑时 50% 中间行精确匹配即可

**另外我们独有的优势**:
- `normalize_quotes()` 智能引号归一化（opencode 没有）
- `desanitize()` 处理 `<fnr>`/`<n>`/`<TAB>` 转义（opencode 没有）
- `stale_read` 保护（多读后修改检测 + 外部修改检测）
- 预写竞态检查

### 1.3 差距与设计

#### 差距 1: Escape-Normalized Replacer

**问题**: 弱模型（如 MiniMax）经常输出 `old_string` 包含字面量 `\n` 而不是真正换行。我们的 `desanitize()` 处理 `<fnr>`/`<n>` 但不处理 `\n`/`\t`。

**安全约束**: 不能简单用链式 `replace("\\n", "\n").replace("\\\\", "\\")`
无条件改写输入；代码中真实存在字面量 `\\n` 或转义反斜杠时，这会把合法文本误当成换行。
第一版应只生成候选并返回 diagnostic，只有命中非常窄的安全条件时才允许 auto-apply。

**设计**:

```rust
// 新增 escape-normalized 策略，在 generate_edit_candidates 中加入
fn escape_normalized_match(content: &str, old: &str) -> EditCandidateOutcome {
    let Some(unescaped) = unescape_tool_string_once(old) else {
        return EditCandidateOutcome::Mismatch { detail: "no escape-normalized variant".into() };
    };
    if unescaped == old {
        return EditCandidateOutcome::Mismatch { detail: "unchanged after unescape".into() };
    }

    let positions = find_all_occurrences(content, &unescaped);
    let candidates = positions.into_iter().map(|pos| EditCandidate {
        strategy: "escape-normalized".to_string(),
        occurrence: pos,
        confidence: if positions.len() == 1 { "medium" } else { "low" },
        guidance: "Literal escape sequences were interpreted once; verify this is not a real code literal.".to_string(),
    }).collect::<Vec<_>>();

    // Default: diagnostic only. Auto-apply can be enabled later only when:
    // - exactly one occurrence exists,
    // - old_string contains no doubled backslash literals outside JSON/tool-call escaping,
    // - replacement span is within a fresh file_read snapshot,
    // - regression tests cover real literal "\\n" content.
    EditCandidateOutcome::Candidates { count: candidates.len(), candidates }
}
```

**工作量**: 约 70 行 Rust，插入到 `edit_match.rs` 的 `generate_edit_candidates` 流水线中。

#### 差距 2: Trimmed-Boundary Replacer

**问题**: LLM 有时在 old_string 两端多附带了空白。

**安全约束**: 仅对整段首尾空白变化做处理，不做内部行级模糊；唯一候选可以考虑
auto-apply，但必须经过已有 stale-read / 预写检查。

**设计**:

```rust
fn trimmed_boundary_match(content: &str, old: &str) -> Vec<EditCandidate> {
    let trimmed = old.trim();
    if trimmed.len() == old.len() { return vec![]; } // 已经 trim 过
    let positions = find_all_occurrences(content, trimmed);
    positions.into_iter().map(|pos| EditCandidate {
        strategy: "trimmed-boundary".to_string(),
        occurrence: pos,
        confidence: if positions.len() == 1 { "high" } else { "low" },
        guidance: "Only leading/trailing whitespace around old_string was removed.".to_string(),
    }).collect()
}
```

**工作量**: 约 35 行 Rust。建议作为第一批可 auto-apply 的新增策略，但只允许唯一候选。

#### 差距 3: Block-Anchor 最佳匹配 Shadow

**现状**: 我们的 `block-anchor` 只报告候选列表，不自动应用。
**目标**: 借鉴 opencode Replacer #3 的 Levenshtein 相似度，但第一版只作为
shadow/diagnostic 产出最佳候选和分数，不直接自动应用。

**原因**: priority-agent 目前的编辑恢复契约是“只对 deterministic single-match recovery
auto-apply”。首尾锚点 + 中间模糊匹配容易在 stale anchor、重复代码块、相似配置段里误改。
这类 false green 比让 LLM 重新发一次精确 edit 更危险。

**设计**:

```rust
fn block_anchor_best_match_shadow(content: &str, old: &str) -> Vec<ScoredEditCandidate> {
    let content_lines: Vec<&str> = content.lines().collect();
    let old_lines: Vec<&str> = old.lines().collect();
    if old_lines.len() < 3 { return vec![]; }  // 最少 3 行才有意义
    
    let first = old_lines[0].trim();
    let last = old_lines.last().unwrap().trim();
    let middle = &old_lines[1..old_lines.len()-1];
    
    let mut candidates = Vec::new();
    // 找到所有首尾行匹配的位置
    for i in 0..content_lines.len() {
        if content_lines[i].trim() != first { continue; }
        for j in (i+2)..content_lines.len() {
            if content_lines[j].trim() != last { continue; }
            let middle_content = &content_lines[i+1..j];
            // 计算中间行相似度
            let similarity = levenshtein_similarity(middle_content, middle);
            candidates.push((i, j, similarity));
            break; // 只取最近的闭合锚点
        }
    }
    
    if candidates.is_empty() { return vec![]; }

    candidates
        .into_iter()
        .map(|(start, end, similarity)| ScoredEditCandidate {
            strategy: "block-anchor-levenshtein".to_string(),
            start,
            end,
            similarity,
            auto_apply_eligible: false,
        })
        .collect()
}
```

**后续 guarded auto-apply 条件**:
- 只有一个候选；
- 首尾行精确匹配，replacement span 行数与 old_string 行数一致或在明确阈值内；
- similarity 高于实测阈值，例如 `>= 0.85`，而不是直接使用 opencode 的宽松阈值；
- 文件快照新鲜，通过 stale-read 和预写竞态检查；
- 新增重复块、相似块、stale anchor、弱模型错误 old_string 回归测试。

**工作量**: 约 100 行 Rust；不替换现有 `block-anchor`，先增强其 diagnostic 输出。

#### 差距 4: 更好的错误报告

**现状**: 我们的编辑失败返回 JSON 错误数据，但 LLM 有时候理解不好。

**设计**: 对标 opencode，在所有 replacer 都失败后，生成指引性错误：

```rust
fn edit_failure_guidance(content: &str, old: &str, candidates: &[EditCandidate]) -> String {
    if candidates.is_empty() {
        format!(
            "Could not find old_string in the file. It must match exactly.\n\
             Tip: try reading the file again to verify the exact text. \n\
             Common issues:\n\
             - Line number prefixes from file_read output (e.g. '12 | ')\n\
             - Smart quotes vs straight quotes\n\
             - Escaped characters (\\n vs actual newline)\n\
             - Trailing whitespace differences"
        )
    } else {
        format!(
            "Found {} possible matches but none are unique enough.\n\
             Provide more surrounding context lines in old_string to disambiguate.\n\
             Candidates found at lines: {}",
            candidates.len(),
            candidates.iter().map(|c| c.line_number.to_string()).collect::<Vec<_>>().join(", ")
        )
    }
}
```

**工作量**: 约 40 行 Rust。

---

## 2. Tree-Sitter Shell 命令解析

### 2.1 opencode 的实现

**核心文件**: `packages/opencode/src/tool/shell.ts` (660 行) + `permission/arity.ts` (163 行)

opencode 的 shell 工具使用 **web-tree-sitter** 对 bash/PowerShell 命令做 AST 解析，用于权限推断（不修改命令本身）。

**完整流程**:

```
LLM 调用 bash(command)
  │
  ├─ 1. 初始化 tree-sitter
  │     └─ 动态加载 WASM 语法 (bash + pwsh)
  │
  ├─ 2. 解析命令为 AST
  │     └─ parser.parse(command) → Tree
  │
  ├─ 3. 遍历 AST 收集信息
  │     ├─ commands(tree) → 所有 command 节点
  │     ├─ parts(command) → 提取命令名 + 参数 tokens
  │     ├─ pathArgs(list) → 识别文件路径参数
  │     └─ 对每个路径: 展开变量 → 检测 glob → 检测动态表达式 → 解析绝对路径
  │
  ├─ 4. 权限检查
  │     ├─ 文件操作命令 (FILES 集合: cd/cp/mv/rm/mkdir/touch 等)
  │     │   └─ 路径在项目外 → 触发 external_directory 权限请求
  │     └─ 所有非 cd 命令
  │         └─ 触发 bash 权限请求 (使用 arity 字典缩小范围)
  │
  ├─ 5. 权限制
  │     ├─ deny → 直接拒绝
  │     ├─ allow → 静默通过
  │     └─ ask → 弹出对话框，用户选择 once/always/deny
  │
  └─ 6. 执行命令（原文不动地传给 shell）
```

**Arity 字典** (~120 条): 将命令前缀映射到"足够理解"的 token 数，用于 always-allow 模式。

| 命令 | arity | always 模式 |
|------|-------|-------------|
| `git` | 2 | `git *` |
| `git remote` | 3 | `git remote *` |
| `docker` | 2 | `docker *` |
| `docker compose` | 3 | `docker compose *` |
| `npm` | 2 | `npm *` |
| `npm run` | 3 | `npm run *` |
| `cargo` | 2 | `cargo *` |
| `kubectl` | 2 | `kubectl *` |
| `terraform` | 2 | `terraform *` |

**文件操作命令检测** (FILES 集合): `cd, chdir, popd, pushd, rm, cp, mv, mkdir, touch, chmod, chown, cat` 等。

### 2.2 priority-agent 的现状

**核心文件**: `src/tools/bash_tool/mod.rs` (1449 行) + `command_classifier.rs` (1433 行)

**我们的分类系统是手写规则型的**:

```
classify_command()
  ├─ normalize_command_for_match()     // 清理引号和转义
  ├─ is_dangerous_command()            // 危险命令检测 (507 行)
  ├─ validation_family()               // 验证命令族 (cargo test, pytest...)
  ├─ shell_command_category()          // 启发式首词匹配
  └─ build_command_classification()    // 结构化分类
```

**分类维度** (11 种):
- Read, List, Search, Validation, TestRun, PackageInstall, DevServer, Interactive, FileMutation, GitMutation, Destructive

**我们已有但较弱**:
- `external_path_access`、`absolute_path_patterns`、`mutation_paths`、`redirections`
  等结构化字段，但来自启发式 tokenizer，不是 AST 语义。
- `PermissionRules.always_allow/always_deny/always_ask` 已支持持久化规则，
  bash match key 形如 `bash:<normalized command>`。
- `permission_rule_suggestions` 已能给出 exact command，以及部分稳定 validation prefix。

**真正缺口**:
- 真正的 shell AST 解析（现在是引号感知 tokenizer + 手写分割）。
- AST 级文件路径提取（区分参数、重定向、命令替换、glob、变量、heredoc）。
- 基于 workspace containment 的精确外部目录判断，而不是只看 `/`、`~/`、`../`。
- arity-derived 的中间粒度 bash permission scope，例如 `git *`、`npm run *`。

### 2.3 差距与设计

#### 差距 1: Shell 语法树解析

**问题**: 我们的 tokenizer 可以处理 90% 的简单命令，但对复杂命令（嵌套引号、heredoc、进程替换、复合命令）可能误判。opencode 的 tree-sitter 方法能精确识别每个 token 的语法角色。

**安全约束**: AST 解析第一版只能作为 observation / shadow classifier 接入，不改变
permission 决策。等 shadow 输出与现有分类器在测试语料上稳定一致，且新增外部路径/高风险命令
回归测试后，再让它参与审批。

**设计**:

虽然我们已经有 `tree-sitter` 依赖（Cargo.toml 中有 `tree-sitter`、`tree-sitter-rust`、`tree-sitter-typescript`、`tree-sitter-python`），但目前只用于代码分析，不用于 shell 解析。

**方案 A: 添加 tree-sitter-bash 依赖（推荐）**

```toml
# Cargo.toml
tree-sitter-bash = "0.23"
```

```rust
// src/tools/bash_tool/shell_parser.rs (新文件)

use tree_sitter::{Parser, Tree, Node};

pub struct ShellAst {
    tree: Tree,
    source: String,
}

pub struct CommandInfo {
    pub executable: String,
    pub args: Vec<String>,
    pub raw: String,
}

pub struct PathArg {
    pub raw: String,
    pub resolved: Option<PathBuf>,
    pub is_external: bool,
    pub has_glob: bool,
    pub has_variable: bool,
}

impl ShellAst {
    /// 解析 shell 命令为 AST
    pub fn parse(command: &str) -> Result<Self, ParseError> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_bash::language())?;
        let tree = parser.parse(command, None)
            .ok_or(ParseError::ParseFailed)?;
        Ok(Self { tree, source: command.to_string() })
    }
    
    /// 提取所有 command 节点
    pub fn commands(&self) -> Vec<Node> {
        let mut result = Vec::new();
        collect_commands(self.tree.root_node(), &mut result);
        result
    }
    
    /// 提取命令的 token 列表 (命令名 + 参数)
    pub fn parts(&self, cmd: &Node) -> Vec<String> {
        // 遍历子节点，跳过重定向、分隔符
        // 提取 command_name, word, string, raw_string, concatenation 节点
        ...
    }
    
    /// 从 token 列表中提取文件路径参数
    pub fn path_args(&self, parts: &[String], ps: bool) -> Vec<PathArg> {
        // 跳过命令行选项 (-x, --flag)
        // 解析路径：~ 展开 → 变量替换检测 → glob 检测 → 绝对路径解析
        ...
    }
}
```

**工作流（shadow-first）**:

```
classify_command() 增强版
  │
  ├─ 尝试 tree-sitter 解析
  │   ├─ 成功 → 遍历 AST:
  │   │   ├─ 识别命令类型 (commands → parts → 命令名)
  │   │   ├─ 提取文件路径 (path_args → workspace containment)
  │   │   ├─ 生成 arity-derived rule suggestions
  │   │   └─ 写入 shell_ast_observation / parser_status
  │   └─ 失败 → parser_status=failed，保留现有 tokenizer 分类
  │
  └─ 权限检查:
      ├─ Phase A: 仍使用现有分类器和 PermissionContext 决策
      ├─ Phase B: AST observation 与现有分类器分歧时 fail-closed / require review
      └─ Phase C: AST-backed external_directory 和 arity scope 才参与审批
```

**工作量**: 约 350 行 Rust（shell_parser.rs 新文件 + observation 数据结构）+
80 行改动到 command_classifier.rs。若要进入 Phase C，还需要额外 UI 和权限规则测试。

#### 差距 2: Arity 字典与智能 Always-Allow

**问题**: 现在的权限系统支持持久化规则，但 bash 规则主要围绕 exact normalized command
和少量 validation prefix。缺少 opencode 那种系统化的 arity 字典来建议中间粒度 scope。

**安全约束**: arity scope 只能作为 permission rule suggestion。默认不能自动写入
`always_allow`；用户必须显式选择。对 destructive、network、external path、compound command、
shell wrapper、package install 等类别，第一版只给 exact command 或 ask，不给宽泛 prefix。

**设计**:

```rust
// src/tools/bash_tool/arity.rs (新文件)

static ARITY_DICT: Lazy<HashMap<&'static str, usize>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // Unix basics
    m.insert("cat", 1); m.insert("cp", 1); m.insert("mv", 1); m.insert("rm", 1);
    m.insert("mkdir", 1); m.insert("ls", 1); m.insert("touch", 1);
    // Git
    m.insert("git", 2);
    m.insert("git remote", 3); m.insert("git stash", 3);
    m.insert("git config", 3);
    // Package managers
    m.insert("npm", 2); m.insert("npm run", 3); m.insert("npm exec", 3);
    m.insert("pnpm", 2); m.insert("pnpm run", 3);
    m.insert("yarn", 2); m.insert("yarn run", 3);
    m.insert("cargo", 2); m.insert("cargo run", 3); m.insert("cargo add", 3);
    m.insert("pip", 2); m.insert("pip install", 3);
    m.insert("brew", 2);
    // Containers
    m.insert("docker", 2); m.insert("docker compose", 3);
    m.insert("docker container", 3); m.insert("docker image", 3);
    m.insert("podman", 2); m.insert("podman container", 3);
    // Cloud/Infra
    m.insert("kubectl", 2); m.insert("kubectl rollout", 3);
    m.insert("terraform", 2); m.insert("terraform workspace", 3);
    m.insert("aws", 3); m.insert("gcloud", 3); m.insert("az", 3);
    // Languages
    m.insert("go", 2); m.insert("python", 2); m.insert("deno", 2);
    m.insert("make", 2); m.insert("cmake", 2); m.insert("bazel", 2);
    // ... 扩展到约 120 条
    m
});

/// 给定命令 tokens，找到最长的 arity 匹配前缀
pub fn arity_prefix(tokens: &[String]) -> Vec<String> {
    for len in (1..=tokens.len()).rev() {
        let prefix = tokens[..len].join(" ");
        if let Some(&arity) = ARITY_DICT.get(prefix.as_str()) {
            return tokens[..arity].to_vec();
        }
    }
    // 无匹配 → 只返回命令名
    tokens.get(0).map(|t| vec![t.clone()]).unwrap_or_default()
}

/// 生成 always-allow 模式
pub fn always_pattern(tokens: &[String]) -> String {
    let prefix = arity_prefix(tokens);
    format!("{} *", prefix.join(" "))
}
```

**规则建议过滤**:

```rust
fn arity_permission_suggestion(classification: &CommandClassification) -> Option<String> {
    if classification.command_kind == CommandKind::Dangerous
        || classification.network_access
        || classification.external_path_access
        || classification.compound_command
        || classification.risky_shell_wrapper
    {
        return None;
    }

    let tokens = ast_tokens_or_shell_tokens(&classification.normalized_command);
    Some(always_pattern(&tokens))
}
```

**集成到权限系统**:

bash 权限审批面板中显示：
```
Command: git push origin main
Permission scope: git *     ← arity 推断
[Allow Once] [Always: exact command] [Always: git *] [Deny]
```

其中 `[Always: git *]` 只在低风险、非 compound、无外部路径、无网络访问时出现。

**工作量**: 约 140 行 Rust（arity.rs + 过滤逻辑）+ 80 行改动到权限面板 UI +
权限规则持久化/匹配测试。

---

## 3. 工具输出智能截断

### 3.1 opencode 的实现

**核心文件**: `packages/opencode/src/tool/truncate.ts` (160 行)

**三层截断体系**:

| 层 | 何时触发 | 行为 |
|----|---------|------|
| **1. 通用 Truncate 服务** | 所有工具执行后 | output → 检测行/字节 → 超限写磁盘 → 返回截断预览 + hint + 文件路径 |
| **2. Shell 流式截断** | shell 命令执行中 | 输出流超出缓冲 → 写前缀到文件 → 追加后续 → 结束取尾 |
| **3. 定时清理** | 每小时 | 删除 7 天前的截断文件 |

**默认限制**: 2000 行 / 51200 字节 (50 KiB)

**LLM 提示消息**（有关键的 Task 委托建议）:

```
The tool call succeeded but the output was truncated.
Full output saved to: ~/.local/share/opencode/tool-output/tool_xxx

Use the Task tool to have explore agent process this file with Grep
and Read (with offset/limit). Do NOT read the full file yourself —
delegate to save context.
```

### 3.2 priority-agent 的现状

**核心文件**: `src/tool_output_store/mod.rs` (787 行) + bash 内置截断

**我们已有的**:

| 层 | 状态 | 说明 |
|----|------|------|
| ToolOutputStore | ✅ 已实现 | 32 KiB 阈值，7 天保留，支持 head/tail/headtail 预览 |
| Bash 内联截断 | ✅ 已实现 | 10000 字符预览 + 写 artifact 文件 |
| 分页读取 | ✅ 已实现 | `read_page()` API 支持 TUI/API 分页 |
| 降级 fallback | ✅ 已实现 | Store 写失败时回退到 head+tail at 2048 |
| Token 感知截断 | ⚠️ 未接入 | `shrink_tool_result_by_tokens()` 实现了但标记 dead_code |
| 流式截断 | ❌ 无 | 必须等命令完成后才能截断 |
| LLM 提示 | ⚠️ 不够精确 | 有“paged reads / 不要粘贴完整日志”提示，但不区分 route 可用工具和存储类型 |

### 3.3 差距与设计

#### 差距 1: Route-Aware LLM 提示

**问题**: 通用 ToolOutputStore 的提示已经说明 `tool-output://` URI、paged reads、不要把完整
log 粘回上下文；bash artifact 也会返回 `output_path`。但提示没有区分：

- `tool-output://...`：应通过 tool-output 分页 API / UI 读取；
- `.priority-agent/tool-results/...` bash artifact：可以用 `file_read offset/limit` 或 `grep`；
- 当前 route 是否暴露 `agent` / Task tool：不能默认建议委托子 agent。

**设计**:

```rust
// 在 truncate_tool_result() 中改进截断消息

fn truncation_hint(reference: &str, route_tools: &RouteToolExposure) -> String {
    let base = format!(
        "The tool output was truncated. Full output reference: {reference}\n\
         Do not paste the full output back into context."
    );

    if reference.starts_with("tool-output://") {
        return format!(
            "{base}\nUse paged tool-output reads or the UI/API paging surface to inspect specific ranges."
        );
    }

    if route_tools.has_file_read {
        return format!(
            "{base}\nUse grep/search for targeted terms, or file_read with offset/limit for specific ranges."
        );
    }

    if route_tools.has_agent {
        return format!(
            "{base}\nIf deeper inspection is needed, delegate a focused explore agent instead of reading the whole log."
        );
    }

    format!("{base}\nAsk for a narrower command or rerun with a filtered output.")
}
```

**工作量**: 约 30 行 Rust。实现时不要把 Task/agent 建议硬编码进通用 truncation 文案。

#### 差距 2: 接入 Token 感知截断

**问题**: `shrink_tool_result_by_tokens()` 和 `high_signal_tool_result_snippets()` 已经实现但标记为 `#[allow(dead_code)]`。

**设计**:

在 `tool_execution.rs` 中，将 token-aware 截断作为 ToolOutputStore 的**补充策略**（非替代）：

```
truncate_tool_result() 增强版:
  │
  ├─ 1. 检测 output 大小
  │
  ├─ 2. 如果 > size_threshold:
  │   ├─ 先把完整输出写入 ToolOutputStore，保留完整 artifact
  │   ├─ 对 model-facing preview 应用 high_signal_tool_result_snippets()
  │   └─ 再应用 shrink_tool_result_by_tokens() 按 token 预算截断 preview
  │
  └─ 3. 如果 > token_threshold (基于当前上下文剩余 token):
      └─ 触发压缩标记，提示 LLM 可能需要在下一轮做 compaction
```

**工作量**: 约 60 行 Rust（接入现有代码）。注意不要让 high-signal preview 替代完整存储。

#### 差距 3: Shell 流式输出截断

**问题**: 当前 bash 工具等待命令完全执行后才截断。对于长时间运行的命令（如 `cargo build`），用户无法看到中间进度，也无法提前发现输出过大。

**设计**:

```rust
// src/tools/bash_tool/streaming.rs (新文件)

pub struct StreamingOutput {
    buffer: Vec<u8>,
    file: Option<File>,
    output_path: Option<PathBuf>,
    max_chars: usize,
    truncated: bool,
}

impl StreamingOutput {
    pub fn new(max_chars: usize) -> Self { ... }
    
    /// 流式写入。如果超过阈值，自动切换为文件模式
    pub fn write(&mut self, chunk: &[u8]) -> io::Result<()> {
        if !self.truncated && self.buffer.len() + chunk.len() > self.max_chars {
            // 触发截断：把已有缓冲写到文件
            self.file = Some(self.create_truncation_file()?);
            self.file.as_mut().unwrap().write_all(&self.buffer)?;
            self.truncated = true;
        }
        if self.truncated {
            self.file.as_mut().unwrap().write_all(chunk)?;
            // 保留最后 N 字节用于 tail 预览
            self.buffer = ...;
        } else {
            self.buffer.extend_from_slice(chunk);
        }
        Ok(())
    }
    
    /// 获取截断后的预览（tail 优先，最后 2000 字符）
    pub fn preview(&self) -> &str { ... }
    
    /// 获取完整输出（从文件读回）
    pub async fn full_output(&self) -> io::Result<String> { ... }
}
```

**工作量**: 约 150 行 Rust（新文件）。

---

## 4. 总体实施路线

### Phase 1: 编辑匹配增强（预计 2-3 天）

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 1.1 新增 Escape-Normalized diagnostic | `edit_match.rs` | ~70 行 |
| 1.2 新增 Trimmed-Boundary Replacer | `edit_match.rs` | ~35 行 |
| 1.3 增强 Block-Anchor 为 Levenshtein shadow scoring | `edit_match.rs` | ~100 行 |
| 1.4 改进编辑失败错误指引 | `edit_tool.rs` | ~40 行 |
| 1.5 添加 fallback 安全回归测试 | `edit_match.rs` | ~180 行（测试） |

### Phase 2: Shell 命令系统增强（预计 5-7 天）

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 2.1 添加 tree-sitter-bash 依赖 | `Cargo.toml` | 1 行 |
| 2.2 实现 ShellAst shadow parser | `bash_tool/shell_parser.rs` (新) | ~300 行 |
| 2.3 实现 Arity 字典与过滤 | `bash_tool/arity.rs` (新) | ~140 行 |
| 2.4 以 observation 接入 command_classifier | `command_classifier.rs` | ~100 行 |
| 2.5 增强权限审批面板（always-allow scope） | `tui/` 相关文件 | ~100 行 |
| 2.6 将 existing external_path_access 升级为 workspace containment observation | `bash_tool/mod.rs` | ~80 行 |
| 2.7 parser fallback / disagreement / fail-closed 测试 | `bash_tool/tests.rs` | ~260 行（测试） |

### Phase 3: 工具输出截断增强（预计 2 天）

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 3.1 Route-aware 截断提示 | `tool_output_store/mod.rs` / tool result metadata | ~30 行 |
| 3.2 接入 token-aware 截断 | `tool_execution.rs` | ~60 行 |
| 3.3 Shell 流式输出截断 | `bash_tool/streaming.rs` (新) | ~150 行 |
| 3.4 统一 bash artifact 与 ToolOutputStore 存储 | `bash_tool/mod.rs` | ~40 行 |
| 3.5 截断行为端到端测试 | 测试文件 | ~100 行（测试） |

### 总估计

| Phase | 新增/改动代码 | 测试代码 | 工期 |
|-------|:---:|:---:|:---:|
| Phase 1: 编辑匹配 | ~245 行 | ~180 行 | 2-3 天 |
| Phase 2: Shell 解析 | ~720 行 | ~260 行 | 5-7 天 |
| Phase 3: 输出截断 | ~280 行 | ~100 行 | 2 天 |
| **合计** | **~1245 行** | **~540 行** | **9-12 天** |

### 优先级建议

1. **Phase 1.1 + 1.2** (Escape diagnostic + Trim 策略) — 最快见效，填补弱模型编辑失败的最大缺口
2. **Phase 3.1 + 3.2** (route-aware 提示 + token 截断) — 利用已有代码，改动量最小
3. **Phase 1.3 + 1.4** (Block-anchor shadow scoring + 错误指引) — 完善编辑系统，但不扩大默认 auto-apply
4. **Phase 2A** (Shell tree-sitter shadow + arity suggestions) — 先观察，不改变审批
5. **Phase 3.3** (流式输出截断) — UX 提升但非关键路径
6. **Phase 2B** (AST-backed permission enforcement) — 只有 shadow 数据和回归测试稳定后再进入

---

> 文档版本: v1.0, 2026-06-09
> 基于: opencode-dev (TypeScript) vs priority-agent (Rust) 源码级对比
