# Priority Agent - 加权优先级桌面 Agent

解决 AI Agent 抓不住重点的问题，通过显式的权重系统让 AI 始终专注于最重要的事项。

## 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/priority-agent
cd priority-agent

# 编译
cargo build --release

# 运行
./target/release/priority-agent --help
```

### 基本使用

```bash
# 初始化新项目
priority-agent init

# 添加任务
priority-agent add "实现用户认证系统"
priority-agent add "设计数据库模型"
priority-agent add "编写API文档"

# 查看推荐任务
priority-agent next

# 标记任务完成
priority-agent done task_1234567890

# 查看进度
priority-agent progress

# 分析项目
priority-agent analyze
```

## 核心理念

### 权重分层系统

```
项目目标
├── 一级任务 (权重和 = 100%)
│   ├── 任务 A: 40%
│   │   ├── 子任务 A1: 60% (占整体 24%)
│   │   └── 子任务 A2: 40% (占整体 16%)
│   ├── 任务 B: 35%
│   └── 任务 C: 25%
```

### 优先级计算

1. **绝对权重** - 任务相对于整个项目的重要性
2. **依赖关系** - 阻塞其他任务的数量
3. **依赖深度** - 依赖链的长度
4. **进行状态** - 已经开始的任务优先

## 功能特性

### 已实现 (Phase 1 MVP)

- ✅ 权重计算引擎
- ✅ 任务管理（添加、完成、进度跟踪）
- ✅ 持久化存储
- ✅ 快照功能
- ✅ 基础CLI命令
- ✅ 交互模式

### 开发中 (Phase 2)

- 🚧 AI 自动权重分析
- 🚧 代码结构解析
- 🚧 依赖关系识别

### 计划中 (Phase 3-4)

- 📋 与 Claude Code 对比测试
- 📋 用户体验优化
- 📋 子任务支持
- 📋 任务模板

## 命令参考

| 命令 | 描述 | 示例 |
|------|------|------|
| `init` | 初始化新项目 | `priority-agent init` |
| `add <名称>` | 添加新任务 | `priority-agent add "实现登录"` |
| `list` | 列出所有任务 | `priority-agent list` |
| `next` | 显示推荐任务 | `priority-agent next` |
| `done <ID>` | 完成任务 | `priority-agent done task_123` |
| `progress` | 显示进度 | `priority-agent progress` |
| `analyze` | 分析项目结构 | `priority-agent analyze` |
| `snapshot [名称]` | 创建快照 | `priority-agent snapshot "v1.0"` |
| `restore <ID>` | 恢复快照 | `priority-agent restore snapshot_123` |
| `interactive` | 交互模式 | `priority-agent i` |

## 技术栈

- **语言**: Rust
- **序列化**: serde + JSON
- **配置**: toml
- **存储**: 本地 JSON 文件

## 项目结构

```
rust-agent/
├── src/
│   ├── main.rs              # 主入口
│   ├── cli/                 # CLI 界面
│   │   ├── commands.rs      # 命令解析
│   │   ├── display.rs       # 格式化输出
│   │   └── interactive.rs   # 交互模式
│   ├── weight_engine/       # 权重计算核心
│   │   ├── types.rs         # 核心类型
│   │   └── calculator.rs    # 权重计算器
│   ├── task_analyzer/       # 任务分析器
│   │   ├── parser.rs        # 任务解析
│   │   ├── dependency_graph.rs
│   │   └── analyzer.rs
│   └── context_manager/     # 上下文管理
│       ├── state.rs         # 会话状态
│       ├── persistence.rs   # 持久化
│       └── history.rs       # 历史记录
├── Cargo.toml
└── README.md
```

## 开发计划

### Phase 1: MVP ✅ (已完成)
- [x] 权重计算引擎基础
- [x] 简单的任务解析
- [x] 命令行界面

### Phase 2: 智能分析 (进行中)
- [ ] AI 自动权重分析
- [ ] 代码结构解析
- [ ] 依赖关系识别

### Phase 3: 对比测试
- [ ] 设计标准测试集
- [ ] 与 Claude Code 对比
- [ ] 性能优化

### Phase 4: 产品化
- [ ] 用户体验优化
- [ ] 文档完善
- [ ] 发布准备

## 与 Claude Code 的对比

### 测试场景
**复杂任务**: "实现一个用户认证系统"

| 维度 | Claude Code | Priority Agent |
|------|-------------|----------------|
| 执行顺序 | 线性，容易在细节上绕圈 | 按权重优先级，先核心后细节 |
| 重点把握 | 容易偏离 | 始终聚焦高权重任务 |
| 完成度感知 | 模糊 | 数学化进度计算 |

## 合作模式

- **Liz (AI 助手)**: 技术讨论、代码审查、文档编写、测试设计
- **Gex (产品负责人)**: 需求定义、架构决策、最终验收

## 记录

- 2026-04-09: 项目启动，确定核心想法
- 2026-04-10: Phase 1 MVP 完成，实现核心权重系统和CLI功能

## License

MIT
