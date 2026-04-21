//! CLI 命令定义

/// 命令行参数
#[derive(Debug)]
pub struct Cli {
    /// 子命令
    pub command: Commands,
    /// 配置文件路径
    pub config: Option<String>,
    /// 详细输出
    pub verbose: bool,
}

impl Cli {
    pub fn parse() -> Self {
        // 简化的命令解析
        let args: Vec<String> = std::env::args().collect();

        let mut verbose = false;
        let mut config = None;
        let mut command = Commands::Help;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-v" | "--verbose" => verbose = true,
                "-c" | "--config" => {
                    i += 1;
                    if i < args.len() {
                        config = Some(args[i].clone());
                    }
                }
                "help" | "--help" | "-h" => command = Commands::Help,
                "init" => command = Commands::Init,
                "add" => {
                    i += 1;
                    if i < args.len() {
                        command = Commands::AddTask {
                            name: args[i].clone(),
                        };
                    }
                }
                "list" => command = Commands::List,
                "next" => command = Commands::Next,
                "done" => {
                    i += 1;
                    if i < args.len() {
                        command = Commands::CompleteTask {
                            id: args[i].clone(),
                        };
                    }
                }
                "progress" => command = Commands::Progress,
                "analyze" => command = Commands::Analyze,
                "snapshot" => command = Commands::Snapshot { name: None },
                "restore" => {
                    i += 1;
                    if i < args.len() {
                        command = Commands::Restore {
                            id: args[i].clone(),
                        };
                    }
                }
                "interactive" | "i" => command = Commands::Interactive,
                _ => {}
            }
            i += 1;
        }

        Self {
            command,
            config,
            verbose,
        }
    }
}

/// 子命令枚举
#[derive(Debug, Clone)]
pub enum Commands {
    /// 显示帮助
    Help,
    /// 初始化新项目
    Init,
    /// 添加任务
    AddTask { name: String },
    /// 列出所有任务
    List,
    /// 显示下一个推荐任务
    Next,
    /// 完成任务
    CompleteTask { id: String },
    /// 显示进度
    Progress,
    /// 分析项目
    Analyze,
    /// 创建快照
    Snapshot { name: Option<String> },
    /// 恢复快照
    Restore { id: String },
    /// 交互模式
    Interactive,
}

impl Commands {
    /// 获取命令描述
    pub fn description(&self) -> &'static str {
        match self {
            Commands::Help => "显示帮助信息",
            Commands::Init => "初始化新项目",
            Commands::AddTask { .. } => "添加新任务",
            Commands::List => "列出所有任务",
            Commands::Next => "显示下一个推荐任务",
            Commands::CompleteTask { .. } => "标记任务为完成",
            Commands::Progress => "显示项目进度",
            Commands::Analyze => "分析项目结构",
            Commands::Snapshot { .. } => "创建项目快照",
            Commands::Restore { .. } => "恢复快照",
            Commands::Interactive => "进入交互模式",
        }
    }
}

/// 打印帮助信息
pub fn print_help() {
    println!("Priority Agent - 加权优先级桌面 Agent");
    println!();
    println!("用法: priority-agent [选项] <命令>");
    println!();
    println!("选项:");
    println!("  -v, --verbose    详细输出");
    println!("  -c, --config     配置文件路径");
    println!("  -h, --help       显示帮助");
    println!();
    println!("命令:");
    println!("  init                  初始化新项目");
    println!("  add <任务名>          添加新任务");
    println!("  list                  列出所有任务");
    println!("  next                  显示下一个推荐任务");
    println!("  done <任务ID>         标记任务为完成");
    println!("  progress              显示项目进度");
    println!("  analyze               分析项目结构");
    println!("  snapshot [名称]       创建项目快照");
    println!("  restore <快照ID>      恢复快照");
    println!("  interactive, i        进入交互模式");
    println!("  help                  显示帮助");
    println!();
    println!("示例:");
    println!("  priority-agent init");
    println!("  priority-agent add \"实现用户认证\"");
    println!("  priority-agent next");
    println!("  priority-agent done task-1");
    println!("  priority-agent i");
}
